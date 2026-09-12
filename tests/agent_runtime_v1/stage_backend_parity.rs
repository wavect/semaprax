//! Cross-engine parity for Agent lifecycle stage function BODIES (#142, #143).
//!
//! ## Why this test exists, and what it deliberately does not claim
//!
//! `stages.rs::bind` (src/agent_lifecycle/stages.rs) binds the four
//! deterministic Agent roles -- `initialize`/`observe`/`authorize`/`reduce`
//! -- to ordinary checked functions with zero declared effects, and
//! `CompiledAgentLifecycle::evaluate` (src/agent_lifecycle.rs) executes each
//! bound stage through exactly one seam:
//! `interpreter::retained_call::evaluate_retained_call`. There is no other
//! executor anywhere in the tree: `agent_lifecycle::iterative::driver`'s
//! `run_with_driver_initial` calls only `inner.evaluate(..)`, which is that
//! same interpreter call. Neither #142 (native C11) nor #143 (Core Wasm) has
//! any stage-executor seam implemented today, and this test does not
//! pretend otherwise -- it does not exercise `CompiledAgentLifecycle` at
//! all, and no `AuthorizedRequest`/journal/authority code path here runs on
//! anything but the interpreter, because nothing else exists to run it on.
//!
//! What this test DOES establish, honestly: the four stage functions are
//! ordinary checked functions with no special HIR node and no ambient
//! authority (`stages.rs::function` rejects any declared effect on one), so
//! nothing prevents them from being compiled and called directly through the
//! backends that already exist for ordinary functions -- the general native
//! C11 codegen (`codegen::emit_c`) and the Core Wasm Public Owned Data API v1
//! project profile (`project::derive_public_api_descriptor` +
//! `project::prepare_owned_data_npm_build`, the same profile
//! `tests/project/owned_bytes_npm.rs` exercises), both already used elsewhere
//! for cross-engine parity. This test drives the exact
//! `initialize`/`observe`/`authorize`/`reduce` bodies bound by
//! `stages.rs::bind` -- same signatures, same record/variant shapes, same
//! literal field identities -- through zero-argument wrapper functions
//! (`case.*`) that construct fixed Task/State/Outcome/Proposal-shaped
//! literals in source and project each stage's result down to one `i64`, and
//! asserts the SAME decoded value comes back from:
//!   1. the interpreter, via `interpreter::retained_call` (the exact
//!      mechanism `CompiledAgentLifecycle::evaluate` itself calls);
//!   2. native C11 at `-O0` and `-O2`, compiled and run out-of-process; and
//!   3. Core Wasm, compiled to an owned-data npm build and run under Node.
//!
//! A separate test, `scalar_export_profile_refuses_any_module_with_agent_stage_shaped_records`,
//! pins a real backend divergence found while building this: the OTHER
//! existing Wasm export mechanism (`wasm::build_web_with_scalar_exports`,
//! the Public Scalar Export Profile v1) refuses this module outright with
//! `SPX-W115` because it declares any `record`/`variant` at all -- a
//! permanent admission gap for Agent-stage-shaped modules, not a fixture
//! artifact, and not something this test works around by hiding it.
//!
//! It covers both the `authorize` grant path and the `authorize` refusal
//! path with distinct cases (`case.authorize_granted`,
//! `case.authorize_refused`), specifically to avoid the failure mode of
//! comparing only a happy path where "both backends succeeded" would prove
//! nothing about whether they computed the same thing or took the same
//! branch.
//!
//! This is necessary evidence for any future stage-executor seam (the
//! compute substrate the four stage bodies need is already
//! backend-consistent) but it is NOT that seam: no authorization is minted,
//! no effect is dispatched, and no journal/recovery semantics are exercised
//! here. See `HANDOFF.md` (2026-09-12 entry) for the bounded design this
//! test's finding feeds, and for why building the actual seam was left for
//! independent review rather than self-approved in this slice.

use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::retained_call::{
    evaluate_retained_call, prepare_retained_call, RetainedCallOutcome, RetainedValue,
};
use semaprax::{codegen, hir, parse, project, verify, wasm};

/// The four bound deterministic stage bodies, unchanged in shape from
/// `src/agent_lifecycle/tests.rs`'s own fixture (Task/State/Observation/
/// Decision/Outcome/Report, `initialize`/`observe`/`authorize`/`reduce`),
/// plus five zero-argument `case.*` wrappers that call them directly with
/// literal fixture values and project each result to one `i64` so the
/// existing zero-arg scalar-status cross-engine mechanism can carry it.
const SOURCE: &str = r#"module test.stage_backend_parity;

@id("parity.type.task")
record Task {
    @id("parity.type.task.objective") objective: Bytes,
    @id("parity.type.task.budget") budget: i64,
}

@id("parity.type.state")
record State {
    @id("parity.type.state.objective") objective: Bytes,
    @id("parity.type.state.budget") budget: i64,
    @id("parity.type.state.epoch") epoch: i64,
}

@id("parity.type.observation")
record Observation {
    @id("parity.type.observation.tag") tag: Bytes,
    @id("parity.type.observation.budget") budget: i64,
    @id("parity.type.observation.epoch") epoch: i64,
}

@id("parity.type.decision")
variant Decision {
    @id("parity.type.decision.granted") Granted {
        @id("parity.type.decision.granted.seal") seal: Bytes,
        @id("parity.type.decision.granted.budget") budget: i64,
    },
    @id("parity.type.decision.refused") Refused {
        @id("parity.type.decision.refused.code") code: i64,
    },
}

@id("parity.type.outcome")
record Outcome {
    @id("parity.type.outcome.value") value: Bytes,
    @id("parity.type.outcome.status") status: i64,
}

@id("parity.type.report")
record Report {
    @id("parity.type.report.summary") summary: Bytes,
    @id("parity.type.report.budget") budget: i64,
    @id("parity.type.report.status") status: i64,
}

@id("parity.fn.initialize")
fn initialize(task: own Task) -> State
{
    State { objective: task.objective, budget: task.budget, epoch: 1 }
}

@id("parity.fn.observe")
fn observe(state: borrow State) -> Observation
{
    let tag = [79u8, 66u8];
    Observation { tag: bytes_copy(array_as_slice(tag)), budget: state.budget, epoch: state.epoch }
}

@id("parity.fn.authorize")
fn authorize(state: borrow State, budget: i64, urgent: bool, sequence: usize) -> Decision
{
    let seal = [65u8, 90u8];
    if budget <= state.budget && sequence > 0usize {
        Decision::Granted { seal: bytes_copy(array_as_slice(seal)), budget: budget }
    } else {
        Decision::Refused { code: if urgent { 2 } else { 1 } }
    }
}

@id("parity.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Report
{
    let stamp = [82u8, 80u8];
    Report {
        summary: bytes_copy(array_as_slice(stamp)),
        budget: state.budget - budget,
        status: outcome.status + state.epoch,
    }
}

@id("case.initialize")
fn case_initialize() -> i64
{
    let seed = [9u8, 9u8];
    let task = Task { objective: bytes_copy(array_as_slice(seed)), budget: 10 };
    let state = initialize(task);
    state.budget * 1000 + state.epoch
}

@id("case.observe")
fn case_observe() -> i64
{
    let seed = [1u8, 1u8];
    let state = State { objective: bytes_copy(array_as_slice(seed)), budget: 20, epoch: 3 };
    let observation = observe(state);
    observation.budget * 1000 + observation.epoch
}

@id("case.authorize_granted")
fn case_authorize_granted() -> i64
{
    let seed = [1u8, 1u8];
    let state = State { objective: bytes_copy(array_as_slice(seed)), budget: 10, epoch: 0 };
    let decision = authorize(state, 5, false, 1usize);
    match own decision {
        Decision::Granted { seal: s, budget: b } => 1000 + b,
        Decision::Refused { code: c } => 0 - c,
    }
}

@id("case.authorize_refused")
fn case_authorize_refused() -> i64
{
    let seed = [1u8, 1u8];
    let state = State { objective: bytes_copy(array_as_slice(seed)), budget: 10, epoch: 0 };
    let decision = authorize(state, 20, false, 1usize);
    match own decision {
        Decision::Granted { seal: s, budget: b } => 1000 + b,
        Decision::Refused { code: c } => 0 - c,
    }
}

@id("case.reduce")
fn case_reduce() -> i64
{
    let seed = [1u8, 1u8];
    let state = State { objective: bytes_copy(array_as_slice(seed)), budget: 10, epoch: 2 };
    let outcome = Outcome { value: bytes_copy(array_as_slice(seed)), status: 5 };
    let report = reduce(state, 3, false, 1usize, outcome);
    report.budget * 1000 + report.status
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// The five cases, in the fixed order this module drives every engine with.
const CASES: &[&str] = &[
    "case.initialize",
    "case.observe",
    "case.authorize_granted",
    "case.authorize_refused",
    "case.reduce",
];

/// Hand-computed expected values, one per `CASES` entry, from the fixture
/// bodies above:
/// - `initialize`: state.budget=10, state.epoch=1 -> 10*1000+1 = 10001
/// - `observe`: observation.budget=20, observation.epoch=3 -> 20*1000+3 = 20003
/// - `authorize` granted (budget 5 <= state.budget 10, sequence 1 > 0):
///   Granted { budget: 5 } -> 1000+5 = 1005
/// - `authorize` refused (budget 20 > state.budget 10): Refused { code: 1
///   (urgent=false) } -> 0-1 = -1
/// - `reduce`: report.budget = 10-3 = 7, report.status = 5+2 = 7 ->
///   7*1000+7 = 7007
const EXPECTED: &[i64] = &[10001, 20003, 1005, -1, 7007];

fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn require_tools_or_skip() -> bool {
    tool_available("clang") && tool_available("node")
}

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

fn temporary_root() -> std::path::PathBuf {
    let ordinal = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-stage-backend-parity-{}-{ordinal}",
        std::process::id()
    ))
}

fn c_symbol(declaration_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in declaration_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn normalized_stdout(output: Output, label: &str) -> String {
    assert!(
        output.status.success(),
        "{label} failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

/// A minimal out-of-process C harness calling each `case.*` zero-arg i64
/// export via its `spx_decl_<hex>` symbol, exactly the calling convention
/// `tests/scalar_status_backend_equivalence.rs` already establishes and this
/// module reuses unchanged. Every case here is unconditionally successful
/// (no `requires`/`ensures`), so only the success path is needed.
fn native_probe() -> String {
    let mut source = r#"
typedef spx_status_token (*spx_i64_case)(struct spx_context *, int64_t *);

static int spx_emit_i64(const char *id, spx_i64_case test_case) {
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(501), records, UINT32_C(2), NULL, NULL, NULL)) return 10;
    int64_t value = -INT64_C(777777777777777777);
    spx_status_token token = test_case(&context, &value);
    if (token != SPX_STATUS_SUCCESS || context.status_arena.length != UINT32_C(0)) return 11;
    printf("%s=%lld\n", id, (long long)value);
    return 0;
}

int main(void) {
    int result = 0;
"#
    .to_owned();
    for id in CASES {
        source.push_str(&format!(
            "    result = spx_emit_i64(\"{id}\", {});\n    if (result != 0) return result;\n",
            c_symbol(id)
        ));
    }
    source.push_str("    return 0;\n}\n");
    source
}

fn run_native(generated: &str, root: &Path, optimization: &str) -> String {
    let source = root.join(format!("native-{optimization}.c"));
    let executable = root.join(format!(
        "native-{optimization}{}",
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::write(&source, format!("{generated}\n{}", native_probe())).unwrap();
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
        "native {optimization} compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    normalized_stdout(
        Command::new(&executable).output().unwrap(),
        &format!("native {optimization}"),
    )
}

fn api_subject() -> project::PublicApiSubject<'static> {
    const FACT: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    project::PublicApiSubject {
        project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    }
}

/// The Public Scalar Export Profile v1 (`wasm::build_web_with_scalar_exports`)
/// is the mechanism `tests/scalar_status_backend_equivalence.rs` uses for its
/// own zero-arg i64 cross-engine parity, and it is the first one this module
/// tried. It refuses ANY module with an authored `record`/`variant`
/// declaration -- `SPX-W115` -- regardless of what an exported function's own
/// signature looks like. Since Task/State/Observation/Decision/Outcome/Report
/// are intrinsic to an Agent lifecycle's role types (not a fixture artifact:
/// `stages.rs::payload_shape`/`decision` require exactly these record/variant
/// shapes), this profile can never carry an Agent-stage-shaped module to a
/// Wasm boundary, independent of which functions are selected for export.
/// This is a genuine, permanent backend-admission fact worth pinning as a
/// regression, not a workaround target: #143's Wasm stage-executor seam
/// cannot be built on this profile.
#[test]
fn scalar_export_profile_refuses_any_module_with_agent_stage_shaped_records() {
    let program = parse(SOURCE, Path::new("stage-backend-parity.spx")).unwrap();
    let package = temporary_root();
    let error = wasm::build_web_with_scalar_exports(
        &program,
        &package,
        &[CASES[0].to_owned()],
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W115");
    assert!(!package.exists(), "a refused build must not publish a package");
}

/// Core Wasm, via the Public Owned Data API v1 project profile
/// (`project::derive_public_api_descriptor` +
/// `project::prepare_owned_data_npm_build`) -- the profile
/// `tests/project/owned_bytes_npm.rs` already exercises for owned-`Bytes`
/// exports, and which (unlike the scalar-export profile above) does not
/// reject a module for declaring records/variants elsewhere, only for an
/// export's OWN signature falling outside its parameter/result vocabulary.
/// Every `case.*` wrapper here takes zero parameters and returns plain `i64`,
/// both directly admitted, even though the module also declares
/// Task/State/Observation/Decision/Outcome/Report.
fn run_core_wasm(resolved: &hir::ResolvedProgram, root: &Path) -> String {
    let mut selected = CASES.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    selected.sort();
    let descriptor =
        project::derive_public_api_descriptor(resolved, &selected, api_subject()).unwrap();
    let build = project::prepare_owned_data_npm_build(
        resolved,
        &descriptor,
        "stage-backend-parity",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let directory = root.join("owned-data");
    std::fs::create_dir(&directory).unwrap();
    for row in envelope["artifacts"].as_array().unwrap() {
        let hex = row["hex"].as_str().unwrap();
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect();
        std::fs::write(directory.join(row["path"].as_str().unwrap()), bytes).unwrap();
    }
    let calls = CASES
        .iter()
        .map(|id| format!("process.stdout.write(`{id}=${{api.functions['{id}']()}}\\n`);"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        directory.join("observe.mjs"),
        format!(
            r#"import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm = new Uint8Array(fs.readFileSync(new URL('./app.wasm', import.meta.url)));
const api = await instantiate(wasm);
{calls}
"#
        ),
    )
    .unwrap();
    normalized_stdout(
        Command::new("node")
            .arg("observe.mjs")
            .current_dir(&directory)
            .output()
            .unwrap(),
        "Core-Wasm Node owned-data observer",
    )
}

/// The interpreter reference: each `case.*` function evaluated through
/// `interpreter::retained_call`, the exact mechanism
/// `CompiledAgentLifecycle::evaluate` uses to run a bound Agent stage. No
/// Agent-runtime machinery (authorization, journal, driver) is involved --
/// only the same "call one checked zero-effect function, get its value back"
/// seam the stage binding itself is built on.
fn run_interpreter(resolved: &hir::ResolvedProgram) -> String {
    let mut lines = String::new();
    for id in CASES {
        let prepared = prepare_retained_call(resolved, id)
            .unwrap_or_else(|errors| panic!("prepare {id} failed: {errors:?}"));
        let evaluation = evaluate_retained_call(resolved, &prepared, &[], 4096)
            .unwrap_or_else(|errors| panic!("evaluate {id} failed: {errors:?}"));
        let value = match evaluation.outcome {
            RetainedCallOutcome::Returned(RetainedValue::I64(value)) => value,
            other => panic!("case {id} did not return a plain i64: {other:?}"),
        };
        lines.push_str(&format!("{id}={value}\n"));
    }
    lines
}

fn expected_transcript() -> String {
    CASES
        .iter()
        .zip(EXPECTED)
        .map(|(id, value)| format!("{id}={value}\n"))
        .collect()
}

#[test]
fn interpreter_native_o0_o2_and_core_wasm_agree_on_stage_bodies() {
    let program = parse(SOURCE, Path::new("stage-backend-parity.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.is_empty(),
        "fixture verification failed: {diagnostics:?}"
    );
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    let interpreter = run_interpreter(&resolved);
    assert_eq!(
        interpreter,
        expected_transcript(),
        "interpreter result diverged from the hand-derived reference"
    );

    if !require_tools_or_skip() {
        eprintln!("skipping native/Core-Wasm cross-engine comparison: clang or node unavailable");
        return;
    }

    let generated = codegen::emit_c(&program).unwrap();
    let root = temporary_root();
    std::fs::create_dir(&root).unwrap();

    let native_o0 = run_native(&generated, &root, "-O0");
    let native_o2 = run_native(&generated, &root, "-O2");
    let core_wasm = run_core_wasm(&resolved, &root);

    assert_eq!(
        native_o0, native_o2,
        "native optimization level changed a stage body's result"
    );
    assert_eq!(
        native_o0, core_wasm,
        "native C11 and Core Wasm disagree on a stage body's result"
    );
    assert_eq!(
        native_o0, interpreter,
        "native/Core-Wasm result diverged from the interpreter reference \
         `CompiledAgentLifecycle::evaluate` itself uses"
    );

    let _ = std::fs::remove_dir_all(root);
}
