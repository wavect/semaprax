//! Statement `if`: `if <condition> { <statements> }` with optional `else if`
//! and `else` branches in statement position, including `while` and `for`
//! bodies.
//!
//! It is parse-level sugar for the value discard
//! `let _if<n> = if <condition> { <statements>; 0 } else { 0 };`, so the
//! sugared corpus and its canonical spelling share one AST, one graph, and one
//! result on the reference interpreter, native C, and Core Wasm. A statement
//! `if` that ends a block that needs a value keeps the diagnostics the value
//! grammar always produced.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Copy-scalar statement `if`s in straight-line code and `while` bodies; the
/// Core Wasm profile admits all of it.
const SUGAR: &str = r#"module test.statement_if;

@id("stmt.tally")
fn tally(limit: i64) -> i64
{
    let mut evens = 0;
    let mut marks = 0;
    let mut i = 0;
    while i < limit {
        if i % 2 == 0 { evens = evens + 1; }
        if i == 3 { marks = marks + 100; } else if i == 5 { marks = marks + 1000; } else { marks = marks + 1; }
        if i > 7 {
            marks = marks + 5;
            if marks > 1110 { marks = marks - 1; }
        }
        i = i + 1;
        i < limit
    }
    evens * 10000 + marks
}

@id("stmt.clamp")
fn clamp(value: i64) -> i64
{
    let mut level = value;
    if level < 0 { level = 0; };
    if level > 100 { level = 100; } else { }
    if level == 50 { 7 } else { 8 }
    let _if1 = 2;
    if level == 42 { level = level * _if1; }
    level
}

@id("main")
fn main() -> i64
{
    tally(10) + clamp(-5) + clamp(500) + clamp(42)
}
"#;

/// What `fmt` writes for [`SUGAR`]: every statement `if` is spelled as the
/// discarded value `if`, and a discard skips the `_if1` the source already
/// names.
const CANONICAL: &str = r#"module test.statement_if;

@id("stmt.tally")
fn tally(limit: i64) -> i64
{
    let mut evens = 0;
    let mut marks = 0;
    let mut i = 0;
    while i < limit {
        let _if2 = if i % 2 == 0 { evens = evens + 1; 0 } else { 0 };
        let _if3 = if i == 3 { marks = marks + 100; 0 } else { if i == 5 { marks = marks + 1000; 0 } else { marks = marks + 1; 0 } };
        let _if5 = if i > 7 { marks = marks + 5; let _if4 = if marks > 1110 { marks = marks - 1; 0 } else { 0 }; 0 } else { 0 };
        i = i + 1;
        i < limit
    }
    evens * 10000 + marks
}

@id("stmt.clamp")
fn clamp(value: i64) -> i64
{
    let mut level = value;
    let _if6 = if level < 0 { level = 0; 0 } else { 0 };
    let _if7 = if level > 100 { level = 100; 0 } else { 0 };
    let _if8 = if level == 50 { 7 } else { 8 };
    let _if1 = 2;
    let _if9 = if level == 42 { level = level * _if1; 0 } else { 0 };
    level
}

@id("main")
fn main() -> i64
{
    tally(10) + clamp(-5) + clamp(500) + clamp(42)
}
"#;

/// tally(10): five even `i`; marks 100 (i=3) + 1000 (i=5) + 8 ones + 5 + 5,
/// minus 1 at i=8 and i=9 once past 1110 = 1116. clamp: 0, 100, 84.
const MAIN_RESULT: i64 = 50_000 + 1_116 + 100 + 84;

/// A statement `if` in a `for` body over an immutable `Vec<i64>`.
const FOR_BODY: &str = r#"module test.statement_if_for;

@id("main")
fn main() -> i64
{
    let mut building = vec_with_capacity<i64>(4usize);
    building = vec_push<i64>(building, 10);
    building = vec_push<i64>(building, 33);
    building = vec_push<i64>(building, 7);
    let values = building;
    let mut odd = 0;
    let mut big = 0;
    for item in values {
        if item % 2 == 1 { odd = odd + 1; }
        if item > 9 { big = big + item; } else if item > 5 { big = big + 1; }
        0
    }
    odd * 1000 + big
}
"#;

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn write_source(stem: &str, source: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-statement-if-{}-{stem}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("main.spx");
    std::fs::write(&path, source).unwrap();
    path
}

fn run(source: &str, native: bool) -> String {
    let path = write_source(if native { "native" } else { "run" }, source);
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command.arg("run").arg(&path);
    if native {
        command.arg("--native");
    }
    let output = command.output().unwrap();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    assert!(
        output.status.success(),
        "native={native}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn rejection(source: &str) -> semaprax::diagnostic::Diagnostic {
    parse(source, Path::new("statement-if.spx")).unwrap_err()
}

#[test]
fn statement_if_is_sugar_for_the_canonical_value_discard() {
    let sugar = parse(SUGAR, Path::new("statement-if.spx")).unwrap();
    assert!(verify::verify(&sugar).is_empty());
    assert_eq!(format::canonical(&sugar), CANONICAL);
    let canonical = parse(CANONICAL, Path::new("statement-if.spx")).unwrap();
    assert_eq!(format::canonical(&canonical), CANONICAL);
    assert_eq!(graph::revision(&sugar), graph::revision(&canonical));
    let json = graph::to_json(&sugar).unwrap();
    assert_eq!(json, graph::to_json(&canonical).unwrap());
    let document: Value = serde_json::from_str(&json).unwrap();
    let mut pending = vec![&document];
    let mut ifs = 0;
    while let Some(node) = pending.pop() {
        match node {
            Value::Object(map) => {
                if map.get("kind").and_then(Value::as_str) == Some("if") {
                    ifs += 1;
                }
                pending.extend(map.values());
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    // Eight discarded `if`s plus the nested `else if`.
    assert_eq!(ifs, 9, "if nodes in the graph");
    hir::validate(&hir::resolve(&sugar).unwrap()).unwrap();
}

#[test]
fn statement_if_runs_identically_on_the_interpreter_and_native_c() {
    let path = write_source("interpret", SUGAR);
    let result = interpreter::interpret(&path, "main", &[], &InterpreterOptions::default())
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(
        envelope["payload"]["outcome"]["value"],
        MAIN_RESULT.to_string()
    );
    assert_eq!(run(SUGAR, false), MAIN_RESULT.to_string());
    assert_eq!(run(FOR_BODY, false), "2044");
    if command_available("clang") {
        assert_eq!(run(SUGAR, true), MAIN_RESULT.to_string());
        assert_eq!(run(FOR_BODY, true), "2044");
    }
}

const NODE_RUNNER: &str = r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const SPX_MIN = -(2n ** 63n), SPX_MAX = 2n ** 63n - 1n;
const bounded = (value) => {
  if (value < SPX_MIN || value > SPX_MAX) throw new RangeError("checked failure");
  return value;
};
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => bounded(a + b),
  spx_sub: (a, b) => bounded(a - b),
  spx_mul: (a, b) => bounded(a * b),
  spx_div: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("division"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("remainder"); return a % b; },
  spx_neg: (a) => bounded(-a),
  spx_contract_fail: () => { throw new Error("contract"); },
}});
console.log(String(instance.exports.semaprax_main()));
"#;

#[test]
fn statement_if_runs_on_core_wasm() {
    if !command_available("node") {
        return;
    }
    let program = parse(SUGAR, Path::new("statement-if.spx")).unwrap();
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    let canonical = parse(CANONICAL, Path::new("statement-if.spx")).unwrap();
    assert_eq!(bytes, semaprax::wasm::emit_module(&canonical).unwrap());
    let path = write_source("wasm", SUGAR);
    let directory = path.parent().unwrap();
    std::fs::write(directory.join("program.wasm"), bytes).unwrap();
    std::fs::write(directory.join("runner.mjs"), NODE_RUNNER).unwrap();
    let output = Command::new("node")
        .arg(directory.join("runner.mjs"))
        .arg(directory.join("program.wasm"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(directory);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        MAIN_RESULT.to_string()
    );
}

#[test]
fn a_statement_if_cannot_end_a_block_that_needs_a_value() {
    let diagnostic = rejection(
        "module t;\n@id(\"main\")\nfn main() -> i64\n{\n    let mut x = 0;\n    if x == 0 { x = 1; }\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(
        diagnostic
            .help
            .as_deref()
            .is_some_and(|help| help.contains("statement `if` yields no value")),
        "{diagnostic}"
    );
    // A valued `if` without `else` at the tail keeps its missing-else rule.
    let diagnostic =
        rejection("module t;\n@id(\"main\")\nfn main() -> i64\n{\n    if true { 42 }\n}\n");
    assert_eq!(diagnostic.code, "SPX-P104");
    assert_eq!(diagnostic.message, "expected `else`");
}

#[test]
fn statement_if_keeps_the_statement_rules_inside_its_branches() {
    // An expression statement stays refused inside a branch.
    let diagnostic = rejection(
        "module t;\n@id(\"main\")\nfn main() -> i64\n{\n    let x = 0;\n    if x == 0 { string_len(\"a\"); }\n    x\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    // A valued `if` followed by `;` stays an expression statement.
    let diagnostic = rejection(
        "module t;\n@id(\"main\")\nfn main() -> i64\n{\n    let x = 0;\n    if x == 0 { true } else { false };\n    x\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    // The branches of a statement `if` are checked like any value `if`.
    let program = parse(
        "module t;\n@id(\"main\")\nfn main() -> i64\n{\n    let x = 0;\n    if x { }\n    x\n}\n",
        Path::new("statement-if.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T210"),
        "{diagnostics:?}"
    );
}

#[test]
fn a_branch_value_is_discarded_by_its_own_binding() {
    let source = "module t;\n\n@id(\"main\")\nfn main() -> i64\n{\n    let mut x = 1;\n    if x == 1 { x = 2; x == 2 }\n    x\n}\n";
    let program = parse(source, Path::new("statement-if.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert!(format::canonical(&program)
        .contains("let _if2 = if x == 1 { x = 2; let _if1 = x == 2; 0 } else { 0 };"));
    assert_eq!(run(source, false), "2");
}
