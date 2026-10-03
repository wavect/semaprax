//! Actual first helper entry/park; no inert checkpoint can construct this owner.
use super::observe::LiveObservedStateV8;
use super::*;
use crate::live_invocation::source_journal::LiveWaitStartPermitV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;
pub(crate) struct LiveParkedStateV8 {
    pub(super) parked: OwnedCopyWaitParkedV2,
    pub(super) consumed: u64,
}
impl LiveParkedStateV8 {
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        if self.parked.creator != std::process::id()
            || !self.parked.plan.same_helper(binding.helper())
        {
            return None;
        }
        let root = self.parked.root.as_ref()?;
        if !self.parked.allocations.validate(&[root]) {
            return None;
        }
        root_facts(&self.parked.plan, root)
    }
    pub(crate) fn request(&self) -> &ResumableChannelValue {
        self.parked.request()
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        super::super::super::snapshot::weak_leaves(self.parked.root.as_ref().unwrap())
    }
}
pub(crate) fn restore_live_parked_state_v8(
    binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    input: crate::interpreter::resumable::owned_frame::OwnedFrameInput,
    observation: crate::interpreter::resumable::ResumableChannelValue,
    consumed: u64,
) -> Result<LiveParkedStateV8, ()> {
    let parked = crate::interpreter::resumable::owned_frame::registered_stage::restore_owned_copy_wait_parked_v2(
        binding,
        input,
        observation,
    )?;
    Ok(LiveParkedStateV8 { parked, consumed })
}
pub(crate) enum LiveWaitStartOutcomeV8 {
    Parked(LiveParkedStateV8),
    Refused(LiveObservedStateV8),
    Terminal(OwnedCopyWaitTerminalV2),
    GuardLost(LiveParkedStateV8),
}
pub(crate) fn begin_live_owned_wait_v8(
    permit: LiveWaitStartPermitV8<'_>,
    mut observed: LiveObservedStateV8,
) -> LiveWaitStartOutcomeV8 {
    if permit.validate_guard().is_err() {
        return LiveWaitStartOutcomeV8::Refused(observed);
    }
    let prepared = observed
        .prepared
        .take()
        .expect("consuming live prepared owner");
    let mut budget = OwnedFrameBudget::new(permit.fuel()).expect("checked positive F");
    if permit.validate_guard().is_err() {
        budget.cancel();
    }
    let step = match begin_owned_copy_wait_v2(prepared, &mut budget) {
        Ok(step) => step,
        Err(prepared) => {
            observed.prepared = Some(prepared);
            return LiveWaitStartOutcomeV8::Refused(observed);
        }
    };
    match step {
        OwnedCopyWaitStepV2::Terminal(t) => LiveWaitStartOutcomeV8::Terminal(t),
        OwnedCopyWaitStepV2::Parked(parked) => {
            let owner = LiveParkedStateV8 {
                parked,
                consumed: budget.consumed() as u64,
            };
            if permit.validate_guard().is_err() {
                LiveWaitStartOutcomeV8::GuardLost(owner)
            } else {
                LiveWaitStartOutcomeV8::Parked(owner)
            }
        }
    }
}

pub(super) mod continued;
pub(crate) use continued::{
    begin_live_continued_wait_v8, LiveContinuedParkedStateV8, LiveContinuedTerminalStateV8,
    LiveContinuedWaitStartOutcomeV8,
};

#[cfg(test)]
pub(crate) use continued::model::test_continued_resume_entries_v8;
pub(crate) use continued::model::{
    resume_live_continued_wait_v8, LiveContinuedWaitResumeOutcomeV8,
};

pub(crate) use continued::model::authorize::{
    authorize_live_continued_state_v8, transfer_live_continued_state_v8,
    LiveContinuedAuthorizeOutcomeV8, LiveContinuedStagedAuthorizationV8,
    LiveContinuedTransferOutcomeV8, LiveContinuedTransferredStateV8,
};

#[cfg(test)]
pub(crate) use continued::model::authorize::{
    test_continued_authorize_entries_v8, test_continued_authorize_entry_v8,
};

pub(crate) use continued::model::authorize::effect::{
    promote_live_continued_authorization_v8, LiveContinuedReadyPromotionOutcomeV8,
};

#[cfg(test)]
pub(crate) use continued::model::authorize::effect::test_continued_ready_promotions_v8;
