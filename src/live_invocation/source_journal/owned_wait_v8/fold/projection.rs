//! Ordinary execution projection over already authenticated owned rows.
use super::*;

impl FoldV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn ordinary_projection(
        &self,
    ) -> Result<Vec<SourceJournalEntry>, SourceJournalError> {
        let mut projected = self.ordinary.clone();
        observe_settlement::project_initial_failed_stop(self, &mut projected)?;
        for entry in &mut projected {
            if let SourceJournalEntry::ReplayStageReservation { causal_seq, .. } = entry {
                *causal_seq = u32::try_from(
                    self.ordinary_sequences
                        .iter()
                        .position(|seq| *seq == *causal_seq)
                        .ok_or(SourceJournalError::Order)?,
                )
                .map_err(|_| SourceJournalError::Capacity)?;
            }
        }
        Ok(projected)
    }
}
