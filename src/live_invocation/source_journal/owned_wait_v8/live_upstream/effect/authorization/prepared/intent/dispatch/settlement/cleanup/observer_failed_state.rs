//! Actual failed-receipt seal installation. No State release or terminal writer.
//! The production constructor is called only by the actual cleanup ACK consumer.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::observer_terminal::ObserverTerminalSealV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FailedDecisionReceiptSealProofV8<
    'p,
    'j,
> {
    owner: &'p LiveCleanedOwnedEffectV8<'j>,
}
impl<'j> FailedDecisionReceiptSealProofV8<'_, 'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn session(
        &self,
    ) -> &AppendSessionV8<'j> {
        &self.owner.lineage.current().session
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_original(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner.lineage.validate_cleanup()?;
        let ack = self
            .owner
            .lineage
            .settled
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        ack.witness.validate_current_session(&ack.session)?;
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled { receipt, .. }) =
            ack.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        if receipt != self.owner.pending.receipt() || receipt["settlement"] != "failed" {
            return Err(SourceJournalError::Binding);
        }
        self.owner.pending.live_observer_failed_state_facts_v8()?;
        self.owner.lineage.validate_cleanup()
    }
}
impl<'j> LiveCleanedOwnedEffectV8<'j> {
    /// Actual ACK consumer attaches once, before returning this surviving owner.
    pub(super) fn install_observer_seal(&mut self) -> Result<(), SourceJournalError> {
        if self.pending.receipt()["settlement"] == "completed" {
            return Ok(());
        }
        if self.observer_seal.is_some() {
            self.lineage.recorded.intent.journal.quarantine();
            return Err(SourceJournalError::Order);
        }
        let proof = FailedDecisionReceiptSealProofV8 { owner: self };
        let seal = ObserverTerminalSealV8::install(&proof)?;
        self.observer_seal = Some(seal);
        Ok(())
    }
    pub(super) fn validate_observer_seal(&self) -> Result<(), SourceJournalError> {
        validate_sealed_owner(
            &self.pending,
            &self.lineage,
            self.observer_seal
                .as_ref()
                .ok_or(SourceJournalError::Binding)?,
        )
    }
}
impl LiveFailedOwnedEffectV8<'_> {
    /// Prospective State intent/Stop guard, distinct from incurred receipt checks.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_observer_state_intent(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.validate_observer_terminal()?;
            let origin = &self.lineage.recorded.intent;
            if origin.cancellation.is_cancelled() {
                return Err(SourceJournalError::Binding);
            }
            let ordinary = origin.journal.context().ordinary();
            let domain = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                origin.clock.clock_domain()
            }))
            .map_err(|_| SourceJournalError::Poisoned)?;
            self.validate_observer_terminal()?;
            if origin.cancellation.is_cancelled() || domain != ordinary.clock_domain() {
                return Err(SourceJournalError::Binding);
            }
            let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                origin.clock.now_millis()
            }))
            .map_err(|_| SourceJournalError::Poisoned)?;
            self.validate_observer_terminal()?;
            if origin.cancellation.is_cancelled() {
                return Err(SourceJournalError::Binding);
            }
            if now < ordinary.initial_millis() || now >= ordinary.deadline_millis() {
                return Err(SourceJournalError::Time);
            }
            Ok(())
        })();
        result.inspect_err(|_| self.lineage.recorded.intent.journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_observer_terminal(
        &self,
    ) -> Result<(), SourceJournalError> {
        validate_sealed_owner(
            &self.pending,
            &self.lineage,
            self.observer_seal
                .as_ref()
                .ok_or(SourceJournalError::Binding)?,
        )
    }
}
fn validate_sealed_owner(
    pending: &PendingOwnedEffectReceiptV8<'_>,
    lineage: &CleanupLineageV8<'_>,
    seal: &ObserverTerminalSealV8<'_>,
) -> Result<(), SourceJournalError> {
    let result = (|| {
        seal.validate_guard()?;
        pending.live_observer_failed_state_facts_v8()?;
        let origin = &lineage.recorded.intent;
        let journal = origin.journal;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &journal.context().registration().expected_facts().scope,
            &origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        seal.validate_guard()
    })();
    result.inspect_err(|_| lineage.recorded.intent.journal.quarantine())
}
