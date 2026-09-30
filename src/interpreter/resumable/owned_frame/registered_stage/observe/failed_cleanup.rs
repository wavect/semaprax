//! Exact observations of the existing failed-Observe release loop. This is an
//! internal engine helper, not an ACK, owner constructor or cleanup grant.
use super::*;
use std::cell::{Cell, RefCell};

pub(crate) struct ObservedFailedObserveCleanupV8 {
    pub(crate) settled: OwnedObserveSettledV2,
    outcomes: Vec<bool>,
}
impl ObservedFailedObserveCleanupV8 {
    pub(crate) fn outcomes(&self) -> &[bool] {
        &self.outcomes
    }
}
impl FailedOwnedObserveV2 {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn validate_cleanup_binding_v8(
        &self,
        permit: &crate::live_invocation::source_journal::LiveFailedObserveStateCleanupPermitV8<
            '_,
            '_,
        >,
        state: &serde_json::Value,
        consumed: u64,
    ) -> Result<(), crate::live_invocation::source_journal::SourceJournalError> {
        permit.validate_actual_failed(&self.plan, state, &self.failure, consumed)
    }
}
/// A rejected primitive retains its actual, possibly partly settled owner.
/// A capture error after full release retains the actual released result and
/// can never be treated as an observer failure or retried cleanup.
pub(crate) enum ActualFailedObserveCleanupRejectionV8 {
    Owner(OwnedObserveSettlementRejectionV2),
    Capture {
        _settled: OwnedObserveSettledV2,
        diagnostic: Diagnostic,
    },
}

pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn capture_failed_observe_cleanup_v8(
    failed: FailedOwnedObserveV2,
    mut current: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<ObservedFailedObserveCleanupV8, ActualFailedObserveCleanupRejectionV8> {
    // Copy canonical metadata and allocate all result slots before removal of
    // any owned leaf. Owning callers validate their sealed Started permit and
    // the actual helper/root/failure before entering this private helper.
    let actions = failed.plan.liveness().failure_cleanup.clone();
    let slots = RefCell::new(vec![None; actions.len()]);
    let next = Cell::new(0usize);
    let mismatch = Cell::new(false);
    let settled = settle_failed_owned_observe_v2(
        failed,
        || !mismatch.get() && current(),
        |action| {
            let index = next.get();
            if actions.get(index) != Some(action) {
                mismatch.set(true);
                return;
            }
            next.set(index + 1);
            // Only the external observer is inside this catch. Recording the
            // exact result and resuming its panic lets the original catch keep
            // its aggregate semantics and continue later physical releases.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action)));
            slots.borrow_mut()[index] = Some(result.is_ok());
            if let Err(payload) = result {
                std::panic::resume_unwind(payload);
            }
        },
    )
    .map_err(|mut rejection| {
        // An attempted cleanup envelope is consumed even if prevalidation
        // refuses before the first leaf; it cannot become a retry permit.
        rejection.failed.settlement_started = true;
        ActualFailedObserveCleanupRejectionV8::Owner(rejection)
    })?;
    let slots = slots.into_inner();
    if mismatch.get()
        || next.get() != actions.len()
        || settled.receipt.operations != actions
        || slots.iter().any(Option::is_none)
        || settled.observations_succeeded != slots.iter().all(|v| *v == Some(true))
    {
        return Err(ActualFailedObserveCleanupRejectionV8::Capture {
            _settled: settled,
            diagnostic: rejected("failed Observe action/outcome capture differs"),
        });
    }
    Ok(ObservedFailedObserveCleanupV8 {
        settled,
        outcomes: slots
            .into_iter()
            .map(|v| v.expect("checked canonical slot"))
            .collect(),
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
impl FailedOwnedObserveV2 {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn test_cleanup_weak_v8(
        &self,
    ) -> Vec<std::sync::Weak<[u8]>> {
        super::super::super::snapshot::weak_leaves(self.root.as_ref().expect("actual failed root"))
    }
}
