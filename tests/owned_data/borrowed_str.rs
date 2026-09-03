use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{OwnershipMode, ResolvedType};
use semaprax::interpreter::{self, ArgumentValue, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const PROGRAM: &str = r#"
module test.borrowed_str;

@id("text.classify")
fn classify(value: borrow str, prefix: borrow str, needle: borrow str) -> i64 {
    if str_is_empty(value) {
        0
    } else {
        if str_starts_with(value, prefix) && str_contains(value, needle) {
            str_len_bytes(value)
        } else {
            -1
        }
    }
}

@id("text.forward")
fn forward(value: borrow str, needle: borrow str) -> bool {
    str_contains(value, needle)
}

@id("text.same")
fn same(value: borrow str) -> bool {
    str_starts_with(value, value) && str_contains(value, value)
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("borrowed_str.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn owned_string_view_is_rooted_borrow_only_and_reaches_graph_native_and_wasm() {
    let source = r#"
module test.owned_string_view;
@id("text.owned-view")
fn owned_view(value: string) -> i64 {
    let view = string_as_str(value);
    str_len_bytes(view)
}
@id("app.main")
fn main() -> i64 { owned_view("aé") }
"#;
    let program = parse(source, Path::new("owned-string-view.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let projected = graph::to_json(&program).unwrap();
    assert!(projected.contains("core.string.as-str"));
    let native = semaprax::codegen::emit_c(&program).unwrap();
    assert!(native.contains("spx_string_as_str"));
    let wasm = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(wasm, semaprax::wasm::emit_module(&program).unwrap());
}

#[test]
fn owned_string_view_rejects_temporary_literal_and_projected_roots() {
    for body in [
        "string_as_str(\"temporary\")",
        "string_as_str(string_concat(\"left\", \"right\"))",
        "string_as_str(packet.text)",
    ] {
        let declaration = if body.contains("packet") {
            "record Packet { text: string, }\n@id(\"bad.id\") fn bad(packet: own Packet) -> i64"
        } else {
            "@id(\"bad.id\") fn bad() -> i64"
        };
        let source = format!(
            "module test.invalid_owned_view;\n{declaration} {{ let view = {body}; str_len_bytes(view) }}\n"
        );
        let found = diagnostics(&source);
        assert!(
            found.iter().any(|item| item.code == "SPX-T266"),
            "{found:?}"
        );
    }
}

#[test]
fn owned_string_cannot_move_while_its_borrowed_str_view_remains_live() {
    let source = r#"
module test.invalid_owned_view_move;
@id("bad.id")
fn bad(value: string) -> i64 {
    let view = string_as_str(value);
    let moved = value;
    str_len_bytes(view)
}
"#;
    let found = diagnostics(source);
    assert!(
        found.iter().any(|item| item.code == "SPX-T265"),
        "{found:?}"
    );
}

#[test]
fn borrowed_str_parameter_can_still_form_an_immutable_local_alias() {
    let source = r#"
module test.borrowed_str_alias;
@id("alias.id")
fn alias(value: borrow str) -> i64 {
    let local = value;
    str_len_bytes(local)
}
"#;
    let program = parse(source, Path::new("borrowed-str-alias.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
}

#[test]
fn command_argument_utf8_can_form_an_authenticated_local_borrow() {
    let source = r#"
module test.command_argument_str;
permit { process.args.read }
@id("command.run")
fn run() -> i64 uses { process.args.read } {
    let argument = arg_utf8(0usize);
    str_len_bytes(argument)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("command-argument-str.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
}

fn temporary_source(source: &str) -> PathBuf {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-borrowed-str-{}-{ordinal}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

#[test]
fn borrowed_str_round_trips_resolves_and_projects_without_owned_string_aliasing() {
    let program = parse(PROGRAM, Path::new("borrowed_str.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("value: borrow str"));
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);

    let resolved = hir::resolve(&program).unwrap();
    let classify = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "text.classify")
        .unwrap();
    assert!(classify.params.iter().all(|param| {
        param.ty == ResolvedType::Str && param.ownership == OwnershipMode::Borrow
    }));
    assert_eq!(classify.return_type, ResolvedType::I64);

    let projected = graph::to_json(&program).unwrap();
    assert!(projected.contains("\"kind\":\"primitive\",\"name\":\"str\""));
    for id in [
        "core.str.len_bytes",
        "core.str.is_empty",
        "core.str.starts_with",
        "core.str.contains",
    ] {
        assert!(projected.contains(id), "missing intrinsic identity {id}");
    }
}

#[test]
fn borrowed_str_is_input_only_borrow_only_and_compiler_owned() {
    let cases = [
        (
            "fn bad(value: str) -> i64 { 0 }",
            "SPX-O115",
        ),
        (
            "fn bad(value: own str) -> i64 { 0 }",
            "SPX-O115",
        ),
        (
            "fn bad(value: borrow str) -> str { value }",
            "SPX-O116",
        ),
        (
            "record Box { value: str, }\nfn bad() -> i64 { 0 }",
            "SPX-O116",
        ),
        (
            "fn str_contains(value: borrow str, needle: borrow str) -> bool { true }\nfn bad() -> i64 { 0 }",
            "SPX-S113",
        ),
    ];
    for (declaration, expected) in cases {
        let source = format!("module test.invalid;\n@id(\"bad.id\")\n{declaration}\n");
        let found = diagnostics(&source);
        assert!(found.iter().any(|item| item.code == expected), "{found:?}");
    }
}

#[test]
fn hostile_hir_cannot_widen_borrowed_str_ownership_or_escape_it() {
    let program = parse(PROGRAM, Path::new("borrowed_str.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let index = resolved
        .functions
        .iter()
        .position(|function| function.id.as_str() == "text.forward")
        .unwrap();

    let mut widened = resolved.clone();
    widened.functions[index].params[0].ownership = OwnershipMode::Own;
    let error = hir::validate(&widened).unwrap_err();
    assert!(error
        .message
        .contains("borrowed `str` must have borrow ownership"));

    let mut escaping = resolved;
    escaping.functions[index].return_type = ResolvedType::Str;
    let error = hir::validate(&escaping).unwrap_err();
    assert!(error.message.contains("cannot return borrowed `str`"));
}

#[test]
fn interpreter_accepts_canonical_json_utf8_and_executes_borrowed_operations() {
    assert_eq!(
        interpreter::parse_argument("\"aé\\u0000z\"").unwrap(),
        ArgumentValue::BorrowedStr("aé\0z".to_owned())
    );
    assert!(interpreter::parse_argument("\"\\u0061\"").is_err());

    let path = temporary_source(PROGRAM);
    let arguments = ["\"aé\\u0000z\"", "\"aé\"", "\"\\u0000\""].map(str::to_owned);
    let result = interpreter::interpret(
        &path,
        "text.classify",
        &arguments,
        &InterpreterOptions::default(),
    )
    .unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(result.returned);
    assert!(result.envelope.contains("\"type\":\"str\""));
    assert!(result.envelope.contains("\"type\":\"i64\",\"value\":\"5\""));
}

#[test]
fn interpreter_contains_has_linear_periodic_worst_case_and_exact_budget() {
    let path = temporary_source(PROGRAM);
    let mut value = "a".repeat(49_152);
    value.replace_range(49_151.., "b");
    let mut needle = "a".repeat(16_384);
    needle.replace_range(16_383.., "b");
    let arguments = [
        serde_json::to_string(&value).unwrap(),
        serde_json::to_string(&needle).unwrap(),
    ];
    let options = InterpreterOptions::new(1_048_576, 1_000_000).unwrap();
    let matched = interpreter::interpret(&path, "text.forward", &arguments, &options).unwrap();
    assert!(matched
        .envelope
        .contains("\"type\":\"bool\",\"value\":\"true\""));

    value.replace_range(49_151.., "a");
    let arguments = [
        serde_json::to_string(&value).unwrap(),
        serde_json::to_string(&needle).unwrap(),
    ];
    let missed = interpreter::interpret(&path, "text.forward", &arguments, &options).unwrap();
    assert!(missed
        .envelope
        .contains("\"type\":\"bool\",\"value\":\"false\""));

    let over = serde_json::to_string(&"a".repeat(32_769)).unwrap();
    let error =
        interpreter::interpret(&path, "text.forward", &[over.clone(), over], &options).unwrap_err();

    // One invocation-root view is charged once even when an internal call
    // aliases it into both operands.
    let exact = serde_json::to_string(&"a".repeat(65_536)).unwrap();
    let aliased = interpreter::interpret(&path, "text.same", &[exact], &options).unwrap();
    assert!(aliased
        .envelope
        .contains("\"type\":\"bool\",\"value\":\"true\""));
    let _ = std::fs::remove_file(path);
    assert!(error.iter().any(|diagnostic| diagnostic
        .message
        .contains("borrowed string invocation exceeds byte budget")));
}
