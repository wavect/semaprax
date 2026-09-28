//! Actual renewed Consumed → Prepared. The whole ledger/token lineage is retained.
use super::*;
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LivePreparedContinuedEffectV8<
    'j,
> {
    owner: LiveContinuedEffectV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedEffectPreparationFailureV8<
    'j,
> {
    Before {
        owner: LiveContinuedEffectV8<'j>,
        error: SourceJournalError,
    },
    After {
        owner: LivePreparedContinuedEffectV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveContinuedEffectV8<'j> {
    fn preparation_references(&self) -> Result<(u32, u32, u32), SourceJournalError> {
        if self.acks.len() != 2 {
            return Err(SourceJournalError::Order);
        }
        let ready = self.acks[0]
            .session
            .sequence()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?;
        let consumed = self.acks[1]
            .session
            .sequence()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?;
        if self.staged.checked_add(1) != Some(ready) || ready.checked_add(1) != Some(consumed) {
            return Err(SourceJournalError::Binding);
        }
        Ok((self.staged, ready, consumed))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_actual_effect(
        self,
    ) -> Result<LivePreparedContinuedEffectV8<'j>, LiveContinuedEffectPreparationFailureV8<'j>>
    {
        let references = match self
            .preparation_references()
            .and_then(|r| self.validate_live().map(|_| r))
        {
            Ok(r) => r,
            Err(error) => {
                return Err(LiveContinuedEffectPreparationFailureV8::Before { owner: self, error })
            }
        };
        let LiveContinuedEffectV8 {
            authorization,
            commitments,
            staged,
            acks,
        } = self;
        let LiveContinuedAuthorizationV8 {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks: aacks,
        } = authorization;
        let super::super::super::LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: ma,
            dispatched,
            proposal,
        } = completed;
        let current = acks.last().expect("real C ACK");
        let owner = owner.prepare_effect_actual(
            &current.session,
            &current.witness,
            proposal.as_ref().expect("checked actual K"),
            &commitments,
            references,
        );
        let completed = super::super::super::LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: ma,
            dispatched,
            proposal,
        };
        let authorization = LiveContinuedAuthorizationV8 {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks: aacks,
        };
        let owner = LiveContinuedEffectV8 {
            authorization,
            commitments,
            staged,
            acks,
        };
        let actual = LivePreparedContinuedEffectV8 { owner };
        match actual.validate_live() {
            Ok(()) => Ok(actual),
            Err(error) => Err(LiveContinuedEffectPreparationFailureV8::After {
                owner: actual,
                error,
            }),
        }
    }
}
impl LivePreparedContinuedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let owner = &self.owner;
        (|| {
            let refs = owner.preparation_references()?;
            let current = owner.acks.last().ok_or(SourceJournalError::Order)?;
            owner
                .authorization
                .actual()?
                .owner
                .validate_prepared_effect(
                    &current.session,
                    &current.witness,
                    owner.authorization.proposal()?,
                    &owner.commitments,
                    refs,
                )?;
            current.witness.validate_current_session(&current.session)
        })()
        .inspect_err(|_| owner.journal().quarantine())
    }
}
#[cfg(test)]
mod tests;
