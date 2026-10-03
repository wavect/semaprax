//! A callback-borrowed permit dispatches only after the exact later Intent ACK.
use super::*;
use crate::live_invocation::source_journal::SourceAttemptFailure;

pub(crate) struct LiveLaterModelIntentPermitV8<'p, 'j> {
    owner: &'p LiveLaterModelIntentV8<'j>,
    clock: &'p dyn crate::live_invocation::SourceInvocationClock,
    admission: std::cell::Cell<Option<SourceAttemptFailure>>,
}
impl LiveLaterModelIntentPermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.owner.dispatched.is_some() {
            return Err(SourceJournalError::Order);
        }
        let checked = (|| {
            let actual = self
                .owner
                .owner
                .owner
                .owner
                .validate_model_admission(&self.owner.session, &self.owner.witness)?;
            let (_, _, turn, _) = self.owner.session.continued_model_facts()?;
            if turn != self.owner.owner.owner.owner.turn()
                || self.owner.session.continued_model_accounting()?
                    != *self.owner.owner.owner.owner.model_accounting()
            {
                return Err(SourceJournalError::Binding);
            }
            self.owner
                .witness
                .validate_current_session(&self.owner.session)?;
            Ok(actual)
        })();
        match checked {
            Err(error) => {
                self.owner.owner.owner.journal().quarantine();
                Err(error)
            }
            Ok(admission) => {
                if let Some(failure) = admission.failure() {
                    if self.admission.get().is_none() {
                        self.admission.set(Some(failure));
                    }
                }
                admission.error().map_or(Ok(()), Err)
            }
        }
    }
    pub(crate) fn validate_store(&self) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .owner
            .validate_model_incurred(&self.owner.session, &self.owner.witness)
    }
    pub(crate) fn request(&self) -> &CheckedOwnedModelRequestV8 {
        &self.owner.request
    }
    pub(crate) fn clock(&self) -> &dyn crate::live_invocation::SourceInvocationClock {
        self.clock
    }
    pub(crate) fn guard_failure(&self) -> SourceAttemptFailure {
        self.admission.get().unwrap_or_else(|| {
            if self.owner.owner.owner.owner.model_cancelled() {
                SourceAttemptFailure::Cancelled
            } else {
                SourceAttemptFailure::Refused
            }
        })
    }
    pub(crate) fn quarantine(&self) {
        self.owner.owner.owner.journal().quarantine()
    }
}
impl<'j> LiveLaterModelIntentV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn dispatch_model(
        mut self,
        adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) -> Result<Self, (Self, SourceJournalError)> {
        let before = self
            .owner
            .owner
            .owner
            .validate_model_incurred(&self.session, &self.witness)
            .and_then(|_| self.owner.owner.owner.model_clock().map(|_| ()))
            .and_then(|_| {
                if self.dispatched.is_none() {
                    Ok(())
                } else {
                    Err(SourceJournalError::Order)
                }
            });
        if let Err(error) = before {
            return Err((self, error));
        }
        self.owner.owner.owner.configure_model_adapter(adapter);
        let result = {
            let clock = match self.owner.owner.owner.model_clock() {
                Ok(clock) => clock,
                Err(error) => return Err((self, error)),
            };
            let permit = LiveLaterModelIntentPermitV8 {
                owner: &self,
                clock,
                admission: std::cell::Cell::new(None),
            };
            adapter.dispatch_later_wait_v8(&permit)
        };
        self.dispatched = Some(result);
        if let Err(error) = self
            .owner
            .owner
            .owner
            .validate_model_incurred(&self.session, &self.witness)
        {
            return Err((self, error));
        }
        Ok(self)
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_dispatched(
        &self,
    ) -> bool {
        self.dispatched.is_some()
    }
}
