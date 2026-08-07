use std::path::Path;
use std::process::Command;

use semaprax::{codegen, graph, parse, verify};

const VALID: &str = r#"
module test.answer;

@id("math.add")
fn add(a: i64, b: i64) -> i64
    requires a >= 0
    ensures result == a + b
{
    a + b
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(20, 22)
}
"#;

#[test]
fn valid_program_has_stable_graph() {
    let first = parse(VALID, Path::new("valid.spx")).unwrap();
    let second = parse(VALID, Path::new("elsewhere.spx")).unwrap();
    assert!(verify::verify(&first).is_empty());
    assert_eq!(graph::revision(&first), graph::revision(&second));
    let json = graph::to_json(&first);
    assert!(json.contains("\"id\":\"math.add\""));
    assert!(json.contains("\"calls\":[\"add\"]"));
}

#[test]
fn context_slice_follows_calls() {
    let program = parse(VALID, Path::new("valid.spx")).unwrap();
    let context = graph::context_json(&program, "app.main", 1).unwrap();
    assert!(context.contains("\"name\":\"main\""));
    assert!(context.contains("\"name\":\"add\""));
}

#[test]
fn context_and_graph_include_contract_dependencies() {
    let source = r#"
module test.contract_graph;
@id("contract.pure")
fn pure(value: i64) -> i64 { value }
@id("contract.guarded")
fn guarded() -> i64 requires pure(1) == 1 ensures pure(result) == 42 { 42 }
@id("app.main")
fn main() -> i64 { guarded() }
"#;
    let program = parse(source, Path::new("contract-graph.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let context = graph::context_json(&program, "contract.guarded", 1).unwrap();
    assert!(context.contains("\"id\":\"contract.pure\""));
    assert!(context.contains("\"requires_graph\":[{"));
    assert!(context.contains("\"ensures_graph\":[{"));
    assert!(context.contains("\"calls\":[\"pure\",\"pure\"]"));
}

#[test]
fn missing_effect_is_rejected() {
    let source = r#"
module test.effects;
permit { clock.read }
@id("clock.tick")
fn tick(value: i64) -> i64 uses { clock.read } { value + 1 }
@id("app.main")
fn main() -> i64 { tick(41) }
"#;
    let program = parse(source, Path::new("effects.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-E102"));
}

#[test]
fn contracts_cannot_call_effectful_functions() {
    let source = r#"
module test.contract_effect;
permit { clock.read }
@id("clock.tick")
fn tick(value: i64) -> i64 uses { clock.read } { value + 1 }
@id("app.main")
fn main() -> i64 ensures tick(result) == 43 { 42 }
"#;
    let program = parse(source, Path::new("contract-effect.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-C102"));
}

#[test]
fn native_backend_produces_executable() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(VALID, Path::new("valid.spx")).unwrap();
    let output = std::env::temp_dir().join(format!("semaprax-test-{}", std::process::id()));
    codegen::build(&program, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    let _ = std::fs::remove_file(&output);
    assert!(result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
}

#[test]
fn backends_reject_unverified_programs_without_panicking() {
    let source = r#"
module test.invalid_backend;
@id("app.main")
fn main() -> i64 { if true { missing } else { 0 } }
"#;
    let program = parse(source, Path::new("invalid-backend.spx")).unwrap();
    assert_eq!(codegen::emit_c(&program).unwrap_err().code, "SPX-T202");
    assert_eq!(
        semaprax::wasm::emit_module(&program).unwrap_err().code,
        "SPX-T202"
    );
}
