//! A terminal ACK can claim only the actual mapped Complete Report once.
//! The terminal row and recovered evidence remain proof data, never owners.
mod driver;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use driver::{
    LiveContinuedTerminalDriverFailureV8, LiveContinuedTerminalPhaseV8,
};
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::ClaimedExecutedOwnedReportV2;
use crate::resumable_effects::owned_frame::v2::compile_owned_reduce_v2;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveClaimedReportV8<'j> {
    owner: LiveContinuedStagedStepV8<'j>,
    report: ClaimedExecutedOwnedReportV2<'j>,
}

impl LiveClaimedReportV8<'_> {
    /// Borrows a delivery projection while retaining the original Report and
    /// the authenticated terminal owner. No JSON value can recreate either.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn delivery_projection(
        &self,
    ) -> Result<serde_json::Value, SourceJournalError> {
        self.owner.validate_live()?;
        let actual = self.report.live_report_v8()?;
        let (_, witness) = self
            .owner
            .terminal_ack
            .as_ref()
            .ok_or(SourceJournalError::Order)?;
        let EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
            status: crate::live_invocation::source_journal::SourceTerminalStatus::Complete,
            carrier: Some(carrier),
            ..
        }) = witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        let (_, execution) = self
            .owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan =
            compile_owned_reduce_v2(execution.wait()).map_err(|_| SourceJournalError::Binding)?;
        let step = self.owner.checked_step()?;
        step.matches_target(&actual)?;
        if step.ordinary_carrier_bytes(&plan)? != *carrier {
            return Err(SourceJournalError::Binding);
        }
        Ok(actual)
    }
}

impl<'j> LiveContinuedStagedStepV8<'j> {
    /// Consumes the acknowledged terminal owner. Failure returns that same
    /// owner; no Report field moves before every journal and carrier check.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn claim_complete_report(
        mut self,
    ) -> Result<LiveClaimedReportV8<'j>, (Self, SourceJournalError)> {
        let checked = (|| {
            self.validate_live()?;
            let (session, witness) = self
                .terminal_ack
                .as_ref()
                .ok_or(SourceJournalError::Order)?;
            witness.validate_current_session(session)?;
            let EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
                turn: Some(turn),
                status: crate::live_invocation::source_journal::SourceTerminalStatus::Complete,
                carrier_digest: Some(digest),
                carrier: Some(carrier),
                ..
            }) = witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            if *turn != self.coordinates()?.0 || self.kind() != Some("complete") {
                return Err(SourceJournalError::Binding);
            }
            let held = self.held.as_ref().ok_or(SourceJournalError::Binding)?;
            let target = held.live_target_v8()?;
            let step = self.checked_step()?;
            step.matches_target(&target)?;
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = compile_owned_reduce_v2(execution.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            let actual = step.ordinary_carrier_bytes(&plan)?;
            if &actual != carrier
                || crate::live_invocation::identity::digest(
                    b"semaprax.agent-step.value.v2\0",
                    &actual,
                ) != *digest
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        if let Err(error) = checked {
            if error != SourceJournalError::Order {
                self.journal().quarantine();
            }
            return Err((self, error));
        }
        let held = self.held.take().expect("checked mapped Complete owner");
        match held.claim_complete_report() {
            Ok(report) => Ok(LiveClaimedReportV8 {
                owner: self,
                report,
            }),
            Err(held) => {
                self.held = Some(held);
                self.journal().quarantine();
                Err((self, SourceJournalError::Binding))
            }
        }
    }
}
