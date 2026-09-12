//! Issue #162's own divergence 7: found by this issue's compound
//! cleanup-after-an-earlier-failure case
//! (`execution_failure_with_compounding_cleanup_injection`), not by issue
//! #240's original investigation.
//!
//! **The case.** `ExecutionStarted`'s own injected failure is armed
//! alongside a `LeafRelease` cleanup injection, in the same call, on all
//! four engines. `ExecutionStarted` settles first (its own failure is the
//! authoritative one); `LeafRelease`'s injected attempt during the input
//! release that immediately follows is a genuinely later, distinct
//! attempt that "cleanup cannot replace the selected status" requires be
//! rejected, not applied.
//!
//! **Confirmed, by running it.** Every engine agrees on accept/reject and
//! normalized status (both unaffected either way: a rejected settlement
//! attempt never changes which status wins, only whether it is counted).
//! Interpreter and core-wasm both report exactly ONE settlement-overwrite
//! attempt for this case; native-O0 and native-O2 both report ZERO.
//!
//! **Root cause.** Interpreter/Wasm's sticky check
//! (`CallLedger::settle`, `src/public_generic_abi/carrier.rs`) compares by
//! the typed `Settlement` ENUM VARIANT: `Settlement::ContractFailure`
//! (from `ExecutionStarted`) and `Settlement::CleanupFailure` (from the
//! compounding `LeafRelease` injection) are different variants, so the
//! second `settle` call is recognized as a genuinely different outcome and
//! counted as an overwrite attempt — even though both variants map to the
//! identical `ContractFailure` WIRE status. Native's sticky check
//! (`spx_pg_settle`, `src/public_generic_abi/native/provider_body.c`) has
//! no such typed "reason" independent of the wire status it compares by
//! raw integer value: both its `ExecutionStarted` and `LeafRelease`
//! injection sites settle the identical `SPX_PG_STATUS_CONTRACT_FAILURE`
//! value, so the second attempt is indistinguishable from re-asserting the
//! SAME outcome (`CallLedger::settle`'s own `Some(existing) if existing ==
//! outcome => Ok(())` idempotent-reassertion branch has no native
//! equivalent because native has nothing narrower than the wire status to
//! compare) and is never counted.
//!
//! **Verdict: SPECIFIED — a permanently permitted difference, for the
//! identical class of reason issue #240 divergence 4 is not fixed.**
//! Giving native's settlement mechanism a typed reason distinct from its
//! wire status, purely to make one diagnostic counter agree on a case with
//! no independent behavioral consequence (the STATUS the caller observes
//! is identical either way), is a materially larger change than this
//! issue's compound-failure-coverage scope calls for — the same
//! architecture-reworking class of change divergence 4 already declined.
//! Pinned here exactly, with a negative control proving the assertion is
//! real and failable, rather than left as an unexamined gap.

use semaprax::public_generic_abi::carrier::trace::TraceLabel;
use semaprax::public_generic_abi::interpreter::InterpreterPgStatus;

use super::{Case, EngineOutcome};

/// The one case this whole module exists to prove: `ExecutionStarted`'s
/// own injected failure is the primary; a release-ordinal injection ALSO
/// armed for the same call is a later, distinct attempt the sticky rule
/// must reject, not apply — matching
/// `src/public_generic_abi/carrier/settlement_corpus.rs`'s own identical
/// case exactly. `expected_accepted`/`expected_status` are unaffected by
/// whether the compounding rejection genuinely happens; only
/// [`assert_divergence_7`]'s `settlement_overwrite_attempts` comparison is
/// sensitive to that.
pub(super) fn compounding_case() -> Case {
    Case {
        case_id: "execution_failure_with_compounding_cleanup_injection",
        input_leaves: vec![b"inject-me".to_vec(), b"second-leaf".to_vec()],
        failure_injection: Some(TraceLabel::ExecutionStarted),
        compound_cleanup_injection: Some(TraceLabel::LeafRelease),
        expected_accepted: false,
        expected_status: InterpreterPgStatus::ContractFailure as i32,
    }
}

/// Called from [`super::compare_case`] for every case. A no-op unless
/// `case` is the one compounding-cleanup-injection case this divergence
/// names, mirroring exactly how `ordinal_timing_extension`'s own function
/// self-gates on case id.
pub(super) fn assert_divergence_7(
    case: &Case,
    interpreter: &EngineOutcome,
    wasm: &EngineOutcome,
    native_o0: &EngineOutcome,
    native_o2: &EngineOutcome,
) {
    if case.case_id != "execution_failure_with_compounding_cleanup_injection" {
        return;
    }
    assert_eq!(
        interpreter.settlement_overwrite_attempts, 1,
        "case {:?}: interpreter no longer counts the compounding cleanup injection; issue #162 \
         divergence 7's premise no longer holds",
        case.case_id
    );
    assert_eq!(
        wasm.settlement_overwrite_attempts, 1,
        "case {:?}: core-wasm no longer counts the compounding cleanup injection; issue #162 \
         divergence 7's premise no longer holds",
        case.case_id
    );
    for native in [native_o0, native_o2] {
        assert_eq!(
            native.settlement_overwrite_attempts, 0,
            "case {:?}: {}'s overwrite-attempt count is no longer zero on this case; issue #162 \
             divergence 7's permitted difference no longer holds as specified",
            case.case_id, native.engine_id
        );
    }
}

// ---------------------------------------------------------------------
// Negative control: proof that `assert_divergence_7` can fail, not merely
// pass, following the identical pattern
// `compare_case_rejects_a_native_settlement_overwrite_count_disagreement`
// already established in the parent module for divergence 5.
// ---------------------------------------------------------------------

/// `compare_case`'s own generic divergence-5 block (immediately above the
/// call to `assert_divergence_7`) already excludes this case id from its
/// blanket 4-way equality check — see its own comment — so corrupting
/// native's overwrite count here to a NONZERO value exercises this file's
/// assertion specifically, not that earlier, unrelated one.
#[test]
#[should_panic(
    expected = "native-c11-O0's overwrite-attempt count is no longer zero on this case; issue \
                #162 divergence 7's permitted difference no longer holds as specified"
)]
fn compare_case_rejects_a_native_divergence_7_overwrite_count_that_is_no_longer_zero() {
    let (cases, o0, o2) = super::corpus_and_native_outcomes();
    let index = cases
        .iter()
        .position(|case| case.case_id == "execution_failure_with_compounding_cleanup_injection")
        .unwrap();
    let case = &cases[index];
    let interpreter = super::run_interpreter_case(case);
    let wasm = super::run_wasm_case(case);
    let mut corrupted_o0 = o0[index].outcome.clone();
    assert_eq!(
        corrupted_o0.settlement_overwrite_attempts, 0,
        "native must genuinely report zero overwrite attempts on this case before this control \
         can prove a nonzero value is detected"
    );
    corrupted_o0.settlement_overwrite_attempts = 1;
    super::compare_case(case, &interpreter, &wasm, &corrupted_o0, &o2[index].outcome);
}

/// Sibling control: proves the interpreter/core-wasm side of the same
/// assertion is real too, by corrupting interpreter's count to zero
/// (matching what a since-changed engine that no longer counts the
/// compounding attempt would report) rather than native's.
#[test]
#[should_panic(
    expected = "interpreter no longer counts the compounding cleanup injection; issue #162 \
                divergence 7's premise no longer holds"
)]
fn compare_case_rejects_an_interpreter_divergence_7_overwrite_count_that_is_no_longer_one() {
    let (cases, o0, o2) = super::corpus_and_native_outcomes();
    let index = cases
        .iter()
        .position(|case| case.case_id == "execution_failure_with_compounding_cleanup_injection")
        .unwrap();
    let case = &cases[index];
    let mut interpreter = super::run_interpreter_case(case);
    let wasm = super::run_wasm_case(case);
    assert_eq!(
        interpreter.settlement_overwrite_attempts, 1,
        "interpreter must genuinely report one overwrite attempt on this case before this \
         control can prove a changed value is detected"
    );
    interpreter.settlement_overwrite_attempts = 0;
    super::compare_case(
        case,
        &interpreter,
        &wasm,
        &o0[index].outcome,
        &o2[index].outcome,
    );
}
