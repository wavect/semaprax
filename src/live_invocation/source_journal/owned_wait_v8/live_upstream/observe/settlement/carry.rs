//! Live continued State, its ledger and spent token stay in one authenticated
//! owner while the next wait is prepared. This child grants no source entry.
use super::*;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8;

/// The complete actual State-containing predecessor is first; its existing
/// ledger and held token remain nested in their original drop order.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedWaitV8<'j> {
    owner: LiveSettledObserveV8<'j>,
    observation: CheckedOwnedWaitObservationV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveTurnCarryFailureV8<'j> {
    owner: LiveSettledObserveV8<'j>,
    error: SourceJournalError,
}
impl<'j> LiveSettledObserveV8<'j> {
    /// Only the actual successful continued Observe plus its immediate
    /// TurnObserved ACK can form this owner. A decoded history cannot do so.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_continued_wait(
        self,
    ) -> Result<LiveContinuedWaitV8<'j>, LiveTurnCarryFailureV8<'j>> {
        let selected = (|| {
            let LiveObserveSettlementOwnerV8::Continued(_) = &self.owner else {
                return Err(SourceJournalError::Binding);
            };
            if self.acks.len() != 2 {
                return Err(SourceJournalError::Order);
            }
            validate_current(&self)?;
            let data = self
                .owner
                .data()
                .inspect_err(|_| self.owner.journal().quarantine())?;
            if data.failure.is_some() {
                return Err(SourceJournalError::Order);
            }
            let held = self.owner.journal().hold()?;
            let (_, execution) = self
                .owner
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            bind_owned_wait_observation_v8(
                execution.wait(),
                &held.registration().expected_facts().scope,
                &data.observation.ok_or(SourceJournalError::Binding)?,
            )
            .map_err(|_| SourceJournalError::Binding)
        })();
        let observation = match selected {
            Ok(value) => value,
            Err(error) => return Err(LiveTurnCarryFailureV8 { owner: self, error }),
        };
        let owner = LiveContinuedWaitV8 {
            owner: self,
            observation,
        };
        if let Err(error) = owner.validate_live() {
            return Err(LiveTurnCarryFailureV8 {
                owner: owner.owner,
                error,
            });
        }
        Ok(owner)
    }
}
fn validate_current(owner: &LiveSettledObserveV8<'_>) -> Result<(), SourceJournalError> {
    let journal = owner.owner.journal();
    let result = (|| {
        let LiveObserveSettlementOwnerV8::Continued(_) = &owner.owner else {
            return Err(SourceJournalError::Binding);
        };
        if owner.acks.len() != 2 {
            return Err(SourceJournalError::Order);
        }
        let current = owner.acks.last().ok_or(SourceJournalError::Order)?;
        current.witness.validate_current_session(&current.session)?;
        // Old Observe/settlement reservations are now immutable causal facts.
        // Only the true latest TurnObserved witness guards physical authority.
        if current.witness.selected_row() != &owner.owner.selected_turn_observed()? {
            return Err(SourceJournalError::Binding);
        }
        let context = journal.context();
        let turn = owner.owner.turn();
        if turn == 0 || turn >= context.ordinary().max_iterations() {
            return Err(SourceJournalError::Binding);
        }
        owner.owner.guard_at(
            current.session.sequence(),
            current.session.acknowledged_bytes(),
        )?;
        current.witness.validate_current_session(&current.session)
    })();
    result.inspect_err(|_| journal.quarantine())
}
impl LiveContinuedWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            validate_current(&self.owner)?;
            let held = self.owner.owner.journal().hold()?;
            let execution = self
                .owner
                .owner
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            let data = self.owner.owner.data()?;
            let checked = bind_owned_wait_observation_v8(
                execution.wait(),
                &held.registration().expected_facts().scope,
                &data.observation.ok_or(SourceJournalError::Binding)?,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if checked.ordinary_digest() != self.observation.ordinary_digest() {
                self.owner.owner.journal().quarantine();
                return Err(SourceJournalError::Binding);
            }
            validate_current(&self.owner)
        })();
        result.inspect_err(|_| self.owner.owner.journal().quarantine())
    }
}
#[cfg(test)]
mod tests;
