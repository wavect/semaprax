//! Consuming actual continued Ready into Prepared. No ledger or target entry.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    live_append::EffectPreparationGuardV8, PreparedOwnedEffectV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::authorize::LiveReadyEffectPreparationRejectionV8;
use crate::live_invocation::source_journal::{
    LiveContinuedEffectAuthorizationPermitV8, SourceJournalError,
};

pub(crate) enum ContinuedPreparationOwnerV8<'j> {
    Before(LiveContinuedReadyAuthorizationV8<'j>),
    Preparation {
        owner: LiveReadyEffectPreparationRejectionV8<'j>,
        predecessor: PreparedHeldContinuedWaitV2<'j>,
    },
    Prepared {
        owner: PreparedOwnedEffectV8<'j>,
        predecessor: PreparedHeldContinuedWaitV2<'j>,
        authorize_consumed: u64,
    },
}
pub(crate) struct LiveContinuedEffectPreparationV8<'j> {
    owner: ContinuedPreparationOwnerV8<'j>,
    error: Option<SourceJournalError>,
    helper_consumed: (u64, u64),
}
pub(crate) fn prepare_live_continued_effect_v8<'j>(
    permit: &LiveContinuedEffectAuthorizationPermitV8<'_, 'j>,
    owner: LiveContinuedReadyAuthorizationV8<'j>,
) -> LiveContinuedEffectPreparationV8<'j> {
    let helper_consumed = owner.helper_consumed();
    let checked = (|| {
        owner.predecessor.validate_retained_context()?;
        if !owner.predecessor.matches_effect_preparation_permit(permit) {
            return Err(SourceJournalError::Binding);
        }
        permit.inputs()
    })();
    let inputs = match checked {
        Ok(inputs) => inputs,
        Err(error) => {
            return LiveContinuedEffectPreparationV8 {
                owner: ContinuedPreparationOwnerV8::Before(owner),
                error: Some(error),
                helper_consumed,
            }
        }
    };
    let LiveContinuedReadyAuthorizationV8 {
        ready, predecessor, ..
    } = owner;
    let authorize_consumed = ready.consumed();
    let result = ready.prepare_with_guard_v8(inputs, EffectPreparationGuardV8::Continued(permit));
    match result {
        Ok(owner) => {
            let error = predecessor
                .validate_retained_context()
                .and_then(|_| {
                    owner.validate_preparation_guard(&EffectPreparationGuardV8::Continued(permit))
                })
                .err();
            if error.is_some() {
                owner.quarantine_live_authorization();
            }
            LiveContinuedEffectPreparationV8 {
                owner: ContinuedPreparationOwnerV8::Prepared {
                    owner,
                    predecessor,
                    authorize_consumed,
                },
                error,
                helper_consumed,
            }
        }
        Err(owner) => {
            owner.quarantine();
            let error = Some(owner.error());
            LiveContinuedEffectPreparationV8 {
                owner: ContinuedPreparationOwnerV8::Preparation { owner, predecessor },
                error,
                helper_consumed,
            }
        }
    }
}
impl LiveContinuedEffectPreparationV8<'_> {
    pub(crate) fn selected_error(&self) -> Option<SourceJournalError> {
        self.error
    }
    pub(crate) fn validate_live(
        &self,
        permit: &LiveContinuedEffectAuthorizationPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        match &self.owner {
            ContinuedPreparationOwnerV8::Prepared {
                owner, predecessor, ..
            } => {
                predecessor.validate_retained_context()?;
                owner.validate_preparation_guard(&EffectPreparationGuardV8::Continued(permit))
            }
            _ => Err(SourceJournalError::Order),
        }
    }
    #[cfg(test)]
    pub(crate) fn test_after(&self) -> bool {
        matches!(
            &self.owner,
            ContinuedPreparationOwnerV8::Preparation {
                owner: LiveReadyEffectPreparationRejectionV8::After { .. },
                ..
            } | ContinuedPreparationOwnerV8::Prepared { .. }
        )
    }
    #[cfg(test)]
    pub(crate) fn test_metadata(&self) -> Option<(u32, u32, u32, u64)> {
        match &self.owner {
            ContinuedPreparationOwnerV8::Prepared {
                owner,
                authorize_consumed,
                ..
            } => {
                let (s, r, c, _, _, _) = owner.live_test_metadata();
                Some((s, r, c, *authorize_consumed))
            }
            _ => None,
        }
    }
}
#[cfg(test)]
thread_local! { static PREPARATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn test_continued_effect_preparations_v8() -> usize {
    PREPARATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn test_note_preparation() {
    PREPARATIONS.with(|x| x.set(x.get() + 1));
}

pub(crate) mod intent;
