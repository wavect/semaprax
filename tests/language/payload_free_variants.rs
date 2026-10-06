//! Executable evidence for equality on payload-free variants.
//!
//! `a == b` and `a != b` over one non-generic variant whose cases all carry no
//! payload compare the selected case. The same corpus returns the same value in
//! the reference interpreter, the native C backend (`run --native`), and Node's
//! WebAssembly engine; the canonical formatter round-trips it; the graph keeps
//! the comparison as an ordinary `binary` node over the nominal operands; and a
//! payload-carrying or generic variant stays a stable `SPX-T207` whose help
//! names `match`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions, DEFAULT_MAX_STEPS};
use semaprax::{format, graph, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const EQUALITY: &str = r#"module app.vareq;

@id("vareq.status")
variant Status {
    @id("vareq.status.todo")
    Todo,
    @id("vareq.status.doing")
    Doing,
    @id("vareq.status.done")
    Done,
}

@id("vareq.rank")
fn rank(code: i64) -> i64
{
    let s = if code == 0 { Status::Todo {} } else { if code == 1 { Status::Doing {} } else { Status::Done {} } };
    let open = s != Status::Done {};
    let doing = Status::Doing {} == s;
    if open && doing { 10 } else { if open { 20 } else { 30 } }
}

@id("app.main")
fn main() -> i64
{
    rank(0) + rank(1) * 100 + rank(2) * 10000
}
"#;

const EQUALITY_RESULT: &str = "301020";

/// Variant-typed parameters and contracts: outside the interpreter's admitted
/// call shapes, so this corpus runs on the two compiled backends.
const CONTRACT: &str = r#"module app.varcontract;

@id("varcontract.status")
variant Status {
    @id("varcontract.status.todo")
    Todo,
    @id("varcontract.status.done")
    Done,
}

@id("varcontract.finish")
fn finish(s: Status, t: Status) -> i64
    requires (s != Status::Done {})
{
    if s == t { 1 } else { 2 }
}

@id("app.main")
fn main() -> i64
{
    finish(Status::Todo {}, Status::Todo {}) + finish(Status::Todo {}, Status::Done {}) * 10 + finish(Status::Done {}, Status::Done {})
}
"#;

fn write_source(source: &str, stem: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-vareq-{}-{stem}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join("main.spx");
    std::fs::write(&path, source).expect("corpus source");
    path
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    verify::verify(&parse(source, Path::new("vareq-diag.spx")).unwrap())
}

fn interpret_main(source: &str, stem: &str) -> serde_json::Value {
    let path = write_source(source, stem);
    let options = InterpreterOptions::new(65536, DEFAULT_MAX_STEPS).unwrap();
    let envelope = interpreter::interpret(&path, "app.main", &[], &options)
        .expect("interpretation")
        .envelope;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    serde_json::from_str(&envelope).unwrap()
}

/// `semaprax run <file> --native`: stdout and the exit status.
fn run_native(source: &str, stem: &str) -> Option<(bool, String, String)> {
    if !command_available("clang") && !command_available("cc") {
        return None;
    }
    let path = write_source(source, stem);
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("run")
        .arg(&path)
        .arg("--native")
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    Some((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

const NODE_RUNNER: &str = r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const SPX_MIN = -(2n ** 63n), SPX_MAX = 2n ** 63n - 1n;
const bounded = (value, what) => {
  if (value < SPX_MIN || value > SPX_MAX) throw new RangeError(`checked ${what} failure`);
  return value;
};
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => bounded(a + b, "addition"),
  spx_sub: (a, b) => bounded(a - b, "subtraction"),
  spx_mul: (a, b) => bounded(a * b, "multiplication"),
  spx_div: (a, b) => a / b,
  spx_rem: (a, b) => a % b,
  spx_neg: (a) => bounded(-a, "negation"),
  spx_contract_fail: () => { console.log("contract-failure"); process.exit(0); },
}});
console.log(String(instance.exports.semaprax_main()));
"#;

/// Runs `semaprax_main` of the module's Wasm bytes in Node; the printed
/// result, or `contract-failure` when the contract import fired.
fn run_wasm(source: &str, stem: &str) -> Option<String> {
    if !command_available("node") {
        return None;
    }
    let program = parse(source, Path::new("vareq-wasm.spx")).unwrap();
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, semaprax::wasm::emit_module(&program).unwrap());
    let path = write_source(source, stem);
    let directory = path.parent().unwrap().to_owned();
    let wasm = directory.join("main.wasm");
    let script = directory.join("runner.mjs");
    std::fs::write(&wasm, bytes).unwrap();
    std::fs::write(&script, NODE_RUNNER).unwrap();
    let output = Command::new("node")
        .arg(&script)
        .arg(&wasm)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "Node leg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[test]
fn variant_equality_verifies_and_round_trips_canonically() {
    for source in [EQUALITY, CONTRACT] {
        let program = parse(source, Path::new("vareq.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        assert_eq!(canonical, source, "the corpus is already canonical");
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    }
}

#[test]
fn variant_equality_graph_keeps_a_binary_node_over_nominal_operands() {
    let program = parse(EQUALITY, Path::new("vareq.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(json, graph::to_json(&program).unwrap());
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut pending = vec![&wire];
    let mut variant_comparisons = Vec::new();
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if object.get("kind").and_then(|kind| kind.as_str()) == Some("binary")
                    && object["left"]["type_id"] == "nominal:12:vareq.status:0:"
                {
                    assert_eq!(object["type_id"], "bool");
                    assert_eq!(object["right"]["type_id"], object["left"]["type_id"]);
                    variant_comparisons.push(object["op"].as_str().unwrap().to_owned());
                }
                pending.extend(object.values());
            }
            serde_json::Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    variant_comparisons.sort();
    assert_eq!(variant_comparisons, ["!=", "=="], "{json}");
}

#[test]
fn variant_equality_agrees_on_interpreter_native_and_wasm() {
    let parsed = interpret_main(EQUALITY, "interpret");
    let outcome = &parsed["payload"]["outcome"];
    assert_eq!(outcome["kind"], "returned", "{parsed}");
    assert_eq!(outcome["value"], EQUALITY_RESULT, "{parsed}");
    if let Some((success, stdout, stderr)) = run_native(EQUALITY, "native") {
        assert!(success, "{stderr}");
        assert_eq!(stdout, EQUALITY_RESULT);
    }
    if let Some(stdout) = run_wasm(EQUALITY, "wasm") {
        assert_eq!(stdout, EQUALITY_RESULT);
    }
}

#[test]
fn variant_inequality_contract_fails_on_native_and_wasm() {
    if let Some((success, _, stderr)) = run_native(CONTRACT, "contract-native") {
        assert!(!success);
        assert!(
            stderr.contains("requires s != Status::Done {} in varcontract.finish"),
            "{stderr}"
        );
    }
    if let Some(stdout) = run_wasm(CONTRACT, "contract-wasm") {
        assert_eq!(stdout, "contract-failure");
    }
    let passing = CONTRACT.replace(" + finish(Status::Done {}, Status::Done {})", "");
    if let Some((success, stdout, stderr)) = run_native(&passing, "contract-native-ok") {
        assert!(success, "{stderr}");
        assert_eq!(stdout, "21");
    }
    if let Some(stdout) = run_wasm(&passing, "contract-wasm-ok") {
        assert_eq!(stdout, "21");
    }
}

#[test]
fn payload_or_generic_variant_equality_is_spx_t207_naming_match() {
    let source = r#"module app.eqbad;

@id("eqbad.shape")
variant Shape {
    @id("eqbad.shape.dot")
    Dot,
    @id("eqbad.shape.square")
    Square {
        @id("eqbad.shape.square.side")
        side: i64,
    },
}

@id("app.main")
fn main() -> i64
{
    let a = Shape::Dot {};
    let o = Option<i64>::None {};
    if a == Shape::Dot {} { 1 } else { if o != Option<i64>::None {} { 2 } else { 0 } }
}
"#;
    let found = diagnostics(source);
    let rendered = found
        .iter()
        .map(|diagnostic| {
            format!(
                "{} {} | {}",
                diagnostic.code,
                diagnostic.message,
                diagnostic.help.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rendered,
        [
            "SPX-T207 equality on variant `Shape` requires a non-generic variant whose cases \
             all carry no payload | test the case with `match` instead, e.g. \
             `match value { Shape::Dot {} => true, _ => false, }`",
            "SPX-T207 equality on variant `Option<i64>` requires a non-generic variant whose \
             cases all carry no payload | test the case with `match` instead, e.g. \
             `match value { Option::None {} => true, _ => false, }`",
        ],
        "{found:?}"
    );
}

#[test]
fn variant_equality_across_distinct_variants_is_spx_t207() {
    let source = EQUALITY.replace(
        "@id(\"vareq.rank\")",
        "@id(\"vareq.other\")\nvariant Other {\n    @id(\"vareq.other.only\")\n    Only,\n}\n\n@id(\"vareq.rank\")",
    )
    .replace("Status::Doing {} == s", "Other::Only {} == s");
    let found = diagnostics(&source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code, "SPX-T207");
    assert_eq!(
        found[0].message,
        "equality operands must have the same type"
    );
}
