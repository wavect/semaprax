//! The sole source checkpoint write cursor; store acknowledgment controls publication.
use super::*;

impl<'a> SourceCheckpointSink<'a> {
    pub fn new(store: &'a mut dyn CheckpointStore, binding: SourceInvocationBinding) -> Self {
        Self {
            store,
            journal: SourceJournal::new(binding),
            generation: 0,
            poisoned: false,
        }
    }
    pub fn resume(
        store: &'a mut dyn CheckpointStore,
        recovered: RecoveredSourceCheckpoint,
    ) -> Result<Self, SourceJournalError> {
        if recovered.is_uncertain() {
            return Err(SourceJournalError::Uncertain);
        }
        Ok(Self {
            store,
            journal: recovered.journal,
            generation: recovered.generation,
            poisoned: false,
        })
    }
    pub fn append_at(
        &mut self,
        entry: SourceJournalEntry,
        now: i64,
    ) -> Result<(), SourceJournalError> {
        let (next, generation, document) = self.prepare_append(entry, now)?;
        if let Err(error) = self.store.commit(generation, &document) {
            self.poisoned = true;
            return Err(SourceJournalError::Store(error));
        }
        self.journal = next;
        self.generation = generation;
        Ok(())
    }

    /// Checks phase, clock and bounded settlement capacity before a caller
    /// reserves budget. No store write or dispatch permission is produced;
    /// `append_at` revalidates and must acknowledge the actual intent first.
    pub fn preflight_at(
        &self,
        entry: &SourceJournalEntry,
        now: i64,
    ) -> Result<(), SourceJournalError> {
        self.prepare_append(entry.clone(), now).map(|_| ())
    }

    /// Constructs the profile-specific durable model intent.  The caller must
    /// still append it and await the checkpoint acknowledgement before dispatch.
    pub fn attempt_intent(
        &self,
        turn: u32,
        attempt: u32,
        request_digest: String,
        prompt_digest: String,
        request_bytes: usize,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        self.journal
            .attempt_intent(turn, attempt, request_digest, prompt_digest, request_bytes)
    }

    /// Constructs explicit V4 settlement evidence for a prior priced intent.
    /// `None` is serialized as `usage: unknown`; callers must pass
    /// `ProviderChargeObservation::Unknown` unless they hold exact bound
    /// currency/minor-unit evidence.
    pub fn priced_attempt_usage(
        &self,
        turn: u32,
        attempt: u32,
        reported: Option<SourceReportedUsage>,
        charge: crate::live_invocation::pricing::ProviderChargeObservation,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        self.journal
            .priced_attempt_usage(turn, attempt, reported, charge)
    }

    /// Constructs the final v2 event from validated causal commitments.
    /// The caller still must pass it to `append_at` and await its store ACK.
    pub fn terminal_snapshot_entry(
        &self,
        turn: Option<u32>,
        status: SourceTerminalStatus,
        carrier: Option<Vec<u8>>,
        input: SourceTerminalEvidenceInput,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        if !self.journal.binding.is_execution_profile() {
            return Err(SourceJournalError::Binding);
        }
        let fold = self.journal.execution_fold()?;
        execution::terminal_entry(&self.journal.binding, &fold, turn, status, carrier, input)
    }

    fn prepare_append(
        &self,
        entry: SourceJournalEntry,
        now: i64,
    ) -> Result<(SourceJournal, u64, String), SourceJournalError> {
        if self.poisoned {
            return Err(SourceJournalError::Poisoned);
        }
        let next = self.journal.candidate(entry, now)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(SourceJournalError::Generation)?;
        let document = wire::encode_envelope(&next, generation)?;
        if next.binding.wait.is_some() {
            wait_v7::check_capacity(&next, document.len())?;
            return Ok((next, generation, document));
        }
        // An intent is unusable if its worst-case bounded result cannot be
        // checkpointed. Reserve room before granting a physical dispatch.
        let (future_bytes, future_entries): (usize, usize) = match next.entries.last() {
            Some(SourceJournalEntry::MigrationEvaluationIntent { .. }) => (
                MAX_SOURCE_CARRIER_BYTES
                    .saturating_mul(2)
                    .saturating_add(4_096),
                2,
            ),
            Some(SourceJournalEntry::AttemptIntent { response_limit, .. }) => {
                (response_limit.saturating_mul(2).saturating_add(4_096), 5)
            }
            Some(SourceJournalEntry::PricedAttemptIntent(intent)) => (
                intent
                    .response_limit
                    .saturating_mul(2)
                    .saturating_add(4_096),
                5,
            ),
            Some(SourceJournalEntry::PolicyAttemptIntent(intent)) => (
                intent
                    .response_limit
                    .saturating_mul(2)
                    .saturating_add(4_096),
                5,
            ),
            Some(SourceJournalEntry::EffectIntent { .. }) => (
                MAX_SOURCE_EFFECT_BYTES
                    .saturating_mul(2)
                    .saturating_add(4_096),
                3,
            ),
            _ => (0, 0),
        };
        let (future_bytes, future_entries) = if next.binding.is_execution_profile()
            && !matches!(
                next.entries.last(),
                Some(SourceJournalEntry::TerminalSnapshot { .. })
            ) {
            (
                future_bytes.saturating_add(execution::TERMINAL_ROOM_BYTES),
                future_entries.saturating_add(2),
            )
        } else {
            (future_bytes, future_entries)
        };
        if document
            .len()
            .checked_add(future_bytes)
            .is_none_or(|size| size > MAX_SOURCE_DOCUMENT_BYTES)
            || next
                .entries
                .len()
                .checked_add(future_entries)
                .is_none_or(|count| count > MAX_SOURCE_ENTRIES)
        {
            return Err(SourceJournalError::Capacity);
        }
        Ok((next, generation, document))
    }
    pub fn journal(&self) -> &SourceJournal {
        &self.journal
    }
    /// Revalidates this cursor's last ACKed generation. A poisoned cursor may
    /// have a newer store generation, so this is not a latest-store claim.
    pub fn checkpoint(&self) -> Result<RecoveredSourceCheckpoint, SourceJournalError> {
        let document = wire::encode_envelope(&self.journal, self.generation)?;
        recover_source_checkpoint(&document, &self.journal.binding)
    }
    pub fn committed_stage_fuel(&self) -> Result<u64, SourceJournalError> {
        if self.journal.binding.is_execution_profile() {
            Ok(self.journal.execution_fold()?.stage_fuel)
        } else {
            Ok(0)
        }
    }
    pub fn io_totals(&self) -> Result<Option<SourceIoTotals>, SourceJournalError> {
        if self.journal.binding.io.is_none() {
            return Ok(None);
        }
        Ok(self.journal.execution_fold()?.io)
    }
    pub fn priced_totals(&self) -> Result<Option<PricedTotalsV4>, SourceJournalError> {
        if !self.journal.binding.is_priced_profile() {
            return Ok(None);
        }
        Ok(self.journal.execution_fold()?.priced)
    }
    pub fn policy_totals(&self) -> Result<Option<SourcePolicyTotalsV6>, SourceJournalError> {
        if self.journal.binding.policy_binding().is_none() {
            return Ok(None);
        }
        Ok(self
            .journal
            .execution_fold()?
            .policy
            .map(|fold| fold.totals))
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub const fn poisoned(&self) -> bool {
        self.poisoned
    }
}
