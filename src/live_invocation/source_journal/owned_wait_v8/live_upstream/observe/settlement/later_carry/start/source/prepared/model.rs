//! A borrowed turn-two request origin stays inside the acknowledged Parked owner.
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod dispatch;
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod settlement;
use super::*;
use crate::interpreter::resumable::ResumableChannelValue;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedModelSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::FixedOwnedContinuedModelAppendPermitV8;
use crate::provider_adapter_sdk::{CheckedOwnedModelRequestV8, OwnedModelSettlementV8};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;

pub(crate) struct LiveLaterModelRequestOriginV8<'p, 'j> {
    owner: &'p LiveLaterPreparedPhaseV8<'j>,
}
impl LiveLaterModelRequestOriginV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.owner.validate_live()
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        self.validate_guard().ok()?;
        self.owner.owner.owner.checked_model_facts(binding)
    }
    pub(crate) fn request(&self) -> Option<&ResumableChannelValue> {
        self.owner.owner.owner.model_request()
    }
    pub(crate) fn observation(&self) -> &CheckedOwnedWaitObservationV8 {
        self.owner.owner.observation()
    }
    pub(crate) fn turn(&self) -> u32 {
        self.owner.owner.owner.turn()
    }
    pub(crate) fn coordinates(&self) -> Result<(u32, Option<Vec<u8>>), SourceJournalError> {
        self.validate_guard()?;
        let (ordinal, previous, total) = self.owner.session.continued_model_request_basis()?;
        if total != *self.owner.owner.owner.model_accounting() {
            return Err(SourceJournalError::Binding);
        }
        Ok((ordinal, previous))
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedLaterModelIntentAppendV8<
    'j,
> {
    owner: LiveLaterPreparedPhaseV8<'j>,
    request: CheckedOwnedModelRequestV8,
    ordinal: u32,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterModelIntentV8<'j> {
    ordinal: u32,
    owner: LiveLaterPreparedPhaseV8<'j>,
    request: CheckedOwnedModelRequestV8,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
    dispatched: Option<OwnedModelSettlementV8>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterModelIntentFailureV8<'j>
{
    Prepared {
        owner: LiveLaterPreparedPhaseV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelIntentAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveLaterPreparedPhaseV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_model_intent(
        self,
        adapter: &crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) -> Result<LiveOwnedLaterModelIntentAppendV8<'j>, LiveLaterModelIntentFailureV8<'j>> {
        let built = (|| {
            let origin = LiveLaterModelRequestOriginV8 { owner: &self };
            let (ordinal, _) = origin.coordinates()?;
            let journal = self.owner.journal();
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let request = adapter
                .checked_later_model_request_v8(
                    runtime,
                    execution,
                    &journal.context().registration().expected_facts().scope,
                    &origin,
                )
                .map_err(|_| SourceJournalError::Binding)?;
            origin.validate_guard()?;
            let id = request.identity();
            let selected =
                EntryV8::Ordinary(journal.context().ordinary().attempt_intent_at_ordinal(
                    origin.turn(),
                    0,
                    id.request_digest.clone(),
                    id.prompt_digest.clone(),
                    id.request_bytes,
                    ordinal,
                )?);
            Ok((request, ordinal, selected))
        })();
        match built {
            Ok((request, ordinal, selected)) => Ok(LiveOwnedLaterModelIntentAppendV8 {
                owner: self,
                request,
                ordinal,
                selected,
            }),
            Err(error) => Err(LiveLaterModelIntentFailureV8::Prepared { owner: self, error }),
        }
    }
}
impl<'j> LiveOwnedLaterModelIntentAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.owner.journal();
        let result = (|| {
            self.owner.validate_live()?;
            let (ordinal, _, total) = self.owner.session.continued_model_request_basis()?;
            if ordinal != self.ordinal || total != *self.owner.owner.owner.model_accounting() {
                return Err(SourceJournalError::Binding);
            }
            let id = self.request.identity();
            let selected =
                EntryV8::Ordinary(journal.context().ordinary().attempt_intent_at_ordinal(
                    self.owner.owner.owner.turn(),
                    0,
                    id.request_digest.clone(),
                    id.prompt_digest.clone(),
                    id.request_bytes,
                    ordinal,
                )?);
            if selected != self.selected {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        result.inspect_err(|_| journal.quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedModelAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedModelAppendPermitV8::later(self))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.belongs_to(journal)
            || inventory.sequence() != self.sequence()
            || inventory.acknowledged_bytes() != self.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner
            .owner
            .owner
            .validate_model_append_prefix(journal, inventory, &self.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .advance_model_registry(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.owner.journal();
        let result = (|| {
            witness.validate_predecessor(
                journal,
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner
                .owner
                .owner
                .validate_model_live(session, witness)?;
            let (_, _, turn, selected) = session.continued_model_facts()?;
            if turn != self.owner.owner.owner.turn()
                || selected != &self.selected
                || session.continued_model_accounting()?
                    != *self.owner.owner.owner.model_accounting()
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_model_intent_v8<
    'j,
>(
    obligation: LiveOwnedLaterModelIntentAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
) -> Result<LiveLaterModelIntentV8<'j>, LiveLaterModelIntentFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterModelIntentFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveLaterModelIntentV8 {
        ordinal: obligation.ordinal,
        owner: obligation.owner,
        request: obligation.request,
        session,
        witness,
        dispatched: None,
    })
}
impl LiveLaterModelIntentV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .owner
            .validate_model_live(&self.session, &self.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.session.sequence()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.owner.owner.model_accounting()
    }
}
