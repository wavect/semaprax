//! Consuming actual C3a Activated; no caller ledger or raw dispatch permission.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{TargetAccounting, TargetHostHandler};
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveDispatchedContinuedEffectV8<
    'j,
> {
    phase: ContinuedIntentPhaseV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedDispatchFailureV8<
    'j,
> {
    Before {
        owner: LiveActivatedContinuedEffectV8<'j>,
        error: SourceJournalError,
    },
    After {
        owner: LiveDispatchedContinuedEffectV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveActivatedContinuedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn dispatch(
        self,
        handler: &mut dyn TargetHostHandler,
    ) -> Result<LiveDispatchedContinuedEffectV8<'j>, LiveContinuedDispatchFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveContinuedDispatchFailureV8::Before { owner: self, error });
        }
        let ContinuedIntentPhaseV8 { owner, ack } = self.phase;
        let refs = owner
            .owner
            .preparation_references()
            .expect("actual adjacent C references");
        let LivePreparedContinuedEffectV8 { owner } = owner;
        let LiveContinuedEffectV8 {
            authorization,
            commitments,
            staged,
            acks,
        } = owner;
        let LiveContinuedAuthorizationV8 {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks: aacks,
        } = authorization;
        let super::super::super::super::super::LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: ma,
            dispatched,
            proposal,
        } = completed;
        let prior = acks.last().expect("actual C ACK");
        let result = owner.dispatch_continued_effect(
            &prior.session,
            &prior.witness,
            &ack.session,
            &ack.witness,
            proposal.as_ref().expect("actual K"),
            &commitments,
            refs,
            handler,
        );
        let (owner, error) = match result {
            Ok(owner) => (owner, None),
            Err((owner, error)) => (owner, Some(error)),
        };
        let completed = super::super::super::super::super::LiveContinuedModelV8 {
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
        let phase = ContinuedIntentPhaseV8 {
            owner: LivePreparedContinuedEffectV8 {
                owner: LiveContinuedEffectV8 {
                    authorization,
                    commitments,
                    staged,
                    acks,
                },
            },
            ack,
        };
        if let Some(error) = error {
            return Err(LiveContinuedDispatchFailureV8::Before {
                owner: LiveActivatedContinuedEffectV8 { phase },
                error,
            });
        }
        let owner = LiveDispatchedContinuedEffectV8 { phase };
        if let Err(error) = owner.validate_live() {
            return Err(LiveContinuedDispatchFailureV8::After { owner, error });
        }
        Ok(owner)
    }
}
impl LiveDispatchedContinuedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let owner = &self.phase.owner.owner;
        let prior = owner.acks.last().ok_or(SourceJournalError::Order)?;
        owner
            .authorization
            .actual()?
            .owner
            .validate_continued_dispatch(
                &prior.session,
                &prior.witness,
                &self.phase.ack.session,
                &self.phase.ack.witness,
                owner.authorization.proposal()?,
                &owner.commitments,
                owner.preparation_references()?,
            )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.phase
            .owner
            .owner
            .authorization
            .completed
            .owner
            .accounting()
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod settlement;
#[cfg(all(test, unix))]
mod tests;

impl<'j> LiveDispatchedContinuedEffectV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.phase.owner.owner.journal()
    }
}
