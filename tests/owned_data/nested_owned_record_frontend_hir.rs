use std::path::Path;

use semaprax::hir::{self, DeclarationId, PlaceProjection, ResolvedProgram};
use semaprax::loan_plan::LoanCause;
use semaprax::{parse, verify};

const NESTED_SOURCE: &str = r#"
module test.nested_owned_record_frontend_hir;

@id("nested.leaf")
record Leaf {
    @id("nested.leaf.payload") payload: Bytes,
    @id("nested.leaf.marker") marker: i64,
}

@id("nested.branch")
record Branch {
    @id("nested.branch.leaf") leaf: Leaf,
    @id("nested.branch.enabled") enabled: bool,
}

@id("nested.envelope")
record Envelope {
    @id("nested.envelope.left") left: Branch,
    @id("nested.envelope.right") right: Branch,
    @id("nested.envelope.sequence") sequence: usize,
}

@id("nested.inspect")
fn inspect(packet: own Envelope) -> usize {
    let left = bytes_as_slice(packet.left.leaf.payload);
    let right = bytes_as_slice(packet.right.leaf.payload);
    byte_len(left) + byte_len(right)
}

@id("nested.identity")
fn identity(packet: own Envelope) -> Envelope { packet }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let parsed = parse(source, Path::new("nested-owned-records-v1.spx")).expect("source parses");
    verify::verify(&parsed)
}

fn resolved() -> ResolvedProgram {
    let parsed = parse(NESTED_SOURCE, Path::new("nested-owned-records-v1.spx"))
        .expect("nested fixture parses");
    let report = verify::verify(&parsed);
    assert!(
        report
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "nested fixture source verification failed: {report:?}"
    );
    hir::resolve(&parsed).expect("nested fixture resolves and validates")
}

fn projection_ids(projections: &[PlaceProjection]) -> Vec<&str> {
    projections
        .iter()
        .map(|projection| match projection {
            PlaceProjection::Field(field) => field.as_str(),
            PlaceProjection::VariantField { .. } => panic!("record path contains variant step"),
        })
        .collect()
}

#[test]
fn nested_whole_owners_and_multi_projection_loans_retain_stable_field_paths() {
    let program = resolved();
    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested.inspect")
        .expect("inspect function");
    let mut paths = inspect
        .loan_plan
        .loans
        .iter()
        .filter(|loan| loan.cause == LoanCause::SliceView)
        .map(|loan| projection_ids(&loan.origin.projections))
        .collect::<Vec<_>>();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            vec![
                "nested.envelope.left",
                "nested.branch.leaf",
                "nested.leaf.payload",
            ],
            vec![
                "nested.envelope.right",
                "nested.branch.leaf",
                "nested.leaf.payload",
            ],
        ]
    );

    let identity = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested.identity")
        .expect("identity function");
    assert_eq!(identity.params[0].ty, identity.return_type);
}

#[test]
fn nested_projection_move_conflict_remains_path_exact() {
    let source = NESTED_SOURCE.replace(
        "byte_len(left) + byte_len(right)",
        "let moved = identity(packet); byte_len(left) + byte_len(right)",
    );
    assert!(
        diagnostics(&source)
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T265"),
        "parent transfer while nested loans remain live must fail"
    );
}

#[test]
fn forged_nested_stable_field_paths_fail_hir_replay() {
    let mut forged_loan = resolved();
    let inspect = forged_loan
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "nested.inspect")
        .expect("inspect function");
    let loan = inspect
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| loan.cause == LoanCause::SliceView)
        .expect("nested slice loan");
    *loan.origin.projections.last_mut().expect("leaf projection") =
        PlaceProjection::Field(DeclarationId::new("nested.leaf.marker"));
    assert_eq!(hir::validate(&forged_loan).unwrap_err().code, "SPX-H006");
}

#[test]
fn nested_record_updates_remain_closed_without_regressing_flat_updates() {
    let nested = r#"
module test.nested_update_closed;
@id("update.leaf") record Leaf {
  @id("update.leaf.payload") payload: Bytes,
}
@id("update.root") record Root {
  @id("update.root.leaf") leaf: Leaf,
  @id("update.root.marker") marker: i64,
}
@id("update.nested") fn update(value: own Root) -> Root {
  value with { marker: 1 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let errors = diagnostics(nested);
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic.code == "SPX-O117"
                && diagnostic
                    .message
                    .contains("record updates over nested owned-Bytes records remain closed")
        }),
        "nested update must fail with the closed-profile diagnostic: {errors:?}"
    );

    let flat = r#"
module test.flat_update_retained;
@id("update.packet") record Packet {
  @id("update.packet.payload") payload: Bytes,
  @id("update.packet.marker") marker: i64,
}
@id("update.flat") fn update(value: own Packet) -> Packet {
  value with { marker: 1 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let errors = diagnostics(flat);
    assert!(
        errors
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "legacy flat owned-Bytes updates remain admitted: {errors:?}"
    );
}

#[test]
fn variants_generics_classes_and_noncopy_leaves_stay_closed() {
    let cases = [
        r#"
module test.nested_variant_closed;
variant Choice { Some { payload: Bytes, }, }
record Root { choice: Choice, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.nested_generic_closed;
record Leaf { payload: Bytes, }
record Root<T> { leaf: Leaf, marker: T, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.nested_class_closed;
class Metadata { marker: i64, }
record Root { payload: Bytes, metadata: Metadata, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.nested_string_closed;
record Root { payload: Bytes, text: string, }
fn main() -> i64 { 0 }
"#,
    ];
    for source in cases {
        assert!(
            diagnostics(source)
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T268"),
            "forbidden nested shape was admitted:\n{source}"
        );
    }
}

fn nested_depth_source(depth: usize) -> String {
    let mut source = String::from("module test.nested_depth_bound;\n");
    for index in (0..depth).rev() {
        source.push_str(&format!("@id(\"depth.r{index}\") record R{index} {{ "));
        if index + 1 == depth {
            source.push_str(&format!(
                "@id(\"depth.r{index}.payload\") payload: Bytes, }}\n"
            ));
        } else {
            source.push_str(&format!(
                "@id(\"depth.r{index}.next\") next: R{}, }}\n",
                index + 1
            ));
        }
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

fn wide_record_source(fields: usize, bytes_fields: usize) -> String {
    let mut source =
        String::from("module test.nested_width_bound;\n@id(\"width.root\") record Root {\n");
    for index in 0..fields {
        let ty = if index < bytes_fields {
            "Bytes"
        } else {
            "bool"
        };
        source.push_str(&format!("@id(\"width.root.f{index}\") f{index}: {ty},\n"));
    }
    source.push_str("}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

fn error_codes(source: &str) -> Vec<&'static str> {
    diagnostics(source)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn nested_record_depth_and_owned_leaf_bounds_are_exact() {
    assert!(error_codes(&nested_depth_source(64)).is_empty());
    assert!(error_codes(&nested_depth_source(65)).contains(&"SPX-T268"));

    assert!(error_codes(&wide_record_source(256, 256)).is_empty());
    assert!(error_codes(&wide_record_source(257, 257)).contains(&"SPX-T268"));
}

#[test]
fn nested_record_visited_field_bound_is_exact() {
    assert!(error_codes(&wide_record_source(4_096, 1)).is_empty());
    assert!(error_codes(&wide_record_source(4_097, 1)).contains(&"SPX-T268"));
}
