//! Actual Continue State moves into one checked Observe under the same held
//! store. ACK constructors stay test-only until the §23 adapter is joined.
use super::super::super::observe::{
    observe_owned_agent_state_v2, FailedOwnedObserveV2, ObservedOwnedAgentStateV2,
    OwnedObserveStepV2,
};
use super::*;

/// Contains the exact moved State and its Transition/next-Observe funding ACKs.
/// No production constructor accepts raw sequences or a caller-held owner.
pub(crate) struct CommittedContinueObserveV2<'a> {
    held: HeldExecutedOwnedStepV2<'a>,
    transition: u32,
    reservation: u32,
    turn: u32,
    fuel: usize,
}
pub(crate) struct ContinuedObserveRejectionV2<'a> {
    pub(crate) committed: CommittedContinueObserveV2<'a>,
    pub(crate) diagnostic: Diagnostic,
}
/// Current context retains the original store/runtime/E, but no earlier K.
struct HeldOwnedTurnContextV2<'a> {
    runtime: &'a crate::execution_revision::typed::AgentRuntimeV2,
    execution: &'a crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8,
    store: crate::live_invocation::source_journal::HeldOwnedWaitStoreV8<'a>,
    policy: &'a crate::resumable_effects::capability::CapabilityPolicy,
    cancellation: &'a crate::agent_runtime::AgentCancellation,
    effect_id: String,
    creator: u32,
}
impl HeldOwnedTurnContextV2<'_> {
    // Incurred settlement retains physical authority after cancellation.
    fn validate_incurred_guard(&self) -> bool {
        self.creator == std::process::id()
            && self.store.validate_guard().is_ok()
            && self.runtime.owned_wait_effects_v8(self.execution).is_ok()
            && self.policy.allows(&self.effect_id)
    }
    fn validate_guard(&self) -> bool {
        self.validate_incurred_guard() && !self.cancellation.is_cancelled()
    }
}
pub(crate) struct ObservedHeldOwnedStateV2<'a> {
    observed: ObservedOwnedAgentStateV2,
    // Current context contains no previous Proposal or grant.
    context: HeldOwnedTurnContextV2<'a>,
    turn: u32,
    transition: u32,
    reservation: u32,
    consumed: usize,
}
pub(crate) struct FailedHeldOwnedObserveV2<'a> {
    failed: FailedOwnedObserveV2,
    context: HeldOwnedTurnContextV2<'a>,
    turn: u32,
    reservation: u32,
    consumed: usize,
}
pub(crate) struct QuarantinedHeldOwnedObserveV2<'a> {
    step: OwnedObserveStepV2,
    context: HeldOwnedTurnContextV2<'a>,
}
pub(crate) enum ContinuedOwnedObserveV2<'a> {
    Observed(ObservedHeldOwnedStateV2<'a>),
    Failed(FailedHeldOwnedObserveV2<'a>),
    Quarantined(QuarantinedHeldOwnedObserveV2<'a>),
}
impl ObservedHeldOwnedStateV2<'_> {
    /// Copy data only; there is no owner/helper/model/dispatch extraction API.
    pub(crate) fn observation(&self) -> &ResumableChannelValue {
        self.observed.observation()
    }
    pub(crate) fn turn(&self) -> u32 {
        self.turn
    }
    pub(crate) fn consumed(&self) -> usize {
        self.consumed
    }
    pub(crate) fn causal_refs(&self) -> (u32, u32) {
        (self.transition, self.reservation)
    }
    pub(crate) fn validate_store(&self) -> bool {
        self.context.validate_guard()
    }
}
impl FailedHeldOwnedObserveV2<'_> {
    pub(crate) fn consumed(&self) -> usize {
        self.consumed
    }
    pub(crate) fn turn(&self) -> u32 {
        self.turn
    }
    pub(crate) fn validate_store(&self) -> bool {
        self.context.validate_guard()
    }
}

pub(crate) fn observe_continued_owned_state_v2<'a>(
    mut committed: CommittedContinueObserveV2<'a>,
    budget: &mut OwnedFrameBudget,
    mut current: impl FnMut() -> bool,
) -> Result<ContinuedOwnedObserveV2<'a>, ContinuedObserveRejectionV2<'a>> {
    let inputs = committed.held.inputs.as_ref().expect("held context");
    let effect = plan_owned_effect_v8(
        inputs.runtime,
        inputs.execution,
        &inputs.store.registration().expected_facts().scope,
        &inputs.proposal,
    )
    .map(|p| p.operation().effect_id().to_owned());
    let expected_turn = inputs.turn.checked_add(1);
    let expected_fuel = inputs.execution.evaluation_fuel();
    let valid = effect.is_ok()
        && committed.held.kind() == "continue"
        && expected_turn == Some(committed.turn)
        && committed.turn < inputs.execution.ordinary().max_iterations()
        && committed.transition > committed.held.transfer_reserved
        && committed.reservation > committed.transition
        && committed.fuel == expected_fuel
        && budget.remaining == expected_fuel
        && budget.consumed == 0
        && !budget.cancelled
        && !inputs.cancellation.is_cancelled()
        && physical_guard(inputs, committed.held.creator, &mut current)
        && !inputs.cancellation.is_cancelled();
    if !valid {
        return Err(ContinuedObserveRejectionV2 {
            committed,
            diagnostic: rejected("Continue Observe reservation/current budget differs"),
        });
    }
    let Some(OwnedStepTransferV2::Continue(state)) = committed.held.owner.take() else {
        unreachable!()
    };
    let proof = inputs.execution.wait().observe().clone();
    let step = match observe_owned_agent_state_v2(state, &proof, budget) {
        Ok(step) => step,
        Err(error) => {
            committed.held.owner = Some(OwnedStepTransferV2::Continue(error.argument));
            return Err(ContinuedObserveRejectionV2 {
                committed,
                diagnostic: error.diagnostic,
            });
        }
    };
    let live = !inputs.cancellation.is_cancelled()
        && physical_guard(inputs, committed.held.creator, &mut current)
        && !inputs.cancellation.is_cancelled();
    // Keep the historical borrower while moving the actual Observe holder.
    // Its old Proposal is never passed to Observe or installed as next-turn K.
    let inputs = committed.held.inputs.take().expect("held context");
    let context = HeldOwnedTurnContextV2 {
        runtime: inputs.runtime,
        execution: inputs.execution,
        store: inputs.store,
        policy: inputs.policy,
        cancellation: inputs.cancellation,
        effect_id: effect.expect("checked effect"),
        creator: committed.held.creator,
    };
    // The previous Proposal borrow is retired here. It is not carried into
    // this turn's Observe result and cannot fund/admit a new model response.
    if !live {
        return Ok(ContinuedOwnedObserveV2::Quarantined(
            QuarantinedHeldOwnedObserveV2 { step, context },
        ));
    }
    Ok(match step {
        OwnedObserveStepV2::Observed(observed) => {
            ContinuedOwnedObserveV2::Observed(ObservedHeldOwnedStateV2 {
                observed,
                context,
                turn: committed.turn,
                transition: committed.transition,
                reservation: committed.reservation,
                consumed: budget.consumed,
            })
        }
        OwnedObserveStepV2::Failed(failed) => {
            ContinuedOwnedObserveV2::Failed(FailedHeldOwnedObserveV2 {
                failed,
                context,
                turn: committed.turn,
                reservation: committed.reservation,
                consumed: budget.consumed,
            })
        }
    })
}

#[cfg(test)]
mod tests;

mod live_append;
pub(crate) use live_append::{observe_live_continued_state_v8, LiveContinuedObserveFailureV8};

#[cfg(test)]
pub(crate) use live_append::{test_continue_observe_entries_v8, test_continue_observe_oracle_v8};

pub(crate) use live_append::checked_continued_observe_facts_v8;

pub(crate) use live_append::{
    prepare_continued_copy_wait_v8, ContinuedWaitPreparationFailureV8, PreparedHeldContinuedWaitV2,
};

mod failed_cleanup;
pub(crate) use failed_cleanup::{
    ContinuedObserveStateCleanupFailureV8, ReleasedContinuedObserveStateV8,
};

pub(in crate::interpreter::resumable::owned_frame::registered_stage) use live_append::{
    enter_continued_wait_v8, EvaluatedContinuedWaitV2,
};
