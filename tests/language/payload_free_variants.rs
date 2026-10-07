//! Executable evidence for equality on payload-free variants and for
//! or-patterns over payload-free variant cases.
//!
//! `a == b` and `a != b` over one non-generic variant whose cases all carry no
//! payload compare the selected case; `A {} | B {} => …` selects one arm for
//! several payload-free cases. Each corpus returns the same value in the
//! reference interpreter (where its shapes are admitted), the native C backend
//! (`run --native`), and Node's WebAssembly engine; the canonical formatter
//! round-trips it; the graph keeps the comparison as an ordinary `binary` node
//! and the or-pattern as an `or_pattern` of `variant_pattern` alternatives; and
//! every rejected shape keeps a stable diagnostic.

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

const OR_PATTERNS: &str = r#"module app.orpat;

@id("orpat.status")
variant Status {
    @id("orpat.status.todo")
    Todo,
    @id("orpat.status.doing")
    Doing,
    @id("orpat.status.review")
    Review,
    @id("orpat.status.done")
    Done,
}

@id("orpat.shape")
variant Shape {
    @id("orpat.shape.dot")
    Dot,
    @id("orpat.shape.empty")
    Empty,
    @id("orpat.shape.square")
    Square {
        @id("orpat.shape.square.side")
        side: i64,
    },
}

@id("orpat.weight")
fn weight(code: i64) -> i64
{
    let s = if code == 0 { Status::Todo {} } else { if code == 1 { Status::Doing {} } else { if code == 2 { Status::Review {} } else { Status::Done {} } } };
    let open = match s { Status::Todo {} | Status::Doing {} | Status::Review {} => 1, Status::Done {} => 0, };
    let late = match s { Status::Todo {} => 0, Status::Review {} | Status::Done {} => 100, Status::Doing {} => 0, };
    open + late
}

@id("orpat.area")
fn area(code: i64) -> i64
{
    let shape = if code == 0 { Shape::Dot {} } else { if code == 1 { Shape::Empty {} } else { Shape::Square { side: code } } };
    match shape { Shape::Dot {} | Shape::Empty {} => 0, Shape::Square { side: s } => s * s, }
}

@id("app.main")
fn main() -> i64
{
    weight(0) + weight(1) * 1000 + weight(2) * 1000000 + weight(3) * 1000000000 + area(0) + area(1) * 3 + area(3) * 7
}
"#;

const OR_PATTERNS_RESULT: &str = "100101001064";

/// A wildcard after an or-pattern arm: outside the interpreter's admitted
/// variant-match shapes, so this corpus runs on the two compiled backends.
const OR_WILDCARD: &str = r#"module app.orwild;

@id("orwild.status")
variant Status {
    @id("orwild.status.todo")
    Todo,
    @id("orwild.status.doing")
    Doing,
    @id("orwild.status.done")
    Done,
}

@id("orwild.active")
fn active(s: Status) -> bool
{
    match s { Status::Todo {} | Status::Doing {} => true, _ => false, }
}

@id("app.main")
fn main() -> i64
{
    let a = if (active(Status::Todo {})) { 1 } else { 0 };
    let b = if (active(Status::Doing {})) { 10 } else { 0 };
    let c = if (active(Status::Done {})) { 100 } else { 0 };
    a + b + c
}
"#;

#[test]
fn or_patterns_verify_and_round_trip_canonically() {
    for source in [OR_PATTERNS, OR_WILDCARD] {
        let program = parse(source, Path::new("orpat.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        assert_eq!(canonical, source, "the corpus is already canonical");
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    }
}

#[test]
fn or_pattern_graph_keeps_variant_alternatives_in_one_arm() {
    let program = parse(OR_PATTERNS, Path::new("orpat.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(json, graph::to_json(&program).unwrap());
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v16");
    let mut pending = vec![&wire];
    let mut or_patterns = Vec::new();
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if object.get("kind").and_then(|kind| kind.as_str()) == Some("or_pattern") {
                    let cases = object["alternatives"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|alternative| {
                            assert_eq!(alternative["kind"], "variant_pattern");
                            assert_eq!(alternative["fields"], serde_json::json!([]));
                            alternative["case"].as_str().unwrap().to_owned()
                        })
                        .collect::<Vec<_>>();
                    or_patterns.push(cases.join("|"));
                }
                pending.extend(object.values());
            }
            serde_json::Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    or_patterns.sort();
    assert_eq!(
        or_patterns,
        [
            "orpat.shape.dot|orpat.shape.empty",
            "orpat.status.review|orpat.status.done",
            "orpat.status.todo|orpat.status.doing|orpat.status.review",
        ],
        "{json}"
    );
}

#[test]
fn or_patterns_agree_on_interpreter_native_and_wasm() {
    let parsed = interpret_main(OR_PATTERNS, "or-interpret");
    let outcome = &parsed["payload"]["outcome"];
    assert_eq!(outcome["kind"], "returned", "{parsed}");
    assert_eq!(outcome["value"], OR_PATTERNS_RESULT, "{parsed}");
    if let Some((success, stdout, stderr)) = run_native(OR_PATTERNS, "or-native") {
        assert!(success, "{stderr}");
        assert_eq!(stdout, OR_PATTERNS_RESULT);
    }
    if let Some(stdout) = run_wasm(OR_PATTERNS, "or-wasm") {
        assert_eq!(stdout, OR_PATTERNS_RESULT);
    }
    if let Some((success, stdout, stderr)) = run_native(OR_WILDCARD, "or-wild-native") {
        assert!(success, "{stderr}");
        assert_eq!(stdout, "11");
    }
    if let Some(stdout) = run_wasm(OR_WILDCARD, "or-wild-wasm") {
        assert_eq!(stdout, "11");
    }
}

#[test]
fn rejected_or_patterns_keep_stable_diagnostics() {
    let prefix = &OR_PATTERNS[..OR_PATTERNS.find("@id(\"orpat.weight\")").unwrap()];
    let help = "records/variants admit case patterns and `_`; `|` joins payload-free cases. \
                Exact Copy-payload cases also admit scalar-operator guards with unguarded \
                fallbacks. Literal/binding patterns and broader guards require scalar scrutinees";
    let cases: [(&str, Vec<String>); 7] = [
        (
            "Shape::Dot {} | Shape::Square { side: s } => 0, Shape::Empty {} => 1,",
            vec![
                "SPX-M105 or-pattern alternative `Shape::Square` must be a payload-free case \
                  without bindings | match a case that carries a payload in an arm of its own"
                    .to_owned(),
            ],
        ),
        (
            "Shape::Dot {} | Shape::Dot {} => 0, _ => 1,",
            vec!["SPX-M102 unreachable duplicate case `Shape::Dot` | ".to_owned()],
        ),
        (
            "Shape::Dot {} | Shape::Empty {} => 0,",
            vec![
                "SPX-M101 non-exhaustive match; missing case `Shape::Square { .. }` | ".to_owned(),
            ],
        ),
        (
            "Shape::Dot {} | Status::Todo {} => 0, _ => 1,",
            vec![
                "SPX-M103 pattern `Status::Todo` is incompatible with the match scrutinee | "
                    .to_owned(),
            ],
        ),
        (
            "Shape::Dot {} | Shape::Emty {} => 0, _ => 1,",
            vec![
                "SPX-M103 pattern `Shape::Emty` is incompatible with the match scrutinee | \
                  did you mean `Shape::Empty { ... }`?"
                    .to_owned(),
            ],
        ),
        (
            "Shape::Dot {} | 3 => 0, _ => 1,",
            vec![
                format!(
                    "SPX-T254 guards and literal/or/binding patterns require a Copy-scalar \
                     scrutinee (i64/i32/u8/char/bool) | {help}"
                ),
                format!(
                    "SPX-T254 or-pattern alternatives over a variant scrutinee must all be \
                     payload-free cases of its type | {help}"
                ),
            ],
        ),
        (
            "Shape::Dot {} | Shape::Empty {} if code > 1 => 0, _ => 1,",
            vec![format!(
                "SPX-T254 guards and literal/or/binding patterns require a Copy-scalar \
                 scrutinee (i64/i32/u8/char/bool) | {help}"
            )],
        ),
    ];
    for (arms, expected) in cases {
        let source = format!(
            "{prefix}@id(\"app.main\")\nfn main() -> i64\n{{\n    let code = 2;\n    let shape = Shape::Dot {{}};\n    match shape {{ {arms} }}\n}}\n"
        );
        let rendered = diagnostics(&source)
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
        assert_eq!(rendered, expected, "{arms}");
    }
}

/// A payload-free case may omit `{}` in expressions and patterns; the
/// formatter writes it back, so the bare spelling is the same program.
#[test]
fn bare_payload_free_cases_parse_to_the_braced_program() {
    for source in [EQUALITY, CONTRACT] {
        let bare = source.replace(" {}", "");
        assert_ne!(bare, source, "the corpus must spell some empty payloads");
        let program = parse(&bare, Path::new("vareq-bare.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        assert_eq!(format::canonical(&program), source);
        let braced = parse(source, Path::new("vareq.spx")).unwrap();
        assert_eq!(graph::revision(&program), graph::revision(&braced));
    }
    // A block after a bare case stays a block, and a pattern needs no braces.
    let source = r#"module app.bare;

@id("bare.status")
variant Status {
    @id("bare.status.todo")
    Todo,
    @id("bare.status.done")
    Done,
}

@id("bare.score")
fn score(s: Status) -> i64
{
    let a = if s == Status::Done { 1 } else { 2 };
    a + match s { Status::Todo => 10, Status::Done => 20, }
}

@id("app.main")
fn main() -> i64
{
    score(Status::Done) * 100 + score(Status::Todo)
}
"#;
    let program = parse(source, Path::new("bare.spx")).unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "{:?}",
        diagnostics(source)
    );
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("if (s == Status::Done {}) { 1 } else { 2 }"),
        "{canonical}"
    );
    assert!(canonical.contains("Status::Todo {} => 10"), "{canonical}");
    if let Some((ok, stdout, _)) = run_native(source, "bare") {
        assert!(ok);
        assert_eq!(stdout.trim(), "2112");
    }
}
