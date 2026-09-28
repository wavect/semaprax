//! Actual failed continued Observe, preserving its original held context.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::observe::{
    capture_failed_observe_cleanup_v8, ActualFailedObserveCleanupRejectionV8,
    ObservedFailedObserveCleanupV8,
};
use crate::live_invocation::source_journal::{
    LiveFailedObserveStateCleanupPermitV8, SourceJournalError,
};

pub(crate) struct ReleasedContinuedObserveStateV8<'j> {
    released: ObservedFailedObserveCleanupV8,
    context: HeldOwnedTurnContextV2<'j>,
    turn: u32,
    reservation: u32,
    consumed: usize,
}
impl ReleasedContinuedObserveStateV8<'_> {
    pub(crate) fn cleanup(&self) -> &ObservedFailedObserveCleanupV8 {
        &self.released
    }
    pub(crate) fn consumed(&self) -> usize {
        self.consumed
    }
    pub(crate) fn causal_refs(&self) -> (u32, u32) {
        (self.turn, self.reservation)
    }
    pub(crate) fn validate_incurred_guard(&self) -> bool {
        incurred(&self.context)
    }
}
pub(crate) enum ContinuedObserveStateCleanupFailureV8<'j> {
    Before {
        owner: FailedHeldOwnedObserveV2<'j>,
        error: SourceJournalError,
    },
    Interrupted {
        owner: FailedHeldOwnedObserveV2<'j>,
        error: SourceJournalError,
    },
    Released {
        owner: ReleasedContinuedObserveStateV8<'j>,
        error: SourceJournalError,
    },
    Capture(CapturedContinuedObserveReleaseV8<'j>),
}
pub(crate) struct CapturedContinuedObserveReleaseV8<'j> {
    _actual: ActualFailedObserveCleanupRejectionV8,
    _context: HeldOwnedTurnContextV2<'j>,
    _turn: u32,
    _reservation: u32,
    _consumed: usize,
}
fn incurred(context: &HeldOwnedTurnContextV2<'_>) -> bool {
    context.creator == std::process::id()
        && context.store.validate_guard().is_ok()
        && context
            .runtime
            .owned_wait_effects_v8(context.execution)
            .is_ok()
        && context.policy.allows(&context.effect_id)
    // Cancellation is deliberately excluded after the incurred Started ACK.
    // This does not grant a successful Outcome or any next source stage.
}
impl<'j> FailedHeldOwnedObserveV2<'j> {
    pub(crate) fn live_failed_state_facts_v8(&self) -> Result<serde_json::Value, Diagnostic> {
        if !incurred(&self.context) {
            return Err(rejected("failed Observe incurred context differs"));
        }
        self.failed.live_state_facts_v8()
    }
    pub(crate) fn failure(&self) -> &OwnedFrameFailure {
        self.failed.failure()
    }
    pub(crate) fn causal_refs(&self) -> (u32, u32) {
        (self.turn, self.reservation)
    }

    pub(crate) fn release_failed_state_v8(
        self,
        permit: &LiveFailedObserveStateCleanupPermitV8<'_, 'j>,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<ReleasedContinuedObserveStateV8<'j>, ContinuedObserveStateCleanupFailureV8<'j>>
    {
        let checked = (|| {
            let state = self
                .failed
                .live_state_facts_v8()
                .map_err(|_| SourceJournalError::Binding)?;
            if !incurred(&self.context) {
                return Err(SourceJournalError::Binding);
            }
            permit.validate_continued(
                self.context.runtime,
                self.context.execution,
                &self.context.store,
                self.context.policy,
            )?;
            self.failed
                .validate_cleanup_binding_v8(permit, &state, self.consumed as u64)?;
            permit.validate_cleanup_current()
        })();
        if let Err(error) = checked {
            permit.quarantine();
            return Err(ContinuedObserveStateCleanupFailureV8::Before { owner: self, error });
        }
        let Self {
            failed,
            context,
            turn,
            reservation,
            consumed,
        } = self;
        let mut guard_error = None;
        let result = capture_failed_observe_cleanup_v8(
            failed,
            || {
                let valid = if incurred(&context) {
                    permit.validate_cleanup_current()
                } else {
                    Err(SourceJournalError::Binding)
                };
                match valid {
                    Ok(()) => true,
                    Err(error) => {
                        if guard_error.is_none() {
                            guard_error = Some(error);
                        }
                        false
                    }
                }
            },
            observe,
        );
        let released = match result {
            Ok(released) => ReleasedContinuedObserveStateV8 {
                released,
                context,
                turn,
                reservation,
                consumed,
            },
            Err(ActualFailedObserveCleanupRejectionV8::Owner(rejection)) => {
                permit.quarantine();
                return Err(ContinuedObserveStateCleanupFailureV8::Interrupted {
                    owner: Self {
                        failed: rejection.failed,
                        context,
                        turn,
                        reservation,
                        consumed,
                    },
                    error: guard_error.unwrap_or(SourceJournalError::Binding),
                });
            }
            Err(actual) => {
                permit.quarantine();
                return Err(ContinuedObserveStateCleanupFailureV8::Capture(
                    CapturedContinuedObserveReleaseV8 {
                        _actual: actual,
                        _context: context,
                        _turn: turn,
                        _reservation: reservation,
                        _consumed: consumed,
                    },
                ));
            }
        };
        if let Err(error) = permit.validate_cleanup_current().and_then(|_| {
            if released.validate_incurred_guard() {
                Ok(())
            } else {
                Err(SourceJournalError::Binding)
            }
        }) {
            permit.quarantine();
            return Err(ContinuedObserveStateCleanupFailureV8::Released {
                owner: released,
                error,
            });
        }
        Ok(released)
    }
}

#[cfg(test)]
impl FailedHeldOwnedObserveV2<'_> {
    pub(crate) fn test_cleanup_weak_v8(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.failed.test_cleanup_weak_v8()
    }
}
