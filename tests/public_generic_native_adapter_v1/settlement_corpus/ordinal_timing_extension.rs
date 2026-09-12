//! Issue #103's extension of issue #240 divergence 4.
//!
//! Issue #240 pinned exactly one preparation-phase ordinal
//! (`InputValuePrepared`) as a permanently permitted timing difference
//! between native and interpreter/Wasm, and named the root cause as a
//! broader architectural split: interpreter/Wasm gate their whole physical
//! allocation loop behind injection checks and defer ALL logical trace
//! recording for the handle set to one later call
//! (`CarrierCallMachine::prepare_input`/`fill_and_trace`,
//! `src/public_generic_abi/carrier/machine.rs`); native
//! (`spx_pg_fill_leaves`, `src/public_generic_abi/native/provider_body.c`)
//! records each label immediately, in place, as it goes. Issue #240's own
//! text flagged that this same split plausibly also affects
//! `LeafAllocationStarted`, `LeafAllocationCommitted`, and
//! `LeafPayloadCopied`, but left investigating and pinning those three
//! explicitly out of its scope and into issue #103's.
//!
//! **Confirmed, by reading both sides.**
//! `InterpreterProvider::input_prepare` (`src/public_generic_abi/interpreter.rs`)
//! checks `take_injection_if` for `LeafAllocationStarted`,
//! `LeafAllocationCommitted`, and `LeafPayloadCopied` inside its per-leaf
//! physical-allocation loop, exactly like it does for `InputValuePrepared`
//! right after that loop. Injecting on ANY of the three sets `fail_after`
//! and `break`s the loop before `machine.prepare_input()` — the one call
//! that records the whole batch — is ever reached, so interpreter/Wasm's
//! trace contains NONE of {LeafAllocationStarted, LeafAllocationCommitted,
//! LeafPayloadCopied, InputValuePrepared} for these three cases, not only
//! the one label under injection. Native's `spx_pg_fill_leaves` records
//! `started_label`/`committed_label`/`payload_label` immediately before
//! each of its own three injection checks in the per-leaf loop (plus an
//! unconditional, un-injectable copy of the same triple for the root
//! handle — issue #240 divergence 1's own fix), so the injected-upon label
//! (and any earlier one in the per-leaf sequence) DOES appear on native.
//!
//! **Verdict: SPECIFIED — a permanently permitted difference, for the
//! identical reason divergence 4 itself is not fixed.** This is not three
//! new independent bugs; it is the SAME architectural difference,
//! triggered one ordinal earlier each time. Reconciling it would mean
//! reworking one engine's injection-timing architecture (batch, checked
//! before recording) to match the other's (incremental, recorded then
//! checked) — a materially larger change than a narrow per-ordinal fix, for
//! the same reason issue #240 gave for `InputValuePrepared`. Pinned here
//! exactly, with negative controls proving each assertion is real and
//! failable, rather than left as the unexamined gap issue #240 flagged.

use semaprax::public_generic_abi::carrier::trace::TraceLabel;

use super::{Case, EngineOutcome};

/// Called from [`super::compare_case`] for every case. A no-op unless
/// `case` is one of the three cases this extension names; the case-id
/// filter mirrors exactly how `compare_case`'s own divergence-3/4 blocks
/// gate on specific case ids.
pub(super) fn assert_ordinal_timing_extension(
    case: &Case,
    interpreter: &EngineOutcome,
    wasm: &EngineOutcome,
    native_o0: &EngineOutcome,
    native_o2: &EngineOutcome,
) {
    for label in [
        TraceLabel::LeafAllocationStarted,
        TraceLabel::LeafAllocationCommitted,
        TraceLabel::LeafPayloadCopied,
    ] {
        if case.case_id != format!("failure_injection_{label:?}") {
            continue;
        }
        let ordinal = label as u32;
        assert!(
            !interpreter.trace.contains(&ordinal),
            "case {:?}: interpreter's trace now contains {:?}; issue #103's extension of issue \
             #240 divergence 4 no longer holds as specified",
            case.case_id,
            label
        );
        assert!(
            !wasm.trace.contains(&ordinal),
            "case {:?}: core-wasm's trace now contains {:?}; issue #103's extension of issue \
             #240 divergence 4 no longer holds as specified",
            case.case_id,
            label
        );
        for native in [native_o0, native_o2] {
            assert!(
                native.trace.contains(&ordinal),
                "case {:?}: {}'s trace no longer contains {:?}; issue #103's extension of issue \
                 #240 divergence 4 no longer holds as specified",
                case.case_id,
                native.engine_id,
                label
            );
        }
    }
}

// ---------------------------------------------------------------------
// Negative controls: proof that `assert_ordinal_timing_extension` can
// fail, not merely pass, following the identical pattern
// `compare_case_rejects_a_native_trace_that_no_longer_contains_input_value_prepared`
// already established in the parent module for `InputValuePrepared` itself.
// ---------------------------------------------------------------------

fn rejects_a_native_trace_missing_its_case_label(label: TraceLabel) {
    let (cases, o0, o2) = super::corpus_and_native_outcomes();
    let case_id = format!("failure_injection_{label:?}");
    let index = cases
        .iter()
        .position(|case| case.case_id == case_id)
        .unwrap();
    let case = &cases[index];
    let interpreter = super::run_interpreter_case(case);
    let wasm = super::run_wasm_case(case);
    let ordinal = label as u32;
    let mut corrupted_o0 = o0[index].outcome.clone();
    let mut corrupted_o2 = o2[index].outcome.clone();
    assert!(
        corrupted_o0.trace.contains(&ordinal) && corrupted_o2.trace.contains(&ordinal),
        "native must have recorded {label:?} for its own injected-upon case before this control \
         can prove removing it is detected"
    );
    corrupted_o0.trace.retain(|&item| item != ordinal);
    corrupted_o2.trace.retain(|&item| item != ordinal);
    super::compare_case(case, &interpreter, &wasm, &corrupted_o0, &corrupted_o2);
}

#[test]
#[should_panic(
    expected = "native-c11-O0's trace no longer contains LeafAllocationStarted; issue #103's \
                extension of issue #240 divergence 4 no longer holds as specified"
)]
fn compare_case_rejects_a_native_trace_that_no_longer_contains_leaf_allocation_started() {
    rejects_a_native_trace_missing_its_case_label(TraceLabel::LeafAllocationStarted);
}

#[test]
#[should_panic(
    expected = "native-c11-O0's trace no longer contains LeafAllocationCommitted; issue #103's \
                extension of issue #240 divergence 4 no longer holds as specified"
)]
fn compare_case_rejects_a_native_trace_that_no_longer_contains_leaf_allocation_committed() {
    rejects_a_native_trace_missing_its_case_label(TraceLabel::LeafAllocationCommitted);
}

#[test]
#[should_panic(
    expected = "native-c11-O0's trace no longer contains LeafPayloadCopied; issue #103's \
                extension of issue #240 divergence 4 no longer holds as specified"
)]
fn compare_case_rejects_a_native_trace_that_no_longer_contains_leaf_payload_copied() {
    rejects_a_native_trace_missing_its_case_label(TraceLabel::LeafPayloadCopied);
}
