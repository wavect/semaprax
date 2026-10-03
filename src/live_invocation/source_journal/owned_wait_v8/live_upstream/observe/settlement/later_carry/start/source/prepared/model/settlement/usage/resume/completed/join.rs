//! Consuming join is bound to the physical successful Resume and exact ACK pair.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedModelV8;

impl<'j> LiveLaterModelCompletedV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_continued_model(
        self,
    ) -> Result<LiveContinuedModelV8<'j>, (Self, SourceJournalError)> {
        let checked = (|| {
            self.validate_live()?;
            self.owner
                .witness
                .validate_against_acknowledged_session(&self.owner.session)?;
            self.witness
                .validate_actual_predecessor(&self.owner.session)?;
            self.witness.validate_current_session(&self.session)?;
            let expected = self.owner.completed_facts()?;
            if self.witness.selected_row() != &expected
                || self.session.continued_model_accounting()? != *self.owner.accounting()
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        if let Err(error) = checked {
            self.owner.journal().quarantine();
            return Err((self, error));
        }
        let Self {
            owner,
            session,
            witness,
        } = self;
        let LiveLaterModelResumedV8 {
            owner,
            history,
            proposal,
            wait,
            session: resume_session,
            witness: resume_witness,
        } = owner;
        let owner = match owner.into_continued() {
            Ok(owner) => owner,
            Err((owner, error)) => {
                owner.journal().quarantine();
                return Err((
                    Self {
                        owner: LiveLaterModelResumedV8 {
                            owner,
                            history,
                            proposal,
                            wait,
                            session: resume_session,
                            witness: resume_witness,
                        },
                        session,
                        witness,
                    },
                    error,
                ));
            }
        };
        Ok(history.join(
            owner,
            proposal,
            resume_session,
            resume_witness,
            session,
            witness,
        ))
    }
}

#[cfg(test)]
impl LiveLaterModelCompletedV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_corrupt_join_wait(
        &mut self,
    ) {
        self.owner.wait.push_str("-foreign");
    }
}
