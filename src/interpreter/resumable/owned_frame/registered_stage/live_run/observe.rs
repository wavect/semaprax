//! Actual consuming Observe behind the actor's one-use reservation permit.
use super::super::observe::{
    observe_owned_agent_state_v2, prepare_observed_owned_copy_wait_v2, FailedOwnedObserveV2,
    OwnedObserveStepV2,
};
use super::*;
use crate::live_invocation::source_journal::LiveObservePermitV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedObserveV2;
pub(crate) struct LiveObservedStateV8 {
    pub(super) prepared: Option<PreparedOwnedCopyWaitV2>,
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
/// Actual failed Observe holder and consumption from its sole evaluator call.
pub(crate) struct LiveFailedObserveV8 {
    pub(super) owner: FailedOwnedObserveV2,
    facts: serde_json::Value,
    consumed: u64,
}
impl LiveFailedObserveV8 {
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    pub(crate) fn facts(&self) -> &serde_json::Value {
        &self.facts
    }
    pub(crate) fn failure(&self) -> &OwnedFrameFailure {
        self.owner.failure()
    }
}
pub(crate) enum LiveObserveOutcomeV8 {
    Observed(LiveObservedStateV8),
    Refused(LiveInitializedStateV8),
    Failed(LiveFailedObserveV8),
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
    #[cfg(test)]
    crate::live_invocation::source_journal::test_initial_observe_entry_v8();
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
        return LiveObserveOutcomeV8::Failed(LiveFailedObserveV8 {
            owner: failed,
            facts: initialized.facts.clone(),
            consumed: budget.consumed() as u64,
        });
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

impl LiveObservedStateV8 {
    pub(crate) fn live_state_facts_v8(&self) -> Result<serde_json::Value, Diagnostic> {
        let prepared = self
            .prepared
            .as_ref()
            .ok_or_else(|| rejected("live Observe prepared root missing"))?;
        let argument = &prepared.argument;
        let root = argument
            .root
            .as_ref()
            .ok_or_else(|| rejected("live Observe State missing"))?;
        if argument.creator != std::process::id()
            || !argument
                .allocations
                .as_ref()
                .is_some_and(|proof| proof.validate(&[root]))
        {
            return Err(rejected("live Observe creator/allocation witnesses differ"));
        }
        super::root_facts(&argument.plan, root)
            .ok_or_else(|| rejected("live Observe State schema differs"))
    }
}
impl LiveFailedObserveV8 {
    pub(crate) fn live_state_facts_v8(&self) -> Result<serde_json::Value, Diagnostic> {
        self.owner.live_state_facts_v8()
    }
}
