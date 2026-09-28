//! Actual consuming Observe behind the actor's one-use reservation permit.
use super::super::observe::{
    observe_owned_agent_state_v2, prepare_observed_owned_copy_wait_v2, FailedOwnedObserveV2,
    OwnedObserveStepV2,
};
use super::*;
use crate::live_invocation::source_journal::LiveObservePermitV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedObserveV2;
pub(crate) struct LiveObservedStateV8 {
    prepared: Option<PreparedOwnedCopyWaitV2>,
    facts: serde_json::Value,
    consumed: u64,
}
impl Drop for LiveObservedStateV8 {
    fn drop(&mut self) {
        if let Some(prepared) = self.prepared.as_mut() {
            drop(prepared.argument.root.take());
        }
    }
}
impl LiveObservedStateV8 {
    pub(crate) fn facts(&self) -> &serde_json::Value {
        &self.facts
    }
    pub(crate) fn observation(&self) -> &ResumableChannelValue {
        &self
            .prepared
            .as_ref()
            .expect("live prepared State")
            .observation
    }
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        super::super::super::snapshot::weak_leaves(
            self.prepared
                .as_ref()
                .unwrap()
                .argument
                .root
                .as_ref()
                .unwrap(),
        )
    }
}
pub(crate) enum LiveObserveOutcomeV8 {
    Observed(LiveObservedStateV8),
    Refused(LiveInitializedStateV8),
    Failed(FailedOwnedObserveV2),
    GuardLost(LiveObservedStateV8),
}
pub(crate) fn observe_live_owned_run_v8(
    permit: LiveObservePermitV8<'_>,
    mut initialized: LiveInitializedStateV8,
    plan: &CheckedOwnedObserveV2,
) -> LiveObserveOutcomeV8 {
    if permit.validate_guard().is_err() {
        return LiveObserveOutcomeV8::Refused(initialized);
    }
    let argument = initialized
        .state
        .take()
        .expect("consuming live initialized State");
    let mut budget =
        OwnedFrameBudget::new(permit.fuel()).expect("checked positive stage allowance");
    if permit.validate_guard().is_err() {
        budget.cancel();
    }
    let step = match observe_owned_agent_state_v2(argument, plan, &mut budget) {
        Ok(step) => step,
        Err(rejection) => {
            initialized.state = Some(rejection.argument);
            return LiveObserveOutcomeV8::Refused(initialized);
        }
    };
    let OwnedObserveStepV2::Observed(observed) = step else {
        let OwnedObserveStepV2::Failed(failed) = step else {
            unreachable!()
        };
        return LiveObserveOutcomeV8::Failed(failed);
    };
    let prepared = prepare_observed_owned_copy_wait_v2(observed)
        .unwrap_or_else(|_| panic!("fresh checked Observe result and same root proof"));
    let live = LiveObservedStateV8 {
        prepared: Some(prepared),
        facts: initialized.facts.clone(),
        consumed: budget.consumed() as u64,
    };
    if permit.validate_guard().is_err() {
        LiveObserveOutcomeV8::GuardLost(live)
    } else {
        LiveObserveOutcomeV8::Observed(live)
    }
}
