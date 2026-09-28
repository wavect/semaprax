//! Actual continued Activated consumed into the existing target body once.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{TargetAccounting, TargetHostHandler};
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    dispatch_continued_owned_effect_v8, CheckedLiveOwnedEffectSettlementV8, StagedOwnedEffectV8,
};
use crate::live_invocation::source_journal::LiveContinuedSettlementPermitV8;
pub(crate) struct LiveContinuedDispatchedEffectV8<'j> {
    owner: StagedOwnedEffectV8<'j>,
    predecessor: PreparedHeldContinuedWaitV2<'j>,
    helper_consumed: (u64, u64),
    authorize_consumed: u64,
    selected: Option<SourceJournalError>,
}
impl<'j> LiveContinuedEffectActivationV8<'j> {
    pub(crate) fn dispatch_actual(
        self,
        accounting: &mut TargetAccounting,
        permit: &LiveContinuedIntentPermitV8<'_, 'j>,
        handler: &mut dyn TargetHostHandler,
    ) -> Result<LiveContinuedDispatchedEffectV8<'j>, (Self, SourceJournalError)> {
        if let Err(error) = self.validate_live(permit) {
            return Err((self, error));
        }
        let Self {
            owner,
            error,
            helper_consumed,
            authorize_consumed,
        } = self;
        let ActivationOwnerV8::Activated { owner, predecessor } = owner else {
            return Err((
                Self {
                    owner,
                    error,
                    helper_consumed,
                    authorize_consumed,
                },
                SourceJournalError::Order,
            ));
        };
        let (owner, selected) =
            dispatch_continued_owned_effect_v8(owner, accounting, permit, handler);
        let mut actual = LiveContinuedDispatchedEffectV8 {
            owner,
            predecessor,
            helper_consumed,
            authorize_consumed,
            selected,
        };
        if actual.selected.is_none() {
            actual.selected = actual.validate_intent(permit).err();
        }
        Ok(actual)
    }
}
impl LiveContinuedDispatchedEffectV8<'_> {
    pub(crate) fn selected_error(&self) -> Option<SourceJournalError> {
        self.selected
    }
    pub(crate) fn validate_intent(
        &self,
        permit: &LiveContinuedIntentPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if let Some(error) = self.selected {
            return Err(error);
        }
        self.predecessor
            .validate_retained_context()
            .and_then(|_| self.owner.validate_continued_dispatch(permit))
            .inspect_err(|_| self.owner.quarantine_live_dispatch())
    }
    pub(crate) fn accounting_matches(
        &self,
        prior: &TargetAccounting,
        current: &TargetAccounting,
    ) -> bool {
        self.owner.dispatch().map_or(*current == *prior, |actual| {
            actual.evidence().accounting() == *current
        })
    }
    pub(crate) fn settlement(
        &self,
        accounting: &TargetAccounting,
        permit: &LiveContinuedSettlementPermitV8<'_, '_>,
    ) -> Result<CheckedLiveOwnedEffectSettlementV8, SourceJournalError> {
        if let Some(error) = self.selected {
            return Err(error);
        }
        self.predecessor.validate_retained_context()?;
        self.owner
            .checked_live_continued_settlement_v8(accounting, permit)
    }
    #[cfg(test)]
    pub(crate) fn test_retired(&self) -> bool {
        self.owner.live_test_retired()
    }
    #[cfg(test)]
    pub(crate) fn test_reason(
        &self,
    ) -> Option<crate::live_invocation::source_journal::SourceEffectFailure> {
        self.owner.reason()
    }
    #[cfg(test)]
    pub(crate) fn test_exchange(&self) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
        self.owner.dispatch().map(|d| {
            (
                d.evidence().canonical_wire(),
                self.owner.target_result_wire(),
            )
        })
    }
}
