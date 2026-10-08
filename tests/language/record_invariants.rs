//! Executable evidence for Record Invariants v1.
//!
//! `requires` clauses after a record's closing brace hold for every value of
//! the record that a literal, a `with` update, or a field assignment
//! produces; a violation is the contract-failure status of a failing
//! precondition. The same corpora run in the reference interpreter (literals
//! and field assignments; it admits no Copy-record `with`), the native C
//! backend (`run --native`), and Node's WebAssembly engine. The canonical
//! formatter round-trips the clauses, the graph carries them as the
//! preconditions of the record's `#invariant` function, and the non-bool,
//! unknown-field, effectful, and generic-record shapes keep stable
//! diagnostics.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions, DEFAULT_MAX_STEPS};
use semaprax::{format, graph, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const RANGE: &str = r#"module app.inv;

@id("inv.range")
record Range {
    @id("inv.range.lo")
    lo: i64,
    @id("inv.range.hi")
    hi: i64,
}
    requires lo <= hi
    requires hi - lo <= 100

@id("inv.span")
fn span(lo: i64, hi: i64, grown: i64) -> i64
{
    let mut range = Range { lo: lo, hi: hi };
    range.hi = range.hi + grown;
    range.hi - range.lo
}

@id("app.main")
fn main() -> i64
{
    span(1, 5, 4) * 100 + span(0, 0, 100)
}
"#;

const RANGE_RESULT: &str = "900";

/// The `with` update path: outside the interpreter's admitted record shapes,
/// so this corpus runs on the two compiled backends.
const UPDATE: &str = r#"module app.invupdate;

@id("invupdate.range")
record Range {
    @id("invupdate.range.lo")
    lo: i64,
    @id("invupdate.range.hi")
    hi: i64,
}
    requires lo <= hi

@id("invupdate.shift")
fn shift(lo: i64) -> i64
{
    let range = Range { lo: 0, hi: 10 };
    let moved = range with { lo: lo };
    moved.hi - moved.lo
}

@id("app.main")
fn main() -> i64
{
    shift(4)
}
"#;

/// A record holding an owned `string` has no executable layout on any
/// backend; its clauses still verify, format, and reach the graph.
const TEAM: &str = r#"module app.team;

@id("team.team")
record Team {
    @id("team.team.name")
    name: string,
    @id("team.team.seats")
    seats: i64,
}
    requires string_len(name) >= 2
    requires seats >= 1

@id("team.seats")
fn seats(count: i64) -> i64
{
    count
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn write_source(source: &str, stem: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-invariant-{}-{stem}-{}",
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

/// The interpreter's `app.main` outcome object.
fn interpret_main(source: &str, stem: &str) -> serde_json::Value {
    let path = write_source(source, stem);
    let options = InterpreterOptions::new(65536, DEFAULT_MAX_STEPS).unwrap();
    let envelope = interpreter::interpret(&path, "app.main", &[], &options)
        .expect("interpretation")
        .envelope;
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    parsed["payload"]["outcome"].clone()
}

/// `semaprax run <file> --native`: success, stdout, and stderr.
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

/// `semaprax_main` in Node: the printed result, or `contract-failure`.
fn run_wasm(source: &str, stem: &str) -> Option<String> {
    if !command_available("node") {
        return None;
    }
    let program = parse(source, Path::new("invariant-wasm.spx")).unwrap();
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

fn assert_contract_failure(outcome: &serde_json::Value) {
    assert_eq!(outcome["kind"], "failed", "{outcome}");
    assert_eq!(outcome["status"]["domain_id"], "semaprax.contract.v1");
    assert_eq!(outcome["status"]["class"], "contract");
}

#[test]
fn invariants_verify_and_round_trip_canonically() {
    // Invariant-bearing String records are schema-only; production remains closed.
    let executable = TEAM.replace(
        "    count\n}",
        "    let team = Team { name: \"core\", seats: count };\n    team.seats\n}",
    );
    let refused = parse(&executable, Path::new("invariant-string-production.spx")).unwrap();
    assert!(verify::verify(&refused)
        .iter()
        .any(|d| d.code == "SPX-T309"));
    assert!(hir::resolve(&refused).is_err());
    for source in [RANGE, UPDATE, TEAM] {
        let program = parse(source, Path::new("invariant.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        assert_eq!(canonical, source, "the corpus is already canonical");
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
        assert_eq!(program.types, reparsed.types);
    }
    // Each clause renders on its own indented line after the closing brace.
    let source = RANGE.replace("    requires hi - lo <= 100\n", "");
    let program = parse(&source, Path::new("invariant.spx")).unwrap();
    assert!(format::canonical(&program).contains("}\n    requires lo <= hi\n\n@id"));
}

#[test]
fn invariants_hold_on_interpreter_native_and_wasm() {
    let outcome = interpret_main(RANGE, "interpret");
    assert_eq!(outcome["kind"], "returned", "{outcome}");
    assert_eq!(outcome["value"], RANGE_RESULT, "{outcome}");
    if let Some((success, stdout, stderr)) = run_native(RANGE, "native") {
        assert!(success, "{stderr}");
        assert_eq!(stdout, RANGE_RESULT);
    }
    if let Some(stdout) = run_wasm(RANGE, "wasm") {
        assert_eq!(stdout, RANGE_RESULT);
    }
    if let Some((success, stdout, stderr)) = run_native(UPDATE, "update-native") {
        assert!(success, "{stderr}");
        assert_eq!(stdout, "6");
    }
    if let Some(stdout) = run_wasm(UPDATE, "update-wasm") {
        assert_eq!(stdout, "6");
    }
}

#[test]
fn literal_violation_is_a_contract_failure_on_every_backend() {
    let source = RANGE.replace("span(1, 5, 4) * 100", "span(5, 1, 0) * 100");
    assert_contract_failure(&interpret_main(&source, "literal-interpret"));
    if let Some((success, _, stderr)) = run_native(&source, "literal-native") {
        assert!(!success);
        assert!(
            stderr.contains("contract: requires lo <= hi in inv.range#invariant"),
            "{stderr}"
        );
        assert!(stderr.contains("arguments: lo = 5, hi = 1"), "{stderr}");
    }
    if let Some(stdout) = run_wasm(&source, "literal-wasm") {
        assert_eq!(stdout, "contract-failure");
    }
}

#[test]
fn field_assignment_violation_is_a_contract_failure_on_every_backend() {
    let source = RANGE.replace("span(0, 0, 100)", "span(0, 0, 101)");
    assert_contract_failure(&interpret_main(&source, "assign-interpret"));
    if let Some((success, _, stderr)) = run_native(&source, "assign-native") {
        assert!(!success);
        assert!(
            stderr.contains("contract: requires hi - lo <= 100 in inv.range#invariant"),
            "{stderr}"
        );
        assert!(stderr.contains("arguments: lo = 0, hi = 101"), "{stderr}");
    }
    if let Some(stdout) = run_wasm(&source, "assign-wasm") {
        assert_eq!(stdout, "contract-failure");
    }
}

#[test]
fn update_violation_is_a_contract_failure_on_native_and_wasm() {
    let source = UPDATE.replace("shift(4)", "shift(11)");
    if let Some((success, _, stderr)) = run_native(&source, "update-native-violation") {
        assert!(!success);
        assert!(
            stderr.contains("contract: requires lo <= hi in invupdate.range#invariant"),
            "{stderr}"
        );
        assert!(stderr.contains("arguments: lo = 11, hi = 10"), "{stderr}");
    }
    if let Some(stdout) = run_wasm(&source, "update-wasm-violation") {
        assert_eq!(stdout, "contract-failure");
    }
}

/// The graph function nodes, by identity.
fn graph_functions(source: &str) -> serde_json::Map<String, serde_json::Value> {
    let program = parse(source, Path::new("invariant-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(json, graph::to_json(&program).unwrap());
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    wire["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["kind"] == "function")
        .map(|node| (node["id"].as_str().unwrap().to_owned(), node.clone()))
        .collect()
}

#[test]
fn graph_carries_invariants_as_preconditions_of_the_record_function() {
    let functions = graph_functions(RANGE);
    let invariant = &functions["inv.range#invariant"];
    assert_eq!(invariant["name"], "Range#invariant");
    assert_eq!(invariant["return_type_id"], "bool");
    let params = invariant["params"]
        .as_array()
        .unwrap()
        .iter()
        .map(|param| param["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(params, ["lo", "hi"]);
    let requires = invariant["requires_graph"].as_array().unwrap();
    assert_eq!(requires.len(), 2);
    assert_eq!(requires[0]["op"], "<=");
    assert_eq!(requires[1]["op"], "<=");
    // The Copy record's productions route through its check.
    let check = &functions["inv.range#check"];
    assert_eq!(check["return_type_id"], "nominal:9:inv.range:0:");
    let mut pending = vec![&functions["inv.span"]["body"]];
    let mut checks = 0;
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if object.get("kind").and_then(|kind| kind.as_str()) == Some("call")
                    && object["callee"] == "inv.range#check"
                {
                    checks += 1;
                }
                pending.extend(object.values());
            }
            serde_json::Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    assert_eq!(
        checks, 2,
        "the literal and the field assignment are checked"
    );

    // A record holding a `string` carries its clauses without a check.
    let functions = graph_functions(TEAM);
    let requires = functions["team.team#invariant"]["requires_graph"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(requires, 2);
    assert!(!functions.contains_key("team.team#check"));
}

#[test]
fn rejected_invariants_keep_stable_diagnostics() {
    let header = &TEAM[..TEAM.find("    requires string_len").unwrap()];
    let tail = r#"
@id("team.log")
fn log_seats(n: i64) -> bool
    uses { io }
{
    true
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let cases: [(&str, &str); 4] = [
        ("seats", "SPX-C101 invariant on `Team` must be bool | "),
        (
            "seat >= 1",
            "SPX-T202 unknown value `seat` in `Team` | an invariant names the fields of \
             `Team` by bare name: `name`, `seats`",
        ),
        (
            "log_seats(seats)",
            "SPX-C102 invariant on `Team` calls effectful function `log_seats` with effects \
             {io} | contracts must be deterministic and effect-free",
        ),
        ("seats >= 1 && name == \"x\"", ""),
    ];
    for (clause, expected) in cases {
        let source = format!(
            "module app.team;\npermit {{ io }}\n{}",
            &header["module app.team;\n".len()..]
        ) + &format!("    requires {clause}\n")
            + tail;
        let program = parse(&source, Path::new("invariant-diag.spx")).unwrap();
        let rendered = verify::verify(&program)
            .iter()
            .filter(|diagnostic| diagnostic.severity.is_error())
            .map(|diagnostic| {
                format!(
                    "{} {} | {}",
                    diagnostic.code,
                    diagnostic.message,
                    diagnostic.help.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>();
        let expected = (!expected.is_empty())
            .then_some(expected)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(rendered, expected, "{clause}");
    }

    let generic = r#"module app.pair;

@id("pair.pair")
record Pair<T> {
    @id("pair.pair.left")
    left: T,
    @id("pair.pair.count")
    count: i64,
}
    requires count >= 0

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = parse(generic, Path::new("invariant-generic.spx")).unwrap();
    let found = verify::verify(&program);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code, "SPX-C103");
    assert_eq!(
        found[0].message,
        "invariants on generic record `Pair` are outside Record Invariants v1"
    );
}

#[test]
fn executable_owned_invariants_refuse_before_backend_admission() {
    let source = r#"module test.owned_invariant;
@id("packet") record Packet {
    @id("packet.bytes") payload: Bytes,
    @id("packet.quantity") quantity: i64,
} requires quantity > 0
@id("app.main") fn main() -> i64 {
    let packet = Packet { payload: bytes_zeroed(1usize), quantity: 0 };
    match own packet { Packet { payload, quantity } => quantity, }
}
"#;
    for source in [
        source.to_owned(),
        source.replace("quantity: 0", "quantity: 1"),
        source.replace(
            "match own packet",
            "let changed = packet with { quantity: 0 }; match own changed",
        ),
    ] {
        let program = parse(&source, Path::new("owned-invariant.spx")).unwrap();
        let errors = verify::verify(&program);
        assert!(
            errors.iter().any(|error| error.code == "SPX-C104"),
            "{errors:?}"
        );
        assert!(semaprax::hir::resolve(&program)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-C104"));
        assert_eq!(
            semaprax::codegen::emit_c(&program).unwrap_err().code,
            "SPX-C104"
        );
        assert_eq!(
            semaprax::wasm::emit_module(&program).unwrap_err().code,
            "SPX-C104"
        );
    }
}

const NESTED: &str = r#"
module review.nested_invariant;
@id("review.positive")
record Positive { @id("review.positive.value") value: i64, }
    requires value >= 0
@id("review.wrapper")
record Wrapper { @id("review.wrapper.value") value: i64, }
    requires { let nested = Positive { value: value }; nested.value == value }
@id("app.main")
fn main() -> i64 { let wrapper = Wrapper { value: -1 }; wrapper.value }
"#;

#[test]
fn sg01_nested_invariant_productions_are_checked_before_publication() {
    for source in [NESTED.to_owned(), NESTED.replace(
        "let nested = Positive { value: value }; nested.value == value",
        "let nested = Positive { value: 0 }; let updated = nested with { value: value }; updated.value == value",
    )] {
        let updated = source.contains("with {");
        if !updated {
            assert_contract_failure(&interpret_main(&source, "nested-invalid"));
        }
        if let Some((success, _, stderr)) = run_native(&source, "nested-native") {
            assert!(!success, "{stderr}");
            assert!(stderr.contains("review.positive#invariant"), "{stderr}");
        }
        if let Some(output) = run_wasm(&source, "nested-wasm") {
            assert_eq!(output, "contract-failure");
        }
        let valid = source.replace("Wrapper { value: -1 }", "Wrapper { value: 1 }");
        if !updated {
            assert_eq!(interpret_main(&valid, "nested-valid")["value"], "1");
        }
        if let Some((success, output, stderr)) = run_native(&valid, "nested-valid-native") {
            assert!(success, "{stderr}");
            assert_eq!(output, "1");
        }
        if let Some(output) = run_wasm(&valid, "nested-valid-wasm") {
            assert_eq!(output, "1");
        }
        let functions = graph_functions(&source);
        assert!(functions["review.wrapper#invariant"]["requires_graph"]
            .to_string().contains("review.positive#check"));
        let program = parse(&source, Path::new("nested.spx")).unwrap();
        let canonical = format::canonical(&program);
        let reparsed = parse(&canonical, Path::new("nested.spx")).unwrap();
        assert_eq!(canonical, format::canonical(&reparsed));
        assert!(graph::to_json(&reparsed).unwrap().contains("review.positive#check"));
    }
}

#[test]
fn sg01_indirect_clause_chain_and_function_contract_enforce_inner_invariants() {
    let chain = NESTED
        .replace(
            "@id(\"app.main\")",
            r#"
@id("review.outer")
record Outer { @id("review.outer.value") value: i64, }
    requires { let nested = Wrapper { value: value }; nested.value == value }
@id("app.main")"#,
        )
        .replace(
            "let wrapper = Wrapper { value: -1 }",
            "let wrapper = Outer { value: -1 }",
        );
    let contract = NESTED
        .replace(
            "@id(\"app.main\")",
            r#"
@id("review.contract")
fn checked(value: i64) -> i64
    requires { let nested = Positive { value: value }; nested.value == value }
{ value }
@id("app.main")"#,
        )
        .replace(
            "let wrapper = Wrapper { value: -1 }; wrapper.value",
            "checked(-1)",
        );
    for source in [chain, contract] {
        assert_contract_failure(&interpret_main(&source, "nested-chain"));
        if let Some((success, _, error)) = run_native(&source, "nested-chain-native") {
            assert!(!success, "{error}");
        }
        if let Some(output) = run_wasm(&source, "nested-chain-wasm") {
            assert_eq!(output, "contract-failure");
        }
    }
}
