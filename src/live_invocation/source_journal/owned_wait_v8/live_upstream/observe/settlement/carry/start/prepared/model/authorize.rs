//! Actual continued Completed→transfer→full staged Decision. Old model ACKs
//! remain inert lineage after the first new authorization ACK.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedAuthorizeSuccessorV8;
use crate::live_invocation::source_journal::SourceStageRole;

struct AuthorizeAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedAuthorizationV8<
    'j,
> {
    completed: LiveContinuedModelV8<'j>,
    state: serde_json::Value,
    state_digest: String,
    transfer_digest: String,
    acks: Vec<AuthorizeAckV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedAuthorizeAppendV8<
    'j,
> {
    owner: LiveContinuedAuthorizationV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedAuthorizationFailureV8<
    'j,
> {
    owner: LiveContinuedAuthorizationV8<'j>,
    error: SourceJournalError,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedAuthorizationAdmissionFailureV8<
    'j,
> {
    owner: LiveContinuedModelV8<'j>,
    error: SourceJournalError,
}
impl<'j> LiveContinuedModelV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_authorize(
        self,
    ) -> Result<
        LiveOwnedContinuedAuthorizeAppendV8<'j>,
        LiveContinuedAuthorizationAdmissionFailureV8<'j>,
    > {
        let checked = (|| {
            self.validate_at(true)?;
            let ModelOwnerV8::Resumed(resumed) = &self.owner else {
                return Err(SourceJournalError::Order);
            };
            let last = self.acks.last().ok_or(SourceJournalError::Order)?;
            if self.acks.len() != 5
                || !matches!(last.witness.selected_row(),EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitCompleted{turn,attempt:0,..}) if *turn==self.owner.turn())
            {
                return Err(SourceJournalError::Order);
            }
            let binding = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1
                .wait();
            let proposal = self.proposal.as_ref().ok_or(SourceJournalError::Binding)?;
            let scope = self
                .journal()
                .context()
                .registration()
                .expected_facts()
                .scope
                .clone();
            let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
            if !proposal.matches(binding.binding(), &scope)
                || !resumed.owner.transfer_ready(binding, proposal)
            {
                return Err(SourceJournalError::Binding);
            }
            let state = resumed
                .owner
                .checked_model_facts(binding)
                .ok_or(SourceJournalError::Binding)?;
            let state_digest = wire::record_argument_digest(&state);
            let transfer_digest = wire::recipe_digest(
                wire::RecipeV8::Transfer,
                &serde_json::json!({"scope":scope,"generation":self.journal().context().registration().generation(),"turn":self.owner.turn(),"attempt":0,"wait":self.owner.wait()?,"from":binding.helper().function().id.as_str(),"to":binding.authorize().function().id.as_str(),"state_digest":state_digest,"proposal_digest":proposal.ordinary_digest()}),
            )?;
            Ok((state, state_digest, transfer_digest))
        })();
        let (state, state_digest, transfer_digest) = match checked {
            Ok(x) => x,
            Err(error) => {
                return Err(LiveContinuedAuthorizationAdmissionFailureV8 { owner: self, error })
            }
        };
        let owner = LiveContinuedAuthorizationV8 {
            completed: self,
            state,
            state_digest,
            transfer_digest,
            acks: Vec::new(),
        };
        let selected = EntryV8::Ordinary(SourceJournalEntry::ProposalAdmitted {
            turn: owner.completed.owner.turn(),
            attempt: 0,
            proposal_digest: owner
                .completed
                .proposal
                .as_ref()
                .expect("checked original K")
                .ordinary_digest()
                .to_owned(),
        });
        Ok(LiveOwnedContinuedAuthorizeAppendV8 { owner, selected })
    }
}
impl<'j> LiveContinuedAuthorizationV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.completed.journal()
    }
    fn actual(&self) -> Result<&super::resume::ResumedModelOwnerV8<'j>, SourceJournalError> {
        match &self.completed.owner {
            ModelOwnerV8::Resumed(o) => Ok(o),
            ModelOwnerV8::Parked(_) => Err(SourceJournalError::Binding),
        }
    }
    fn current(&self) -> &AppendSessionV8<'j> {
        self.acks
            .last()
            .map_or_else(|| self.completed.session(), |x| &x.session)
    }
    fn proposal(&self) -> Result<&CheckedOwnedWaitProposalV8, SourceJournalError> {
        self.completed
            .proposal
            .as_ref()
            .ok_or(SourceJournalError::Binding)
    }
    fn binding(
        &self,
    ) -> Result<
        &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
        SourceJournalError,
    > {
        self.journal()
            .context()
            .ready_runtime()
            .map(|x| x.1.wait())
            .ok_or(SourceJournalError::Binding)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = if let Some(ack) = self.acks.last() {
            (|| {
                ack.witness.validate_current_session(&ack.session)?;
                self.actual()?
                    .owner
                    .validate_authorize_live(&ack.session, &ack.witness)?;
                ack.witness.validate_current_session(&ack.session)
            })()
        } else {
            self.completed.validate_at(true)
        };
        result
            .and_then(|_| {
                if self.current().continued_authorize_accounting()?
                    != *self.completed.owner.accounting()
                {
                    return Err(SourceJournalError::Binding);
                }
                Ok(())
            })
            .inspect_err(|_| self.journal().quarantine())
    }
    fn next_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let turn = self.completed.owner.turn();
        let proposal = self.proposal()?;
        let binding = self.binding()?;
        let row = match self.acks.len() {
            0 => EntryV8::Ordinary(SourceJournalEntry::ProposalAdmitted {
                turn,
                attempt: 0,
                proposal_digest: proposal.ordinary_digest().to_owned(),
            }),
            1 => EntryV8::Owned(journal_model::OwnedBodyV8::OwnedStateTransferReserved {
                turn,
                attempt: 0,
                wait: self.completed.owner.wait()?.to_owned(),
                from: binding.helper().function().id.as_str().to_owned(),
                to: binding.authorize().function().id.as_str().to_owned(),
                state_digest: self.state_digest.clone(),
                proposal_digest: proposal.ordinary_digest().to_owned(),
                transfer_digest: self.transfer_digest.clone(),
            }),
            2 => {
                let state = self
                    .actual()?
                    .owner
                    .transferred_facts(binding, proposal)
                    .ok_or(SourceJournalError::Binding)?;
                if state != self.state {
                    return Err(SourceJournalError::Binding);
                }
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedStateTransferCompleted {
                    turn,
                    attempt: 0,
                    wait: self.completed.owner.wait()?.to_owned(),
                    reservation: u32::try_from(self.acks[1].session.sequence() - 1)
                        .map_err(|_| SourceJournalError::Capacity)?,
                    state,
                    state_digest: self.state_digest.clone(),
                    proposal: proposal.value().clone(),
                    proposal_digest: proposal.ordinary_digest().to_owned(),
                    transfer_digest: self.transfer_digest.clone(),
                })
            }
            3 => {
                let fuel = self
                    .journal()
                    .context()
                    .ready_runtime()
                    .ok_or(SourceJournalError::Binding)?
                    .1
                    .evaluation_fuel();
                if self
                    .journal()
                    .context()
                    .fold()
                    .ordinary
                    .max_steps_per_stage()
                    != Some(fuel)
                {
                    return Err(SourceJournalError::Binding);
                }
                EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    turn,
                    attempt: Some(0),
                    role: SourceStageRole::Authorize,
                    fuel,
                })
            }
            4 => {
                let (state, decision, consumed) = self
                    .actual()?
                    .owner
                    .staged_authorize_facts(binding)
                    .ok_or(SourceJournalError::Binding)?;
                if state != self.state {
                    return Err(SourceJournalError::Binding);
                }
                let scope = self
                    .journal()
                    .context()
                    .registration()
                    .expected_facts()
                    .scope
                    .clone();
                let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
                let decision_digest = wire::recipe_digest(
                    wire::RecipeV8::Decision,
                    &serde_json::json!({"scope":scope,"turn":turn,"attempt":0,"authorize":binding.authorize().function().id.as_str(),"decision":decision}),
                )?;
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedAuthorizationStaged {
                    turn,
                    attempt: 0,
                    stage_reservation: u32::try_from(self.acks[3].session.sequence() - 1)
                        .map_err(|_| SourceJournalError::Capacity)?,
                    transfer: u32::try_from(self.acks[2].session.sequence() - 1)
                        .map_err(|_| SourceJournalError::Capacity)?,
                    state_digest: self.state_digest.clone(),
                    proposal_digest: proposal.ordinary_digest().to_owned(),
                    decision,
                    decision_digest,
                    consumed,
                })
            }
            _ => return Err(SourceJournalError::Order),
        };
        Ok(row)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_next(
        self,
    ) -> Result<LiveOwnedContinuedAuthorizeAppendV8<'j>, LiveContinuedAuthorizationFailureV8<'j>>
    {
        match self.next_row() {
            Ok(selected) => Ok(LiveOwnedContinuedAuthorizeAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveContinuedAuthorizationFailureV8 { owner: self, error }),
        }
    }
    fn enter_stage(self) -> Result<Self, LiveContinuedAuthorizationFailureV8<'j>> {
        let Self {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks,
        } = self;
        let LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: model_acks,
            dispatched,
            proposal,
        } = completed;
        let current = acks.last().expect("real A ACK");
        let owner = match acks.len() {
            2 => owner.transfer_actual(
                &current.session,
                &current.witness,
                proposal.as_ref().expect("actual K"),
            ),
            4 => owner.authorize_actual(&current.session, &current.witness),
            _ => owner,
        };
        let completed = LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: model_acks,
            dispatched,
            proposal,
        };
        let owner = Self {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks,
        };
        let checked = (|| {
            owner.validate_live()?;
            if owner.acks.len() == 2
                && owner
                    .actual()?
                    .owner
                    .transferred_facts(owner.binding()?, owner.proposal()?)
                    .is_none()
            {
                return Err(SourceJournalError::Binding);
            }
            if owner.acks.len() == 4
                && owner
                    .actual()?
                    .owner
                    .staged_authorize_facts(owner.binding()?)
                    .is_none()
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        match checked {
            Ok(()) => Ok(owner),
            Err(error) => {
                owner.journal().quarantine();
                Err(LiveContinuedAuthorizationFailureV8 { owner, error })
            }
        }
    }
}
impl<'j> LiveOwnedContinuedAuthorizeAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.current().sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.current().acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        (|| {
            self.owner.validate_live()?;
            if self.owner.next_row()? != self.selected {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })()
        .inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        (|| {
            witness.validate_predecessor(
                self.owner.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner
                .actual()?
                .owner
                .validate_authorize_live(session, witness)?;
            witness.validate_current_session(session)
        })()
        .inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedAuthorizeAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedAuthorizeAppendPermitV8 { owner: self })
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedAuthorizeAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinuedAuthorizeAppendV8<'j>,
}
impl<'j> FixedOwnedContinuedAuthorizeAppendPermitV8<'_, 'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        self.owner.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal)
            || inventory.sequence() != self.owner.sequence()
            || inventory.acknowledged_bytes() != self.owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner
            .owner
            .actual()?
            .owner
            .validate_authorize_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .actual()?
            .owner
            .advance_authorize_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedAuthorizeAcknowledgmentFailureV8<
    'j,
> {
    Acknowledged {
        owner: LiveOwnedContinuedAuthorizeAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Entered(LiveContinuedAuthorizationFailureV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_authorize_v8<
    'j,
>(
    obligation: LiveOwnedContinuedAuthorizeAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
) -> Result<LiveContinuedAuthorizationV8<'j>, LiveContinuedAuthorizeAcknowledgmentFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(
            LiveContinuedAuthorizeAcknowledgmentFailureV8::Acknowledged {
                owner: obligation,
                session,
                witness,
                error,
            },
        );
    }
    let mut owner = obligation.owner;
    owner.acks.push(AuthorizeAckV8 { session, witness });
    owner
        .enter_stage()
        .map_err(LiveContinuedAuthorizeAcknowledgmentFailureV8::Entered)
}
#[cfg(all(test, unix))]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod effect;
