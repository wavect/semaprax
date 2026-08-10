use std::path::Path;
use std::process::Command;

use semaprax::{codegen, format, graph, parse, verify, wasm};

const CONTROL_FLOW: &str = r#"
module examples.control_flow;

@id("flow.choose")
fn choose(flag: bool, base: i64) -> i64 {
    let first = base + 1;
    if flag {
        let second = first + 1;
        second
    } else {
        0
    }
}

@id("app.main")
fn main() -> i64 {
    let answer = choose(true, 40);
    answer
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("control-flow.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn let_and_if_are_canonical_and_graph_visible() {
    let program = parse(CONTROL_FLOW, Path::new("control-flow.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert!(canonical.contains("let answer = choose(true, 40);"));
    assert!(canonical.contains("if flag"));
    let graph = graph::to_json(&program).unwrap();
    assert!(graph.contains("\"schema\":\"semaprax.graph.v9\""));
    assert!(graph.contains("\"kind\":\"let\""));
    assert!(graph.contains("\"kind\":\"if\""));
    assert!(graph.contains("\"expressions\":\"revision-scoped-structural\""));
    assert_eq!(
        format!("{graph}\n"),
        include_str!("snapshots/control_flow.graph.json")
    );
}

#[test]
fn malformed_let_and_if_have_stable_parser_diagnostics() {
    let missing_equals = r#"
module test.malformed;
@id("app.main")
fn main() -> i64 { let answer 42; answer }
"#;
    assert_eq!(
        parse(missing_equals, Path::new("missing-equals.spx"))
            .unwrap_err()
            .code,
        "SPX-P106"
    );

    let missing_else = r#"
module test.malformed;
@id("app.main")
fn main() -> i64 { if true { 42 } }
"#;
    assert_eq!(
        parse(missing_else, Path::new("missing-else.spx"))
            .unwrap_err()
            .code,
        "SPX-P104"
    );

    let empty_block = r#"
module test.malformed;
@id("app.main")
fn main() -> i64 { if true {} else { 42 } }
"#;
    assert_eq!(
        parse(empty_block, Path::new("empty-block.spx"))
            .unwrap_err()
            .code,
        "SPX-P203"
    );
}

#[test]
fn native_and_wasm_execute_control_flow() {
    let program = parse(CONTROL_FLOW, Path::new("control-flow.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());

    if Command::new("clang").arg("--version").output().is_ok() {
        let output = std::env::temp_dir().join(format!(
            "semaprax-control-flow-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        codegen::build(&program, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        let _ = std::fs::remove_file(output);
        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    }

    let first = wasm::emit_module(&program).unwrap();
    let second = wasm::emit_module(&program).unwrap();
    assert_eq!(first, second);

    if Command::new("node").arg("--version").output().is_ok() {
        let output =
            std::env::temp_dir().join(format!("semaprax-control-flow-web-{}", std::process::id()));
        wasm::build_web(&program, &output).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(output);
        assert!(
            result.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    }
}

#[test]
fn native_backend_evaluates_call_arguments_left_to_right() {
    let source = r#"
module test.order;
@id("order.first")
fn first() -> i64 { 1 / 0 }
@id("order.second")
fn second() -> i64 { 9223372036854775807 + 1 }
@id("order.pick")
fn pick(left: i64, right: i64) -> i64 { left }
@id("app.main")
fn main() -> i64 { pick(first(), second()) }
"#;
    let program = parse(source, Path::new("order.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    if Command::new("clang").arg("--version").output().is_ok() {
        let output = std::env::temp_dir().join(format!(
            "semaprax-order-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        codegen::build(&program, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        let _ = std::fs::remove_file(output);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("invalid division"));
    }
    if Command::new("node").arg("--version").output().is_ok() {
        let output =
            std::env::temp_dir().join(format!("semaprax-order-web-{}", std::process::id()));
        wasm::build_web(&program, &output).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .arg("error:invalid division")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(output);
        assert!(
            result.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            "error:invalid division"
        );
    }
}

#[test]
fn short_circuit_skips_trapping_branch() {
    let source = r#"
module test.short_circuit;
@id("app.main")
fn main() -> i64 {
    if false && 1 / 0 == 0 { 0 } else { 42 }
}
"#;
    let program = parse(source, Path::new("short-circuit.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    if Command::new("clang").arg("--version").output().is_ok() {
        let output = std::env::temp_dir().join(format!(
            "semaprax-short-circuit-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        codegen::build(&program, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        let _ = std::fs::remove_file(output);
        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    }
    if Command::new("node").arg("--version").output().is_ok() {
        let output =
            std::env::temp_dir().join(format!("semaprax-short-circuit-web-{}", std::process::id()));
        wasm::build_web(&program, &output).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .arg("42")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(output);
        assert!(
            result.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    }
}

#[test]
fn lexical_ownership_tracks_moves_and_branch_joins() {
    let header = r#"
module test.lexical_ownership;
@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64 { 1 }
"#;
    let moved = format!(
        "{header}\n@id(\"buffer.bad\")\nfn bad(buffer: own Buffer) -> i64 {{ let alias = buffer; inspect(buffer) + consume(alias) }}\n@id(\"app.main\")\nfn main() -> i64 {{ 42 }}"
    );
    assert!(diagnostics(&moved)
        .iter()
        .any(|item| item.code == "SPX-O101"));

    let maybe_moved = format!(
        "{header}\n@id(\"buffer.maybe\")\nfn maybe(flag: bool, buffer: own Buffer) -> i64 {{ let value = if flag {{ consume(buffer) }} else {{ 0 }}; inspect(buffer) + value }}\n@id(\"app.main\")\nfn main() -> i64 {{ 42 }}"
    );
    assert!(diagnostics(&maybe_moved)
        .iter()
        .any(|item| item.code == "SPX-O107"));

    let selected = format!(
        "{header}\n@id(\"buffer.select\")\nfn select(flag: bool, left: own Buffer, right: own Buffer) -> Buffer {{ let chosen = if flag {{ left }} else {{ right }}; let observed = inspect(left) + inspect(right); chosen }}\n@id(\"app.main\")\nfn main() -> i64 {{ 42 }}"
    );
    let selected_diagnostics = diagnostics(&selected);
    assert_eq!(
        selected_diagnostics
            .iter()
            .filter(|item| item.code == "SPX-O107")
            .count(),
        2
    );
    assert!(!selected_diagnostics
        .iter()
        .any(|item| item.code == "SPX-O101"));

    let borrowed = format!(
        "{header}\n@id(\"buffer.good\")\nfn good(buffer: borrow Buffer) -> i64 {{ let alias = buffer; inspect(alias) + inspect(buffer) }}\n@id(\"app.main\")\nfn main() -> i64 {{ 42 }}"
    );
    assert!(diagnostics(&borrowed).is_empty());
}

#[test]
fn preconditions_use_entry_state_and_extra_arguments_are_checked() {
    let source = r#"
module test.contract_state;
@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64 { 1 }
@id("buffer.guarded")
fn guarded(buffer: own Buffer) -> i64
    requires inspect(buffer) == 1
{
    consume(buffer)
}
@id("arity.zero")
fn zero() -> i64 { 0 }
@id("arity.bad")
fn bad(buffer: own Buffer) -> i64 { zero(consume(buffer)) + inspect(buffer) }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let found = diagnostics(source);
    assert!(!found
        .iter()
        .any(|item| { item.code == "SPX-O101" && item.message.contains("guarded") }));
    assert!(found.iter().any(|item| item.code == "SPX-T204"));
    assert!(found.iter().any(|item| item.code == "SPX-O101"));
}

#[test]
fn invalid_control_flow_and_reserved_locals_are_rejected() {
    let source = r#"
module test.invalid_flow;
@id("flow.bad_condition")
fn bad_condition() -> i64 { if 1 { 1 } else { 2 } }
@id("flow.bad_branch")
fn bad_branch() -> i64 { if true { 1 } else { false } }
@id("flow.bad_local")
fn bad_local() -> i64 { let result = 1; result }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let found = diagnostics(source);
    assert!(found.iter().any(|item| item.code == "SPX-T210"));
    assert!(found.iter().any(|item| item.code == "SPX-T211"));
    assert!(found.iter().any(|item| item.code == "SPX-S109"));
}
