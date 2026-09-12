//! Closes the exact gap `carrier::settlement_corpus`'s own module
//! documentation names: "Native C11 is out of scope here... it has no
//! in-process Rust adapter analogous to `WasmProvider`/`InterpreterProvider`
//! to run this same case table against." This module runs the identical
//! base-shape and failure-injection corpus against all FOUR engines in one
//! test — `InterpreterProvider` and `WasmProvider` in-process (the exact
//! same public, unconditionally-compiled methods `carrier::settlement_corpus`
//! itself drives: `test_inject_failure`, `test_last_trace`,
//! `live_allocations`/`live_handles`/`live_bytes`,
//! `test_settlement_overwrite_attempts`), and native C11 compiled and
//! executed out-of-process at both `-O0` and `-O2`, reusing exactly the
//! `render_reference_provider`-plus-compiled-and-run mechanism
//! `fixture.rs` (#154) already established, rather than inventing a second
//! one.
//!
//! **How native crosses the process boundary.** `settlement_corpus_probe.c`
//! (generated below, one call site) embeds every case's already-encoded
//! canonical input carrier bytes as a `static const uint8_t[]`, drives
//! `spx_pg_input_prepare_v1`/`spx_pg_call_v1`/`spx_pg_result_export_v1` for
//! each case in one compiled binary, and prints one canonical
//! `CASE case_id=... accepted=... status=... live_alloc=... live_handles=...
//! fixture_live=... fixture_peak=... overwrite=... trace=... result=...`
//! line per case to stdout. [`parse_native_line`] parses those lines back
//! into [`NativeCaseOutcome`]; nothing here reinterprets or repairs the
//! printed sequence.
//!
//! **Issue #240: six confirmed divergences, each given a verdict — fix,
//! specify, or (for the sixth) specify-as-pre-existing — rather than left
//! as an unexamined nonclaim.** Issue #162 originally found five
//! trace-shape/order/counter divergences plus one wire-format divergence by
//! running this comparison across the FULL corpus, and filed all six as a
//! follow-up rather than fixing them (the owning files were outside that
//! issue's lease). Issue #240 is that follow-up: it re-examined each one
//! against [Public Generic Carrier v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md)
//! and its "The normalized trace" section specifically, decided which
//! engine (if either) was wrong, and either fixed the wrong side or pinned
//! the difference as a permanently permitted, explicitly asserted one —
//! never by narrowing what [`compare_case`] checks or silently dropping a
//! comparison.
//!
//! 1. **Root handle allocation (FIXED — native was wrong).**
//!    Interpreter/Wasm's flat-`Bytes` root handle always got its own real
//!    (if zero-byte) physical allocation slot and a matching
//!    `LeafAllocationStarted`/`Committed`/`PayloadCopied` triple (input
//!    side) or `Started`/`Committed` pair (result side) — see
//!    `CarrierCallMachine::fill_and_trace`
//!    (`src/public_generic_abi/carrier/machine.rs`), which iterates every
//!    entry in the handle set, root included. Native's own
//!    `spx_pg_fill_leaves` and its inline result-leaf-allocation loop
//!    (`src/public_generic_abi/native/provider_body.c`) never repeated that
//!    triple/pair for the root, in direct violation of the carrier spec's
//!    "Allocation/copy events ... are per-handle, one triple per root or
//!    leaf." Fixed: `spx_pg_fill_leaves` now records the root's
//!    unconditional, un-injectable triple before its per-leaf loop, the
//!    inline result-leaf loop records the matching pair before ITS loop,
//!    and `spx_pg_release_leaves` now records the root's own `LeafRelease`
//!    last (mirroring `CarrierCallMachine::release_set`'s reversed
//!    root-then-leaves iteration) before the one `CarrierRelease` — the
//!    release-side half of the identical root-handling gap, found while
//!    fixing this and completed here rather than left half-fixed.
//! 2. **Input release vs. `ExecutionFinished` order (FIXED — native was
//!    wrong).** `WasmProvider::call`/`InterpreterProvider::call` call
//!    `machine.finish_execution()` (recording `ExecutionFinished`) BEFORE
//!    `release_input_after_transfer()`, capturing (not yet acting on) the
//!    `ExecutionFinished` injection flag first and applying it only after
//!    the release completes. Native's `spx_pg_call_v1` used to release the
//!    input (`spx_pg_release_leaves`) BEFORE recording
//!    `SPX_PG_TRACE_EXECUTION_FINISHED` — a real ordering divergence in the
//!    normalized trace's own "record then act" convention, not a
//!    formatting artifact. Fixed: `spx_pg_call_v1` now records
//!    `ExecutionFinished` and captures its injection flag immediately after
//!    the endpoint call, then releases the input unconditionally, then acts
//!    on the captured flag — the identical order and the identical
//!    release-runs-regardless-of-injection discipline interpreter/Wasm use.
//! 3. **`TerminalStatus` on bound rejection (SPECIFIED — a permanently
//!    permitted difference).** Interpreter/Wasm's own earliest bound checks
//!    in `input_prepare` (`leaves.len() > MAX_OWNED_LEAVES_PER_INSTANCE`,
//!    `leaf.len() > MAX_BYTES_PER_LEAF`) return before a
//!    `CarrierCallMachine` is ever constructed, so nothing calls `settle`
//!    and their trace is empty; native's `spx_pg_settle` — the only
//!    status-selection mechanism `provider_body.c` has — unconditionally
//!    records `TerminalStatus`, even here, before `FrameValidated` is ever
//!    recorded. Both converge on the identical accept/reject and
//!    normalized status. Not fixed: nothing in the carrier spec requires
//!    the trace mechanism to exist before a `CarrierCallMachine` does, and
//!    giving native a parallel "reject without a trace" path purely to
//!    suppress one `TerminalStatus` event would add a second status-
//!    selection mechanism to a file whose whole discipline is exactly one.
//!    [`compare_case`] pins the exact permitted shape instead of merely
//!    skipping the comparison: interpreter/Wasm's trace is `[]`, native's
//!    is exactly `[TerminalStatus]`, on both
//!    `first_over_max_bytes_per_leaf` and `first_over_max_leaf_count`.
//! 4. **`InputValuePrepared` injection ordinal (SPECIFIED — a permanently
//!    permitted difference).** `InterpreterProvider::input_prepare` checks
//!    `self.take_injection_if(TraceLabel::InputValuePrepared)` and, if
//!    armed, settles and returns BEFORE ever calling
//!    `machine.prepare_input()` — the call that actually records
//!    `InputValuePrepared` — so the label never appears in the trace at
//!    all; native's `spx_pg_input_prepare_v1` records
//!    `SPX_PG_TRACE_INPUT_VALUE_PREPARED` FIRST and only then checks
//!    `spx_pg_should_inject(...)`, so the label DOES appear. Both converge
//!    on the identical `accepted`/`status` outcome (`AllocationFailure`) on
//!    `failure_injection_InputValuePrepared` — only the trace differs. Not
//!    fixed: this is one instance of a broader, deliberate architectural
//!    difference between the two engines' failure-injection timing for
//!    preparation-phase ordinals (interpreter/Wasm gate their own physical
//!    allocation loop behind injection checks and defer ALL logical
//!    recording to a single later call; native records incrementally as it
//!    goes), not a narrow one-line bug — reconciling it would mean
//!    reworking one engine's injection architecture to match the other's,
//!    which is a materially larger change than this issue's six confirmed
//!    cases call for. [`compare_case`] pins the exact permitted difference:
//!    native's trace contains `InputValuePrepared`; interpreter/Wasm's does
//!    not.
//!
//!    **Issue #103 extension: the identical batch-vs-incremental timing
//!    difference also affects `LeafAllocationStarted`,
//!    `LeafAllocationCommitted`, and `LeafPayloadCopied` — confirmed, not
//!    only `InputValuePrepared`.** `InterpreterProvider::input_prepare`'s
//!    per-leaf loop checks `take_injection_if` for all three of these
//!    labels too, and on any of them sets `fail_after` and `break`s BEFORE
//!    `machine.prepare_input()` — the one call that records
//!    `LeafAllocationStarted`/`LeafAllocationCommitted`/`LeafPayloadCopied`/
//!    `InputValuePrepared` for the whole handle set — is ever reached. So
//!    injecting on any one of these three ordinals leaves ALL FOUR labels
//!    absent from interpreter/Wasm's trace, not just the one injected on.
//!    Native's `spx_pg_fill_leaves` records each label immediately before
//!    its own injection check (`spx_pg_trace_record(started_label)` then
//!    `spx_pg_should_inject(started_label)`, and likewise for
//!    `committed_label`/`payload_label`), so the injected-upon label (and
//!    any earlier one in the per-leaf sequence) DOES appear. This is the
//!    SAME architectural difference divergence 4 already names, one
//!    ordinal earlier each time — not three new independent bugs. Verdict:
//!    SPECIFIED, a permanently permitted difference, for the identical
//!    reason divergence 4 is not fixed. [`compare_case`] pins it exactly:
//!    on `failure_injection_LeafAllocationStarted`,
//!    `failure_injection_LeafAllocationCommitted`, and
//!    `failure_injection_LeafPayloadCopied`, interpreter/Wasm's trace never
//!    contains the case's own injected-upon label; native's always does.
//! 5. **Post-cleanup-failure settle attempt (FIXED — native was wrong).**
//!    `WasmProvider::call`/`InterpreterProvider::call` check
//!    `state.machine.settlement()`/`machine.settlement()` immediately after
//!    the non-result input release and return early once it is already
//!    `Some`, never attempting a further settle call. Native's
//!    `spx_pg_call_v1` used to keep executing toward its own final,
//!    unconditional `spx_pg_settle(SPX_PG_STATUS_OK)` call even after a
//!    cleanup failure during that same release had already selected a
//!    sticky outcome — proposing a DIFFERENT status against an
//!    already-sticky one counts as a settlement-overwrite attempt, so
//!    native reported one on `failure_injection_LeafRelease` where
//!    interpreter/Wasm reported zero. The STICKY STATUS itself was always
//!    correct either way (the sticky rule discards the later proposal, and
//!    `spx_pg_call_v1` already only publishes `*out_result` when the
//!    sticky outcome is truly OK) — only the diagnostic overwrite counter
//!    differed. Fixed: that final settle now reads the already-sticky
//!    status directly instead of re-proposing `SPX_PG_STATUS_OK` against
//!    it, whenever one is already selected — the same "don't attempt a
//!    settle you already know is redundant" discipline interpreter/Wasm
//!    apply.
//! 6. **Result export wire format (SPECIFIED — a permanently permitted,
//!    pre-existing difference; scope note in the issue explicitly warns
//!    against "fixing" this without checking who depends on the current
//!    bytes).** `spx_pg_result_export_v1` (the
//!    `spx_pg_write_u64le(out_bytes + offset, result->leaf_count)` call
//!    before its per-leaf loop) prepends an 8-byte little-endian leaf-count
//!    field to the exported result carrier bytes that
//!    `InterpreterProvider::result_export`/`WasmProvider::result_export`
//!    (both frame each result leaf directly, no leading count) do not. Not
//!    fixed, per the issue's own scope note; [`strip_native_leaf_count_header`]
//!    pins the exact permitted difference instead of silently accepting it:
//!    it verifies the leading field really does equal the case's leaf
//!    count, THEN strips it, so the remaining byte-level check still proves
//!    the reversed-per-leaf payload bytes agree exactly.
//!
//! Divergences 1, 2, and 5 are real adapter-implementation choices that
//! WERE genuinely wrong on native's side against the carrier spec's own
//! text, and are fixed in `native/provider_body.c` (this issue's lease
//! grants that file, unlike issue #162's). Divergences 3, 4, and 6 are
//! specified: permanently permitted, and pinned by an explicit assertion
//! (with its own negative control) rather than a comparison [`compare_case`]
//! silently skips. Given this, [`compare_case`] still does NOT claim one
//! blanket "all four traces are identical," nor any reconciling filter
//! across the native/interpreter-Wasm boundary (a first draft of issue
//! #162's own investigation tried exactly that — a "singleton milestone"
//! filter — and divergence 4 broke it too, on a case the initial
//! single-case check never exercised: proof that a filter narrow enough to
//! pass on one case is not evidence it holds in general). It compares the
//! FULL trace exactly only where two engines genuinely share one physical
//! shape: interpreter vs. Wasm, and native-O0 vs. native-O2 (this module's
//! "native optimization equivalence" proof for trace shape, not only for
//! status/result bytes — `compare_case_rejects_a_native_o0_o2_full_trace_mismatch`
//! proves it is a real, failable check). It additionally compares, across
//! ALL FOUR engines: accept/reject, normalized status, the sticky-
//! settlement overwrite count (now equal across all four post-fix, not
//! merely within family), live resource counts, (normalized) result bytes,
//! and — for every case with no failure injected that the independently
//! pinned expectation accepts — native's own trace up to and including its
//! `TerminalStatus`, truncated there because native's trace buffer is
//! global/cumulative across this harness's own separate post-comparison
//! `result_release` call in a way interpreter/Wasm's per-call trace is not
//! (a harness artifact, not an engine divergence).
//!
//! **Other nonclaims.** Native's normalized trace (`spx_pg_test_trace_label_v1`)
//! records only the label ordinal per event, not a leaf index
//! (`g_spx_pg_trace` is `uint32_t[]`, not a `(label, leaf)` pair) — unlike
//! `semaprax::public_generic_abi::carrier::trace::TraceEvent`, which carries
//! both — so this module cannot additionally pin native's *per-leaf*
//! canonical release order the way
//! `structural_leaf_order_is_left_to_right_staged_and_exact_reverse_released`
//! does for interpreter vs. Wasm; extending native's test-only trace
//! surface to carry a leaf index is native/provider_body.c work, outside
//! this issue's lease. `spx_pg_test_settlement_overwrite_attempts_v1` and
//! `spx_pg_test_live_allocations_v1`/`fixture_peak` are process-global, not
//! per-provider (native has no per-instance accessor for either): this
//! module computes the settlement-overwrite count as a running delta so
//! per-case values are still comparable to interpreter/Wasm's per-provider
//! counters, and treats `fixture_peak` (a cumulative high-water mark since
//! the probe process started) only as an O0-vs-O2 self-consistency check —
//! both binaries execute the identical scripted case sequence, so the
//! cumulative peak at a given case index must match between them, which is
//! a real "native optimization equivalence" proof for peak counters, not an
//! isolated per-case expectation. #119's flat-owned-`Bytes`-leaves limit
//! applies here exactly as it does to the in-process engines: no nested
//! record case exists for native either. No compiled `.wasm` participates
//! anywhere in this repository (`WasmProvider` is an in-process Rust model,
//! matching `carrier::settlement_corpus`'s own nonclaim); this module does
//! not change that.

/// Issue #103's extension of issue #240 divergence 4 to three more
/// preparation-phase ordinals. Factored into its own file purely to stay
/// inside this file's line budget (`tests/module-size-budget.tsv`); it is
/// otherwise exactly as much a part of [`compare_case`] as any block that
/// stayed inline, and reaches this module's private items through `super`.
#[path = "settlement_corpus/ordinal_timing_extension.rs"]
mod ordinal_timing_extension;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use semaprax::public_generic_abi::boundary_profile::{
    MAX_BYTES_PER_LEAF, MAX_OWNED_LEAVES_PER_INSTANCE,
};
use semaprax::public_generic_abi::carrier::trace::TraceLabel;
use semaprax::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use semaprax::public_generic_abi::interpreter::{InterpreterPgStatus, InterpreterProvider};
use semaprax::public_generic_abi::native::binding::NativeProviderBindingV1;
use semaprax::public_generic_abi::native::template::render_reference_provider;
use semaprax::public_generic_abi::wasm::binding::WasmProviderBindingV1;
use semaprax::public_generic_abi::wasm::provider::{WasmPgStatus, WasmProvider};

static NEXT: AtomicU64 = AtomicU64::new(0);

const DESCRIPTOR_FIXTURE: &[u8] =
    b"semaprax.public-generic-settlement-corpus.v1.native-cross-engine-fixture";

fn interpreter_binding() -> CarrierBindingV1 {
    CarrierBindingV1::new(
        "sha256:settlement-corpus-native-descriptor-identity",
        TargetProfile::Interpreter,
        "sha256:settlement-corpus-native-runtime-identity",
    )
}

fn wasm_binding() -> WasmProviderBindingV1 {
    let carrier_binding = CarrierBindingV1::new(
        "sha256:settlement-corpus-native-descriptor-identity",
        TargetProfile::CoreWasm,
        "sha256:settlement-corpus-native-runtime-identity",
    );
    WasmProviderBindingV1::new(
        carrier_binding,
        "sha256:settlement-corpus-native-wasm-provider-artifact-fixture",
        semaprax::public_generic_abi::wasm::provider::FIXTURE_ENDPOINT_EXPORT_NAME,
        "fixture-compiler-backend-v1",
    )
}

fn native_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:settlement-corpus-native-descriptor-identity",
            TargetProfile::NativeC11,
            "sha256:settlement-corpus-native-runtime-identity",
        ),
        "sha256:settlement-corpus-native-provider-artifact-fixture",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
}

/// One shared settlement-corpus case, mirrored one-for-one from
/// `carrier::settlement_corpus::corpus()` (7 base shapes + one case per
/// non-terminal `TraceLabel` ordinal = 21), which this file cannot import
/// directly: that module is `#[cfg(test)]`-gated inside the `semaprax` lib
/// (only compiled for the crate's own unit-test build), invisible to an
/// external integration-test crate regardless of visibility modifiers.
#[derive(Debug, Clone)]
struct Case {
    case_id: &'static str,
    input_leaves: Vec<Vec<u8>>,
    failure_injection: Option<TraceLabel>,
    expected_accepted: bool,
    expected_status: i32,
}

fn corpus() -> Vec<Case> {
    let mut cases = vec![
        Case {
            case_id: "minimal_success",
            input_leaves: vec![b"hello".to_vec()],
            failure_injection: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "zero_length_owned_bytes",
            input_leaves: vec![Vec::new()],
            failure_injection: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "embedded_zero_bytes",
            input_leaves: vec![vec![0u8, 1, 0, 2, 0, 3, 0]],
            failure_injection: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "two_leaves_structural_order",
            input_leaves: vec![b"AA".to_vec(), b"BBB".to_vec()],
            failure_injection: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "max_bytes_per_leaf",
            input_leaves: vec![vec![0xABu8; MAX_BYTES_PER_LEAF]],
            failure_injection: None,
            expected_accepted: true,
            expected_status: InterpreterPgStatus::Ok as i32,
        },
        Case {
            case_id: "first_over_max_bytes_per_leaf",
            input_leaves: vec![vec![0u8; MAX_BYTES_PER_LEAF + 1]],
            failure_injection: None,
            expected_accepted: false,
            expected_status: InterpreterPgStatus::CarrierCapacity as i32,
        },
        Case {
            case_id: "first_over_max_leaf_count",
            input_leaves: vec![Vec::new(); MAX_OWNED_LEAVES_PER_INSTANCE + 1],
            failure_injection: None,
            expected_accepted: false,
            expected_status: InterpreterPgStatus::CarrierCapacity as i32,
        },
    ];

    // One case per non-terminal trace ordinal, matching
    // `carrier::settlement_corpus::corpus`'s injection matrix exactly, so
    // native is checked against the identical per-ordinal expected status
    // the in-process engines already are.
    let injection_matrix: &[(TraceLabel, i32)] = &[
        (
            TraceLabel::FrameValidated,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::LeafAllocationStarted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::LeafAllocationCommitted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::LeafPayloadCopied,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::InputValuePrepared,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::InputTransferCommitted,
            InterpreterPgStatus::IllegalTransition as i32,
        ),
        (
            TraceLabel::ExecutionStarted,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::ExecutionFinished,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::ResultLeafAllocationStarted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::ResultLeafAllocationCommitted,
            InterpreterPgStatus::AllocationFailure as i32,
        ),
        (
            TraceLabel::ResultValuePrepared,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::ResultCommit,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::LeafRelease,
            InterpreterPgStatus::ContractFailure as i32,
        ),
        (
            TraceLabel::CarrierRelease,
            InterpreterPgStatus::ContractFailure as i32,
        ),
    ];
    for (label, status) in injection_matrix {
        cases.push(Case {
            case_id: Box::leak(format!("failure_injection_{label:?}").into_boxed_str()),
            input_leaves: vec![b"inject-me".to_vec(), b"second-leaf".to_vec()],
            failure_injection: Some(*label),
            expected_accepted: false,
            expected_status: *status,
        });
    }
    cases
}

/// Length-prefix one field exactly like `public_generic_abi::frame`
/// (`pub(crate)`, unreachable from here): an 8-byte little-endian length,
/// then the bytes. Reimplemented rather than imported for the same reason
/// every existing C fixture in this directory already reimplements framing
/// by hand; the algorithm is a five-line, purely mechanical restatement.
fn frame(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// The independently pinned expected result: each leaf reversed, then
/// framed, concatenated in order — matching the fixture endpoint every
/// engine binds (`spx_pg_*_endpoint_reverse_bytes_v1`) and
/// `carrier::settlement_corpus::expected_result_bytes` exactly. This is
/// what `InterpreterProvider`/`WasmProvider::result_export` return
/// directly, and what native's export equals after
/// [`strip_native_leaf_count_header`] removes the one documented extra
/// field.
fn expected_result_bytes(leaves: &[Vec<u8>]) -> Vec<u8> {
    let mut framed = Vec::new();
    for leaf in leaves {
        let mut reversed = leaf.clone();
        reversed.reverse();
        frame(&mut framed, &reversed);
    }
    framed
}

/// Canonical input carrier bytes: an 8-byte little-endian leaf count, then
/// per leaf an 8-byte little-endian length and the raw bytes — the format
/// `spx_pg_input_prepare_v1` parses and `probe.c`'s own `build_carrier`
/// constructs by hand. Generated once here, in Rust, and embedded as a
/// literal byte array in the rendered C source below: native never
/// hand-authors different carrier bytes than what this same function
/// would produce for the identical leaves.
fn encode_input_carrier(leaves: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(leaves.len() as u64).to_le_bytes());
    for leaf in leaves {
        out.extend_from_slice(&(leaf.len() as u64).to_le_bytes());
        out.extend_from_slice(leaf);
    }
    out
}

/// Strip and verify native's one documented extra field (see this module's
/// own header doc): the leading 8-byte little-endian leaf count
/// `spx_pg_result_export_v1` writes ahead of the per-leaf frames that
/// `InterpreterProvider`/`WasmProvider::result_export` emit with no such
/// prefix. Panics (rather than silently truncating) if the field is
/// missing or does not equal `expected_leaf_count` — the normalization is
/// allowed to exist only because it is independently checked every time.
fn strip_native_leaf_count_header(native_result_bytes: &[u8], expected_leaf_count: u64) -> Vec<u8> {
    assert!(
        native_result_bytes.len() >= 8,
        "native result bytes too short to carry the documented leading leaf-count field"
    );
    let count = u64::from_le_bytes(native_result_bytes[0..8].try_into().unwrap());
    assert_eq!(
        count, expected_leaf_count,
        "native result's leading leaf-count field does not match this case's actual leaf count"
    );
    native_result_bytes[8..].to_vec()
}

/// The portion of a native outcome's trace that corresponds to ONE call,
/// for comparison against interpreter/Wasm's own per-call
/// `test_last_trace()`. Native's trace buffer is global and cumulative
/// across the whole probe process, not reset per call, so a case's own
/// `spx_pg_result_release_v1` (this harness's own post-comparison cleanup,
/// mirroring `provider.result_release(result_handle)` on the interpreter/
/// Wasm side) appends its own `LeafRelease`/`CarrierRelease` events AFTER
/// the call's own terminal event — a harness artifact, not an engine
/// divergence. Truncating at (and including) the first `TerminalStatus`
/// isolates the call itself. Panics if the outcome never recorded one,
/// which every ACCEPTED case must.
fn native_trace_through_terminal_status(native: &EngineOutcome) -> &[u32] {
    let terminal = TraceLabel::TerminalStatus as u32;
    let index = native
        .trace
        .iter()
        .position(|&label| label == terminal)
        .unwrap_or_else(|| {
            panic!(
                "{} never recorded TerminalStatus in its trace: {:?}",
                native.engine_id, native.trace
            )
        });
    &native.trace[..=index]
}

/// One engine's observed outcome for one case, shaped so
/// interpreter/Wasm/native-O0/native-O2 are all directly comparable:
/// `trace` is the label-ordinal sequence only (native has no per-leaf
/// index; see this module's header doc), and `result_bytes` is already
/// normalized (native's leading count field verified and stripped) so a
/// plain `Vec<u8>` equality is a real, exact check across every engine.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineOutcome {
    engine_id: &'static str,
    accepted: bool,
    primary_status: i32,
    result_bytes: Option<Vec<u8>>,
    trace: Vec<u32>,
    live_allocations: u64,
    live_handles: u64,
    settlement_overwrite_attempts: u64,
}

fn run_interpreter_case(case: &Case) -> EngineOutcome {
    let trusted = interpreter_binding();
    let mut provider = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted.encode(),
        &trusted,
    )
    .expect("the settlement-corpus fixture binding must open on the interpreter engine");
    if let Some(label) = case.failure_injection {
        provider.test_inject_failure(label);
    }
    let outcome = provider
        .input_prepare(&case.input_leaves)
        .and_then(|value| provider.call(value));
    let (accepted, primary_status, result_bytes) = match outcome {
        Ok(result_handle) => {
            let bytes = provider
                .result_export(result_handle, 1 << 20)
                .expect("export must succeed for an accepted interpreter-engine case");
            provider.result_release(result_handle);
            (true, InterpreterPgStatus::Ok as i32, Some(bytes))
        }
        Err(status) => (false, status as i32, None),
    };
    let trace = provider
        .test_last_trace()
        .iter()
        .map(|event| event.label as u32)
        .collect();
    let live_allocations = provider.live_allocations() as u64;
    let live_handles = provider.live_handles() as u64;
    let settlement_overwrite_attempts = provider.test_settlement_overwrite_attempts() as u64;
    let _ = provider.close();
    EngineOutcome {
        engine_id: "interpreter",
        accepted,
        primary_status,
        result_bytes,
        trace,
        live_allocations,
        live_handles,
        settlement_overwrite_attempts,
    }
}

fn run_wasm_case(case: &Case) -> EngineOutcome {
    let trusted = wasm_binding();
    let mut provider = WasmProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted.encode(),
        &trusted,
    )
    .expect("the settlement-corpus fixture binding must open on the Wasm engine");
    if let Some(label) = case.failure_injection {
        provider.test_inject_failure(label);
    }
    let outcome = provider
        .input_prepare(&case.input_leaves)
        .and_then(|value| provider.call(value));
    let (accepted, primary_status, result_bytes) = match outcome {
        Ok(result_handle) => {
            let bytes = provider
                .result_export(result_handle, 1 << 20)
                .expect("export must succeed for an accepted Wasm-engine case");
            provider.result_release(result_handle);
            (true, WasmPgStatus::Ok as i32, Some(bytes))
        }
        Err(status) => (false, status as i32, None),
    };
    let trace = provider
        .test_last_trace()
        .iter()
        .map(|event| event.label as u32)
        .collect();
    let live_allocations = provider.live_allocations() as u64;
    let live_handles = provider.live_handles() as u64;
    let settlement_overwrite_attempts = provider.test_settlement_overwrite_attempts() as u64;
    let _ = provider.close();
    EngineOutcome {
        engine_id: "core-wasm",
        accepted,
        primary_status,
        result_bytes,
        trace,
        live_allocations,
        live_handles,
        settlement_overwrite_attempts,
    }
}

fn c_byte_array(name: &str, bytes: &[u8]) -> String {
    let mut out = format!("static const uint8_t {name}[] = {{\n");
    for chunk in bytes.chunks(20) {
        out.push_str("    ");
        for byte in chunk {
            out.push_str(&format!("0x{byte:02x},"));
        }
        out.push('\n');
    }
    out.push_str("};\n");
    out
}

fn c_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{byte:02x}\"\"")),
        }
    }
    out.push('"');
    out
}

/// Render one compilable C11 translation unit: the rendered reference
/// provider (`render_reference_provider`, #154's own mechanism, reused
/// unmodified), plus one driver function per corpus case that runs it and
/// prints its canonical outcome line. Every case's carrier bytes come from
/// [`encode_input_carrier`] applied to the exact same `Case::input_leaves`
/// [`run_interpreter_case`]/[`run_wasm_case`] consume — native never
/// hand-authors different bytes for the identical logical input.
fn render_settlement_corpus_probe(cases: &[Case]) -> String {
    let provider_source = render_reference_provider(DESCRIPTOR_FIXTURE, &native_binding());

    let mut source = String::new();
    source.push_str(include_str!("../support/native_fixture_stdio.c"));
    source.push('\n');
    source.push_str(include_str!("allocations.c"));
    source.push('\n');
    source.push_str(&provider_source);
    source.push('\n');
    source.push_str(
        r#"/* --- settlement-corpus driver (issue #162): one function per case,
 * generated from the identical Rust `Case` table `run_interpreter_case`/
 * `run_wasm_case` consume. --- */
static size_t g_prev_overwrite_total = 0;
/* `g_spx_pg_trace`/`g_spx_pg_trace_len` (provider_body.c) are process-global
 * and append-only for the whole probe run, with no reset entry point
 * exposed -- exactly like the settlement-overwrite counter above, one
 * case's own trace is the slice recorded since the previous case, not the
 * cumulative total. */
static size_t g_prev_trace_len = 0;

/* Fixed scratch, deliberately never routed through `malloc`/`free`
 * (tracked by `allocations.c` above): this driver's OWN copy of the
 * exported result bytes must not appear in either counter this function
 * reports, or it would compare the provider's real settlement against a
 * count polluted by test-harness bookkeeping rather than the provider
 * itself. Sized for the corpus's own maximum case (one
 * `MAX_BYTES_PER_LEAF`-sized leaf plus the small fixed header). */
static uint8_t g_result_scratch[1 << 18];

static spx_pg_provider_v1 *open_trusted_provider(void) {
    spx_pg_provider_v1 *provider = NULL;
    spx_pg_status_v1 status =
        spx_pg_provider_open_v1(SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_DESCRIPTOR_LEN,
                                 SPX_PG_TRUSTED_BINDING_BYTES, SPX_PG_TRUSTED_BINDING_LEN, &provider);
    REQUIRE(status == SPX_PG_STATUS_OK);
    REQUIRE(provider != NULL);
    return provider;
}

static void run_one_case(const char *case_id, const uint8_t *carrier, size_t carrier_len,
                          int32_t injection_ordinal) {
    spx_pg_provider_v1 *provider = open_trusted_provider();
    if (injection_ordinal >= 0) {
        spx_pg_test_inject_failure_v1((uint32_t)injection_ordinal);
    }
    spx_pg_value_v1 *input = NULL;
    spx_pg_status_v1 status = spx_pg_input_prepare_v1(provider, carrier, carrier_len, &input);
    spx_pg_result_v1 *result = NULL;
    if (status == SPX_PG_STATUS_OK) {
        if (injection_ordinal >= 0) {
            spx_pg_test_inject_failure_v1((uint32_t)injection_ordinal);
        }
        status = spx_pg_call_v1(provider, input, &result);
    }
    int has_result = 0;
    size_t result_len = 0;
    if (status == SPX_PG_STATUS_OK) {
        REQUIRE(result != NULL);
        size_t required = 0;
        spx_pg_status_v1 sized = spx_pg_result_export_v1(result, NULL, 0, &required);
        REQUIRE(sized == SPX_PG_STATUS_BUFFER_TOO_SMALL || sized == SPX_PG_STATUS_OK);
        REQUIRE(required <= sizeof(g_result_scratch));
        size_t reported = 0;
        REQUIRE(spx_pg_result_export_v1(result, g_result_scratch, required, &reported) ==
                SPX_PG_STATUS_OK);
        result_len = reported;
        has_result = 1;
        REQUIRE(spx_pg_result_release_v1(&result) == SPX_PG_STATUS_OK);
    } else {
        REQUIRE(result == NULL);
    }
    /* Handle count is meaningful only while `provider` is still a live
     * pointer; alloc/byte counters are read AFTER close instead, exactly
     * like every existing `assert_fully_settled()` call in probe.c, so
     * the provider's own control-block allocation (freed by close) does
     * not read as a false "still live" resource. */
    size_t live_handles = spx_pg_test_live_handles_v1(provider);
    size_t trace_total = spx_pg_test_trace_len_v1();
    size_t trace_start = g_prev_trace_len;
    g_prev_trace_len = trace_total;
    size_t overwrite_total = spx_pg_test_settlement_overwrite_attempts_v1();
    size_t overwrite_delta = overwrite_total - g_prev_overwrite_total;
    g_prev_overwrite_total = overwrite_total;
    spx_pg_test_clear_failure_injection_v1();
    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    size_t live_alloc = spx_pg_test_live_allocations_v1();

    printf("CASE case_id=%s accepted=%d status=%d live_alloc=%zu live_handles=%zu "
           "fixture_live=%zu fixture_peak=%zu overwrite=%zu trace=",
           case_id, status == SPX_PG_STATUS_OK ? 1 : 0, (int)status, live_alloc, live_handles,
           fixture_live, fixture_peak, overwrite_delta);
    for (size_t index = trace_start; index < trace_total; ++index) {
        printf("%u%s", spx_pg_test_trace_label_v1(index), (index + 1 < trace_total) ? "," : "");
    }
    printf(" result=");
    if (has_result) {
        for (size_t index = 0; index < result_len; ++index) {
            printf("%02x", g_result_scratch[index]);
        }
    } else {
        printf("-");
    }
    printf("\n");
}

"#,
    );

    for (index, case) in cases.iter().enumerate() {
        let carrier = encode_input_carrier(&case.input_leaves);
        source.push_str(&c_byte_array(&format!("CASE_{index}_CARRIER"), &carrier));
    }

    source.push_str("int main(void) {\n    REQUIRE(fixture_binary_stdout());\n");
    for (index, case) in cases.iter().enumerate() {
        let ordinal = match case.failure_injection {
            Some(label) => (label as i64).to_string(),
            None => "-1".to_owned(),
        };
        source.push_str(&format!(
            "    run_one_case({}, CASE_{index}_CARRIER, sizeof(CASE_{index}_CARRIER), {ordinal});\n",
            c_string_literal(case.case_id)
        ));
    }
    source.push_str("    (void)puts(\"settlement-corpus-native-probe-done\");\n    return 0;\n}\n");
    source
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeCaseOutcome {
    outcome: EngineOutcome,
    fixture_peak_so_far: u64,
}

fn parse_native_line(line: &str, engine_id: &'static str) -> NativeCaseOutcome {
    // `CASE case_id=<id> accepted=<0|1> status=<n> live_alloc=<n>
    // live_handles=<n> fixture_live=<n> fixture_peak=<n> overwrite=<n>
    // trace=<csv-or-empty> result=<hex-or-dash>`
    let rest = line
        .strip_prefix("CASE ")
        .unwrap_or_else(|| panic!("unexpected native probe line: {line:?}"));
    let mut case_id = None;
    let mut accepted = None;
    let mut status = None;
    let mut live_alloc = None;
    let mut live_handles = None;
    let mut fixture_live = None;
    let mut fixture_peak = None;
    let mut overwrite = None;
    let mut trace = None;
    let mut result = None;
    for field in rest.split(' ') {
        let (key, value) = field
            .split_once('=')
            .unwrap_or_else(|| panic!("malformed native probe field {field:?} in {line:?}"));
        match key {
            "case_id" => case_id = Some(value.to_owned()),
            "accepted" => accepted = Some(value == "1"),
            "status" => status = Some(value.parse::<i32>().unwrap()),
            "live_alloc" => live_alloc = Some(value.parse::<u64>().unwrap()),
            "live_handles" => live_handles = Some(value.parse::<u64>().unwrap()),
            "fixture_live" => fixture_live = Some(value.parse::<u64>().unwrap()),
            "fixture_peak" => fixture_peak = Some(value.parse::<u64>().unwrap()),
            "overwrite" => overwrite = Some(value.parse::<u64>().unwrap()),
            "trace" => {
                trace = Some(if value.is_empty() {
                    Vec::new()
                } else {
                    value
                        .split(',')
                        .map(|ordinal| ordinal.parse::<u32>().unwrap())
                        .collect()
                })
            }
            "result" => result = Some(value.to_owned()),
            other => panic!("unknown native probe field {other:?} in {line:?}"),
        }
    }
    let live_alloc = live_alloc.expect("missing live_alloc");
    let live_handles = live_handles.expect("missing live_handles");
    let fixture_live = fixture_live.expect("missing fixture_live");
    assert_eq!(
        live_alloc, fixture_live,
        "the provider's own live-allocation counter and the independent external allocator \
         observation must agree for case {case_id:?}"
    );
    let result_hex = result.expect("missing result");
    let result_bytes = if result_hex == "-" {
        None
    } else {
        Some(
            (0..result_hex.len())
                .step_by(2)
                .map(|offset| u8::from_str_radix(&result_hex[offset..offset + 2], 16).unwrap())
                .collect::<Vec<u8>>(),
        )
    };
    NativeCaseOutcome {
        outcome: EngineOutcome {
            engine_id,
            accepted: accepted.expect("missing accepted"),
            primary_status: status.expect("missing status"),
            result_bytes,
            trace: trace.expect("missing trace"),
            live_allocations: live_alloc,
            live_handles,
            settlement_overwrite_attempts: overwrite.expect("missing overwrite"),
        },
        fixture_peak_so_far: fixture_peak.expect("missing fixture_peak"),
    }
}

fn compiler() -> PathBuf {
    env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from)
}

/// Compile [`render_settlement_corpus_probe`]'s output at `optimization`
/// and run it once, returning every case's parsed outcome in corpus order.
/// Mirrors `fixture.rs::run`'s own compile-then-execute pattern exactly.
fn compile_and_run_native(cases: &[Case], optimization: &str) -> Vec<NativeCaseOutcome> {
    let source_text = render_settlement_corpus_probe(cases);
    let root = env::temp_dir().join(format!(
        "semaprax-settlement-corpus-native-{optimization}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!(
        "retained native settlement-corpus probe evidence: {}",
        root.display()
    );

    let source = root.join("settlement_corpus_probe.c");
    fs::write(&source, &source_text).unwrap();

    let executable = root.join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    let compiled = Command::new(compiler())
        .current_dir(&root)
        .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}: {}",
        root.display(),
        String::from_utf8_lossy(&compiled.stderr)
    );

    let executed = Command::new(&executable)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}: stdout={} stderr={}",
        root.display(),
        String::from_utf8_lossy(&executed.stdout),
        String::from_utf8_lossy(&executed.stderr)
    );
    assert!(executed.stderr.is_empty());

    let stdout = String::from_utf8(executed.stdout).unwrap();
    let mut lines: Vec<&str> = stdout.lines().collect();
    let last = lines.pop();
    assert_eq!(last, Some("settlement-corpus-native-probe-done"));
    assert_eq!(
        lines.len(),
        cases.len(),
        "expected exactly one CASE line per corpus case"
    );
    let engine_id: &'static str = match optimization {
        "-O0" => "native-c11-O0",
        "-O2" => "native-c11-O2",
        other => panic!("unexpected optimization level {other:?}"),
    };
    lines
        .into_iter()
        .map(|line| parse_native_line(line, engine_id))
        .collect()
}

fn corpus_and_native_outcomes(
) -> &'static (Vec<Case>, Vec<NativeCaseOutcome>, Vec<NativeCaseOutcome>) {
    static CACHE: OnceLock<(Vec<Case>, Vec<NativeCaseOutcome>, Vec<NativeCaseOutcome>)> =
        OnceLock::new();
    CACHE.get_or_init(|| {
        let cases = corpus();
        let o0 = compile_and_run_native(&cases, "-O0");
        let o2 = compile_and_run_native(&cases, "-O2");
        (cases, o0, o2)
    })
}

/// The one checker every case funnels through: interpreter/Wasm run
/// in-process here; native-O0/native-O2 outcomes are passed in already
/// captured. Panics naming the case, the field, and the disagreeing
/// engines on any mismatch, mirroring
/// `carrier::settlement_corpus::compare`'s shape at the four-engine
/// boundary this file adds.
///
/// This function does NOT claim one blanket "all four engines are
/// identical in every field" — running it against the full corpus (not
/// only the one case that motivates a comparison) found FIVE genuine,
/// independent divergences between native and interpreter/Wasm, all cited
/// in this module's header doc: two in trace shape, one in trace-vs-empty
/// behavior on an early rejection, one in failure-injection timing, and
/// one in the sticky-settlement overwrite count on a specific cleanup-
/// failure shape (native's `spx_pg_call_v1` keeps executing toward its own
/// final `spx_pg_settle(SPX_PG_STATUS_OK)` after a cleanup failure during
/// input release already selected a sticky outcome, incurring one counted
/// overwrite attempt; `WasmProvider::call`/`InterpreterProvider::call`
/// check `machine.settlement()` after that same release step and return
/// early, never attempting a further settle). Papering over any of these
/// with a "reconciling" filter would hide a real finding rather than
/// report it — an earlier draft of this function tried exactly that for
/// the trace shape (a "singleton milestone" filter) and a fuller corpus
/// run broke it too. So: the FULL trace and the FULL settlement-overwrite
/// count are compared exactly only where two engines share the identical
/// physical/control-flow shape — interpreter vs. Wasm; native-O0 vs.
/// native-O2 (the latter is this module's "native optimization
/// equivalence" proof for both). Across the native/interpreter-Wasm family
/// boundary, this function compares only the fields the header doc does
/// NOT record as diverging: accept/reject, normalized status, live
/// resource counts, and (normalized) result bytes.
fn compare_case(
    case: &Case,
    interpreter: &EngineOutcome,
    wasm: &EngineOutcome,
    native_o0: &EngineOutcome,
    native_o2: &EngineOutcome,
) {
    let engines: [&EngineOutcome; 4] = [interpreter, wasm, native_o0, native_o2];
    for engine in engines {
        assert_eq!(
            engine.accepted, case.expected_accepted,
            "case {:?}: {} accept/reject does not match the independently pinned expectation",
            case.case_id, engine.engine_id
        );
        assert_eq!(
            engine.primary_status, case.expected_status,
            "case {:?}: {} normalized status does not match the independently pinned expectation",
            case.case_id, engine.engine_id
        );
        assert_eq!(
            engine.live_allocations, 0,
            "case {:?}: {} left live allocations",
            case.case_id, engine.engine_id
        );
        assert_eq!(
            engine.live_handles, 0,
            "case {:?}: {} left live handles",
            case.case_id, engine.engine_id
        );
    }
    for pair in engines.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert_eq!(
            a.accepted, b.accepted,
            "case {:?}: accept/reject disagrees between {} and {}",
            case.case_id, a.engine_id, b.engine_id
        );
        assert_eq!(
            a.primary_status, b.primary_status,
            "case {:?}: normalized status disagrees between {} and {}",
            case.case_id, a.engine_id, b.engine_id
        );
    }
    // Interpreter and Wasm share the identical physical trace shape
    // (including the zero-byte root slot's own event triple/pair) and
    // control flow (both short-circuit on an already-sticky settlement),
    // so their FULL trace must agree exactly.
    assert_eq!(
        interpreter.trace, wasm.trace,
        "case {:?}: full normalized trace disagrees between interpreter and core-wasm",
        case.case_id
    );
    // Native-O0 and native-O2 are the identical C source at two
    // optimization levels: their full trace must agree exactly, which is
    // this module's "native optimization equivalence" proof for trace
    // shape, not only for status/result bytes.
    assert_eq!(
        native_o0.trace, native_o2.trace,
        "case {:?}: full normalized trace disagrees between {} and {}",
        case.case_id, native_o0.engine_id, native_o2.engine_id
    );
    // Issue #240 divergence 5 (fixed): native used to keep attempting its
    // own final `spx_pg_settle(SPX_PG_STATUS_OK)` even after a cleanup
    // failure during input release had already selected a sticky outcome,
    // incurring one settlement-overwrite attempt interpreter/Wasm's early
    // return (once `machine.settlement()`/`state.machine.settlement()` is
    // already `Some`) never did. `provider_body.c`'s `spx_pg_call_v1` now
    // reads the already-sticky status directly instead of re-proposing
    // `SPX_PG_STATUS_OK` against it, so the overwrite-attempt count is
    // compared across ALL FOUR engines, not only within each family.
    for engine in engines {
        assert_eq!(
            engine.settlement_overwrite_attempts, interpreter.settlement_overwrite_attempts,
            "case {:?}: sticky-settlement overwrite-attempt count disagrees between {} and \
             interpreter (issue #240 divergence 5)",
            case.case_id, engine.engine_id
        );
    }
    // Issue #240 divergences 1 and 2 (fixed): native's flat-`Bytes` root
    // handle now gets the identical LeafAllocationStarted/Committed/
    // PayloadCopied triple (input side) and ResultLeafAllocationStarted/
    // Committed pair (result side) that interpreter/Wasm's shared
    // `CarrierCallMachine::fill_and_trace` always recorded for it, and
    // `ExecutionFinished` is now recorded before the non-result input
    // release, not after, matching `WasmProvider::call`/
    // `InterpreterProvider::call`'s placement exactly. For the base
    // success shapes (no failure injected), native's normalized trace up
    // to and including its own `TerminalStatus` is therefore now
    // byte-identical to interpreter/Wasm's full trace — checked here for
    // every such case, not merely the one that first motivated the fix.
    // (Native's trace can carry MORE events after that point: this
    // harness's own separate `spx_pg_result_release_v1`/`result_release`
    // call releases the published result after the comparison above
    // already captured it, and only native's trace buffer is global/
    // cumulative across that later call — a test-harness artifact, not an
    // engine divergence, so it is excluded by truncating at the first
    // `TerminalStatus` rather than compared.)
    if case.expected_accepted && case.failure_injection.is_none() {
        for native in [native_o0, native_o2] {
            let native_call_trace = native_trace_through_terminal_status(native);
            assert_eq!(
                native_call_trace,
                interpreter.trace.as_slice(),
                "case {:?}: {}'s normalized trace through TerminalStatus does not match \
                 interpreter/core-wasm's full trace (issue #240 divergences 1 and 2)",
                case.case_id,
                native.engine_id
            );
        }
    }
    // Issue #240 divergence 3 (specified: a permanently permitted
    // difference, not fixed). Interpreter/Wasm's own earliest bound checks
    // (`leaves.len() > MAX_OWNED_LEAVES_PER_INSTANCE`,
    // `leaf.len() > MAX_BYTES_PER_LEAF`) reject before a
    // `CarrierCallMachine` is ever constructed, so nothing calls `settle`
    // and their trace is empty. Native's `spx_pg_preflight_carrier`
    // performs the identical bound check, but native's own `spx_pg_settle`
    // (the only status-selection mechanism this file has) unconditionally
    // records `TerminalStatus`, even here, before `FrameValidated` is ever
    // recorded. The accept/reject outcome and normalized status already
    // agree across all four engines (checked above); this block pins the
    // one permitted difference exactly, rather than merely not comparing
    // it, per docs/PUBLIC-GENERIC-CARRIER-V1.md's "Nonclaims (reference
    // interpreter adapter and cross-engine corpus)".
    if case.case_id == "first_over_max_bytes_per_leaf"
        || case.case_id == "first_over_max_leaf_count"
    {
        assert_eq!(
            interpreter.trace,
            Vec::<u32>::new(),
            "case {:?}: interpreter's trace is no longer empty on this bound rejection; issue \
             #240 divergence 3's permitted difference no longer holds as specified",
            case.case_id
        );
        assert_eq!(
            wasm.trace,
            Vec::<u32>::new(),
            "case {:?}: core-wasm's trace is no longer empty on this bound rejection; issue #240 \
             divergence 3's permitted difference no longer holds as specified",
            case.case_id
        );
        for native in [native_o0, native_o2] {
            assert_eq!(
                native.trace,
                vec![TraceLabel::TerminalStatus as u32],
                "case {:?}: {}'s trace on this bound rejection is no longer exactly \
                 [TerminalStatus]; issue #240 divergence 3's permitted difference no longer \
                 holds as specified",
                case.case_id,
                native.engine_id
            );
        }
    }
    // Issue #240 divergence 4 (specified: a permanently permitted
    // difference, not fixed). `InterpreterProvider::input_prepare` checks
    // `self.take_injection_if(TraceLabel::InputValuePrepared)` and, if
    // armed, settles and returns BEFORE ever calling
    // `machine.prepare_input()` — the call that actually records
    // `InputValuePrepared` — so the label never appears in the trace at
    // all. Native's `spx_pg_input_prepare_v1` records
    // `SPX_PG_TRACE_INPUT_VALUE_PREPARED` first and only then checks
    // `spx_pg_should_inject(...)`, so the label DOES appear. Both converge
    // on the identical accepted/status outcome (checked above); this block
    // pins the one permitted trace difference exactly.
    if case.case_id == "failure_injection_InputValuePrepared" {
        let input_value_prepared = TraceLabel::InputValuePrepared as u32;
        assert!(
            !interpreter.trace.contains(&input_value_prepared),
            "case {:?}: interpreter's trace now contains InputValuePrepared; issue #240 \
             divergence 4's permitted difference no longer holds as specified",
            case.case_id
        );
        assert!(
            !wasm.trace.contains(&input_value_prepared),
            "case {:?}: core-wasm's trace now contains InputValuePrepared; issue #240 \
             divergence 4's permitted difference no longer holds as specified",
            case.case_id
        );
        for native in [native_o0, native_o2] {
            assert!(
                native.trace.contains(&input_value_prepared),
                "case {:?}: {}'s trace no longer contains InputValuePrepared; issue #240 \
                 divergence 4's permitted difference no longer holds as specified",
                case.case_id,
                native.engine_id
            );
        }
    }
    // Issue #103's investigation of the lead issue #240 flagged as out of
    // its own scope: the SAME architectural difference the block above pins
    // for `InputValuePrepared` also applies to `LeafAllocationStarted`,
    // `LeafAllocationCommitted`, and `LeafPayloadCopied`. Factored into its
    // own module (`ordinal_timing_extension.rs`) purely to stay inside this
    // file's line budget; see that module's header doc for the full
    // reasoning and verdict.
    ordinal_timing_extension::assert_ordinal_timing_extension(
        case,
        interpreter,
        wasm,
        native_o0,
        native_o2,
    );
    // Interpreter and Wasm already agree byte-for-byte on the raw result
    // (both frame each leaf directly, no leading count).
    assert_eq!(
        interpreter.result_bytes, wasm.result_bytes,
        "case {:?}: result carrier bytes disagree between interpreter and core-wasm",
        case.case_id
    );
    if case.expected_accepted {
        let expected = expected_result_bytes(&case.input_leaves);
        assert_eq!(
            interpreter.result_bytes.as_deref(),
            Some(expected.as_slice()),
            "case {:?}: interpreter result does not match the independently computed expected bytes",
            case.case_id
        );
        // Native's export carries one documented extra field (this
        // module's header doc); strip and verify it, then the remainder
        // must be byte-identical to interpreter/Wasm's raw export.
        for native in [native_o0, native_o2] {
            let native_bytes = native.result_bytes.as_ref().unwrap_or_else(|| {
                panic!(
                    "case {:?}: {} accepted but has no result",
                    case.case_id, native.engine_id
                )
            });
            let normalized =
                strip_native_leaf_count_header(native_bytes, case.input_leaves.len() as u64);
            assert_eq!(
                normalized, expected,
                "case {:?}: {}'s normalized result does not match the independently computed \
                 expected bytes",
                case.case_id, native.engine_id
            );
        }
    } else {
        assert_eq!(interpreter.result_bytes, None);
        assert_eq!(wasm.result_bytes, None);
        assert_eq!(
            native_o0.result_bytes, None,
            "case {:?}: native-O0 exposed a result on a rejected case",
            case.case_id
        );
        assert_eq!(
            native_o2.result_bytes, None,
            "case {:?}: native-O2 exposed a result on a rejected case",
            case.case_id
        );
    }
}

#[test]
fn native_o0_and_o2_agree_with_interpreter_and_wasm_across_the_shared_settlement_corpus() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    assert!(
        cases.len() >= 21,
        "the corpus must cover the base shapes and the full 14-ordinal injection matrix"
    );
    assert_eq!(o0.len(), cases.len());
    assert_eq!(o2.len(), cases.len());
    for ((case, native_o0), native_o2) in cases.iter().zip(o0.iter()).zip(o2.iter()) {
        let interpreter = run_interpreter_case(case);
        let wasm = run_wasm_case(case);
        compare_case(
            case,
            &interpreter,
            &wasm,
            &native_o0.outcome,
            &native_o2.outcome,
        );
    }
}

/// Native optimization equivalence (a required PG-7 criterion): the
/// cumulative external-allocator peak `allocations.c` observes must match
/// between the `-O0` and `-O2` binaries at every case index, since both
/// execute the byte-identical scripted case sequence from a fresh process.
/// This is the one comparison this module makes for a peak counter,
/// documented as cumulative-history rather than isolated-per-case (see
/// this module's header doc).
#[test]
fn native_o0_and_o2_cumulative_peak_allocations_agree_at_every_case_index() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(
            o0[index].fixture_peak_so_far, o2[index].fixture_peak_so_far,
            "case {:?} (index {index}): cumulative peak allocation count disagrees between \
             native -O0 and -O2",
            case.case_id
        );
    }
}

// ---------------------------------------------------------------------
// Negative controls: proof that `compare_case` can fail, not merely pass.
// ---------------------------------------------------------------------

#[test]
#[should_panic(
    expected = "normalized result does not match the independently computed expected bytes"
)]
fn compare_case_rejects_a_perturbed_native_result() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let case = &cases[0];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    let mut corrupted_o0 = o0[0].outcome.clone();
    let bytes = corrupted_o0
        .result_bytes
        .as_mut()
        .expect("the minimal-success case must have a native result");
    *bytes.last_mut().unwrap() ^= 0xFF;
    compare_case(case, &interpreter, &wasm, &corrupted_o0, &o2[0].outcome);
}

/// Proof that native-O0-vs-O2 full-trace equality (this module's "native
/// optimization equivalence" proof for trace shape) is a real, failable
/// check: perturb one recorded label in the -O2 run's trace and confirm
/// the comparison rejects it.
#[test]
#[should_panic(
    expected = "full normalized trace disagrees between native-c11-O0 and native-c11-O2"
)]
fn compare_case_rejects_a_native_o0_o2_full_trace_mismatch() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let case = &cases[0];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    let mut corrupted_o2 = o2[0].outcome.clone();
    assert!(
        !corrupted_o2.trace.is_empty(),
        "the minimal-success case must record a trace"
    );
    corrupted_o2.trace[0] = 9999;
    compare_case(case, &interpreter, &wasm, &o0[0].outcome, &corrupted_o2);
}

#[test]
#[should_panic(expected = "left live allocations")]
fn compare_case_rejects_a_nonzero_native_live_allocation_count() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let case = &cases[0];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    let mut leaked_o0 = o0[0].outcome.clone();
    leaked_o0.live_allocations = 1;
    compare_case(case, &interpreter, &wasm, &leaked_o0, &o2[0].outcome);
}

/// `spx_pg_test_settlement_overwrite_attempts_v1` is a process-global
/// monotonic counter (see this module's header doc): confirms the
/// per-case delta this module computes is exactly zero for a case that
/// injects no cleanup-failure at all, matching interpreter/Wasm's own
/// fresh-per-provider counter for the identical case.
#[test]
fn native_settlement_overwrite_delta_is_zero_for_a_case_with_no_cleanup_injection() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let index = cases
        .iter()
        .position(|case| case.case_id == "minimal_success")
        .unwrap();
    assert_eq!(o0[index].outcome.settlement_overwrite_attempts, 0);
    assert_eq!(o2[index].outcome.settlement_overwrite_attempts, 0);
}

// ---------------------------------------------------------------------
// Issue #240: negative controls for the four assertions tightened above,
// each proving its check is real and failable, not merely passing because
// nothing exercises it (the same pattern the three controls above already
// established for the pre-existing checks).
// ---------------------------------------------------------------------

/// Divergence 5 (fixed): proves the new 4-way settlement-overwrite-attempt
/// comparison actually fires. Before the fix, this exact perturbation is
/// what native's own unconditional final `spx_pg_settle(SPX_PG_STATUS_OK)`
/// produced for real on `failure_injection_LeafRelease`.
#[test]
#[should_panic(
    expected = "sticky-settlement overwrite-attempt count disagrees between native-c11-O0 and \
                interpreter (issue #240 divergence 5)"
)]
fn compare_case_rejects_a_native_settlement_overwrite_count_disagreement() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let case = &cases[0];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    let mut corrupted_o0 = o0[0].outcome.clone();
    corrupted_o0.settlement_overwrite_attempts = 1;
    compare_case(case, &interpreter, &wasm, &corrupted_o0, &o2[0].outcome);
}

/// Divergences 1 and 2 (fixed): proves the new truncated-trace comparison
/// on an accepted, non-injected case actually fires. Removes the root's own
/// `LeafAllocationStarted`/`Committed`/`PayloadCopied` triple (indices 1..4,
/// right after `FrameValidated` at index 0) from both native optimization
/// levels' traces — reproducing exactly what native's pre-fix "rootless leaf
/// loop" produced for real — and confirms `compare_case` rejects it, rather
/// than silently comparing only the pre-existing fields.
#[test]
#[should_panic(
    expected = "normalized trace through TerminalStatus does not match interpreter/core-wasm's \
                full trace (issue #240 divergences 1 and 2)"
)]
fn compare_case_rejects_a_native_trace_missing_the_root_allocation_triple() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let case = &cases[0];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    let mut corrupted_o0 = o0[0].outcome.clone();
    let mut corrupted_o2 = o2[0].outcome.clone();
    assert!(
        corrupted_o0.trace.len() > 4 && corrupted_o2.trace.len() > 4,
        "the minimal-success case must have a long enough native trace to corrupt"
    );
    corrupted_o0.trace.drain(1..4);
    corrupted_o2.trace.drain(1..4);
    compare_case(case, &interpreter, &wasm, &corrupted_o0, &corrupted_o2);
}

/// Divergence 3 (specified): proves the pinned "empty on interpreter/Wasm,
/// exactly `[TerminalStatus]` on native" assertion for the earliest bound
/// rejection actually fires, rather than the corpus merely never comparing
/// it. Both interpreter's and core-wasm's traces are perturbed identically
/// (so the earlier interpreter-vs-wasm full-trace check still passes) to a
/// non-empty trace, which the divergence-3 block must still reject.
#[test]
#[should_panic(
    expected = "interpreter's trace is no longer empty on this bound rejection; issue #240 \
                divergence 3's permitted difference no longer holds as specified"
)]
fn compare_case_rejects_a_bound_rejection_trace_that_is_no_longer_empty() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let index = cases
        .iter()
        .position(|case| case.case_id == "first_over_max_bytes_per_leaf")
        .unwrap();
    let case = &cases[index];
    let mut interpreter = run_interpreter_case(case);
    let mut wasm = run_wasm_case(case);
    interpreter.trace.push(TraceLabel::FrameValidated as u32);
    wasm.trace.push(TraceLabel::FrameValidated as u32);
    compare_case(
        case,
        &interpreter,
        &wasm,
        &o0[index].outcome,
        &o2[index].outcome,
    );
}

/// Divergence 4 (specified): proves the pinned "native's trace contains
/// `InputValuePrepared`, interpreter/Wasm's does not" assertion actually
/// fires. Simulates native no longer diverging (as if its own
/// record-then-check ordering for this one ordinal were fixed to match
/// interpreter/Wasm) by stripping the label from both native optimization
/// levels' traces identically, and confirms the specified permitted
/// difference is then correctly reported as no longer holding.
#[test]
#[should_panic(
    expected = "native-c11-O0's trace no longer contains InputValuePrepared; issue #240 \
                divergence 4's permitted difference no longer holds as specified"
)]
fn compare_case_rejects_a_native_trace_that_no_longer_contains_input_value_prepared() {
    let (cases, o0, o2) = corpus_and_native_outcomes();
    let index = cases
        .iter()
        .position(|case| case.case_id == "failure_injection_InputValuePrepared")
        .unwrap();
    let case = &cases[index];
    let interpreter = run_interpreter_case(case);
    let wasm = run_wasm_case(case);
    let input_value_prepared = TraceLabel::InputValuePrepared as u32;
    let mut corrupted_o0 = o0[index].outcome.clone();
    let mut corrupted_o2 = o2[index].outcome.clone();
    corrupted_o0
        .trace
        .retain(|&label| label != input_value_prepared);
    corrupted_o2
        .trace
        .retain(|&label| label != input_value_prepared);
    compare_case(case, &interpreter, &wasm, &corrupted_o0, &corrupted_o2);
}
