//! One actual continued host body; prior proof is distinct from mutated ledger.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveContinuedDispatchedEffectV8;
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn dispatch_continued_effect(
        self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
        handler: &mut dyn TargetHostHandler,
    ) -> Result<Self, (Self, SourceJournalError)> {
        if let Err(error) = self.validate_continued_activation(
            prior,
            consumed,
            session,
            witness,
            proposal,
            commitments,
            references,
        ) {
            return Err((self, error));
        }
        let Self {
            outcome,
            mut accounting,
            lineage,
        } = self;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Activation(owner),
        )) = outcome
        else {
            return Err((
                Self {
                    outcome,
                    accounting,
                    lineage,
                },
                SourceJournalError::Order,
            ));
        };
        let preceding = accounting;
        let permit = LiveContinuedIntentPermitV8 {
            lineage: &lineage,
            prior,
            consumed,
            current: Some((session, witness)),
            proposal,
            commitments,
            references,
            accounting: &preceding,
        };
        match owner.dispatch_actual(&mut accounting, &permit, handler) {
            Err((owner, error)) => Err((
                Self {
                    outcome: ContinuedResumeOutcomeV8::Authorization(
                        ContinuedAuthorizationOutcomeV8::Effect(
                            ContinuedEffectOutcomeV8::Activation(owner),
                        ),
                    ),
                    accounting,
                    lineage,
                },
                error,
            )),
            Ok(owner) => Ok(Self {
                outcome: ContinuedResumeOutcomeV8::Authorization(
                    ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Dispatch(
                        owner, preceding,
                    )),
                ),
                accounting,
                lineage,
            }),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_dispatch(
        &self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<(), SourceJournalError> {
        (|| {
            let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Dispatch(owner, preceding),
            )) = &self.outcome
            else {
                return Err(SourceJournalError::Order);
            };
            if let Some(error) = owner.selected_error() {
                return Err(error);
            }
            let permit = LiveContinuedIntentPermitV8 {
                lineage: &self.lineage,
                prior,
                consumed,
                current: Some((session, witness)),
                proposal,
                commitments,
                references,
                accounting: preceding,
            };
            permit.validate_current()?;
            owner.validate_intent(&permit)?;
            if !owner.accounting_matches(preceding, &self.accounting) {
                return Err(SourceJournalError::Binding);
            }
            permit.validate_current()
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_dispatched(
        &self,
    ) -> Option<(
        Option<crate::live_invocation::source_journal::SourceEffectFailure>,
        bool,
        Option<(Vec<u8>, Option<Vec<u8>>)>,
    )> {
        match &self.outcome {
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Dispatch(owner, _),
            )) => Some((
                owner.test_reason(),
                owner.test_retired(),
                owner.test_exchange(),
            )),
            _ => None,
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod settlement;

#[cfg(test)]
impl ContinuedResumedWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_dispatch_cancellation(
        &self,
    ) -> crate::agent_runtime::AgentCancellation {
        self.lineage.step.origin().cancellation.clone()
    }
}
