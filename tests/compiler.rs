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
    let json = graph::to_json(&first).unwrap();
    assert!(json.contains("\"id\":\"math.add\""));
    assert!(json.contains("\"calls\":[\"math.add\"]"));
}

#[test]
fn revision_is_a_canonical_sha256_content_address() {
    let program = parse(VALID, Path::new("revision.spx")).unwrap();
    let revision = graph::revision(&program);
    let digest = revision.strip_prefix("sha256:").unwrap();
    assert_eq!(revision.len(), 71);
    assert_eq!(digest.len(), 64);
    assert!(digest
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert!(!revision.contains("fnv1a64"));

    let changed = VALID.replace("add(20, 22)", "add(20, 23)");
    let changed = parse(&changed, Path::new("revision-changed.spx")).unwrap();
    assert_ne!(revision, graph::revision(&changed));

    let graph_json = graph::to_json(&program).unwrap();
    let context = graph::context_json(&program, "app.main", 0)
        .unwrap()
        .unwrap();
    let encoded = semaprax::diagnostic::quote_json(&revision);
    assert!(graph_json.contains(&format!("\"revision\":{encoded}")));
    assert!(context.contains(&format!("\"revision\":{encoded}")));
}

#[test]
fn context_slice_follows_calls() {
    let program = parse(VALID, Path::new("valid.spx")).unwrap();
    let by_id = graph::context_json(&program, "app.main", 1)
        .unwrap()
        .unwrap();
    let by_name = graph::context_json(&program, "main", 1).unwrap().unwrap();
    assert_eq!(by_id, by_name);
    assert!(by_id.contains("\"name\":\"main\""));
    assert!(by_id.contains("\"name\":\"add\""));
    assert!(by_id.contains(
        "\"view\":{\"kind\":\"context\",\"root\":\"app.main\",\"depth\":1,\"truncated\":false,\"frontier\":[]}"
    ));

    let bounded = graph::context_json(&program, "app.main", 0)
        .unwrap()
        .unwrap();
    assert!(bounded.contains(
        "\"view\":{\"kind\":\"context\",\"root\":\"app.main\",\"depth\":0,\"truncated\":true,\"frontier\":[\"math.add\"]}"
    ));
    assert!(!bounded.contains("\"id\":\"math.add\",\"kind\":\"function\""));
}

#[test]
fn graph_v10_exposes_resolved_identity_types_ownership_and_facts() {
    let program = parse(VALID, Path::new("resolved-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(json.contains("\"entrypoint\":\"app.main\""));
    assert!(json.contains("\"identity_origin\":\"explicit\",\"persistent\":true"));
    assert!(json.contains("\"id\":\"declaration:8:math.add:value:param:1:0\",\"name\":\"a\""));
    assert!(json.contains("\"result_id\":\"declaration:8:math.add:value:result:0:\""));
    assert!(json.contains("\"callee\":\"math.add\""));
    assert!(json.contains("\"type_id\":\"i64\",\"ownership_mode\":\"value\""));
    assert!(json.contains("\"layout_key\":\"scalar:i64\""));
}

#[test]
fn graph_i64_literals_are_lossless_for_javascript_agents() {
    let source = r#"
module test.lossless_i64_graph;
@id("app.main")
fn main() -> i64 { 9223372036854775807 }
"#;
    let program = parse(source, Path::new("lossless-i64-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"kind\":\"int\",\"value\":\"9223372036854775807\""));
    assert!(!json.contains("\"value\":9223372036854775807"));

    if Command::new("node").arg("--version").output().is_ok() {
        let script = r#"
const graph = JSON.parse(process.argv[1]);
let found = false;
JSON.stringify(graph, (key, value) => {
  if (key === "value" && value === "9223372036854775807") found = true;
  return value;
});
if (!found) process.exit(2);
"#;
        let output = Command::new("node")
            .arg("-e")
            .arg(script)
            .arg(&json)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Node failed to preserve graph i64: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn context_includes_referenced_nominal_types_without_unrelated_resources() {
    let source = r#"
module test.type_context;
@id("type.used")
resource Used {
    @id("type.used.drop")
    drop trivial;
}
@id("type.unused")
resource Unused {
    @id("type.unused.drop")
    drop trivial;
}
@id("used.inspect")
fn inspect(value: borrow Used) -> i64 { 1 }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let program = parse(source, Path::new("type-context.spx")).unwrap();
    let context = graph::context_json(&program, "used.inspect", 0)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"id\":\"type.used\""));
    assert!(context.contains("\"type\":{\"kind\":\"nominal\",\"declaration\":\"type.used\""));
    assert!(!context.contains("type.unused"));
    assert!(!context.contains("\"id\":\"app.main\",\"kind\":\"function\""));
}

#[test]
fn graph_boundaries_reject_invalid_ast() {
    let source = r#"
module test.invalid_graph;
@id("app.main")
fn main() -> i64 { missing }
"#;
    let program = parse(source, Path::new("invalid-graph.spx")).unwrap();
    assert_eq!(graph::to_json(&program).unwrap_err()[0].code, "SPX-T202");
    assert_eq!(
        graph::context_json(&program, "main", 0).unwrap_err()[0].code,
        "SPX-T202"
    );
}

#[test]
fn graph_uses_canonical_source_revision_and_marks_automatic_ids_unstable() {
    let automatic = r#"
module test.automatic_graph;
fn main() -> i64 { 42 }
"#;
    let automatic = parse(automatic, Path::new("automatic.spx")).unwrap();
    let json = graph::to_json(&automatic).unwrap();
    assert!(json.contains(&format!(
        "\"revision\":{}",
        semaprax::diagnostic::quote_json(&graph::revision(&automatic))
    )));
    assert!(json.contains("\"identity_origin\":\"automatic\",\"persistent\":false"));
}

#[test]
fn context_prefers_an_exact_declaration_id_over_a_colliding_display_name() {
    let source = r#"
module test.context_collision;
@id("x")
fn target() -> i64 { 1 }
@id("other.x")
fn x() -> i64 { 2 }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let program = parse(source, Path::new("context-collision.spx")).unwrap();
    let context = graph::context_json(&program, "x", 0).unwrap().unwrap();
    assert!(context.contains("\"root\":\"x\""));
    assert!(context.contains("\"id\":\"x\",\"kind\":\"function\",\"name\":\"target\""));
    assert!(!context.contains("other.x"));
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
    let context = graph::context_json(&program, "contract.guarded", 1)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"id\":\"contract.pure\""));
    assert!(context.contains("\"requires_graph\":[{"));
    assert!(context.contains("\"ensures_graph\":[{"));
    assert!(context.contains("\"calls\":[\"contract.pure\"]"));
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
