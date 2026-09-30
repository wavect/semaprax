//! Actual continued Activated consumed into the existing target body once.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{TargetAccounting, TargetHostHandler};
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    dispatch_continued_owned_effect_v8, CheckedLiveOwnedEffectSettlementV8, StagedOwnedEffectV8,
};
use crate::live_invocation::source_journal::LiveContinuedSettlementPermitV8;
pub(crate) struct LiveContinuedDispatchedEffectV8<'j> {
    owner: Option<StagedOwnedEffectV8<'j>>,
    released: Option<Box<Result<crate::interpreter::resumable::owned_frame::registered_stage::effect::PendingOwnedEffectReceiptV8<'j>, crate::interpreter::resumable::owned_frame::registered_stage::effect::LiveEffectDecisionReleaseFailureV8<'j>>>>,
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
            owner: Some(owner),
            released: None,
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
impl<'j> LiveContinuedDispatchedEffectV8<'j> {
    fn actual(&self) -> Result<&StagedOwnedEffectV8<'j>, SourceJournalError> {
        self.owner.as_ref().ok_or(SourceJournalError::Order)
    }
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
            .and_then(|_| self.actual()?.validate_continued_dispatch(permit))
            .inspect_err(|_| {
                if let Some(owner) = &self.owner {
                    owner.quarantine_live_dispatch();
                }
            })
    }
    pub(crate) fn accounting_matches(
        &self,
        prior: &TargetAccounting,
        current: &TargetAccounting,
    ) -> bool {
        self.owner.as_ref().is_some_and(|owner| {
            owner.dispatch().map_or(*current == *prior, |actual| {
                actual.evidence().accounting() == *current
            })
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
        self.actual()?
            .checked_live_continued_settlement_v8(accounting, permit)
    }
    /// Borrow the actual retained Decision only under its current settlement permit.
    pub(crate) fn decision_cleanup_values(
        &self,
        permit: &LiveContinuedSettlementPermitV8<'_, '_>,
    ) -> Result<(serde_json::Value, serde_json::Value), SourceJournalError> {
        if let Some(error) = self.selected {
            return Err(error);
        }
        self.predecessor.validate_retained_context()?;
        self.actual()?.continued_decision_cleanup_values_v8(permit)
    }
    #[cfg(test)]
    pub(crate) fn test_retired(&self) -> bool {
        self.owner
            .as_ref()
            .is_none_or(|owner| owner.live_test_retired())
    }
    #[cfg(test)]
    pub(crate) fn test_reason(
        &self,
    ) -> Option<crate::live_invocation::source_journal::SourceEffectFailure> {
        self.owner.as_ref().and_then(|owner| owner.reason())
    }
    #[cfg(test)]
    pub(crate) fn test_exchange(&self) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
        self.owner.as_ref()?.dispatch().map(|d| {
            (
                d.evidence().canonical_wire(),
                self.owner.as_ref().unwrap().target_result_wire(),
            )
        })
    }
}

impl<'j> LiveContinuedDispatchedEffectV8<'j> {
    pub(crate) fn release_decision(
        &mut self,
        permit: &crate::live_invocation::source_journal::LiveContinuedDecisionCleanupPermitV8<
            '_,
            'j,
        >,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    ) -> Result<(), SourceJournalError> {
        use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
            release_with_guard_v8, DecisionCleanupGuardV8,
        };
        self.predecessor.validate_incurred_context()?;
        permit.validate_cleanup_current()?;
        if self.selected.is_some() || self.released.is_some() {
            return Err(SourceJournalError::Order);
        }
        let owner = self.owner.take().ok_or(SourceJournalError::Order)?;
        let released =
            release_with_guard_v8(owner, DecisionCleanupGuardV8::Continued(permit), observe);
        let success = released.is_ok();
        // Every outcome retains its physical owner, including partial release.
        self.released = Some(Box::new(released));
        if success {
            permit.validate_cleanup_current()
        } else {
            self.selected = Some(SourceJournalError::Binding);
            Err(SourceJournalError::Binding)
        }
    }
    pub(crate) fn decision_receipt(&self) -> Result<&serde_json::Value, SourceJournalError> {
        match self.released.as_deref() {
            Some(Ok(owner)) => Ok(owner.receipt()),
            _ => Err(SourceJournalError::Order),
        }
    }
}
