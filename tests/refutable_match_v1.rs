//! Executable evidence for Refutable Match v1 (literal patterns + guards).
//!
//! Proves that `match` over admitted Copy-scalar scrutinees with integer,
//! bool, and char literal patterns, or-patterns, binding catch-alls, and
//! boolean guards produces identical observable results on native C11 O0/O2
//! and Node/Wasm, that the reference interpreter agrees and fails closed on
//! fuel exhaustion, that every new diagnostic (SPX-T254–T257, SPX-M105,
//! SPX-P206) is stable, and that Graph v16 is selected additively above the
//! whole v10–v15 lattice only when an authenticated refutable node exists.
//! Programs without refutable-match syntax keep their exact pre-feature bytes.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::ResolvedExprKind;
use semaprax::interpreter::{self, InterpreterOptions, DEFAULT_MAX_STEPS};
use semaprax::{codegen, format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn scalar_match_retains_owned_string_arm_result_classification() {
    let source = r#"module test.owned_match;
@id("match.text") fn text(value: i64) -> string {
    match value { 0 => "zero", _ => "other", }
}
@id("app.main") fn main() -> i64 { string_len(text(0)) }
"#;
    let program = semaprax::check(source, "owned-match.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let ResolvedExprKind::Block { tail, .. } = &resolved.functions[0].body.kind else {
        panic!("expected function block");
    };
    assert!(matches!(tail.kind, ResolvedExprKind::Match { .. }));
    assert_eq!(tail.ownership, hir::OwnershipMode::Own);
    let mut hostile = resolved;
    let ResolvedExprKind::Block { tail, .. } = &mut hostile.functions[0].body.kind else {
        panic!("expected function block");
    };
    tail.ownership = hir::OwnershipMode::Value;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

const CORPUS: &str = r#"
module test.refutable_match_v1;

@id("rm.int_dispatch")
fn int_dispatch(x: i64) -> i64 {
    match x {
        0 => 10,
        -5 => 20,
        7 if x > 3 => 30,
        n => n * 2,
    }
}

@id("rm.or_digit")
fn or_digit(d: i64) -> i64 {
    match d {
        0 | 1 | 2 => 1,
        3 | 4 => 2,
        _ => 3,
    }
}

@id("rm.u8_edges")
fn u8_edges(v: u8) -> i64 {
    match v {
        0u8 => -1,
        255u8 => 255,
        k if k > 100u8 => 1,
        _ => 0,
    }
}

@id("rm.bool_pick")
fn bool_pick(flag: bool) -> i64 {
    match flag {
        true => 1,
        _ => 2,
    }
}

@id("rm.char_route")
fn char_route(c: char) -> i64 {
    match c {
        'a' => 1,
        'ü' => 2,
        _ => 0,
    }
}

@id("rm.nested_guard")
fn nested_guard(v: i64) -> i64 {
    match v {
        k if if k % 2 == 0 { k > 10 } else { k < -10 } => 1,
        _ => 0,
    }
}

@id("rm.loop_arm")
fn loop_arm(limit: i64) -> i64 {
    let mut total = 0;
    let mut counter = 0;
    total = match limit {
        0 => 0,
        _ if limit > 0 => {
            while counter < limit {
                counter = counter + 1;
                total = total + counter;
                counter < limit
            }
            total
        },
        _ => -1,
    };
    total
}

@id("rm.countdown")
fn countdown(n: i64) -> i64 {
    match n {
        0 => 0,
        m if m > 0 => countdown(m - 1),
        _ => -1,
    }
}

@id("main")
fn main() -> i64 {
    int_dispatch(0) + or_digit(2) + u8_edges(150u8) + bool_pick(true)
        + char_route('ü') + nested_guard(-11) + loop_arm(3)
}
"#;

/// A pre-feature aggregate program: exhaustive copy-variant matching plus an
/// explicit authenticated record pattern. Its schema and graph bytes must
/// never drift now that refutable matches exist.
const AGGREGATE_STABLE: &str = r#"
module test.refutable_aggregate_stable;

@id("shape.type")
variant Shape {
    @id("shape.dot")
    Dot,
    @id("shape.box")
    Box { width: i64, },
}

@id("pair.type")
record Pair {
    @id("pair.left")
    left: i64,
    @id("pair.right")
    right: i64,
}

@id("agg.variant_dispatch")
fn variant_dispatch(shape: Shape) -> i64 {
    match shape {
        Shape::Dot {} => 0,
        Shape::Box { width } => width,
    }
}

@id("agg.record_destructure")
fn record_destructure(pair: Pair) -> i64 {
    match pair {
        Pair { left, right } => left + right,
    }
}

@id("main")
fn main() -> i64 { variant_dispatch(Shape::Box { width: 4 }) + record_destructure(Pair { left: 1, right: 2 }) }
"#;

/// Programs without any match syntax at all: the shared Explicit Mutation /
/// While-Loops byte pin.
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

fn write_corpus(source: &str, stem: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-refutable-{}-{stem}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join("corpus.spx");
    std::fs::write(&path, source).expect("corpus source");
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
    let program = parse(source, Path::new("refutable-diag.spx")).unwrap();
    verify::verify(&program)
}

fn digest_of(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

#[test]
fn refutable_programs_round_trip_canonically_and_keep_revisions_stable() {
    let program = parse(CORPUS, Path::new("roundtrip.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("0 | 1 | 2 => 1"),
        "or-patterns render with spaced pipes: {canonical}"
    );
    assert!(
        canonical.contains("7 if x > 3 => 30"),
        "guards render between the pattern and the arrow: {canonical}"
    );
    assert!(
        canonical.contains("'a'") && canonical.contains("'\\u{fc}'"),
        "printable ASCII chars render literally and others as escapes: {canonical}"
    );
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn graph_serialization_is_deterministic_and_selects_v16_only_with_refutable_nodes() {
    let program = parse(CORPUS, Path::new("graph.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"literal_pattern\""), "{first}");
    assert!(first.contains("\"kind\":\"or_pattern\""));
    assert!(first.contains("\"kind\":\"binding_pattern\""));
    assert!(first.contains("\"kind\":\"guard\""));
    assert!(first.contains("\"exhaustive\":false"));
    let wire = serde_json::from_str::<serde_json::Value>(&first).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v16");

    // Pre-feature surfaces keep their exact lattice entries: plain stays at
    // v10 and the aggregate corpus stays at v13 even though both compile
    // alongside the new feature.
    let aggregate = parse(AGGREGATE_STABLE, Path::new("aggregate.spx")).unwrap();
    let aggregate_json = graph::to_json(&aggregate).unwrap();
    let wire = serde_json::from_str::<serde_json::Value>(&aggregate_json).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v13");
    assert!(!aggregate_json.contains("\"exhaustive\":false"));

    let plain = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let wire = serde_json::from_str::<serde_json::Value>(&graph::to_json(&plain).unwrap()).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v10");
}

#[test]
fn non_refutable_graph_bytes_are_pinned_to_pre_feature_output() {
    // Captured from a build without Refutable Match v1; shared with the
    // Explicit Mutation v1 / While-Loops v1 pin family.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(
        digest_of(&json),
        "sha256:6fe42635e96022507876aabd25acfe06f28521aba50132a5dc16b5070c45cfa7"
    );

    // The aggregate corpus exercises exhaustive variant matches and record
    // patterns plus their cleanup projections; its bytes are pinned from a
    // build without Refutable Match v1 and must never drift.
    let program = parse(AGGREGATE_STABLE, Path::new("aggregate.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(
        json.contains("cleanup"),
        "cleanup projections are part of the pinned bytes"
    );
    assert_eq!(
        digest_of(&json),
        "sha256:ec17d8ee51fbc6892ffe528793e72a93753143d6d2560141f6ac5039334e3e29"
    );
}

#[test]
fn scalar_matches_add_no_cleanup_slots_and_replay_exactly() {
    let program = parse(CORPUS, Path::new("cleanup.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "rm.int_dispatch")
        .expect("resolved dispatch function");
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());
    assert!(
        function
            .cleanup_plan
            .exits
            .iter()
            .all(|exit| exit.finalize_in_order.is_empty()),
        "scalar decision chains never finalize"
    );
    // The decision chain authenticates one ArmSelected pair per non-final arm.
    use semaprax::cleanup_plan::EdgeCondition;
    let selected_arms: Vec<u32> = function
        .cleanup_plan
        .edges
        .iter()
        .filter_map(|edge| match &edge.condition {
            EdgeCondition::ArmSelected { arm, selected, .. } if *selected => Some(*arm),
            _ => None,
        })
        .collect();
    assert_eq!(
        selected_arms,
        vec![0, 1, 2],
        "one selection edge per refutable arm"
    );
    // Success is itself replay evidence: hir::resolve re-validates every plan
    // against the independent gate before returning.
}

#[test]
fn missing_trailing_catch_all_is_spx_t257() {
    let report = verify_diagnostics(
        r#"
module test.rm_exhaustive;
@id("app.main")
fn main(x: i64) -> i64 {
    match x {
        0 => 1,
        1 => 2,
    }
}
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-T257")
        .expect("refutable matches require a trailing catch-all");
    assert!(diagnostic.message.contains("catch-all"), "{diagnostic:?}");
}

#[test]
fn guarded_final_arm_is_spx_t257() {
    let report = verify_diagnostics(
        r#"
module test.rm_guarded_last;
@id("app.main")
fn main(x: i64) -> i64 {
    match x {
        0 => 1,
        n if n < 0 => 2,
    }
}
"#,
    );
    assert!(report.iter().any(|item| item.code == "SPX-T257"));
}

#[test]
fn mismatched_literal_type_is_spx_t255() {
    let sources = [
        ("bool against i64", "match x { true => 1, _ => 0, }"),
        ("i32 against i64", "match x { 1i32 => 1, _ => 0, }"),
    ];
    for (label, body) in sources {
        let source = format!(
            r#"
module test.rm_literal_ty;
@id("app.main")
fn main(x: i64) -> i64 {{ {body} }}
"#
        );
        let report = verify_diagnostics(&source);
        assert!(
            report.iter().any(|item| item.code == "SPX-T255"),
            "{label} is rejected: {report:?}"
        );
    }
}

#[test]
fn non_bool_guard_is_spx_t256() {
    let report = verify_diagnostics(
        r#"
module test.rm_guard_ty;
@id("app.main")
fn main(x: i64) -> i64 {
    match x {
        n if n + 1 => 1,
        _ => 0,
    }
}
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-T256")
        .expect("guards must be exactly bool");
    assert!(diagnostic.message.contains("bool"), "{diagnostic:?}");
}

#[test]
fn unbound_guard_name_is_rejected() {
    let report = verify_diagnostics(
        r#"
module test.rm_guard_name;
@id("app.main")
fn main(x: i64) -> i64 {
    match x {
        n if mystery > 0 => 1,
        _ => 0,
    }
}
"#,
    );
    assert!(
        report.iter().any(|item| item.severity.is_error()),
        "unbound guard names are compile-time diagnostics: {report:?}"
    );
}

#[test]
fn mixed_or_pattern_types_are_spx_m105() {
    let report = verify_diagnostics(
        r#"
module test.rm_or_mix;
@id("app.main")
fn main(x: i64) -> i64 {
    match x {
        1 | true => 1,
        _ => 0,
    }
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-M105"),
        "mixed alternative types are rejected: {report:?}"
    );
}

#[test]
fn refutable_constructs_on_aggregate_scrutinees_are_spx_t254() {
    let sources = [
        (
            "literal against variant",
            r#"
module test.rm_agg_literal;
@id("sh.type")
variant Sh { @id("sh.a") A, @id("sh.b") B, }
@id("app.main")
fn main(sh: Sh) -> i64 {
    match sh {
        0 => 1,
        Sh::A {} => 2,
        Sh::B {} => 3,
    }
}
"#,
        ),
        (
            "guard on variant",
            r#"
module test.rm_agg_guard;
@id("sh2.type")
variant Sh2 { @id("sh2.a") A { v: i64, }, @id("sh2.b") B { v: i64, }, }
@id("app.main")
fn main(sh: Sh2) -> i64 {
    match sh {
        Sh2::A { v } if v > 0 => v,
        Sh2::B { v } => v,
    }
}
"#,
        ),
        (
            "string scrutinee",
            r#"
module test.rm_string;
@id("app.main")
fn main(s: string) -> i64 { match s { _ => 0, } }
"#,
        ),
    ];
    for (index, (label, source)) in sources.iter().enumerate() {
        let report = verify_diagnostics(source);
        // The string-scrutinee entry keeps its pre-feature M103 rejection;
        // every other entry must select SPX-T254.
        let admitted = report.iter().any(|item| item.code == "SPX-T254")
            || (index == 2 && report.iter().any(|item| item.code == "SPX-M103"));
        assert!(
            admitted,
            "{label} stays outside the Copy-scalar surface: {report:?}"
        );
    }
}

#[test]
fn malformed_literal_patterns_are_spx_p206() {
    let parsed = parse(
        r#"
module test.rm_parse_float;
@id("app.main")
fn main(x: f64) -> i64 { match x { 1.0 => 1, _ => 0 } }
"#,
        Path::new("p.spx"),
    )
    .unwrap_err();
    assert_eq!(parsed.code, "SPX-P206", "{parsed:?}");

    let parsed = parse(
        r#"
module test.rm_parse_minus;
@id("app.main")
fn main(x: i64) -> i64 { match x { -true => 1, _ => 0 } }
"#,
        Path::new("p.spx"),
    )
    .unwrap_err();
    assert_eq!(parsed.code, "SPX-P206", "{parsed:?}");
}

#[test]
fn native_refutable_matches_execute_identically_at_o0_o2() {
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
    if ({int_dispatch}(&context, INT64_C(0), &out) != SPX_STATUS_SUCCESS || out != INT64_C(10)) return 11;
    if ({int_dispatch}(&context, -INT64_C(5), &out) != SPX_STATUS_SUCCESS || out != INT64_C(20)) return 12;
    if ({int_dispatch}(&context, INT64_C(7), &out) != SPX_STATUS_SUCCESS || out != INT64_C(30)) return 13;
    if ({int_dispatch}(&context, INT64_C(21), &out) != SPX_STATUS_SUCCESS || out != INT64_C(42)) return 14;
    if ({int_dispatch}(&context, INT64_C(5), &out) != SPX_STATUS_SUCCESS || out != INT64_C(10)) return 15;
    if ({or_digit}(&context, INT64_C(2), &out) != SPX_STATUS_SUCCESS || out != INT64_C(1)) return 16;
    if ({or_digit}(&context, INT64_C(4), &out) != SPX_STATUS_SUCCESS || out != INT64_C(2)) return 17;
    if ({or_digit}(&context, INT64_C(9), &out) != SPX_STATUS_SUCCESS || out != INT64_C(3)) return 18;
    if ({u8_edges}(&context, UINT8_C(0), &out) != SPX_STATUS_SUCCESS || out != -INT64_C(1)) return 19;
    if ({u8_edges}(&context, UINT8_C(255), &out) != SPX_STATUS_SUCCESS || out != INT64_C(255)) return 20;
    if ({u8_edges}(&context, UINT8_C(150), &out) != SPX_STATUS_SUCCESS || out != INT64_C(1)) return 21;
    if ({bool_pick}(&context, UINT8_C(1), &out) != SPX_STATUS_SUCCESS || out != INT64_C(1)) return 22;
    if ({char_route}(&context, UINT32_C(0xfc), &out) != SPX_STATUS_SUCCESS || out != INT64_C(2)) return 23;
    if ({nested_guard}(&context, INT64_C(12), &out) != SPX_STATUS_SUCCESS || out != INT64_C(1)) return 24;
    if ({nested_guard}(&context, INT64_C(4), &out) != SPX_STATUS_SUCCESS || out != INT64_C(0)) return 25;
    if ({loop_arm}(&context, INT64_C(3), &out) != SPX_STATUS_SUCCESS || out != INT64_C(6)) return 26;
    if ({loop_arm}(&context, INT64_C(-4), &out) != SPX_STATUS_SUCCESS || out != -INT64_C(1)) return 27;
    if ({main_fn}(&context, &out) != SPX_STATUS_SUCCESS || out != INT64_C(22)) return 28;
    return 0;
}}
"#,
        int_dispatch = symbol("rm.int_dispatch"),
        or_digit = symbol("rm.or_digit"),
        u8_edges = symbol("rm.u8_edges"),
        bool_pick = symbol("rm.bool_pick"),
        char_route = symbol("rm.char_route"),
        nested_guard = symbol("rm.nested_guard"),
        loop_arm = symbol("rm.loop_arm"),
        main_fn = symbol("main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-refutable-native-{}-{id}", std::process::id());
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
            "native C failed at {optimization}: {}\n{probe}",
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

const NODE_RUNNER: &str = r#"import { readFile } from "node:fs/promises";
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
console.log("refutable-wasm-ok");
"#;

#[test]
fn wasm_refutable_matches_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let corpus_path = write_corpus(CORPUS, "wasm");
    let corpus_source = std::fs::read_to_string(&corpus_path).unwrap();
    let program = parse(&corpus_source, &corpus_path).expect("parse corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, semaprax::wasm::emit_module(&program).unwrap());

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-refutable-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(&script_path, NODE_RUNNER).unwrap();
    // main() = 10+1+1+1+2+1+6 = 22, matching the native probe.
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg("22")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node refutable-match leg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "refutable-wasm-ok"
    );
}

#[test]
fn interpreter_agrees_with_backends_on_refutable_results() {
    let path = write_corpus(CORPUS, "interpret");

    for (token, arguments, expected) in [
        ("rm.int_dispatch", vec!["0"], "10"),
        ("rm.int_dispatch", vec!["-5"], "20"),
        ("rm.int_dispatch", vec!["7"], "30"),
        ("rm.int_dispatch", vec!["21"], "42"),
        ("rm.int_dispatch", vec!["5"], "10"),
        ("rm.or_digit", vec!["1"], "1"),
        ("rm.or_digit", vec!["4"], "2"),
        ("rm.or_digit", vec!["9"], "3"),
        ("rm.u8_edges", vec!["0u8"], "-1"),
        ("rm.u8_edges", vec!["255u8"], "255"),
        ("rm.u8_edges", vec!["150u8"], "1"),
        ("rm.bool_pick", vec!["true"], "1"),
        ("rm.char_route", vec!["'ü'"], "2"),
        ("rm.nested_guard", vec!["12"], "1"),
        ("rm.nested_guard", vec!["-11"], "1"),
        ("rm.nested_guard", vec!["4"], "0"),
        ("rm.loop_arm", vec!["3"], "6"),
        ("rm.loop_arm", vec!["0"], "0"),
        ("rm.loop_arm", vec!["-4"], "-1"),
        ("rm.countdown", vec!["10"], "0"),
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
}

#[test]
fn interpreter_fuel_exhaustion_fails_closed_on_recursive_guards() {
    let path = write_corpus(CORPUS, "fuel");
    let options = InterpreterOptions::new(65536, 16).unwrap();
    let interpretation =
        interpreter::interpret(&path, "rm.countdown", &["1000000".to_owned()], &options)
            .expect("interpretation");
    let parsed: serde_json::Value = serde_json::from_str(&interpretation.envelope).unwrap();
    assert_eq!(parsed["payload"]["outcome"]["kind"], "fuel_exhausted");
    assert_eq!(parsed["payload"]["fuel"]["exhausted"], true);
    assert!(
        !interpretation.returned,
        "exhausted programs return nothing"
    );
}

#[test]
fn evidence_flows_reject_graph_v16_fail_closed() {
    // The patch/evidence flows authenticate only Graph v10-v14; both additive
    // extensions (v15 while loops and v16 refutable matches) are refused up
    // front rather than emitting capsules whose replay would reject.
    let program = parse(CORPUS, Path::new("evidence.spx")).unwrap();
    let schema = graph::to_json(&program).unwrap();
    assert!(schema.contains("semaprax.graph.v16"));
    let refusal = graph::reject_evidence_schema("semaprax.graph.v16").unwrap_err();
    assert_eq!(refusal.code, "SPX-G410");
    let refusal = graph::reject_evidence_schema("semaprax.graph.v15").unwrap_err();
    assert_eq!(refusal.code, "SPX-G410");
    assert!(graph::reject_evidence_schema("semaprax.graph.v10").is_ok());
}
