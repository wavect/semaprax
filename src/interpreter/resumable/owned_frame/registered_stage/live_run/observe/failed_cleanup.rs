//! Consuming failed initial Observe under an actual cleanup-start ACK permit.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::observe::{
    capture_failed_observe_cleanup_v8, ActualFailedObserveCleanupRejectionV8,
    ObservedFailedObserveCleanupV8,
};
use crate::live_invocation::source_journal::{
    LiveFailedObserveStateCleanupPermitV8, SourceJournalError,
};

pub(crate) struct ReleasedInitialObserveStateV8 {
    released: ObservedFailedObserveCleanupV8,
    facts: serde_json::Value,
    consumed: u64,
}
impl ReleasedInitialObserveStateV8 {
    pub(crate) fn cleanup(&self) -> &ObservedFailedObserveCleanupV8 {
        &self.released
    }
    pub(crate) fn facts(&self) -> &serde_json::Value {
        &self.facts
    }
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
}
pub(crate) enum InitialObserveStateCleanupFailureV8 {
    Before {
        owner: LiveFailedObserveV8,
        error: SourceJournalError,
    },
    Interrupted {
        owner: LiveFailedObserveV8,
        error: SourceJournalError,
    },
    Released {
        owner: ReleasedInitialObserveStateV8,
        error: SourceJournalError,
    },
    Capture {
        _actual: ActualFailedObserveCleanupRejectionV8,
        _facts: serde_json::Value,
        _consumed: u64,
    },
}
impl LiveFailedObserveV8 {
    pub(crate) fn release_failed_state_v8(
        self,
        permit: &LiveFailedObserveStateCleanupPermitV8<'_, '_>,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<ReleasedInitialObserveStateV8, InitialObserveStateCleanupFailureV8> {
        let checked = (|| {
            let state = self
                .live_state_facts_v8()
                .map_err(|_| SourceJournalError::Binding)?;
            permit.validate_initial()?;
            self.owner
                .validate_cleanup_binding_v8(permit, &state, self.consumed)?;
            permit.validate_cleanup_current()
        })();
        if let Err(error) = checked {
            permit.quarantine();
            return Err(InitialObserveStateCleanupFailureV8::Before { owner: self, error });
        }
        let Self {
            owner,
            facts,
            consumed,
        } = self;
        let mut guard_error = None;
        let result = capture_failed_observe_cleanup_v8(
            owner,
            || match permit.validate_cleanup_current() {
                Ok(()) => true,
                Err(error) => {
                    if guard_error.is_none() {
                        guard_error = Some(error);
                    }
                    false
                }
            },
            observe,
        );
        let released = match result {
            Ok(released) => ReleasedInitialObserveStateV8 {
                released,
                facts,
                consumed,
            },
            Err(ActualFailedObserveCleanupRejectionV8::Owner(rejection)) => {
                permit.quarantine();
                return Err(InitialObserveStateCleanupFailureV8::Interrupted {
                    owner: Self {
                        owner: rejection.failed,
                        facts,
                        consumed,
                    },
                    error: guard_error.unwrap_or(SourceJournalError::Binding),
                });
            }
            Err(actual) => {
                permit.quarantine();
                return Err(InitialObserveStateCleanupFailureV8::Capture {
                    _actual: actual,
                    _facts: facts,
                    _consumed: consumed,
                });
            }
        };
        // The root has been physically released: use only the actual receipt
        // and original descriptive facts, never the old live-State getter.
        if let Err(error) = permit.validate_cleanup_current() {
            permit.quarantine();
            return Err(InitialObserveStateCleanupFailureV8::Released {
                owner: released,
                error,
            });
        }
        Ok(released)
    }
}
