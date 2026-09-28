//! Same-owner physical Reduce/Step foundation. Committed envelopes have no
//! production constructor until the owner-bound §23 append adapter is joined.
//! Test constructors bypass that pending grammar, never establishing live ACKs.
use super::super::effect::OwnedEffectInputsV8;
use super::*;
use crate::agent_lifecycle::iterative::effects::plan_owned_effect_v8;

/// Closed true lineage. Compiler-empty cleanup has no Started/Settled row;
/// its actual Staged ACK remains the origin, never a synthetic sequence zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedReduceCleanupOriginV8 {
    Observed { started: u32 },
    CompilerEmpty { staged: u32 },
}
/// The original staged owner travels inside the cleanup-start ACK envelope.
/// Matching facts or a boolean cannot create this envelope in production.
pub(crate) struct CommittedExecutedOwnedReduceCleanupV2<'a> {
    staged: StagedExecutedOwnedReduceV2<'a>,
    started: OwnedReduceCleanupOriginV8,
    observations: Vec<OwnedReduceObservationV2>,
}
pub(crate) struct ExecutedOwnedReduceCleanupRejectionV2<'a> {
    pub(crate) committed: CommittedExecutedOwnedReduceCleanupV2<'a>,
    pub(crate) diagnostic: Diagnostic,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedReduceObservationV2 {
    pub(crate) operation: FinalizeAction,
    pub(crate) succeeded: bool,
}
pub(crate) struct ReadyExecutedOwnedStepV2<'a> {
    ready: Option<ReadyOwnedStepV2>,
    inputs: Option<OwnedEffectInputsV8<'a>>,
    creator: u32,
    effect_settled: u32,
    cleanup_started: OwnedReduceCleanupOriginV8,
    receipt_valid: bool,
    quarantined: bool,
    observations: Vec<OwnedReduceObservationV2>,
}
pub(crate) struct FailedExecutedOwnedReduceV2<'a> {
    failure: OwnedFrameFailure,
    operations: Vec<FinalizeAction>,
    observations: Vec<OwnedReduceObservationV2>,
    observations_succeeded: bool,
    inputs: OwnedEffectInputsV8<'a>,
}
pub(crate) enum ExecutedOwnedReduceSettledV2<'a> {
    Ready(ReadyExecutedOwnedStepV2<'a>),
    Failed(FailedExecutedOwnedReduceV2<'a>),
}
/// A transfer ACK retains the exact ReadyStep whose fields it will move.
pub(crate) struct CommittedExecutedOwnedStepTransferV2<'a> {
    ready: ReadyExecutedOwnedStepV2<'a>,
    reserved: u32,
}
pub(crate) struct ExecutedOwnedStepTransferRejectionV2<'a> {
    pub(crate) committed: CommittedExecutedOwnedStepTransferV2<'a>,
    pub(crate) diagnostic: Diagnostic,
}
/// The mapped owner and store stay inseparable; no State/Report getter or
/// public delivery route is provided by this foundation.
pub(crate) struct HeldExecutedOwnedStepV2<'a> {
    owner: Option<OwnedStepTransferV2>,
    inputs: Option<OwnedEffectInputsV8<'a>>,
    creator: u32,
    effect_settled: u32,
    transfer_reserved: u32,
}

fn physical_guard(
    inputs: &OwnedEffectInputsV8<'_>,
    creator: u32,
    check: &mut impl FnMut() -> bool,
) -> bool {
    let valid = || {
        creator == std::process::id()
            && inputs.store.validate_guard().is_ok()
            && plan_owned_effect_v8(
                inputs.runtime,
                inputs.execution,
                &inputs.store.registration().expected_facts().scope,
                &inputs.proposal,
            )
            .is_ok_and(|p| inputs.policy.allows(p.operation().effect_id()))
    };
    // Cancellation does not suppress physical cleanup after its committed
    // start. It separately blocks ReadyStep's consuming result move below.
    valid() && check() && valid()
}
impl ReadyExecutedOwnedStepV2<'_> {
    pub(crate) fn observations(&self) -> &[OwnedReduceObservationV2] {
        &self.observations
    }
    pub(crate) fn validate_store(&self) -> bool {
        self.creator == std::process::id()
            && self
                .inputs
                .as_ref()
                .is_some_and(|i| i.store.validate_guard().is_ok())
    }
}
impl FailedExecutedOwnedReduceV2<'_> {
    pub(crate) fn failure(&self) -> &OwnedFrameFailure {
        &self.failure
    }
    pub(crate) fn operations(&self) -> &[FinalizeAction] {
        &self.operations
    }
    pub(crate) fn observations(&self) -> &[OwnedReduceObservationV2] {
        &self.observations
    }
    pub(crate) fn observations_succeeded(&self) -> bool {
        self.observations_succeeded
    }
    pub(crate) fn validate_store(&self) -> bool {
        self.inputs.store.validate_guard().is_ok()
    }
}
impl HeldExecutedOwnedStepV2<'_> {
    pub(crate) fn kind(&self) -> &'static str {
        match self.owner.as_ref().expect("held Step") {
            OwnedStepTransferV2::Continue(_) => "continue",
            OwnedStepTransferV2::Suspend(_) => "suspend",
            OwnedStepTransferV2::Complete(_) => "complete",
            OwnedStepTransferV2::Fail(_) => "fail",
        }
    }
    pub(crate) fn validate_store(&self) -> bool {
        self.creator == std::process::id()
            && self
                .inputs
                .as_ref()
                .is_some_and(|i| i.store.validate_guard().is_ok())
    }
    pub(crate) fn causal_refs(&self) -> (u32, u32) {
        (self.effect_settled, self.transfer_reserved)
    }
}
impl ReadyExecutedOwnedStepV2<'_> {
    fn discard_unpublished_backing(&mut self) {
        if let Some(ready) = &mut self.ready {
            // No result-disposal ACK/claim is supplied by this foundation.
            // Drain backing under the retained borrower even with valid pins,
            // so the inner Drop cannot perform semantic result settlement.
            drop(ready.root.take());
        }
    }
}
impl HeldExecutedOwnedStepV2<'_> {
    fn discard_unpublished_backing(&mut self) {
        match self.owner.as_mut() {
            Some(OwnedStepTransferV2::Continue(state) | OwnedStepTransferV2::Suspend(state)) => {
                drop(state.root.take());
            }
            Some(OwnedStepTransferV2::Complete(report)) => drop(report.root.take()),
            _ => {}
        }
    }
}
impl Drop for ReadyExecutedOwnedStepV2<'_> {
    fn drop(&mut self) {
        self.discard_unpublished_backing();
    }
}
impl Drop for HeldExecutedOwnedStepV2<'_> {
    fn drop(&mut self) {
        self.discard_unpublished_backing();
    }
}

pub(crate) fn settle_executed_owned_reduce_v2<'a>(
    committed: CommittedExecutedOwnedReduceCleanupV2<'a>,
    mut current: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<ExecutedOwnedReduceSettledV2<'a>, ExecutedOwnedReduceCleanupRejectionV2<'a>> {
    let creator = committed.staged.staged.creator;
    if !physical_guard(&committed.staged.inputs, creator, &mut current) {
        return Err(ExecutedOwnedReduceCleanupRejectionV2 {
            committed,
            diagnostic: rejected("owned reducer cleanup current authority differs"),
        });
    }
    let CommittedExecutedOwnedReduceCleanupV2 {
        staged,
        started,
        mut observations,
    } = committed;
    let StagedExecutedOwnedReduceV2 {
        staged,
        inputs,
        effect_settled,
        allowance,
        consumed,
    } = staged;
    // Clone the exact compiler-selected active actions and allocate all receipt
    // slots before any actual finalizer. Neither order nor guards are repaired.
    let actions = step::actions(&staged);
    let flags = step::active_flags(&staged);
    let selected: Vec<_> = actions
        .into_iter()
        .filter(|a| flags.contains(&a.guard_flag))
        .collect();
    observations.reserve_exact(selected.len());
    let mut expected = selected.into_iter();
    let mut receipt_valid = observations.is_empty();
    let result = settle_owned_reduce_v2(
        staged,
        || physical_guard(&inputs, creator, &mut current),
        |operation| {
            // The underlying loop has already dropped the actual leaf. Record
            // this callback's outcome individually, then preserve its original
            // caught-panic behavior and continuation to subsequent operations.
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(operation)));
            if let Some(expected) = expected.next() {
                receipt_valid &= expected == *operation;
                observations.push(OwnedReduceObservationV2 {
                    operation: expected,
                    succeeded: result.is_ok(),
                });
            } else {
                receipt_valid = false;
            }
            if let Err(payload) = result {
                std::panic::resume_unwind(payload);
            }
        },
    );
    match result {
        Ok(OwnedReduceSettledV2::Ready(ready)) => {
            receipt_valid &= expected.next().is_none()
                && ready
                    .nonresult_operations()
                    .iter()
                    .eq(observations.iter().map(|o| &o.operation));
            Ok(ExecutedOwnedReduceSettledV2::Ready(
                ReadyExecutedOwnedStepV2 {
                    ready: Some(ready),
                    inputs: Some(inputs),
                    creator,
                    effect_settled,
                    cleanup_started: started,
                    receipt_valid,
                    quarantined: false,
                    observations,
                },
            ))
        }
        Ok(OwnedReduceSettledV2::Failed {
            failure,
            operations,
            observations_succeeded,
        }) => {
            receipt_valid &= expected.next().is_none()
                && operations
                    .iter()
                    .eq(observations.iter().map(|o| &o.operation));
            Ok(ExecutedOwnedReduceSettledV2::Failed(
                FailedExecutedOwnedReduceV2 {
                    failure,
                    operations,
                    observations,
                    observations_succeeded: observations_succeeded && receipt_valid,
                    inputs,
                },
            ))
        }
        Err(error) => Err(ExecutedOwnedReduceCleanupRejectionV2 {
            committed: CommittedExecutedOwnedReduceCleanupV2 {
                staged: StagedExecutedOwnedReduceV2 {
                    staged: error.staged,
                    inputs,
                    effect_settled,
                    allowance,
                    consumed,
                },
                started,
                observations,
            },
            diagnostic: error.diagnostic,
        }),
    }
}

pub(crate) fn consume_executed_owned_step_v2<'a>(
    mut committed: CommittedExecutedOwnedStepTransferV2<'a>,
    mut current: impl FnMut() -> bool,
) -> Result<HeldExecutedOwnedStepV2<'a>, ExecutedOwnedStepTransferRejectionV2<'a>> {
    let valid = committed.ready.receipt_valid
        && !committed.ready.quarantined
        && committed
            .ready
            .ready
            .as_ref()
            .is_some_and(|r| r.observations_succeeded())
        && !committed
            .ready
            .inputs
            .as_ref()
            .expect("held inputs")
            .cancellation
            .is_cancelled()
        && physical_guard(
            committed.ready.inputs.as_ref().expect("held inputs"),
            committed.ready.creator,
            &mut current,
        )
        && !committed
            .ready
            .inputs
            .as_ref()
            .expect("held inputs")
            .cancellation
            .is_cancelled();
    if !valid {
        committed.ready.quarantined = true;
        return Err(ExecutedOwnedStepTransferRejectionV2 {
            committed,
            diagnostic: rejected("owned Step transfer current authority differs"),
        });
    }
    let ready = committed.ready.ready.take().expect("checked ReadyStep");
    let owner = match consume_owned_step_v2(ready) {
        Ok(owner) => owner,
        Err(ready) => {
            committed.ready.ready = Some(ready);
            return Err(ExecutedOwnedStepTransferRejectionV2 {
                committed,
                diagnostic: rejected("owned Step actual field move differs"),
            });
        }
    };
    let inputs = committed.ready.inputs.take().expect("held inputs");
    Ok(HeldExecutedOwnedStepV2 {
        owner: Some(owner),
        inputs: Some(inputs),
        creator: committed.ready.creator,
        effect_settled: committed.ready.effect_settled,
        transfer_reserved: committed.reserved,
    })
}

#[cfg(test)]
mod tests;

mod continue_observe;
pub(crate) use continue_observe::{
    observe_continued_owned_state_v2, CommittedContinueObserveV2, ContinuedOwnedObserveV2,
    FailedHeldOwnedObserveV2, ObservedHeldOwnedStateV2,
};

mod live_append;
pub(crate) use live_append::{
    consume_live_owned_step_v8, settle_live_owned_reduce_v8, LiveOwnedReduceCleanupFailureV8,
    LiveOwnedStepTransferFailureV8,
};

pub(crate) use continue_observe::{observe_live_continued_state_v8, LiveContinuedObserveFailureV8};

#[cfg(test)]
pub(crate) use continue_observe::{
    test_continue_observe_entries_v8, test_continue_observe_oracle_v8,
};

pub(crate) use continue_observe::checked_continued_observe_facts_v8;

pub(crate) use continue_observe::{
    prepare_continued_copy_wait_v8, ContinuedWaitPreparationFailureV8, PreparedHeldContinuedWaitV2,
};
