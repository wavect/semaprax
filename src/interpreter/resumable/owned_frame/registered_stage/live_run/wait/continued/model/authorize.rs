//! Consuming transfer/authorize of the actual continued terminal. The original
//! context and observed helper counts survive every boundary outcome.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::authorize::{
    authorize_with_guard_v8, transfer_with_guard_v8, AuthorizeGuardV8, LiveAuthorizeOutcomeV8,
    LiveStagedAuthorizationV8, LiveStateTransferOutcomeV8, LiveTransferredStateV8, TransferGuardV8,
};
use crate::live_invocation::source_journal::{
    LiveContinuedAuthorizePermitV8, LiveContinuedStateTransferPermitV8,
};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;

pub(crate) struct LiveContinuedTransferredStateV8<'j> {
    transferred: LiveTransferredStateV8,
    predecessor: PreparedHeldContinuedWaitV2<'j>,
    start_consumed: u64,
    resume_consumed: u64,
}
pub(crate) enum LiveContinuedTransferOutcomeV8<'j> {
    Moved(LiveContinuedTransferredStateV8<'j>),
    Refused(LiveContinuedResumedStateV8<'j>),
    GuardLost(LiveContinuedTransferredStateV8<'j>),
}
pub(crate) struct LiveContinuedStagedAuthorizationV8<'j> {
    staged: LiveStagedAuthorizationV8,
    predecessor: PreparedHeldContinuedWaitV2<'j>,
    start_consumed: u64,
    resume_consumed: u64,
}
pub(crate) enum LiveContinuedAuthorizeOutcomeV8<'j> {
    Staged(LiveContinuedStagedAuthorizationV8<'j>),
    Refused(LiveContinuedTransferredStateV8<'j>),
    Failed(LiveContinuedStagedAuthorizationV8<'j>),
    GuardLost(LiveContinuedStagedAuthorizationV8<'j>),
}
impl LiveContinuedResumedStateV8<'_> {
    pub(crate) fn transfer_ready(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> bool {
        self.predecessor.validate_retained_context().is_ok()
            && self.resumed.transfer_ready(binding, proposal)
    }
}
pub(crate) fn transfer_live_continued_state_v8<'j>(
    permit: &LiveContinuedStateTransferPermitV8<'_, 'j>,
    owner: LiveContinuedResumedStateV8<'j>,
    proposal: &CheckedOwnedWaitProposalV8,
) -> LiveContinuedTransferOutcomeV8<'j> {
    if !owner.predecessor.matches_transfer_permit(permit)
        || owner.predecessor.validate_retained_context().is_err()
        || permit.validate_guard().is_err()
    {
        return LiveContinuedTransferOutcomeV8::Refused(owner);
    }
    let LiveContinuedResumedStateV8 {
        resumed,
        predecessor,
        start_consumed,
    } = owner;
    let resume_consumed = resumed.consumed();
    let result = transfer_with_guard_v8(TransferGuardV8::Continued(permit), resumed, proposal);
    let lost = matches!(&result, LiveStateTransferOutcomeV8::GuardLost(_));
    match result {
        LiveStateTransferOutcomeV8::Refused(resumed) => {
            LiveContinuedTransferOutcomeV8::Refused(LiveContinuedResumedStateV8 {
                resumed,
                predecessor,
                start_consumed,
            })
        }
        LiveStateTransferOutcomeV8::Moved(transferred)
        | LiveStateTransferOutcomeV8::GuardLost(transferred) => {
            let owner = LiveContinuedTransferredStateV8 {
                transferred,
                predecessor,
                start_consumed,
                resume_consumed,
            };
            if lost
                || permit.validate_guard().is_err()
                || owner.predecessor.validate_retained_context().is_err()
            {
                LiveContinuedTransferOutcomeV8::GuardLost(owner)
            } else {
                LiveContinuedTransferOutcomeV8::Moved(owner)
            }
        }
    }
}
pub(crate) fn authorize_live_continued_state_v8<'j>(
    permit: &LiveContinuedAuthorizePermitV8<'_, 'j>,
    owner: LiveContinuedTransferredStateV8<'j>,
) -> LiveContinuedAuthorizeOutcomeV8<'j> {
    if !owner.predecessor.matches_authorize_permit(permit)
        || owner.predecessor.validate_retained_context().is_err()
        || permit.validate_guard().is_err()
        || permit.fuel() != owner.predecessor.evaluation_fuel()
    {
        return LiveContinuedAuthorizeOutcomeV8::Refused(owner);
    }
    let LiveContinuedTransferredStateV8 {
        transferred,
        predecessor,
        start_consumed,
        resume_consumed,
    } = owner;
    let result = authorize_with_guard_v8(AuthorizeGuardV8::Continued(permit), transferred);
    let (staged, failure, lost) = match result {
        LiveAuthorizeOutcomeV8::Refused(transferred) => {
            return LiveContinuedAuthorizeOutcomeV8::Refused(LiveContinuedTransferredStateV8 {
                transferred,
                predecessor,
                start_consumed,
                resume_consumed,
            });
        }
        LiveAuthorizeOutcomeV8::Staged(x) => (x, false, false),
        LiveAuthorizeOutcomeV8::Failed(x) => (x, true, false),
        LiveAuthorizeOutcomeV8::GuardLost(x) => (x, false, true),
    };
    let owner = LiveContinuedStagedAuthorizationV8 {
        staged,
        predecessor,
        start_consumed,
        resume_consumed,
    };
    if lost
        || permit.validate_guard().is_err()
        || owner.predecessor.validate_retained_context().is_err()
    {
        LiveContinuedAuthorizeOutcomeV8::GuardLost(owner)
    } else if failure {
        LiveContinuedAuthorizeOutcomeV8::Failed(owner)
    } else {
        LiveContinuedAuthorizeOutcomeV8::Staged(owner)
    }
}
impl LiveContinuedTransferredStateV8<'_> {
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Option<serde_json::Value> {
        self.predecessor.validate_retained_context().ok()?;
        self.transferred.checked_facts(binding, proposal)
    }
}
impl LiveContinuedStagedAuthorizationV8<'_> {
    pub(crate) fn failure(
        &self,
    ) -> Option<&crate::interpreter::resumable::owned_frame::OwnedFrameFailure> {
        self.staged.failure()
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value)> {
        self.predecessor.validate_retained_context().ok()?;
        self.staged.checked_facts(binding)
    }
    pub(crate) fn consumed(&self) -> u64 {
        self.staged.consumed()
    }
    pub(crate) fn helper_consumed(&self) -> (u64, u64) {
        (self.start_consumed, self.resume_consumed)
    }
}

#[cfg(test)]
thread_local! {static ENTRIES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
pub(crate) fn test_continued_authorize_entry_v8() {
    ENTRIES.with(|x| x.set(x.get() + 1));
}
#[cfg(test)]
pub(crate) fn test_continued_authorize_entries_v8() -> usize {
    ENTRIES.with(std::cell::Cell::get)
}
#[cfg(test)]
impl LiveContinuedTransferredStateV8<'_> {
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.transferred.test_weak()
    }
}
#[cfg(test)]
impl LiveContinuedStagedAuthorizationV8<'_> {
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.staged.test_weak()
    }
}
#[cfg(test)]
impl LiveContinuedResumedStateV8<'_> {
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.resumed.test_weak()
    }
}
