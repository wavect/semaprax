//! Success-only continued promotion after the actual Ready ACK; zero source or target calls.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::authorize::{
    promote_with_guard_v8, LiveReadyAuthorizationV8, LiveReadyPromotionOutcomeV8,
    ReadyPromotionGuardV8,
};
use crate::live_invocation::source_journal::LiveContinuedReadyPromotionPermitV8;
pub(crate) struct LiveContinuedReadyAuthorizationV8<'j> {
    ready: LiveReadyAuthorizationV8,
    predecessor: PreparedHeldContinuedWaitV2<'j>,
    start_consumed: u64,
    resume_consumed: u64,
}
pub(crate) enum LiveContinuedReadyPromotionOutcomeV8<'j> {
    Ready(LiveContinuedReadyAuthorizationV8<'j>),
    Refused(LiveContinuedStagedAuthorizationV8<'j>),
    GuardLost(LiveContinuedReadyAuthorizationV8<'j>),
}
pub(crate) fn promote_live_continued_authorization_v8<'j>(
    permit: &LiveContinuedReadyPromotionPermitV8<'_, 'j>,
    owner: LiveContinuedStagedAuthorizationV8<'j>,
) -> LiveContinuedReadyPromotionOutcomeV8<'j> {
    if !owner.predecessor.matches_ready_promotion_permit(permit)
        || owner.predecessor.validate_retained_context().is_err()
        || permit.validate_guard().is_err()
    {
        return LiveContinuedReadyPromotionOutcomeV8::Refused(owner);
    }
    let LiveContinuedStagedAuthorizationV8 {
        staged,
        predecessor,
        start_consumed,
        resume_consumed,
    } = owner;
    let result = promote_with_guard_v8(ReadyPromotionGuardV8::Continued(permit), staged);
    let lost = matches!(&result, LiveReadyPromotionOutcomeV8::GuardLost(_));
    #[cfg(test)]
    if matches!(
        &result,
        LiveReadyPromotionOutcomeV8::Ready(_) | LiveReadyPromotionOutcomeV8::GuardLost(_)
    ) {
        PROMOTIONS.with(|x| x.set(x.get() + 1));
    }

    match result {
        LiveReadyPromotionOutcomeV8::Refused(staged) => {
            LiveContinuedReadyPromotionOutcomeV8::Refused(LiveContinuedStagedAuthorizationV8 {
                staged,
                predecessor,
                start_consumed,
                resume_consumed,
            })
        }
        LiveReadyPromotionOutcomeV8::Ready(ready)
        | LiveReadyPromotionOutcomeV8::GuardLost(ready) => {
            let owner = LiveContinuedReadyAuthorizationV8 {
                ready,
                predecessor,
                start_consumed,
                resume_consumed,
            };
            if lost
                || permit.validate_guard().is_err()
                || owner.predecessor.validate_retained_context().is_err()
            {
                LiveContinuedReadyPromotionOutcomeV8::GuardLost(owner)
            } else {
                LiveContinuedReadyPromotionOutcomeV8::Ready(owner)
            }
        }
    }
}
impl LiveContinuedReadyAuthorizationV8<'_> {
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value)> {
        self.predecessor.validate_retained_context().ok()?;
        self.ready.checked_facts(binding)
    }
    pub(crate) fn consumed(&self) -> u64 {
        self.ready.consumed()
    }
    pub(crate) fn helper_consumed(&self) -> (u64, u64) {
        (self.start_consumed, self.resume_consumed)
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.ready.test_weak()
    }
}

#[cfg(test)]
thread_local! {static PROMOTIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
pub(crate) fn test_continued_ready_promotions_v8() -> usize {
    PROMOTIONS.with(std::cell::Cell::get)
}
