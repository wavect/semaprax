//! Turn-two physical State and observed Copy carry into the next wait.
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod start;
use super::*;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterWaitV8<'j> {
    owner: LiveSettledObserveV8<'j>,
    observation: CheckedOwnedWaitObservationV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterTurnCarryFailureV8<'j>
{
    owner: LiveSettledObserveV8<'j>,
    error: SourceJournalError,
}

fn validate_later_current(owner: &LiveSettledObserveV8<'_>) -> Result<(), SourceJournalError> {
    let journal = owner.owner.journal();
    let result = (|| {
        if !matches!(&owner.owner, LiveObserveSettlementOwnerV8::Later(_)) || owner.acks.len() != 2
        {
            return Err(SourceJournalError::Order);
        }
        let current = owner.acks.last().ok_or(SourceJournalError::Order)?;
        current.witness.validate_current_session(&current.session)?;
        if current.witness.selected_row() != &owner.owner.selected_turn_observed()? {
            return Err(SourceJournalError::Binding);
        }
        let turn = owner.owner.turn();
        if turn < 2 || turn >= journal.context().ordinary().max_iterations() {
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

impl<'j> LiveSettledObserveV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_later_wait(
        self,
    ) -> Result<LiveLaterWaitV8<'j>, LiveLaterTurnCarryFailureV8<'j>> {
        let selected = (|| {
            validate_later_current(&self)?;
            let data = self.owner.data()?;
            if data.failure.is_some() {
                return Err(SourceJournalError::Order);
            }
            let journal = self.owner.journal();
            let held = journal.hold()?;
            let (_, execution) = journal
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
            Err(error) => return Err(LiveLaterTurnCarryFailureV8 { owner: self, error }),
        };
        let owner = LiveLaterWaitV8 {
            owner: self,
            observation,
        };
        if let Err(error) = owner.validate_live() {
            return Err(LiveLaterTurnCarryFailureV8 {
                owner: owner.owner,
                error,
            });
        }
        Ok(owner)
    }
}
impl LiveLaterWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        validate_later_current(&self.owner)?;
        let journal = self.owner.owner.journal();
        let held = journal.hold()?;
        let (_, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let data = self.owner.owner.data()?;
        let checked = bind_owned_wait_observation_v8(
            execution.wait(),
            &held.registration().expected_facts().scope,
            &data.observation.ok_or(SourceJournalError::Binding)?,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if checked.ordinary_digest() != self.observation.ordinary_digest() {
            journal.quarantine();
            return Err(SourceJournalError::Binding);
        }
        validate_later_current(&self.owner)
    }
}
