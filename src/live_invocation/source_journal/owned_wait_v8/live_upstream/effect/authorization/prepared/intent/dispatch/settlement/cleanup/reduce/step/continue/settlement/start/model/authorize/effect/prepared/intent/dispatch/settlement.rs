//! Current settlement authority; preceding accounting history stays inert.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    CheckedOwnedEffectSettlementV8, OwnedEffectSettlementInputsV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::effect::CheckedLiveOwnedEffectSettlementV8;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedSettlementSuccessorV8;
fn guard_settlement(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedSettlementSuccessorV8<'_>,
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
        origin.hold.validate_continued_settlement_guard(
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
                origin.hold.validate_continued_settlement_guard(
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
        origin.hold.validate_continued_settlement_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}

pub(crate) struct LiveContinuedSettlementPermitV8<'p, 'j> {
    intent: LiveContinuedIntentPermitV8<'p, 'j>,
    previous: Option<(
        &'p AppendSessionV8<'j>,
        &'p VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
    )>,
    current: Option<(
        &'p AppendSessionV8<'j>,
        &'p VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
    )>,
    actual_accounting: &'p TargetAccounting,
}
impl LiveContinuedSettlementPermitV8<'_, '_> {
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        (||{
      let l=&self.intent;let journal=l.lineage.journal();
      l.consumed.validate_against_acknowledged_session(l.prior)?;
      if l.references.0.checked_add(1)!=Some(l.references.1)
       || l.references.1.checked_add(1)!=Some(l.references.2)
       || usize::try_from(l.references.2).ok().and_then(|n|n.checked_add(1))!=Some(l.prior.sequence())
       || !matches!(l.consumed.selected_row(), EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed{turn,attempt:0,grant_digest}) if *turn==l.lineage.turn && grant_digest==l.commitments.grant_digest()) {return Err(SourceJournalError::Binding);}

      let (intent_session,intent_witness)=l.current.ok_or(SourceJournalError::Order)?;
      intent_witness.validate_actual_predecessor(l.prior)?;
      intent_witness.validate_against_acknowledged_session(intent_session)?;
      if l.prior.continued_effect_accounting()?!=*l.accounting{return Err(SourceJournalError::Binding);}
      let (_,_,consumed,intent)=l.references()?;
      if consumed.checked_add(1)!=Some(intent){return Err(SourceJournalError::Binding);}
      match self.current{
        None=>{if self.previous.is_some(){return Err(SourceJournalError::Order);}l.validate_current()},
        Some((session,witness))=>{
          if let Some((previous,prior_witness))=self.previous{
            prior_witness.validate_actual_predecessor(intent_session)?;prior_witness.validate_against_acknowledged_session(previous)?;
            if !matches!(prior_witness.selected_row(),EntryV8::Ordinary(SourceJournalEntry::EffectObserved{..}|SourceJournalEntry::EffectFailed{..})){return Err(SourceJournalError::Binding);}
            witness.validate_actual_predecessor(previous)?;
            if !matches!(witness.selected_row(),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded{intent:i,settlement,..}) if *i==intent && usize::try_from(*settlement).ok().and_then(|n|n.checked_add(1))==Some(previous.sequence())){return Err(SourceJournalError::Binding);}
            if session.continued_settlement_accounting()?!=*self.actual_accounting{return Err(SourceJournalError::Binding);}
          }else{
            witness.validate_actual_predecessor(intent_session)?;
            if !matches!(witness.selected_row(),EntryV8::Ordinary(SourceJournalEntry::EffectObserved{..}|SourceJournalEntry::EffectFailed{..})) || session.continued_settlement_accounting()?!=*l.accounting{return Err(SourceJournalError::Binding);}
          }
          witness.validate_current_session(session)?;
          guard_settlement(l.lineage,session,witness,l.proposal,true)
        }
      }
    })().inspect_err(|_|self.intent.lineage.journal().quarantine())
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.validate_current()?;
            let l = &self.intent;
            let journal = l.lineage.journal();
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let origin = l.lineage.step.origin();
            let held = journal.hold()?;
            if !std::ptr::eq(runtime, inputs.runtime)
                || !std::ptr::eq(execution, inputs.execution)
                || !std::ptr::eq(origin.policy, inputs.policy)
                || !std::ptr::eq(origin.cancellation, inputs.cancellation)
                || !held.same_container(&inputs.store)
                || inputs.turn != l.lineage.turn
                || inputs.attempt != 0
                || inputs.proposal.carrier() != l.proposal.carrier()
                || inputs.proposal.ordinary_digest() != l.proposal.ordinary_digest()
            {
                return Err(SourceJournalError::Binding);
            }
            let session = self
                .current
                .map_or(l.current.ok_or(SourceJournalError::Order)?.0, |x| x.0);
            inputs
                .store
                .validate_prefix(session.sequence(), session.acknowledged_bytes())?;
            self.validate_current()
        })()
        .inspect_err(|_| self.intent.lineage.journal().quarantine())
    }
    pub(crate) fn check_settlement(
        &self,
        inputs: OwnedEffectSettlementInputsV8<'_>,
        ordinary: &SourceJournalEntry,
        evidence: &[u8],
        result: Option<&[u8]>,
    ) -> Result<CheckedOwnedEffectSettlementV8, SourceJournalError> {
        self.validate_current()?;
        let checked = self
            .intent
            .prior
            .check_continued_effect_settlement(inputs, ordinary, evidence, result)?;
        if checked.evidence().accounting() != *self.actual_accounting {
            return Err(SourceJournalError::Binding);
        }
        self.validate_current()?;
        Ok(checked)
    }
}
impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_settlement_facts(
        &self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        intent_session: &AppendSessionV8<'j>,
        intent_witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
        previous: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
        current: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
    ) -> Result<CheckedLiveOwnedEffectSettlementV8, SourceJournalError> {
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
            if !owner.accounting_matches(preceding, &self.accounting) {
                return Err(SourceJournalError::Binding);
            }
            let intent = LiveContinuedIntentPermitV8 {
                lineage: &self.lineage,
                prior,
                consumed,
                current: Some((intent_session, intent_witness)),
                proposal,
                commitments,
                references,
                accounting: preceding,
            };
            let permit = LiveContinuedSettlementPermitV8 {
                intent,
                previous,
                current,
                actual_accounting: &self.accounting,
            };
            permit.validate_current()?;
            let facts = owner.settlement(&self.accounting, &permit)?;
            permit.validate_current()?;
            Ok(facts)
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_cleanup_values(
        &self,
        prior: &AppendSessionV8<'j>,
        consumed: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        intent_session: &AppendSessionV8<'j>,
        intent_witness: &VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
        previous: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
        current: Option<(
            &AppendSessionV8<'j>,
            &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        )>,
    ) -> Result<(serde_json::Value, serde_json::Value), SourceJournalError> {
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
            if !owner.accounting_matches(preceding, &self.accounting) {
                return Err(SourceJournalError::Binding);
            }
            let intent = LiveContinuedIntentPermitV8 {
                lineage: &self.lineage,
                prior,
                consumed,
                current: Some((intent_session, intent_witness)),
                proposal,
                commitments,
                references,
                accounting: preceding,
            };
            let permit = LiveContinuedSettlementPermitV8 {
                intent,
                previous,
                current,
                actual_accounting: &self.accounting,
            };
            permit.validate_current()?;
            let facts = owner.decision_cleanup_values(&permit)?;
            permit.validate_current()?;
            Ok(facts)
        })()
        .inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_settlement_append_prefix(
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
            .validate_continued_settlement_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continued_settlement_registry(
        &self,
        witness: &VerifiedOwnedContinuedSettlementSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .step
            .origin()
            .hold
            .advance_continued_settlement_ack(witness, session)
    }
}

impl<'j> ContinuedResumedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn cleanup_hold(&self) -> &crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::ProspectiveOwnedReduceHoldV8<'j> {
        &self.lineage.step.origin().hold
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_cleanup_guard(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedEffectCleanupSuccessorV8<'_>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Result<(), SourceJournalError> {
        (|| {
            let journal = self.lineage.journal();
            if !session.belongs_to(journal) { return Err(SourceJournalError::Binding); }
            witness.validate_current_session(session)?;
            self.cleanup_hold().validate_cleanup_guard(journal, session.sequence(), session.acknowledged_bytes())?;
            let held = journal.hold()?;
            let (runtime, execution) = journal.context().ready_runtime().ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(runtime, execution, &held.registration().expected_facts().scope, proposal).map_err(|_| SourceJournalError::Binding)?;
            if !self.lineage.step.origin().policy.allows(plan.operation().effect_id()) { return Err(SourceJournalError::Binding); }
            // Cleanup incurred at Started keeps physical/policy authority, even
            // when cancellation or deadline prevents later source work.
            held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
            self.cleanup_hold().validate_cleanup_guard(journal, session.sequence(), session.acknowledged_bytes())?;
            witness.validate_current_session(session)
        })().inspect_err(|_| self.lineage.journal().quarantine())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod cleanup;
