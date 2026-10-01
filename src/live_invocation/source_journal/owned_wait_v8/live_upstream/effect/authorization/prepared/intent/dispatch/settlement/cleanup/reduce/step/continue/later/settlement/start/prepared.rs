//! The later physical park remains behind its cumulative hold through Prepared.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedPreparedSuccessorV8;

fn guard_prepared(
    lineage: &LaterContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
) -> Result<(), SourceJournalError> {
    let journal = lineage.journal;
    let result = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        lineage.hold()?.validate_continued_prepared_guard(
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
            &lineage.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !lineage.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        let ordinary = journal.context().ordinary();
        check_clock_v8(
            &held,
            session.sequence(),
            session.acknowledged_bytes(),
            lineage.cancellation,
            lineage.source.continued_model_origin()?.1,
            ordinary.clock_domain(),
            ordinary.initial_millis(),
            ordinary.deadline_millis(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}

impl<'j> LaterStartedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn turn(&self) -> u32 {
        self.lineage.turn
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn encode_checkpoint(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
        key: &crate::resumable_effects::source_checkpoint::SourceCheckpointKey,
        expected: &crate::resumable_effects::owned_frame::v2::OwnedWaitCheckpointExpectationV8<'_>,
    ) -> Result<(Vec<u8>, String), SourceJournalError> {
        self.validate_live(session, witness)?;
        if expected.sequence
            != u64::try_from(session.sequence()).map_err(|_| SourceJournalError::Capacity)?
        {
            return Err(SourceJournalError::Binding);
        }
        let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
            return Err(SourceJournalError::Binding);
        };
        let journal = self.lineage.journal;
        let (_, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let permit = LiveWaitStartPermitV8 {
            held: journal.hold()?,
            fuel: execution.evaluation_fuel(),
            cancellation: self.lineage.cancellation,
        };
        let (bytes, checked) = owner
            .encode_checkpoint(&permit, execution.wait(), key, expected)
            .map_err(|_| SourceJournalError::Binding)?;
        self.validate_live(session, witness)?;
        Ok((bytes, checked.outer_digest().into()))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_prepared_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            guard_prepared(&self.lineage, session, witness)?;
            let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
                return Err(SourceJournalError::Binding);
            };
            let execution = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            owner
                .checked_facts(execution.wait())
                .ok_or(SourceJournalError::Binding)?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_prepared_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .validate_continued_prepared_append_prefix(journal, inventory, selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_prepared_registry(
        &self,
        witness: &VerifiedOwnedContinuedPreparedSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .advance_continued_prepared_ack(witness, session)
    }
}
