//! One live failed-receipt seal; independent retirement is never reset.
//! No terminal writer, raw ACK factory, owner recovery, or generic bypass exists.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::observer_failed_state::FailedDecisionReceiptSealProofV8;

/// No `set` method: forgotten raw poison writes fail compilation after migration.
pub(super) struct ObserverRetirementV8 {
    poisoned: Cell<bool>,
    retired: Cell<bool>,
    identity: Cell<u64>,
    active: Cell<Option<u64>>,
}
impl ObserverRetirementV8 {
    pub(super) fn new() -> Self {
        Self {
            poisoned: Cell::new(false),
            retired: Cell::new(false),
            identity: Cell::new(0),
            active: Cell::new(None),
        }
    }
    pub(super) fn get(&self) -> bool {
        self.poisoned.get()
    }
    pub(super) fn retire(&self) {
        self.retired.set(true);
        self.poisoned.set(true);
    }
    fn healthy(&self) -> bool {
        !self.poisoned.get() && !self.retired.get() && self.active.get().is_none()
    }
}
/// Only actual source failed-receipt ACK consumption installs this move-only seal.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ObserverTerminalSealV8<'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    identity: u64,
    sequence: usize,
    bytes: usize,
    authentication: String,
    hold_identity: u64,
    fuel: u64,
    turn: u32,
    attempt: u32,
}
impl Drop for ObserverTerminalSealV8<'_> {
    fn drop(&mut self) {
        self.journal.quarantine();
    }
}
impl<'j> ObserverTerminalSealV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn install(
        proof: &FailedDecisionReceiptSealProofV8<'_, 'j>,
    ) -> Result<Self, SourceJournalError> {
        // All callbacks/physical validation precede atomic mutation. Constructor
        // accepts a sealed actual owner borrower, never raw cursor/history.
        proof.validate_original()?;
        let session = proof.session();
        let journal = session.journal;
        let prepared = (|| {
            let record = journal
                .prospective_reduce
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            let record = record.as_ref().ok_or(SourceJournalError::Binding)?;
            if record.sequence != session.sequence()
                || record.bytes != session.acknowledged_bytes()
                || record.authentication != session.inventory.authentication_tail()
                || !matches!(
                    record.phase,
                    owned_effect::OwnedReduceHoldPhaseV8::CleanupSettled { .. }
                )
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(Self {
                journal,
                identity: journal
                    .poisoned
                    .identity
                    .get()
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?,
                sequence: session.sequence(),
                bytes: session.acknowledged_bytes(),
                authentication: session.inventory.authentication_tail().into(),
                hold_identity: record.identity,
                fuel: record.fuel,
                turn: record.turn,
                attempt: record.attempt,
            })
        })();
        let seal = match prepared {
            Ok(s) => s,
            Err(e) => {
                journal.quarantine();
                return Err(e);
            }
        };
        let registry = journal.prospective_reduce.try_borrow().map_err(|_| {
            journal.quarantine();
            SourceJournalError::Order
        })?;
        let record = registry.as_ref().ok_or_else(|| {
            journal.quarantine();
            SourceJournalError::Binding
        })?;
        if record.identity != seal.hold_identity
            || record.fuel != seal.fuel
            || record.turn != seal.turn
            || record.attempt != seal.attempt
            || record.sequence != seal.sequence
            || record.bytes != seal.bytes
            || record.authentication != seal.authentication
            || !matches!(
                record.phase,
                owned_effect::OwnedReduceHoldPhaseV8::CleanupSettled { .. }
            )
        {
            journal.quarantine();
            return Err(SourceJournalError::Binding);
        }
        // No external call, file access or allocation occurs after this guard.
        if journal.append_active.get()
            || !journal.poisoned.healthy()
            || journal.poisoned.identity.get().checked_add(1) != Some(seal.identity)
        {
            journal.quarantine();
            return Err(SourceJournalError::Poisoned);
        }
        journal.poisoned.identity.set(seal.identity);
        journal.poisoned.active.set(Some(seal.identity));
        journal.poisoned.poisoned.set(true); // Sole nonretiring actual-ACK installation.
        Ok(seal)
    }
    fn validate_data(&self) -> Result<(), SourceJournalError> {
        let journal = self.journal;
        if journal.poisoned.retired.get()
            || !journal.poisoned.get()
            || journal.poisoned.active.get() != Some(self.identity)
            || journal.append_active.get()
        {
            return Err(SourceJournalError::Poisoned);
        }
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        if record.identity != self.hold_identity
            || record.fuel != self.fuel
            || record.turn != self.turn
            || record.attempt != self.attempt
            || record.sequence != self.sequence
            || record.bytes != self.bytes
            || record.authentication != self.authentication
            || !matches!(
                record.phase,
                owned_effect::OwnedReduceHoldPhaseV8::CleanupSettled { .. }
            )
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_guard(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_data()?; // Retirement precedes every physical read.
            let mut lease = self
                .journal
                .lease
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            self.journal.context.validate_lease(&lease)?;
            let bytes = lease.read().map_err(store_error)?;
            let inventory =
                InventoryV8::recover(&self.journal.context, &lease, &self.journal.key, &bytes)?;
            if inventory.sequence() != self.sequence
                || inventory.acknowledged_bytes() != self.bytes
                || inventory.authentication_tail() != self.authentication
            {
                return Err(SourceJournalError::Order);
            }
            self.journal.context.validate_lease(&lease)?;
            drop(lease);
            self.validate_data()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}

#[cfg(test)]
mod tests;
