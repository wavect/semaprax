//! Checked retirement of the unique inherited Reduce hold after a physical
//! Complete Report claim. Terminal bytes alone cannot enter this transition.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveClaimedReportV8;
use crate::live_invocation::source_journal::SourceTerminalStatus;

impl Drop for ProspectiveOwnedReduceHoldV8<'_> {
    fn drop(&mut self) {
        if !self.terminal_completed.get() {
            self.journal.quarantine();
        }
    }
}

impl ProspectiveOwnedReduceHoldV8<'_> {
    /// The only successful terminal disposition. This consumes registry
    /// membership without refunding spent funding or creating another hold.
    /// The Report consumer immediately drops the now-settled lineage normally.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn complete_claimed_report(
        &self,
        report: &LiveClaimedReportV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let checked = (|| {
            if self.terminal_completed.get() || self.journal.append_active.get() {
                return Err(SourceJournalError::Order);
            }
            let (session, selected) = report.validate_reduce_retirement(self)?;
            if !std::ptr::eq(session.journal, self.journal) {
                return Err(SourceJournalError::Binding);
            }
            let EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
                status: SourceTerminalStatus::Complete,
                turn: Some(turn),
                carrier: Some(_),
                carrier_digest: Some(_),
                ..
            }) = selected
            else {
                return Err(SourceJournalError::Binding);
            };
            self.journal.validate_guard()?;
            let current = self.journal.begin_session()?;
            self.validate_step_inventory(
                self.journal,
                &current.inventory,
                session.sequence(),
                session.acknowledged_bytes(),
            )?;
            self.journal.validate_guard()?;
            let mut registry = self
                .journal
                .prospective_reduce
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
            let OwnedReduceHoldPhaseV8::Step {
                selected: actual, ..
            } = &record.phase
            else {
                return Err(SourceJournalError::Binding);
            };
            if record.identity != self.identity
                || record.turn != *turn
                || actual != selected
                || record.sequence != current.sequence()
                || record.bytes != current.acknowledged_bytes()
                || record.authentication != current.inventory.authentication_tail()
                || session.inventory.authentication_tail()
                    != current.inventory.authentication_tail()
            {
                return Err(SourceJournalError::Binding);
            }
            // Callback-free disposition under the registry borrow. Nothing
            // after this point can fail or run host code before normal Drop.
            self.terminal_completed.set(true);
            *registry = None;
            Ok(())
        })();
        checked.inspect_err(|_| self.journal.quarantine())
    }
}
