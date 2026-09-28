//! Consuming read-only Observe. State borrows are drained before the next stage.
use super::*;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedObserveV2;

pub(crate) struct ObservedOwnedAgentStateV2 {
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    observation: ResumableChannelValue,
    creator: u32,
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
}
pub(crate) enum OwnedObserveStepV2 {
    Observed(ObservedOwnedAgentStateV2),
    Failed(FailedOwnedObserveV2),
}
pub(crate) struct OwnedObserveRejectionV2 {
    pub(crate) argument: OwnedAgentStateArgument,
    pub(crate) diagnostic: Diagnostic,
}
// Admission currently mints the complete State's private logical namespace
// 1..=N. Validate it by borrow before seeding the ordinary view evaluator;
// observing a maximum alone would admit arbitrary forged allocation IDs.
fn retained_allocation_ceiling(plan: &CheckedOwnedFrameHelperV2, root: &Value) -> Option<u32> {
    if !root_valid(plan, root) {
        return None;
    }
    let Value::Record(record) = root else {
        return None;
    };
    let count = u32::try_from(plan.liveness().leaves.len()).ok()?;
    let mut seen = Vec::new();
    for leaf in &plan.liveness().leaves {
        let Value::Bytes(bytes) = record.fields.get(&leaf.field)? else {
            return None;
        };
        if bytes.allocation == 0 || bytes.allocation > count || seen.contains(&bytes.allocation) {
            return None;
        }
        seen.push(bytes.allocation);
    }
    Some(count)
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
            .and_then(|v| retained_allocation_ceiling(&argument.plan, v))
            .is_none()
    {
        return Err(OwnedObserveRejectionV2 {
            argument,
            diagnostic: rejected("Observe helper/root/process mismatch"),
        });
    }
    let allocation_ceiling = retained_allocation_ceiling(
        &argument.plan,
        argument.root.as_ref().expect("checked root"),
    )
    .expect("checked allocations");
    let root = argument.root.take();
    let helper = argument.plan.clone();
    let creator = argument.creator;
    let failed = |root, failure| {
        OwnedObserveStepV2::Failed(FailedOwnedObserveV2 {
            plan: helper.clone(),
            root,
            failure,
            creator,
            settlement_started: false,
        })
    };
    if budget.cancelled {
        return Ok(failed(root, OwnedFrameFailure::HostAbandoned));
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
        Err(flow) => Ok(failed(root, failure(flow))),
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
                return Ok(failed(root, OwnedFrameFailure::EvaluationRejected));
            };
            if !root.as_ref().is_some_and(|r| root_valid(&helper, r)) {
                return Ok(failed(root, OwnedFrameFailure::EvaluationRejected));
            }
            Ok(OwnedObserveStepV2::Observed(ObservedOwnedAgentStateV2 {
                plan: helper.clone(),
                root,
                observation,
                creator,
            }))
        }
    }
}
pub(crate) fn prepare_observed_owned_copy_wait_v2(
    observed: ObservedOwnedAgentStateV2,
) -> Result<PreparedOwnedCopyWaitV2, ObservedOwnedAgentStateV2> {
    if observed.creator != std::process::id()
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
