//! Coverage for [`CarrierCallMachine`]: a full success run's trace and
//! settlement, and an exhaustive illegal-transition surface at the
//! orchestration level — double transfer, use after transfer, copy-out
//! after release, a failure arriving mid-transfer, release exactly once,
//! and cleanup attempting to overwrite a sticky failure status.

use super::*;
use crate::public_generic_abi::carrier::STICKY_SETTLEMENT_VIOLATION;

fn committed_machine() -> CarrierCallMachine {
    let mut machine = CarrierCallMachine::new(Handle::root(5), vec![Handle::leaf(0, 5)]);
    machine.validate().unwrap();
    machine.prepare_input().unwrap();
    machine.commit_input_transfer().unwrap();
    machine
}

// ---------------------------------------------------------------------
// Full success run
// ---------------------------------------------------------------------

#[test]
fn a_full_success_run_transitions_every_handle_and_records_the_expected_trace() {
    let mut machine = CarrierCallMachine::new(Handle::root(7), vec![Handle::leaf(0, 7)]);
    machine.validate().unwrap();
    machine.prepare_input().unwrap();
    machine.commit_input_transfer().unwrap();
    machine.begin_execution().unwrap();
    machine.finish_execution().unwrap();
    machine
        .begin_result(Handle::root(7), vec![Handle::leaf(0, 7)])
        .unwrap();
    machine.prepare_result().unwrap();
    machine.commit_result().unwrap();
    machine.settle(Settlement::Success).unwrap();

    assert_eq!(machine.phase(), Phase::Committed);
    assert_eq!(machine.settlement(), Some(Settlement::Success));
    assert_eq!(machine.input().root_state(), CarrierState::Transferred);
    assert_eq!(machine.input().leaf_state(0), CarrierState::Transferred);
    let result = machine.result().expect("result staging must have begun");
    assert_eq!(result.root_state(), CarrierState::Transferred);
    assert_eq!(result.leaf_state(0), CarrierState::Transferred);

    let labels: Vec<TraceLabel> = machine
        .trace()
        .events()
        .iter()
        .map(|event| event.label)
        .collect();
    assert_eq!(
        labels,
        vec![
            TraceLabel::FrameValidated,
            TraceLabel::LeafAllocationStarted,
            TraceLabel::LeafAllocationCommitted,
            TraceLabel::LeafPayloadCopied,
            TraceLabel::LeafAllocationStarted,
            TraceLabel::LeafAllocationCommitted,
            TraceLabel::LeafPayloadCopied,
            TraceLabel::InputValuePrepared,
            TraceLabel::InputTransferCommitted,
            TraceLabel::ExecutionStarted,
            TraceLabel::ExecutionFinished,
            TraceLabel::ResultLeafAllocationStarted,
            TraceLabel::ResultLeafAllocationCommitted,
            TraceLabel::ResultLeafAllocationStarted,
            TraceLabel::ResultLeafAllocationCommitted,
            TraceLabel::ResultValuePrepared,
            TraceLabel::ResultCommit,
            TraceLabel::TerminalStatus,
        ]
    );
    for (index, event) in machine.trace().events().iter().enumerate() {
        assert_eq!(event.ordinal, index as u32, "ordinals must be sequential");
    }
    // No trace event carries a payload byte: every field is an identity,
    // enum, or small integer, checked once at the type level in
    // `trace::tests::trace_carries_no_payload_bytes_by_construction`.
}

// ---------------------------------------------------------------------
// Input-transfer atomicity and illegal transitions
// ---------------------------------------------------------------------

#[test]
fn commit_input_transfer_requires_the_validated_phase() {
    let mut machine = CarrierCallMachine::new(Handle::root(1), vec![Handle::leaf(0, 1)]);
    let error = machine
        .commit_input_transfer()
        .expect_err("commit before validate/prepare must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
    assert_eq!(machine.phase(), Phase::Preparing);
    assert_eq!(machine.input().root_state(), CarrierState::Created);
}

#[test]
fn double_transfer_is_rejected_and_leaves_state_unchanged() {
    let mut machine = committed_machine();
    let error = machine
        .commit_input_transfer()
        .expect_err("a second input transfer commit must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
    // The already-committed handles are untouched by the rejected attempt.
    assert_eq!(machine.input().root_state(), CarrierState::Transferred);
    assert_eq!(machine.input().leaf_state(0), CarrierState::Transferred);
    assert_eq!(machine.phase(), Phase::Committed);
}

#[test]
fn use_after_transfer_is_rejected() {
    let mut machine = committed_machine();
    let error = machine
        .input_mut()
        .root
        .apply(Event::Fill)
        .expect_err("mutating an already-transferred handle must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
    assert_eq!(machine.input().root_state(), CarrierState::Invalid);
}

#[test]
fn copy_out_after_release_is_rejected() {
    let mut machine = committed_machine();
    machine.release_input_after_transfer().unwrap();
    assert_eq!(machine.input().root_state(), CarrierState::Released);
    let error = machine
        .input_mut()
        .root
        .apply(Event::Consume)
        .expect_err("a copy-out (Consume) after release must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn release_happens_exactly_once() {
    let mut machine = committed_machine();
    machine.release_input_after_transfer().unwrap();
    let error = machine
        .release_input_after_transfer()
        .expect_err("releasing an already-released handle set must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn failure_arriving_mid_transfer_leaves_no_handle_transferred() {
    let mut machine = CarrierCallMachine::new(
        Handle::root(1),
        vec![Handle::leaf(0, 1), Handle::leaf(1, 1)],
    );
    machine.validate().unwrap();
    // Simulate an injected failure between the second leaf's allocation and
    // its payload copy: the root and first leaf reach `Initialized`, the
    // second leaf never does.
    {
        let input = machine.input_mut();
        input.root.apply(Event::Fill).unwrap();
        input.leaves[0].apply(Event::Fill).unwrap();
    }
    let error = machine
        .commit_input_transfer()
        .expect_err("a not-fully-initialized input must never commit");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
    // Nothing transferred: the atomic precondition check ran before any
    // handle was mutated, so there is no partial-transfer state to observe.
    assert_eq!(machine.input().root_state(), CarrierState::Initialized);
    assert_eq!(machine.input().leaf_state(0), CarrierState::Initialized);
    assert_eq!(machine.input().leaf_state(1), CarrierState::Created);
    assert_eq!(
        machine.phase(),
        Phase::Validated,
        "phase must not advance on a failed commit"
    );

    // Settle the failure and release exactly the leaves that exist, in the
    // exact reverse of the canonical obligation order — including the leaf
    // that never left `Created`.
    machine.settle(Settlement::AllocationFailure).unwrap();
    machine.release_input_before_transfer().unwrap();
    assert_eq!(machine.input().root_state(), CarrierState::Released);
    assert_eq!(machine.input().leaf_state(0), CarrierState::Released);
    assert_eq!(machine.input().leaf_state(1), CarrierState::Released);
}

// ---------------------------------------------------------------------
// Sticky failure selection
// ---------------------------------------------------------------------

#[test]
fn cleanup_cannot_overwrite_a_sticky_failure_status() {
    let mut machine = CarrierCallMachine::new(Handle::root(2), vec![Handle::leaf(0, 2)]);
    machine.settle(Settlement::ProviderFailure).unwrap();
    let error = machine
        .settle(Settlement::CleanupFailure)
        .expect_err("a later cleanup failure must not replace the sticky primary failure");
    assert_eq!(error.code, STICKY_SETTLEMENT_VIOLATION);
    assert_eq!(machine.settlement(), Some(Settlement::ProviderFailure));

    // Reasserting the identical outcome is idempotent, since it is not a
    // replacement.
    machine.settle(Settlement::ProviderFailure).unwrap();
    assert_eq!(machine.settlement(), Some(Settlement::ProviderFailure));
}

#[test]
fn a_cleanup_failure_may_become_terminal_when_no_earlier_failure_exists() {
    let mut machine = CarrierCallMachine::new(Handle::root(3), vec![Handle::leaf(0, 3)]);
    machine.settle(Settlement::CleanupFailure).unwrap();
    assert_eq!(machine.settlement(), Some(Settlement::CleanupFailure));
    let error = machine
        .settle(Settlement::Success)
        .expect_err("a sticky cleanup failure cannot later become success");
    assert_eq!(error.code, STICKY_SETTLEMENT_VIOLATION);
}

// ---------------------------------------------------------------------
// Result staging ordering
// ---------------------------------------------------------------------

#[test]
fn begin_result_before_execution_finishes_is_rejected() {
    let mut machine = committed_machine();
    let error = machine
        .begin_result(Handle::root(5), vec![Handle::leaf(0, 5)])
        .expect_err("result staging before execution finishes must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
    assert!(machine.result().is_none());
}

#[test]
fn execution_cannot_finish_before_it_begins() {
    let mut machine = committed_machine();
    let error = machine
        .finish_execution()
        .expect_err("execution cannot finish before it begins");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn execution_cannot_begin_twice() {
    let mut machine = committed_machine();
    machine.begin_execution().unwrap();
    let error = machine
        .begin_execution()
        .expect_err("execution cannot begin twice");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn begin_result_twice_is_rejected() {
    let mut machine = committed_machine();
    machine.begin_execution().unwrap();
    machine.finish_execution().unwrap();
    machine
        .begin_result(Handle::root(5), vec![Handle::leaf(0, 5)])
        .unwrap();
    let error = machine
        .begin_result(Handle::root(5), vec![Handle::leaf(0, 5)])
        .expect_err("result staging can only begin once");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn commit_result_before_prepare_is_rejected() {
    let mut machine = committed_machine();
    machine.begin_execution().unwrap();
    machine.finish_execution().unwrap();
    machine
        .begin_result(Handle::root(5), vec![Handle::leaf(0, 5)])
        .unwrap();
    let error = machine
        .commit_result()
        .expect_err("the result cannot commit before its handles are filled");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn double_result_commit_is_rejected() {
    let mut machine = committed_machine();
    machine.begin_execution().unwrap();
    machine.finish_execution().unwrap();
    machine
        .begin_result(Handle::root(5), vec![Handle::leaf(0, 5)])
        .unwrap();
    machine.prepare_result().unwrap();
    machine.commit_result().unwrap();
    let error = machine
        .commit_result()
        .expect_err("a second result commit must be illegal");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

#[test]
fn result_leaves_are_invisible_before_whole_result_commit() {
    let mut machine = committed_machine();
    machine.begin_execution().unwrap();
    machine.finish_execution().unwrap();
    machine
        .begin_result(Handle::root(5), vec![Handle::leaf(0, 5)])
        .unwrap();
    machine.prepare_result().unwrap();
    // Staged but not yet committed: every result handle is `Initialized`,
    // never `Transferred`, so a consumer reading through the public
    // `result()` accessor observes no committed leaf yet.
    let staged = machine.result().unwrap();
    assert_eq!(staged.root_state(), CarrierState::Initialized);
    assert_eq!(staged.leaf_state(0), CarrierState::Initialized);

    machine.commit_result().unwrap();
    let committed = machine.result().unwrap();
    assert_eq!(committed.root_state(), CarrierState::Transferred);
    assert_eq!(committed.leaf_state(0), CarrierState::Transferred);
}

#[test]
fn a_result_staging_failure_releases_only_the_leaves_actually_completed() {
    let mut machine = committed_machine();
    machine.begin_execution().unwrap();
    machine.finish_execution().unwrap();
    machine
        .begin_result(
            Handle::root(5),
            vec![Handle::leaf(0, 5), Handle::leaf(1, 5)],
        )
        .unwrap();
    // Fill only the root and the first result leaf; the second never
    // reaches `Initialized`, simulating a mid-staging failure.
    {
        let result = machine.result_mut().unwrap();
        result.root.apply(Event::Fill).unwrap();
        result.leaves[0].apply(Event::Fill).unwrap();
    }
    let error = machine
        .commit_result()
        .expect_err("a not-fully-initialized result must never commit");
    assert_eq!(error.code, ILLEGAL_TRANSITION);

    machine.settle(Settlement::MalformedResult).unwrap();
    machine.release_result_before_commit().unwrap();
    let result = machine.result().unwrap();
    assert_eq!(result.root_state(), CarrierState::Released);
    assert_eq!(result.leaf_state(0), CarrierState::Released);
    assert_eq!(result.leaf_state(1), CarrierState::Released);
}
