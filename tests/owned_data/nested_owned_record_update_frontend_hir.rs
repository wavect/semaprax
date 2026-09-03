use std::path::Path;

use semaprax::hir::{self, DeclarationId, ResolvedExprKind, ResolvedProgram};
use semaprax::{parse, verify};

const SOURCE: &str = r#"
module test.nested_owned_record_update_frontend_hir;
@id("update.leaf") record Leaf {
  @id("update.leaf.payload") payload: Bytes,
  @id("update.leaf.marker") marker: i64,
}
@id("update.branch") record Branch {
  @id("update.branch.leaf") leaf: Leaf,
  @id("update.branch.enabled") enabled: bool,
}
@id("update.root") record Root {
  @id("update.root.branch") branch: Branch,
  @id("update.root.payload") payload: Bytes,
  @id("update.root.sequence") sequence: i64,
}
@id("update.copy") fn replace_copy(value: own Root) -> Root {
  value with { sequence: 9 }
}
@id("update.bytes") fn replace_bytes(value: own Root, replacement: own Bytes) -> Root {
  value with { payload: replacement }
}
@id("update.subtree") fn replace_subtree(value: own Root, replacement: own Branch) -> Root {
  value with { branch: replacement }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let parsed = parse(source, Path::new("nested-owned-record-update-v1.spx"))
        .expect("nested update source parses");
    verify::verify(&parsed)
}

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("nested-owned-record-update-v1.spx"))
        .expect("nested update fixture parses");
    let report = verify::verify(&parsed);
    assert!(
        report
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "nested update fixture failed source admission: {report:?}"
    );
    hir::resolve(&parsed).expect("nested update fixture resolves and validates")
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function {id}"))
}

fn tail(expression: &hir::ResolvedExpr) -> &hir::ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &expression.kind else {
        panic!("function body remains a block")
    };
    tail
}

#[test]
fn nested_updates_retain_exact_base_replacement_order_and_stable_field_ids() {
    let program = program();
    let cases = [
        ("update.copy", "update.root.sequence"),
        ("update.bytes", "update.root.payload"),
        ("update.subtree", "update.root.branch"),
    ];
    for (id, field) in cases {
        let function = function(&program, id);
        let ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } = &tail(&function.body).kind
        else {
            panic!("{id} tail is not an update")
        };
        assert_eq!(record.as_str(), "update.root");
        assert!(matches!(
            &base.kind,
            ResolvedExprKind::Place(place) if place.root == function.params[0].id
                && place.projections.is_empty()
        ));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field.as_str(), field);
        assert_eq!(tail(&function.body).ty, function.return_type);
    }
}

#[test]
fn nested_update_excluded_bases_retain_stable_source_diagnostics() {
    let prefix = r#"
module test.nested_update_closed;
record Leaf { payload: Bytes, marker: i64, }
record Branch { leaf: Leaf, enabled: bool, }
record Root { branch: Branch, marker: i64, }
"#;
    let cases = [
        (
            r#"fn invalid(value: own Root) -> Branch {
  value.branch with { enabled: false }
}
"#,
            "SPX-O117",
            "nested owned-record update requires an exact named owned base place",
        ),
        (
            r#"fn invalid(value: borrow Root) -> Root {
  value with { marker: 1 }
}
"#,
            "SPX-O108",
            "cannot update an owned record through a borrowed or shared base",
        ),
    ];
    for (body, code, message) in cases {
        let source = format!("{prefix}{body}fn main() -> i64 {{ 0 }}\n");
        let errors = diagnostics(&source);
        assert!(
            errors.iter().any(|diagnostic| {
                diagnostic.code == code && diagnostic.message.contains(message)
            }),
            "missing {code} `{message}`: {errors:?}"
        );
    }
}

#[test]
fn hostile_hir_rejects_foreign_replacement_identity() {
    let mut foreign = program();
    let function = foreign
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "update.copy")
        .expect("copy update function");
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("copy update body remains a block")
    };
    let ResolvedExprKind::UpdateRecord { fields, .. } = &mut tail.kind else {
        panic!("copy update tail remains an update")
    };
    fields[0].field = DeclarationId::new("update.root.foreign");
    let diagnostic = hir::validate(&foreign).expect_err("foreign replacement field fails closed");
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(
        diagnostic
            .message
            .contains("contains foreign field `update.root.foreign`"),
        "{diagnostic:?}"
    );
}

fn depth_source(depth: usize) -> String {
    let mut source = String::from("module test.nested_update_depth;\n");
    for index in (0..depth).rev() {
        source.push_str(&format!("@id(\"depth.r{index}\") record R{index} {{ "));
        if index + 1 == depth {
            source.push_str(&format!("@id(\"depth.r{index}.payload\") payload: Bytes, "));
        } else {
            source.push_str(&format!(
                "@id(\"depth.r{index}.next\") next: R{}, ",
                index + 1
            ));
        }
        if index == 0 {
            source.push_str("@id(\"depth.r0.marker\") marker: i64, ");
        }
        source.push_str("}\n");
    }
    source.push_str(
        "@id(\"depth.update\") fn update(value: own R0) -> R0 { value with { marker: 1 } }\n",
    );
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

#[test]
fn nested_update_inherits_exact_depth_bound_and_flat_legacy_admission() {
    assert!(diagnostics(&depth_source(64))
        .iter()
        .all(|diagnostic| !diagnostic.severity.is_error()));
    assert!(diagnostics(&depth_source(65))
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T268"));

    let flat = r#"
module test.flat_update_preserved;
record Packet { payload: Bytes, marker: i64, }
fn update(value: own Packet) -> Packet { value with { marker: 1 } }
fn main() -> i64 { 0 }
"#;
    assert!(
        diagnostics(flat)
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "legacy flat update must remain admitted"
    );
}
