//! Actual continued Intent lineage. Old Consumed history is inert after ACK.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::OwnedEffectSettlementInputsV8;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedIntentSuccessorV8;
fn guard_intent(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedIntentSuccessorV8<'_>,
    proposal: &CheckedOwnedWaitProposalV8,
    strict: bool,
) -> Result<(), SourceJournalError> {
    let journal = lineage.journal();
    let result = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        let origin = lineage.step.origin();
        origin.hold.validate_continued_intent_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let held = journal.hold()?;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        runtime
            .owned_wait_effects_v8(execution)
            .map_err(|_| SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        if strict {
            let ordinary = journal.context().ordinary();
            // Every clock callback is external. Its postguard checks BOTH the
            // true physical Intent/current prefix and the same registry phase
            // before the next clock or any SDK/source work can occur.
            let clock_guard = || {
                held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
                origin.hold.validate_continued_intent_guard(
                    journal,
                    session.sequence(),
                    session.acknowledged_bytes(),
                )?;
                witness.validate_current_session(session)?;
                if !origin.policy.allows(plan.operation().effect_id())
                    || origin.cancellation.is_cancelled()
                {
                    return Err(SourceJournalError::Binding);
                }
                Ok(())
            };
            clock_guard()?;
            let domain = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                origin.clock.clock_domain()
            }));
            let domain = match domain {
                Ok(v) => v,
                Err(_) => {
                    journal.quarantine();
                    return Err(SourceJournalError::Poisoned);
                }
            };
            clock_guard()?;
            if domain != ordinary.clock_domain() {
                return Err(SourceJournalError::Binding);
            }
            let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                origin.clock.now_millis()
            }));
            let now = match now {
                Ok(v) => v,
                Err(_) => {
                    journal.quarantine();
                    return Err(SourceJournalError::Poisoned);
                }
            };
            clock_guard()?;
            if now < ordinary.initial_millis() || now >= ordinary.deadline_millis() {
                return Err(SourceJournalError::Time);
            }
        }
        origin.hold.validate_continued_intent_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}

pub(crate) struct LiveContinuedIntentPermitV8<'p, 'j> {
    lineage: &'p ContinueLineageV8<'j>,
    prior: &'p AppendSessionV8<'j>,
    consumed: &'p VerifiedOwnedContinuedEffectSuccessorV8<'j>,
    current: Option<(
        &'p AppendSessionV8<'j>,
        &'p VerifiedOwnedContinuedIntentSuccessorV8<'j>,
    )>,
    proposal: &'p CheckedOwnedWaitProposalV8,
    commitments: &'p CheckedOwnedWaitReadyCommitmentsV8,
    references: (u32, u32, u32),
    accounting: &'p TargetAccounting,
}
impl LiveContinuedIntentPermitV8<'_, '_> {
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        (|| {
            let journal=self.lineage.journal();
            self.consumed.validate_against_acknowledged_session(self.prior)?;
            if self.prior.continued_effect_accounting()?!=*self.accounting || self.references.0.checked_add(1)!=Some(self.references.1) || self.references.1.checked_add(1)!=Some(self.references.2) || self.references.2 as usize+1!=self.prior.sequence()
                || !matches!(self.consumed.selected_row(),EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed{turn,attempt:0,grant_digest}) if *turn==self.lineage.turn && grant_digest==self.commitments.grant_digest()) {return Err(SourceJournalError::Binding);}
            match self.current {
                None=>guard_effect(self.lineage,self.prior,self.consumed,self.proposal,true),
                Some((session,witness))=>{
                    witness.validate_actual_predecessor(self.prior)?;
                    witness.validate_current_session(session)?;
                    if !session.belongs_to(journal) || session.continued_intent_accounting()?!=*self.accounting {return Err(SourceJournalError::Binding);}
                    guard_intent(self.lineage,session,witness,self.proposal,true)
                }
            }
        })().inspect_err(|_|self.lineage.journal().quarantine())
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.validate_current()?;
            let journal = self.lineage.journal();
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let origin = self.lineage.step.origin();
            let held = journal.hold()?;
            if !std::ptr::eq(runtime, inputs.runtime)
                || !std::ptr::eq(execution, inputs.execution)
                || !std::ptr::eq(origin.policy, inputs.policy)
                || !std::ptr::eq(origin.cancellation, inputs.cancellation)
                || !held.same_container(&inputs.store)
                || inputs.turn != self.lineage.turn
                || inputs.attempt != 0
                || inputs.proposal.carrier() != self.proposal.carrier()
                || inputs.proposal.ordinary_digest() != self.proposal.ordinary_digest()
            {
                return Err(SourceJournalError::Binding);
            }
            let session = self.current.map_or(self.prior, |x| x.0);
            inputs
                .store
                .validate_prefix(session.sequence(), session.acknowledged_bytes())?;
            self.validate_current()
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(crate) fn check_request(
        &self,
        inputs: &OwnedEffectSettlementInputsV8<'_>,
        request: &str,
        operation: &str,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.validate_current()?;
            // Exactly this same borrowed input participates in the single prefix
            // construction/request check. The old session is immutable ancestry.
            self.prior
                .check_continued_effect_request(inputs, request, operation)?;
            self.validate_current()
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(crate) fn references(&self) -> Result<(u32, u32, u32, u32), SourceJournalError> {
        let (session, witness) = self.current.ok_or(SourceJournalError::Order)?;
        witness.validate_actual_predecessor(self.prior)?;
        let intent = session
            .sequence()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?;
        if self.references.2.checked_add(1) != Some(intent) {
            return Err(SourceJournalError::Binding);
        }
        Ok((
            self.references.0,
            self.references.1,
            self.references.2,
            intent,
        ))
    }
    pub(crate) fn matches_intent_row(&self, row: &SourceJournalEntry) -> bool {
        self.current.is_some_and(
            |(_, w)| matches!(w.selected_row(),EntryV8::Ordinary(actual) if actual==row),
        )
    }
    pub(crate) fn matches_commitments(&self, actual: &CheckedOwnedWaitReadyCommitmentsV8) -> bool {
        actual.authorization_binding() == self.commitments.authorization_binding()
            && actual.grant_digest() == self.commitments.grant_digest()
            && actual.target_grant_digest() == self.commitments.target_grant_digest()
            && actual.argument_digest() == self.commitments.argument_digest()
            && actual.budget() == self.commitments.budget()
    }
}
impl<'j> ContinuedResumedWaitV8<'j> {
    fn intent_permit<'p>(
        &'p self,
        prior: &'p AppendSessionV8<'j>,
        consumed: &'p VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        current: Option<(
            &'p AppendSessionV8<'j>,
            &'p VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        )>,
        proposal: &'p CheckedOwnedWaitProposalV8,
        commitments: &'p CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> LiveContinuedIntentPermitV8<'p, 'j> {
        LiveContinuedIntentPermitV8 {
            lineage: &self.lineage,
            prior,
            consumed,
            current,
            proposal,
            commitments,
            references,
            accounting: &self.accounting,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_intent_row(
        &self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        let permit = self.intent_permit(prior, consumed, None, proposal, commitments, references);
        permit.validate_current()?;
        match &self.outcome {
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Preparation(owner),
            )) => owner.intent_row(&permit),
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_intent_successor(
        &self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<(), SourceJournalError> {
        if let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Activation(owner),
        )) = &self.outcome
        {
            if let Some(error) = owner.selected_error() {
                return Err(error);
            }
        }
        let permit = self.intent_permit(
            prior,
            consumed,
            Some((session, witness)),
            proposal,
            commitments,
            references,
        );
        (|| {
            permit.validate_current()?;
            match &self.outcome {
                ContinuedResumeOutcomeV8::Authorization(
                    ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Preparation(
                        owner,
                    )),
                ) => owner.validate_intent_prepared(&permit),
                ContinuedResumeOutcomeV8::Authorization(
                    ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Activation(
                        owner,
                    )),
                ) => owner.validate_live(&permit),
                _ => Err(SourceJournalError::Order),
            }
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn activate_continued_intent(
        self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<Self, (Self, SourceJournalError)> {
        if let Err(error) = self.validate_continued_intent_successor(
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
            accounting,
            lineage,
        } = self;
        let ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
            ContinuedEffectOutcomeV8::Preparation(owner),
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
        let permit = LiveContinuedIntentPermitV8 {
            lineage: &lineage,
            prior,
            consumed,
            current: Some((session, witness)),
            proposal,
            commitments,
            references,
            accounting: &accounting,
        };
        match owner.activate_intent(&permit) {
            Ok(owner) => Ok(Self {
                outcome: ContinuedResumeOutcomeV8::Authorization(
                    ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Activation(
                        owner,
                    )),
                ),
                accounting,
                lineage,
            }),
            Err((owner, error)) => Err((
                Self {
                    outcome: ContinuedResumeOutcomeV8::Authorization(
                        ContinuedAuthorizationOutcomeV8::Effect(
                            ContinuedEffectOutcomeV8::Preparation(owner),
                        ),
                    ),
                    accounting,
                    lineage,
                },
                error,
            )),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_intent_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .validate_continued_intent_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_intent_registry(
        &self,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .advance_continued_intent_ack(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_activation(
        &self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<(), SourceJournalError> {
        if !matches!(
            &self.outcome,
            ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(
                ContinuedEffectOutcomeV8::Activation(_)
            ))
        ) {
            self.lineage.journal().quarantine();
            return Err(SourceJournalError::Order);
        }
        self.validate_continued_intent_successor(
            prior,
            consumed,
            session,
            witness,
            proposal,
            commitments,
            references,
        )
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_continued_activation_after(
        &self,
    ) -> bool {
        matches!(&self.outcome,ContinuedResumeOutcomeV8::Authorization(ContinuedAuthorizationOutcomeV8::Effect(ContinuedEffectOutcomeV8::Activation(owner))) if owner.test_after())
    }
}
