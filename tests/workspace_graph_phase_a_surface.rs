use std::path::Path;

use semaprax::ast::ModuleUseKind;
use semaprax::{codegen, format, graph, parse, verify, wasm};

const IMPORTING_SOURCE: &str = r#"module consumer.app;
use type @id("provider.number") from provider.types as Number;
use function @id("provider.answer") from provider.math as answer;

@id("consumer.main")
fn main() -> i64
{
    answer()
}
"#;

#[test]
fn authored_module_use_order_is_canonical_and_round_trips_exactly() {
    let parsed = parse(IMPORTING_SOURCE, Path::new("consumer.spx")).unwrap();

    assert_eq!(parsed.module_uses.len(), 2);
    assert_eq!(parsed.module_uses[0].kind, ModuleUseKind::Type);
    assert_eq!(parsed.module_uses[0].persistent_id, "provider.number");
    assert_eq!(parsed.module_uses[0].target_module, "provider.types");
    assert_eq!(parsed.module_uses[0].alias, "Number");
    assert_eq!(parsed.module_uses[1].kind, ModuleUseKind::Function);
    assert_eq!(parsed.module_uses[1].persistent_id, "provider.answer");
    assert_eq!(parsed.module_uses[1].target_module, "provider.math");
    assert_eq!(parsed.module_uses[1].alias, "answer");

    let canonical = format::canonical(&parsed);
    assert_eq!(canonical, IMPORTING_SOURCE);
    let reparsed = parse(&canonical, Path::new("consumer.spx")).unwrap();
    assert_eq!(reparsed.module_uses, parsed.module_uses);
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn formatter_preserves_authored_module_use_order_instead_of_sorting() {
    let source = IMPORTING_SOURCE
        .replace(
            "use type @id(\"provider.number\") from provider.types as Number;\nuse function @id(\"provider.answer\") from provider.math as answer;",
            "use function @id(\"provider.answer\") from provider.math as answer;\nuse type @id(\"provider.number\") from provider.types as Number;",
        );
    let parsed = parse(&source, Path::new("consumer.spx")).unwrap();

    assert_eq!(format::canonical(&parsed), source);
    assert_eq!(parsed.module_uses[0].kind, ModuleUseKind::Function);
    assert_eq!(parsed.module_uses[1].kind, ModuleUseKind::Type);
}

#[test]
fn closed_module_use_prefix_failures_report_g170() {
    let malformed = [
        "use value @id(\"provider.answer\") from provider.math as answer;",
        "use function id(\"provider.answer\") from provider.math as answer;",
        "use function @name(\"provider.answer\") from provider.math as answer;",
        "use function @id(42) from provider.math as answer;",
    ];

    for declaration in malformed {
        let source = format!(
            "module consumer.app;\n{declaration}\n\n@id(\"consumer.main\")\nfn main() -> i64 {{ 42 }}\n"
        );
        let error = parse(&source, Path::new("malformed-use.spx")).unwrap_err();
        assert_eq!(
            error.code, "SPX-G170",
            "unexpected diagnostic for {declaration}"
        );
        assert_eq!(error.path.as_deref(), Some("malformed-use.spx"));
    }
}

#[test]
fn every_malformed_module_use_shape_is_closed_as_g170() {
    let malformed = [
        "use function @id\"provider.answer\" from provider.math as answer;",
        "use function @id(\"provider.answer\" from provider.math as answer;",
        "use function @id(\"provider.answer\") provider.math as answer;",
        "use function @id(\"provider.answer\") via provider.math as answer;",
        "use function @id(\"provider.answer\") from as answer;",
        "use function @id(\"provider.answer\") from provider.math answer;",
        "use function @id(\"provider.answer\") from provider.math alias answer;",
        "use function @id(\"provider.answer\") from provider.math as ;",
        "use function @id(\"provider.answer\") from provider.math as answer",
        "use function @id(\"provider.answer\") from provider.math as answer extra;",
    ];

    for declaration in malformed {
        let source = format!(
            "module consumer.app;\n{declaration}\n\n@id(\"consumer.main\")\nfn main() -> i64 {{ 42 }}\n"
        );
        let error = parse(&source, Path::new("malformed-use-shape.spx")).unwrap_err();
        assert_eq!(
            error.code, "SPX-G170",
            "unexpected diagnostic for {declaration}"
        );
        assert_eq!(error.path.as_deref(), Some("malformed-use-shape.spx"));
        assert!(error.span.is_some(), "missing span for {declaration}");
    }
}

#[test]
fn module_uses_are_rejected_outside_the_closed_header_section() {
    let misplaced = [
        r#"module consumer.app;
permit { clock.read }
use function @id("provider.answer") from provider.math as answer;

@id("consumer.main")
fn main() -> i64 { 42 }
"#,
        r#"module consumer.app;

@id("consumer.helper")
fn helper() -> i64 { 1 }

use function @id("provider.answer") from provider.math as answer;

@id("consumer.main")
fn main() -> i64 { 42 }
"#,
    ];

    for source in misplaced {
        let error = parse(source, Path::new("misplaced-use.spx")).unwrap_err();
        assert_eq!(error.code, "SPX-G170");
        assert_eq!(error.path.as_deref(), Some("misplaced-use.spx"));
        assert!(error.span.is_some());
    }
}

#[test]
fn importing_source_is_rejected_by_every_ordinary_single_file_boundary() {
    let parsed = parse(IMPORTING_SOURCE, Path::new("consumer.spx")).unwrap();
    let expected = "source module imports require Workspace Semantic Graph resolution";

    let diagnostics = verify::verify(&parsed);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-G172");
    assert_eq!(diagnostics[0].message, expected);
    assert_eq!(diagnostics[0].path.as_deref(), Some("consumer.spx"));
    assert!(diagnostics[0].span.is_none());

    let graph_errors = graph::to_json(&parsed).unwrap_err();
    assert_eq!(graph_errors.len(), 1);
    assert_eq!(graph_errors[0].code, "SPX-G172");
    assert_eq!(graph_errors[0].message, expected);

    let native_error = codegen::emit_c(&parsed).unwrap_err();
    assert_eq!(native_error.code, "SPX-G172");
    assert_eq!(native_error.message, expected);

    let wasm_error = wasm::emit_module(&parsed).unwrap_err();
    assert_eq!(wasm_error.code, "SPX-G172");
    assert_eq!(wasm_error.message, expected);
}

#[test]
fn representative_no_use_graph_v10_bytes_remain_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("examples/meaning.spx");
    let source = include_str!("../examples/meaning.spx");
    let parsed = parse(source, &path).unwrap();

    assert!(parsed.module_uses.is_empty());
    assert_eq!(format::canonical(&parsed), source);
    assert_eq!(
        format!("{}\n", graph::to_json(&parsed).unwrap()),
        include_str!("snapshots/meaning.graph.json")
    );
}
