//! One live failed-receipt seal; independent retirement is never reset.
//! The sealed State writer leaves ordinary poison closed; no recovery or generic bypass.
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
    #[cfg(test)]
    pub(super) fn retired_for_test(&self) -> bool {
        self.retired.get()
    }
    fn healthy(&self) -> bool {
        !self.poisoned.get() && !self.retired.get() && self.active.get().is_none()
    }
}
struct ObserverCursorV8 {
    sequence: usize,
    bytes: usize,
    authentication: String,
}
/// Only actual source failed-receipt ACK consumption installs this move-only seal.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ObserverTerminalSealV8<'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    identity: u64,
    hold_identity: u64,
    fuel: u64,
    turn: u32,
    attempt: u32,
    cursor: RefCell<ObserverCursorV8>,
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
        proof.validate_original()?;
        let session = proof.session();
        let journal = session.journal;
        let prepared = (|| {
            let registry = journal
                .prospective_reduce
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
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
                hold_identity: record.identity,
                fuel: record.fuel,
                turn: record.turn,
                attempt: record.attempt,
                cursor: RefCell::new(ObserverCursorV8 {
                    sequence: session.sequence(),
                    bytes: session.acknowledged_bytes(),
                    authentication: session.inventory.authentication_tail().into(),
                }),
            })
        })();
        let seal = prepared.inspect_err(|_| journal.quarantine())?;
        // Allocation/callback/physical checks are complete before atomic mutation.
        let registry = journal.prospective_reduce.try_borrow().map_err(|_| {
            journal.quarantine();
            SourceJournalError::Order
        })?;
        let record = registry.as_ref().ok_or_else(|| {
            journal.quarantine();
            SourceJournalError::Binding
        })?;
        let cursor = seal.cursor.try_borrow().map_err(|_| {
            journal.quarantine();
            SourceJournalError::Order
        })?;
        if record.identity != seal.hold_identity
            || record.fuel != seal.fuel
            || record.turn != seal.turn
            || record.attempt != seal.attempt
            || record.sequence != cursor.sequence
            || record.bytes != cursor.bytes
            || record.authentication != cursor.authentication
            || !matches!(
                record.phase,
                owned_effect::OwnedReduceHoldPhaseV8::CleanupSettled { .. }
            )
            || journal.append_active.get()
            || !journal.poisoned.healthy()
            || journal.poisoned.identity.get().checked_add(1) != Some(seal.identity)
        {
            journal.quarantine();
            return Err(SourceJournalError::Poisoned);
        }
        journal.poisoned.identity.set(seal.identity);
        journal.poisoned.active.set(Some(seal.identity));
        journal.poisoned.poisoned.set(true); // Only the actual failed-receipt ACK installer avoids retirement.
        drop(cursor);
        drop(registry);
        Ok(seal)
    }
    fn validate_registry(&self) -> Result<(), SourceJournalError> {
        let journal = self.journal;
        if journal.poisoned.retired.get()
            || !journal.poisoned.get()
            || journal.poisoned.active.get() != Some(self.identity)
        {
            return Err(SourceJournalError::Poisoned);
        }
        let cursor = self
            .cursor
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let registry = journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
        if record.identity != self.hold_identity
            || record.fuel != self.fuel
            || record.turn != self.turn
            || record.attempt != self.attempt
            || record.sequence != cursor.sequence
            || record.bytes != cursor.bytes
            || record.authentication != cursor.authentication
            || !matches!(
                record.phase,
                owned_effect::OwnedReduceHoldPhaseV8::CleanupSettled { .. }
                    | owned_effect::OwnedReduceHoldPhaseV8::ObserverState { .. }
            )
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn recover_session(
        &self,
    ) -> Result<AppendSessionV8<'j>, SourceJournalError> {
        let result = (|| {
            self.validate_registry()?;
            if self.journal.append_active.get() {
                return Err(SourceJournalError::Order);
            }
            let mut lease = self
                .journal
                .lease
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            self.journal.context.validate_lease(&lease)?;
            let bytes = lease.read().map_err(store_error)?;
            let inventory =
                InventoryV8::recover(&self.journal.context, &lease, &self.journal.key, &bytes)?;
            let cursor = self
                .cursor
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            if inventory.sequence() != cursor.sequence
                || inventory.acknowledged_bytes() != cursor.bytes
                || inventory.authentication_tail() != cursor.authentication
            {
                return Err(SourceJournalError::Order);
            }
            self.journal.context.validate_lease(&lease)?;
            drop(cursor);
            drop(lease);
            self.validate_registry()?;
            Ok(AppendSessionV8 {
                journal: self.journal,
                inventory,
            })
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_guard(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.recover_session().map(|_| ())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !std::ptr::eq(self.journal, journal)
                || !inventory.belongs_to_context(&journal.context)
            {
                return Err(SourceJournalError::Binding);
            }
            self.validate_registry()?;
            let cursor = self
                .cursor
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            if inventory.sequence() != cursor.sequence
                || inventory.acknowledged_bytes() != cursor.bytes
                || inventory.authentication_tail() != cursor.authentication
            {
                return Err(SourceJournalError::Order);
            }
            let registry = journal
                .prospective_reduce
                .try_borrow()
                .map_err(|_| SourceJournalError::Order)?;
            let phase = &registry.as_ref().ok_or(SourceJournalError::Binding)?.phase;
            use super::super::model::OwnedBodyV8 as B;
            let allowed = match (phase, selected) {
                (
                    owned_effect::OwnedReduceHoldPhaseV8::CleanupSettled { .. },
                    EntryV8::Owned(B::OwnedEffectObserverFailureStateCleanupStarted { .. }),
                ) => true,
                (
                    owned_effect::OwnedReduceHoldPhaseV8::ObserverState {
                        selected:
                            EntryV8::Owned(B::OwnedEffectObserverFailureStateCleanupStarted { .. }),
                    },
                    EntryV8::Owned(B::OwnedEffectObserverFailureStateCleanupSettled { .. }),
                ) => true,
                (
                    owned_effect::OwnedReduceHoldPhaseV8::ObserverState {
                        selected:
                            EntryV8::Owned(B::OwnedEffectObserverFailureStateCleanupSettled {
                                receipt,
                                ..
                            }),
                    },
                    EntryV8::Ordinary(SourceJournalEntry::Stop { .. }),
                ) => receipt["settlement"] == "completed",
                _ => false,
            };
            if !allowed {
                return Err(SourceJournalError::Order);
            }
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Callback-free same-lease guard while the fixed adapter holds its marker.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_physical_guard(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_registry()?;
            if !self.journal.append_active.get() {
                return Err(SourceJournalError::Order);
            }
            self.journal.context.validate_lease(lease)?;
            self.validate_registry()
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
    /// Only the actual verified ACK witness/session can advance this cursor.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_ack(
        &self,
        witness: &super::owned_effect::VerifiedObserverFailedStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if !self.journal.append_active.get() || !std::ptr::eq(self.journal, session.journal) {
                return Err(SourceJournalError::Order);
            }
            self.validate_registry()?;
            witness.validate_against_acknowledged_session(session)?;
            let mut cursor = self
                .cursor
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            witness.validate_previous_registry(
                self.journal,
                cursor.sequence,
                cursor.bytes,
                &cursor.authentication,
            )?;
            if session.sequence()
                != cursor
                    .sequence
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?
                || session.acknowledged_bytes() <= cursor.bytes
            {
                return Err(SourceJournalError::Order);
            }
            // All fallible validation and inert allocations precede atomic updates.
            let selected = witness.selected_row().clone();
            let authentication = session.inventory.authentication_tail().to_owned();
            let cursor_authentication = authentication.clone();
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_mut().ok_or(SourceJournalError::Binding)?;
            if record.identity != self.hold_identity
                || record.fuel != self.fuel
                || record.sequence != cursor.sequence
                || record.bytes != cursor.bytes
                || record.authentication != cursor.authentication
            {
                return Err(SourceJournalError::Binding);
            }
            record.phase = owned_effect::OwnedReduceHoldPhaseV8::ObserverState { selected };
            record.sequence = session.sequence();
            record.bytes = session.acknowledged_bytes();
            record.authentication = authentication;
            cursor.sequence = record.sequence;
            cursor.bytes = record.bytes;
            cursor.authentication = cursor_authentication;
            Ok(())
        })();
        result.inspect_err(|_| self.journal.quarantine())
    }
}
#[cfg(all(test, unix))]
mod tests;
