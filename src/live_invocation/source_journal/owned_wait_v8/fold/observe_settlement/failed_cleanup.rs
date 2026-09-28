//! Descriptive failed-Observe cleanup coordinates; no owner restoration.
use super::*;
impl FoldV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_observe_cleanup_turn(
        &self,
    ) -> Result<u32, SourceJournalError> {
        let failure = self
            .observe_settlement
            .as_ref()
            .and_then(|s| s.failure.as_ref())
            .ok_or(SourceJournalError::Order)?;
        require(
            self.continuation_profile_selected
                && self.failure_selected
                && self.wait.is_none()
                && self.effect.is_none()
                && self.reduce.is_none()
                && self.failed_effect_state.is_none()
                && self.observer_state.is_none()
                && self.cleanup_terminal.as_ref() == Some(failure),
        )?;
        match self.tail {
            TailV8::FailedState => require(self.state_basis.is_some() && self.cleanup.is_none())?,
            TailV8::CleanupInDoubt => require(
                self.cleanup
                    .as_ref()
                    .is_some_and(|c| c.owner == OwnerV8::State && !c.settled),
            )?,
            TailV8::MetadataOnly | TailV8::Stopped => require(
                self.cleanup
                    .as_ref()
                    .is_some_and(|c| c.owner == OwnerV8::State && c.settled && !c.host_confirmed),
            )?,
            _ => return order(),
        }
        Ok(self.current_turn)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_observe_cleanup_original(
        &self,
    ) -> Result<(u32, &str, &Value), SourceJournalError> {
        self.failed_observe_cleanup_turn()?;
        require(self.tail == TailV8::FailedState)?;
        Ok((
            self.state_basis.ok_or(SourceJournalError::Order)?,
            self.state_digest
                .as_deref()
                .ok_or(SourceJournalError::Order)?,
            self.cleanup_terminal
                .as_ref()
                .ok_or(SourceJournalError::Order)?,
        ))
    }
}

impl FoldV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_failed_observe_generic_successor(
        &self,
        row: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        if self.failed_observe_cleanup_turn().is_ok()
            && matches!(
                row,
                EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupStarted {
                    owner: model::OwnerV8::State,
                    ..
                }) | EntryV8::Owned(model::OwnedBodyV8::OwnedCleanupSettled {
                    owner: model::OwnerV8::State,
                    ..
                }) | EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
            )
        {
            return Err(SourceJournalError::Order);
        }
        Ok(())
    }
}
