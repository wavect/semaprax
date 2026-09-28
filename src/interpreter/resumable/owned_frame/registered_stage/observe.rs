//! Consuming read-only Observe. State borrows are drained before the next stage.
use super::*;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedObserveV2;

pub(crate) struct ObservedOwnedAgentStateV2 {
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    observation: ResumableChannelValue,
    creator: u32,
    allocations: OwnedAllocationProvenanceV2,
}
impl ObservedOwnedAgentStateV2 {
    pub(crate) fn observation(&self) -> &ResumableChannelValue {
        &self.observation
    }
}
pub(crate) struct FailedOwnedObserveV2 {
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    failure: OwnedFrameFailure,
    creator: u32,
    settlement_started: bool,
    allocations: OwnedAllocationProvenanceV2,
}
pub(crate) enum OwnedObserveStepV2 {
    Observed(ObservedOwnedAgentStateV2),
    Failed(FailedOwnedObserveV2),
}
pub(crate) struct OwnedObserveRejectionV2 {
    pub(crate) argument: OwnedAgentStateArgument,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) fn observe_owned_agent_state_v2(
    mut argument: OwnedAgentStateArgument,
    plan: &CheckedOwnedObserveV2,
    budget: &mut OwnedFrameBudget,
) -> Result<OwnedObserveStepV2, OwnedObserveRejectionV2> {
    if argument.creator != std::process::id()
        || !argument.plan.same_helper(plan.helper())
        || argument
            .root
            .as_ref()
            .and_then(|v| {
                argument
                    .allocations
                    .as_ref()
                    .and_then(|p| p.seed(&[v]).ok())
            })
            .is_none()
    {
        return Err(OwnedObserveRejectionV2 {
            argument,
            diagnostic: rejected("Observe helper/root/process mismatch"),
        });
    }
    let allocation_ceiling = argument
        .allocations
        .as_ref()
        .expect("checked provenance")
        .seed(&[argument.root.as_ref().expect("checked root")])
        .expect("checked allocations");
    let allocations = argument.allocations.take().expect("consumed provenance");
    let root = argument.root.take();
    let helper = argument.plan.clone();
    let creator = argument.creator;
    macro_rules! failed {
        ($failure:expr) => {
            OwnedObserveStepV2::Failed(FailedOwnedObserveV2 {
                plan: helper.clone(),
                root,
                failure: $failure,
                creator,
                settlement_started: false,
                allocations,
            })
        };
    }
    if budget.cancelled {
        return Ok(failed!(OwnedFrameFailure::HostAbandoned));
    }
    let Value::Record(state) = root.as_ref().expect("consumed state") else {
        unreachable!()
    };
    let borrowed = Value::Record(Arc::clone(state));
    let admitted = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&admitted),
        BTreeMap::new(),
        &helper.program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    evaluator.next_byte_allocation = allocation_ceiling;
    // Enter the ordinary checked call frame with a scoped borrow of the real
    // State, not a RetainedValue carrier or freshly staged Bytes allocation.
    let evaluation = evaluator.call_frame(
        plan.function(),
        vec![(plan.function().params[0].id.clone(), borrowed)],
        0,
    );
    let steps = evaluator.steps;
    drop(evaluator);
    budget.remaining -= steps;
    budget.consumed += steps;
    match evaluation {
        Err(flow) => Ok(failed!(failure(flow))),
        Ok(value) => {
            let observation =
                super::super::super::channel_of(&helper.program().declarations, &value);
            drop(value); // Copy record and postcondition aliases only
            let Some(observation) = observation.filter(|v| {
                channel_v2::valid_copy_carrier(
                    &helper.program().declarations,
                    &helper.function().params[1].ty,
                    v,
                )
            }) else {
                return Ok(failed!(OwnedFrameFailure::EvaluationRejected));
            };
            if !root.as_ref().is_some_and(|r| root_valid(&helper, r)) {
                return Ok(failed!(OwnedFrameFailure::EvaluationRejected));
            }
            Ok(OwnedObserveStepV2::Observed(ObservedOwnedAgentStateV2 {
                plan: helper.clone(),
                root,
                observation,
                creator,
                allocations,
            }))
        }
    }
}
pub(crate) fn prepare_observed_owned_copy_wait_v2(
    observed: ObservedOwnedAgentStateV2,
) -> Result<PreparedOwnedCopyWaitV2, ObservedOwnedAgentStateV2> {
    if observed.creator != std::process::id()
        || !observed
            .root
            .as_ref()
            .is_some_and(|r| observed.allocations.validate(&[r]))
        || !channel_v2::valid_copy_carrier(
            &observed.plan.program().declarations,
            &observed.plan.function().params[1].ty,
            &observed.observation,
        )
        || !observed
            .root
            .as_ref()
            .is_some_and(|r| root_valid(&observed.plan, r))
    {
        return Err(observed);
    }
    Ok(PreparedOwnedCopyWaitV2 {
        argument: OwnedAgentStateArgument {
            plan: observed.plan,
            root: observed.root,
            creator: observed.creator,
            allocations: Some(observed.allocations),
        },
        observation: observed.observation,
    })
}
pub(crate) struct OwnedObserveSettlementRejectionV2 {
    pub(crate) failed: FailedOwnedObserveV2,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct OwnedObserveSettledV2 {
    pub(crate) failure: OwnedFrameFailure,
    pub(crate) receipt: OwnedFrameReleaseReceipt,
    pub(crate) observations_succeeded: bool,
}
pub(crate) fn settle_failed_owned_observe_v2(
    mut failed: FailedOwnedObserveV2,
    mut current: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<OwnedObserveSettledV2, OwnedObserveSettlementRejectionV2> {
    if failed.settlement_started
        || !failed
            .root
            .as_ref()
            .is_some_and(|r| failed.allocations.validate(&[r]))
        || !current_in_creator(failed.creator, &mut current)
        || !failed
            .root
            .as_ref()
            .is_some_and(|r| root_valid(&failed.plan, r))
    {
        return Err(OwnedObserveSettlementRejectionV2 {
            failed,
            diagnostic: rejected("Observe cleanup owner/authority mismatch"),
        });
    }
    failed.settlement_started = true;
    let creator = failed.creator;
    let mut observations_succeeded = true;
    match release_guarded(
        &mut failed.root,
        &failed.plan.liveness().failure_cleanup,
        || current_in_creator(creator, &mut current),
        |a| {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(a))).is_err() {
                observations_succeeded = false;
            }
        },
    ) {
        Ok(receipt) => Ok(OwnedObserveSettledV2 {
            failure: failed.failure,
            receipt,
            observations_succeeded,
        }),
        Err(diagnostic) => Err(OwnedObserveSettlementRejectionV2 { failed, diagnostic }),
    }
}

#[cfg(test)]
mod tests;
