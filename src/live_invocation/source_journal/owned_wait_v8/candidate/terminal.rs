//! Authenticated terminal construction and terminal Step cursor facts.
use super::*;

/// Copies checked terminal data only. It carries no physical owner, append
/// cursor, effect grant, or result-delivery right.
pub(crate) struct CheckedOwnedTerminalEvidenceV8 {
    status: super::super::super::SourceTerminalStatus,
    evidence: Vec<u8>,
    carrier: Option<Vec<u8>>,
}
impl CheckedOwnedTerminalEvidenceV8 {
    pub(crate) fn status(&self) -> super::super::super::SourceTerminalStatus {
        self.status
    }
    pub(crate) fn evidence(&self) -> &[u8] {
        &self.evidence
    }
    pub(crate) fn carrier(&self) -> Option<&[u8]> {
        self.carrier.as_deref()
    }
}

impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn terminal_evidence(
        &self,
    ) -> Result<CheckedOwnedTerminalEvidenceV8, SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        if folded.tail != fold::TailV8::Terminal {
            return Err(SourceJournalError::Order);
        }
        let Some(ValidatedEntryV8 {
            entry:
                EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
                    status,
                    evidence,
                    carrier,
                    ..
                }),
            ..
        }) = self.entries.last()
        else {
            return Err(SourceJournalError::Order);
        };
        Ok(CheckedOwnedTerminalEvidenceV8 {
            status: *status,
            evidence: evidence.clone(),
            carrier: carrier.clone(),
        })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn terminal_entry(
        &self,
        turn: u32,
        status: super::super::super::SourceTerminalStatus,
        carrier: Vec<u8>,
        input: super::super::super::SourceTerminalEvidenceInput,
    ) -> Result<EntryV8, SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if folded.tail != fold::TailV8::Reduce
            || folded.current_turn() != turn
            || folded.reduce_fold().is_none_or(|r| {
                r.tail() != super::super::reduce_fold::ReduceTailV8::TerminalPending
            })
            || input.completed_stages != folded.stages
        {
            return Err(SourceJournalError::Order);
        }
        let projected = folded.ordinary_projection()?;
        let ordinary = super::super::super::execution::validate_inner_seeded(
            &context.ordinary,
            &projected,
            folded.wait_fuel(),
            if context.initialized_task.is_some() {
                super::super::super::validate::InitialStage::InitializeThenObserve
            } else {
                super::super::super::validate::InitialStage::ObserveOnly
            },
        )?;
        Ok(EntryV8::Ordinary(
            super::super::super::execution::terminal_entry(
                &context.ordinary,
                &ordinary,
                Some(turn),
                status,
                Some(carrier),
                input,
            )?,
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn step_reduce_facts(
        &self,
    ) -> Result<(u64, u32, u32, u32, &EntryV8), SourceJournalError> {
        let context = self.context.fold();
        let folded = fold::fold(context, &self.entries)?;
        if !matches!(folded.tail, fold::TailV8::Reduce | fold::TailV8::Terminal)
            || folded.reduce_fold().is_none()
        {
            return Err(SourceJournalError::Order);
        }
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let (turn, attempt) = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedReduceStaged { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedReduceCleanupStarted { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedReduceCleanupSettled { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedStepTransferReserved { turn, attempt, .. }
                | model::OwnedBodyV8::OwnedStepTransferCompleted { turn, attempt, .. },
            ) => (*turn, *attempt),
            EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn,
                attempt: Some(attempt),
                role: super::super::super::SourceStageRole::Reduce,
                fuel,
            }) if Some(*fuel) == context.ordinary.max_steps_per_stage() => (*turn, *attempt),
            EntryV8::Ordinary(SourceJournalEntry::Transition { turn, attempt, .. }) => {
                (*turn, *attempt)
            }
            EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn: Some(turn),
                attempt: Some(attempt),
                ..
            }) => (*turn, *attempt),
            EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
                turn: Some(turn), ..
            }) if folded.tail == fold::TailV8::Terminal => {
                let Some(ValidatedEntryV8 {
                    entry: EntryV8::Ordinary(SourceJournalEntry::Transition { attempt, .. }),
                    ..
                }) = self.entries.iter().rev().nth(1)
                else {
                    return Err(SourceJournalError::Order);
                };
                (*turn, *attempt)
            }
            _ => return Err(SourceJournalError::Order),
        };
        if turn != folded.current_turn() {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(context, &folded)?.check(self.document.len(), self.entries.len())?;
        Ok((
            folded.reserved_total,
            folded.stages,
            turn,
            attempt,
            selected,
        ))
    }
}
