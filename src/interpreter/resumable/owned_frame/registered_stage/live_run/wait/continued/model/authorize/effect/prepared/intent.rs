//! Actual Prepared→Activated after the continued physical Intent ACK. No host.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    activate_with_guard_v8, ActivatedOwnedEffectV8, LiveEffectActivationRejectionV8,
    LiveIntentGuardV8,
};
use crate::live_invocation::source_journal::{LiveContinuedIntentPermitV8, SourceJournalEntry};
pub(crate) struct LiveContinuedEffectActivationV8<'j> {
    owner: ActivationOwnerV8<'j>,
    error: Option<SourceJournalError>,
    helper_consumed: (u64, u64),
    authorize_consumed: u64,
}
enum ActivationOwnerV8<'j> {
    Rejected {
        owner: LiveEffectActivationRejectionV8<'j>,
        predecessor: PreparedHeldContinuedWaitV2<'j>,
    },
    Activated {
        owner: ActivatedOwnedEffectV8<'j>,
        predecessor: PreparedHeldContinuedWaitV2<'j>,
    },
}
impl<'j> LiveContinuedEffectPreparationV8<'j> {
    pub(crate) fn intent_row(
        &self,
        permit: &LiveContinuedIntentPermitV8<'_, 'j>,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        match &self.owner {
            ContinuedPreparationOwnerV8::Prepared {
                owner, predecessor, ..
            } => {
                predecessor.validate_retained_context()?;
                owner.live_continued_intent_row(permit)
            }
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(crate) fn validate_intent_prepared(
        &self,
        permit: &LiveContinuedIntentPermitV8<'_, 'j>,
    ) -> Result<(), SourceJournalError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        match &self.owner {
            ContinuedPreparationOwnerV8::Prepared {
                owner, predecessor, ..
            } => {
                predecessor.validate_retained_context()?;
                owner.validate_intent_guard(&LiveIntentGuardV8::Continued(permit))
            }
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(crate) fn activate_intent(
        self,
        permit: &LiveContinuedIntentPermitV8<'_, 'j>,
    ) -> Result<LiveContinuedEffectActivationV8<'j>, (Self, SourceJournalError)> {
        if let Err(error) = self.validate_intent_prepared(permit) {
            return Err((self, error));
        }
        let Self {
            owner,
            error: _,
            helper_consumed,
        } = self;
        let ContinuedPreparationOwnerV8::Prepared {
            owner,
            predecessor,
            authorize_consumed,
        } = owner
        else {
            return Err((
                Self {
                    owner,
                    error: Some(SourceJournalError::Order),
                    helper_consumed,
                },
                SourceJournalError::Order,
            ));
        };
        let result = activate_with_guard_v8(owner, LiveIntentGuardV8::Continued(permit));
        let (owner, error) = match result {
            Ok(owner) => (ActivationOwnerV8::Activated { owner, predecessor }, None),
            Err(owner) => {
                let error = owner.error();
                (
                    ActivationOwnerV8::Rejected { owner, predecessor },
                    Some(error),
                )
            }
        };
        let mut actual = LiveContinuedEffectActivationV8 {
            owner,
            error,
            helper_consumed,
            authorize_consumed,
        };
        if actual.error.is_none() {
            actual.error = actual.validate_live(permit).err();
        }
        Ok(actual)
    }
}
impl LiveContinuedEffectActivationV8<'_> {
    pub(crate) fn selected_error(&self) -> Option<SourceJournalError> {
        self.error
    }
    pub(crate) fn validate_live(
        &self,
        permit: &LiveContinuedIntentPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        match &self.owner {
            ActivationOwnerV8::Activated { owner, predecessor } => {
                let result = predecessor.validate_retained_context().and_then(|_| {
                    owner.validate_intent_guard(&LiveIntentGuardV8::Continued(permit))
                });
                if result.is_err() {
                    owner.quarantine_live_intent();
                }
                result
            }
            _ => Err(SourceJournalError::Order),
        }
    }
    #[cfg(test)]
    pub(crate) fn test_after(&self) -> bool {
        matches!(
            &self.owner,
            ActivationOwnerV8::Activated { .. }
                | ActivationOwnerV8::Rejected {
                    owner: LiveEffectActivationRejectionV8::After { .. },
                    ..
                }
        )
    }
}
#[cfg(test)]
thread_local! {static ACTIVATIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
pub(crate) fn test_note_continued_activation() {
    ACTIVATIONS.with(|x| x.set(x.get() + 1));
}
#[cfg(test)]
pub(crate) fn test_continued_activations() -> usize {
    ACTIVATIONS.with(std::cell::Cell::get)
}
