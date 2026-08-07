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
