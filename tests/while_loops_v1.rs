//! Executable evidence for Bounded While-Loops v1.
//!
//! Proves that statement-level `while` loops over the admitted Copy-scalar
//! profile produce identical observable results on native C11 O0/O2 and
//! Node/Wasm, that condition-dependent checked-arithmetic failures select the
//! exact same normalized status on every backend including the reference
//! interpreter, that fuel exhaustion fails closed, and that every new
//! diagnostic (SPX-T251/T252/T253) plus the additive Graph-v15 selection are
//! stable. Programs without while syntax must stay byte-identical.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions, DEFAULT_MAX_STEPS};
use semaprax::{codegen, format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const CORPUS: &str = r#"
module test.while_loops_v1;

@id("while.count_sum")
fn count_sum(limit: i64) -> i64 {
    let mut counter = 0;
    let mut total = 0;
    while counter < limit {
        counter = counter + 1;
        total = total + counter;
        counter < limit
    }
    total
}

@id("while.digit_sum")
fn digit_sum(value: i64) -> i64 {
    let mut remaining = value;
    let mut total = 0;
    while remaining > 0 {
        total = total + remaining % 10;
        remaining = remaining / 10;
        remaining > 0
    }
    total
}

@id("while.nested")
fn nested(width: i64, height: i64) -> i64 {
    let mut row = 0;
    let mut total = 0;
    while row < height {
        let mut column = 0;
        while column < width {
            column = column + 1;
            total = total + row * width + column;
            column < width
        }
        row = row + 1;
        row < height
    }
    total
}

@id("while.in_if")
fn in_if(flag: bool, limit: i64) -> i64 {
    let mut total = 0;
    if flag {
        let mut counter = 0;
        while counter < limit {
            counter = counter + 1;
            total = total + 2;
            counter < limit
        }
        total
    } else {
        total - 7
    }
}

@id("while.zero_iterations")
fn zero_iterations(flag: bool) -> i64 {
    let mut total = 3;
    while flag {
        total = total + 100;
        flag
    }
    if total == 3 { 30 } else { total }
}

@id("while.div_fails_on_iteration")
fn div_fails(start: i64) -> i64 {
    let mut n = start;
    let mut total = 0;
    while n >= 0 {
        let quotient = 6 / n;
        total = total + quotient;
        n = n - 1;
        n >= 0
    }
    total
}

@id("main")
fn main() -> i64 { count_sum(4) + digit_sum(98765) }
"#;

const PLAIN_STABLE: &str = r#"
module test.mutation_plain;

@id("plain.stable")
fn stable() -> i64 {
    let total = 1;
    let frozen = 3;
    total + frozen
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn write_corpus(stem: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-while-{}-{stem}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join("corpus.spx");
    std::fs::write(&path, CORPUS).expect("corpus source");
    path
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn verify_diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("while-diag.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn while_programs_round_trip_canonically_and_keep_revisions_stable() {
    let program = parse(CORPUS, Path::new("roundtrip.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("while remaining > 0 {"),
        "loops render in canonical header-inline form: {canonical}"
    );
    assert!(canonical.contains("while column < width {"));
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn graph_serialization_is_deterministic_and_selects_v15_only_with_while_nodes() {
    let program = parse(CORPUS, Path::new("graph.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"while\""), "{first}");
    let wire = serde_json::from_str::<serde_json::Value>(&first).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v15");

    // Programs without while syntax keep their exact previous lattice entry.
    let plain = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let plain_json = graph::to_json(&plain).unwrap();
    assert!(!plain_json.contains("\"kind\":\"while\""));
    let wire = serde_json::from_str::<serde_json::Value>(&plain_json).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v10");
}

#[test]
fn non_while_graph_bytes_are_pinned_to_pre_feature_output() {
    // This digest was captured from a build without Bounded While-Loops v1
    // (shared with Explicit Mutation v1's pin) and must never drift.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(!json.contains("\"kind\":\"while\""));
    use sha2::{Digest, Sha256};
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(json.as_bytes()))
    );
    assert_eq!(
        digest,
        "sha256:6fe42635e96022507876aabd25acfe06f28521aba50132a5dc16b5070c45cfa7"
    );
}

#[test]
fn admitted_loops_add_no_cleanup_structure_and_replay_exactly() {
    let program = parse(
        r#"
module test.while_cleanup;

@id("clean.loopy")
fn loopy(limit: i64) -> i64 {
    let mut counter = 0;
    let mut total = 0;
    while counter < limit {
        counter = counter + 1;
        total = total + counter * 2;
        counter < limit
    }
    total
}

@id("main")
fn main() -> i64 { 0 }
"#,
        Path::new("cleanup.spx"),
    )
    .unwrap();
    // `hir::resolve` validates ordinary HIR, rebuilds every CleanupPlan, and
    // exact-compares it against the independent replay gate; success here is
    // itself the replay evidence.
    let resolved = hir::resolve(&program).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "clean.loopy")
        .expect("resolved loop function");
    // Copy-scalar loops own nothing: no slots and no finalizers anywhere.
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());
    assert!(
        function
            .cleanup_plan
            .exits
            .iter()
            .all(|exit| exit.finalize_in_order.is_empty()),
        "scalar loops never finalize"
    );
}

#[test]
fn non_bool_while_condition_is_spx_t251() {
    let report = verify_diagnostics(
        r#"
module test.while_nonbool;
@id("app.main")
fn main() -> i64 {
    let mut count = 0;
    while count {
        count = count + 1;
        true
    }
    0
}
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-T251")
        .expect("non-bool conditions are rejected");
    assert!(diagnostic
        .message
        .contains("`while` condition must be bool"));
}

#[test]
fn record_content_inside_loops_is_spx_t252() {
    let report = verify_diagnostics(
        r#"
module test.while_record;
record Point {
    x: i64,
}
@id("app.main")
fn main() -> i64 {
    let mut count = 0;
    while count < 3 {
        let point = Point { x: count };
        count = count + point.x;
        count < 3
    }
    0
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-T252"),
        "record construction stays outside the loop slice: {report:?}"
    );
}

#[test]
fn strings_and_unsafe_inside_loops_are_spx_t252() {
    let sources = [
        (
            "string literal",
            r#"
module test.while_string;
@id("app.main")
fn main() -> i64 {
    let mut count = 0;
    while count < 2 {
        let label = "x";
        count = count + 1;
        count < 2
    }
    0
}
"#,
        ),
        (
            "unsafe boundary",
            r#"
module test.while_unsafe;
permit { unsafe }
@id("app.main")
fn main() -> i64 {
    while false {
        @audit("discarded") unsafe { 0 }
        false
    }
    0
}
"#,
        ),
    ];
    for (label, source) in sources {
        let report = verify_diagnostics(source);
        assert!(
            report.iter().any(|item| item.code == "SPX-T252"),
            "{label} inside loops is rejected: {report:?}"
        );
    }
}

#[test]
fn nonscalar_calls_inside_loops_are_spx_t252() {
    let report = verify_diagnostics(
        r#"
module test.while_call;
record Token {
    weight: i64,
}
@id("call.consume")
fn consume(token: own Token) -> i64 { token.weight }
@id("app.main")
fn main() -> i64 {
    let token = Token { weight: 1 };
    let mut count = 0;
    while count < 3 {
        count = count + consume(token);
        count < 3
    }
    0
}
"#,
    );
    assert!(
        report
            .iter()
            .any(|item| item.code == "SPX-T252" && item.message.contains("`consume`")),
        "own-parameter calls stay outside loops: {report:?}"
    );
}

#[test]
fn while_in_contract_expression_is_spx_t253() {
    let report = verify_diagnostics(
        r#"
module test.while_contract;
@id("c.check")
fn check(value: i64) -> i64
requires {{
    let mut count = 0;
    while count < value {
        count = count + 1;
        count < value
    }
    count > 0
}}
{ value }
@id("app.main")
fn main() -> i64 { check(1) }
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-T253"),
        "contract expressions remain loop-free: {report:?}"
    );
}

#[test]
fn native_loops_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(CORPUS, Path::new("native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(96), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t out = 0;
    if ({count_sum}(&context, INT64_C(4), &out) != SPX_STATUS_SUCCESS || out != INT64_C(10)) return 11;
    if ({digit_sum}(&context, INT64_C(98765), &out) != SPX_STATUS_SUCCESS || out != INT64_C(35)) return 12;
    if ({nested}(&context, INT64_C(3), INT64_C(2), &out) != SPX_STATUS_SUCCESS || out != INT64_C(21)) return 13;
    if ({in_if}(&context, UINT8_C(1), INT64_C(5), &out) != SPX_STATUS_SUCCESS || out != INT64_C(10)) return 14;
    if ({in_if}(&context, UINT8_C(0), INT64_C(5), &out) != SPX_STATUS_SUCCESS || out != INT64_C(-7)) return 15;
    if ({zero_iterations}(&context, UINT8_C(0), &out) != SPX_STATUS_SUCCESS || out != INT64_C(30)) return 16;
    if ({main_fn}(&context, &out) != SPX_STATUS_SUCCESS || out != INT64_C(45)) return 17;
    return 0;
}}
"#,
        count_sum = symbol("while.count_sum"),
        digit_sum = symbol("while.digit_sum"),
        nested = symbol("while.nested"),
        in_if = symbol("while.in_if"),
        zero_iterations = symbol("while.zero_iterations"),
        main_fn = symbol("main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-while-native-{}-{id}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "native C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let stderr = String::from_utf8_lossy(&executed.stderr).into_owned();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "native failed at {optimization}: {:?} {stderr}",
            executed.status.code()
        );
    }
}

/// The failure probe resolves the selected normalized status directly through
/// the compiler-owned runtime helpers and prints its exact arithmetic code.
const NATIVE_FAILURE_PROBE: &str = r#"
#include <stdio.h>
#include <string.h>

static spx_status_token PROBE_SYMBOL(
    struct spx_context *spx_ctx,
    int64_t spx_start,
    int64_t *spx_result_out
);

__attribute__((unused)) static int probe_main(void) {
    static struct spx_status_entry entries[UINT32_C(32)];
    static struct spx_context context;
    memset(&context, 0, sizeof(context));
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 60;
    int64_t out = 0;
    uint32_t spx_status = PROBE_SYMBOL(&context, INT64_C(3), &out);
    if (spx_status == SPX_STATUS_SUCCESS) return 61;
    const struct spx_normalized_status *status = spx_status_resolve(&context, spx_status);
    if (status == NULL) return 62;
    if (status->status_class != SPX_STATUS_CLASS_ARITHMETIC) return 63;
    if (status->code != UINT32_C(4)) return 64;
    if (strcmp(status->domain_id, "semaprax.arithmetic.v1") != 0) return 65;
    printf("%s/%u\n", status->domain_id, status->code);
    return 66;
}

int main(void) { return probe_main(); }
"#;

#[test]
fn native_condition_dependent_division_by_zero_selects_exact_status_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    // Point `main` at the failing loop in this temporary copy of the corpus
    // so the loop function is part of the entry closure and gets defined.
    let failing = CORPUS.replace(
        "@id(\"main\")\nfn main() -> i64 { count_sum(4) + digit_sum(98765) }",
        "@id(\"main\")\nfn main() -> i64 { div_fails(3) }",
    );
    let source_path = Path::new("native-fail.spx");
    let program = parse(&failing, source_path).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    // start=3 performs three good iterations then divides by zero on the next
    // pass; the normalized arithmetic failure must surface identically at
    // both optimization levels.
    let probe = NATIVE_FAILURE_PROBE.replace(
        "PROBE_SYMBOL",
        &format!("spx_decl_{}", hex_identity("while.div_fails_on_iteration")),
    );
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-while-native-fail-{}-{id}", std::process::id());
        let source_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source_path, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "failure-probe C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let stdout = String::from_utf8_lossy(&executed.stdout).into_owned();
        let _ = std::fs::remove_file(&source_path);
        let _ = std::fs::remove_file(&executable);
        assert_eq!(
            executed.status.code(),
            Some(66),
            "exact normalized status probe at {optimization}: {}",
            String::from_utf8_lossy(&executed.stderr)
        );
        assert!(
            stdout.contains("semaprax.arithmetic.v1/4"),
            "division-by-zero selects code 4 in semaprax.arithmetic.v1 at {optimization}: {stdout}"
        );
    }
}

const NODE_LOOP_RUNNER: &str = r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const expected = BigInt(process.argv[3]);
const SPX_MIN = -(2n ** 63n), SPX_MAX = 2n ** 63n - 1n;
const bounded = (value, what) => {
  if (value < SPX_MIN || value > SPX_MAX) throw new RangeError(`checked ${what} failure`);
  return value;
};
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => bounded(a + b, "addition"),
  spx_sub: (a, b) => bounded(a - b, "subtraction"),
  spx_mul: (a, b) => bounded(a * b, "multiplication"),
  spx_div: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("division"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("remainder"); return a % b; },
  spx_neg: (a) => bounded(-a, "negation"),
  spx_contract_fail: fail("spx_contract_fail"),
}});
for (let index = 0; index < 1024; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("while-wasm-ok");
"#;

#[test]
fn wasm_loops_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let corpus_path = write_corpus("wasm");
    let corpus_source = std::fs::read_to_string(&corpus_path).unwrap();
    let program = parse(&corpus_source, &corpus_path).expect("parse corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, semaprax::wasm::emit_module(&program).unwrap());

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-while-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(&script_path, NODE_LOOP_RUNNER).unwrap();
    // main() = count_sum(4) + digit_sum(98765) = 10 + 35 = 45, matching the
    // native probe exactly.
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg("45")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node while leg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "while-wasm-ok"
    );
}

#[test]
fn wasm_condition_dependent_division_failure_surfaces_in_node() {
    if !command_available("node") {
        return;
    }
    let corpus_path = write_corpus("wasm-fail");
    let corpus_source = std::fs::read_to_string(&corpus_path).unwrap();
    let program = parse(&corpus_source, &corpus_path).expect("parse corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-while-wasm-div-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
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
  spx_div: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("division by zero"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("remainder"); return a % b; },
  spx_neg: (a) => bounded(-a, "negation"),
  spx_contract_fail: () => { throw new Error("SEMAPRAX contract failure"); },
}});
// The loop runs three good iterations and then divides by zero inside the
// body; the host import throws and no wrapped result may exist.
let threw = false;
try {
  instance.exports.semaprax_main();
} catch (error) {
  threw = true;
  if (!String(error).includes("division")) throw error;
}
if (!threw) throw new Error("loop-carried division by zero must not wrap");
console.log("while-wasm-div-ok");
"#,
    )
    .unwrap();
    // Point `main` at the failing loop by rewriting only this temporary copy
    // of the corpus so the exported entry runs div_fails(3).
    let failing = CORPUS.replace(
        "@id(\"main\")\nfn main() -> i64 { count_sum(4) + digit_sum(98765) }",
        "@id(\"main\")\nfn main() -> i64 { div_fails(3) }",
    );
    std::fs::write(&corpus_path, &failing).unwrap();
    let program = parse(&failing, &corpus_path).expect("parse failing corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    std::fs::write(&wasm_path, bytes).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node division leg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "while-wasm-div-ok"
    );
}

fn envelope_for(path: &Path, token: &str, arguments: &[&str], max_steps: usize) -> String {
    let owned: Vec<String> = arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect();
    let options = InterpreterOptions::new(65536, max_steps).unwrap();
    interpreter::interpret(path, token, &owned, &options)
        .expect("interpretation")
        .envelope
}

#[test]
fn interpreter_agrees_with_backends_on_loop_results() {
    let path = write_corpus("interpret");

    for (token, arguments, expected) in [
        ("while.count_sum", vec!["4"], "10"),
        ("while.digit_sum", vec!["98765"], "35"),
        ("while.nested", vec!["3", "2"], "21"),
        ("while.zero_iterations", vec!["false"], "30"),
    ] {
        let owned: Vec<String> = arguments
            .iter()
            .map(|argument| argument.to_string())
            .collect();
        let options = InterpreterOptions::new(65536, DEFAULT_MAX_STEPS).unwrap();
        let envelope = interpreter::interpret(&path, token, &owned, &options)
            .expect("interpretation")
            .envelope;
        let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        let outcome = &parsed["payload"]["outcome"];
        assert_eq!(outcome["kind"], "returned", "{token}: {envelope}");
        assert_eq!(
            outcome["value"], expected,
            "{token} agrees with both backends: {envelope}"
        );
    }

    // Condition-dependent failure selects the exact compiler-owned
    // arithmetic division-by-zero status on the fourth iteration.
    let failed = envelope_for(
        &path,
        "while.div_fails_on_iteration",
        &["3"],
        DEFAULT_MAX_STEPS,
    );
    let parsed: serde_json::Value = serde_json::from_str(&failed).unwrap();
    let outcome = &parsed["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed");
    assert_eq!(outcome["status"]["schema"], "semaprax.status.v1");
    assert_eq!(outcome["status"]["domain_id"], "semaprax.arithmetic.v1");
    assert_eq!(outcome["status"]["code"], 4);
    assert_eq!(outcome["status"]["class"], "arithmetic");
}

#[test]
fn interpreter_fuel_exhaustion_fails_closed_on_nonterminating_loops() {
    let path = write_corpus("fuel");
    let options = InterpreterOptions::new(65536, 16).unwrap();
    let interpretation = interpreter::interpret(
        &path,
        "while.count_sum",
        &["1000000000000".to_owned()],
        &options,
    )
    .expect("interpretation");
    let parsed: serde_json::Value = serde_json::from_str(&interpretation.envelope).unwrap();
    assert_eq!(parsed["payload"]["outcome"]["kind"], "fuel_exhausted");
    assert_eq!(parsed["payload"]["fuel"]["exhausted"], true);
    assert!(!interpretation.returned, "exhausted loops return nothing");
}
