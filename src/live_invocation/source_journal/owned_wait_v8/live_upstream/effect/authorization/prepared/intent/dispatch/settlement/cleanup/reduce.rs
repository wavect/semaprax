//! Consuming successful physical Outcome to the original Reduce reservation.
//! Selection is inert until the fixed same-hold adapter acknowledges this row.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetEvidence;
use crate::live_invocation::source_journal::SourceStageRole;
use crate::resumable_effects::owned_frame::v2::{compile_owned_reduce_v2, CheckedOwnedReduceV2};

/// The actual Executed owner is first; its ledger and SAME held store remain
/// inside it. No facts/JSON admission or recovered-prefix constructor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedReduceReservationAppendV8<
    'j,
> {
    owner: LiveExecutedOwnedEffectV8<'j>,
    plan: CheckedOwnedReduceV2,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReduceReservationRejectionV8<
    'j,
> {
    _owner: LiveExecutedOwnedEffectV8<'j>,
    error: SourceJournalError,
}
impl<'j> LiveExecutedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_reduce(
        self,
    ) -> Result<LiveOwnedReduceReservationAppendV8<'j>, LiveReduceReservationRejectionV8<'j>> {
        let selected = (|| {
            self.validate_live()?;
            let origin = &self.lineage.recorded.intent;
            let journal = origin.journal;
            let (_, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = compile_owned_reduce_v2(execution.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            let fuel = execution.evaluation_fuel();
            if Some(fuel) != journal.context().ordinary().max_steps_per_stage()
                || plan.binding() != execution.wait().binding()
                || !plan.helper().same_helper(execution.wait().helper())
            {
                return Err(SourceJournalError::Binding);
            }
            // The actual ledger was moved through dispatch and every ACK. The
            // authenticated target evidence is comparison data, not its source.
            let evidence = TargetEvidence::decode(self.lineage.recorded.facts.evidence())
                .map_err(|_| SourceJournalError::Binding)?;
            if evidence.accounting() != self.accounting
                || !matches!(
                    self.lineage.recorded.facts.ordinary(),
                    SourceJournalEntry::EffectObserved {
                        turn: 0,
                        attempt: 0,
                        ..
                    }
                )
                || self.lineage.settled.is_none()
            {
                return Err(SourceJournalError::Binding);
            }
            let selected = EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn: 0,
                attempt: Some(0),
                role: SourceStageRole::Reduce,
                fuel,
            });
            self.validate_live()?;
            Ok((plan, selected))
        })();
        match selected {
            Ok((plan, selected)) => Ok(LiveOwnedReduceReservationAppendV8 {
                owner: self,
                plan,
                selected,
            }),
            Err(error) => {
                self.lineage.recorded.intent.journal.quarantine();
                Err(LiveReduceReservationRejectionV8 {
                    _owner: self,
                    error,
                })
            }
        }
    }
}
impl<'j> LiveOwnedReduceReservationAppendV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.lineage.recorded.intent.journal
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.lineage.current().session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.lineage.current().session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.owner.validate_live()?;
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            if self.plan.binding() != execution.wait().binding()
                || !self.plan.helper().same_helper(execution.wait().helper())
                || self.selected
                    != EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                        turn: 0,
                        attempt: Some(0),
                        role: SourceStageRole::Reduce,
                        fuel: execution.evaluation_fuel(),
                    })
            {
                return Err(SourceJournalError::Binding);
            }
            self.owner.validate_live()
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
}

#[cfg(test)]
mod tests;
