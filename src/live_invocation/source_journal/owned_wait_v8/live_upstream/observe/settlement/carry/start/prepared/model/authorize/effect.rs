//! Actual continued Granted promotion and same-token renewal; no Intent/host.
use super::*;
use crate::agent_lifecycle::authorization::{
    checked_owned_wait_ready_commitments_v8, CheckedOwnedWaitReadyCommitmentsV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8;
struct EffectAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedEffectSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedEffectV8<'j> {
    authorization: LiveContinuedAuthorizationV8<'j>,
    commitments: CheckedOwnedWaitReadyCommitmentsV8,
    staged: u32,
    acks: Vec<EffectAckV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedEffectAppendV8<
    'j,
> {
    owner: LiveContinuedEffectV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedEffectAdmissionFailureV8<
    'j,
> {
    owner: LiveContinuedAuthorizationV8<'j>,
    error: SourceJournalError,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedEffectFailureV8<
    'j,
> {
    owner: LiveContinuedEffectV8<'j>,
    error: SourceJournalError,
}
impl<'j> LiveContinuedAuthorizationV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_effect(
        self,
    ) -> Result<LiveOwnedContinuedEffectAppendV8<'j>, LiveContinuedEffectAdmissionFailureV8<'j>>
    {
        let checked = (|| {
            self.validate_live()?;
            if self.acks.len() != 5 {
                return Err(SourceJournalError::Order);
            }
            let (staged_state, decision, _) = self
                .actual()?
                .owner
                .staged_authorize_facts(self.binding()?)
                .ok_or(SourceJournalError::Binding)?;
            if staged_state != self.state
                || decision["case"].as_str() != Some(self.binding()?.authorize().granted().as_str())
            {
                return Err(SourceJournalError::Binding);
            }
            let staged = u32::try_from(self.current().sequence() - 1)
                .map_err(|_| SourceJournalError::Capacity)?;
            let commitments = commitments(&self, staged_state, decision)?;
            Ok((staged, commitments))
        })();
        let (staged, commitments) = match checked {
            Ok(x) => x,
            Err(error) => return Err(LiveContinuedEffectAdmissionFailureV8 { owner: self, error }),
        };
        let owner = LiveContinuedEffectV8 {
            authorization: self,
            commitments,
            staged,
            acks: Vec::new(),
        };
        match owner.next_row() {
            Ok(selected) => Ok(LiveOwnedContinuedEffectAppendV8 { owner, selected }),
            Err(error) => Err(LiveContinuedEffectAdmissionFailureV8 {
                owner: owner.authorization,
                error,
            }),
        }
    }
}
fn commitments(
    a: &LiveContinuedAuthorizationV8<'_>,
    state: serde_json::Value,
    decision: serde_json::Value,
) -> Result<CheckedOwnedWaitReadyCommitmentsV8, SourceJournalError> {
    let (runtime, e) = a
        .journal()
        .context()
        .ready_runtime()
        .ok_or(SourceJournalError::Binding)?;
    checked_owned_wait_ready_commitments_v8(
        runtime,
        e,
        &a.journal().context().registration().expected_facts().scope,
        a.completed.owner.turn(),
        0,
        &state,
        &decision,
        a.proposal()?,
    )
}
impl<'j> LiveContinuedEffectV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.authorization.journal()
    }
    fn current(&self) -> &AppendSessionV8<'j> {
        self.acks
            .last()
            .map_or_else(|| self.authorization.current(), |x| &x.session)
    }
    fn validate_facts(&self) -> Result<(), SourceJournalError> {
        let (state, decision, consumed) = self
            .authorization
            .actual()?
            .owner
            .effect_facts(self.authorization.binding()?)
            .ok_or(SourceJournalError::Binding)?;
        if state != self.authorization.state
            || decision["case"].as_str()
                != Some(self.authorization.binding()?.authorize().granted().as_str())
        {
            return Err(SourceJournalError::Binding);
        }
        let staged = &self
            .authorization
            .acks
            .last()
            .ok_or(SourceJournalError::Order)?
            .witness;
        if !matches!(staged.selected_row(),EntryV8::Owned(journal_model::OwnedBodyV8::OwnedAuthorizationStaged{consumed:actual,..}) if *actual==consumed)
        {
            return Err(SourceJournalError::Binding);
        }
        let actual = commitments(&self.authorization, state, decision)?;
        if actual.authorization_binding() != self.commitments.authorization_binding()
            || actual.grant_digest() != self.commitments.grant_digest()
            || actual.target_grant_digest() != self.commitments.target_grant_digest()
            || actual.argument_digest() != self.commitments.argument_digest()
            || actual.budget() != self.commitments.budget()
        {
            return Err(SourceJournalError::Binding);
        }
        if self.current().continued_effect_accounting()?
            != *self.authorization.completed.owner.accounting()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        (|| {
            if let Some(ack) = self.acks.last() {
                ack.witness.validate_current_session(&ack.session)?;
                self.authorization.actual()?.owner.validate_effect_live(
                    &ack.session,
                    &ack.witness,
                    self.authorization.proposal()?,
                )?;
            } else {
                self.authorization.validate_live()?;
            }
            self.validate_facts()?;
            if let Some(ack) = self.acks.last() {
                self.authorization.actual()?.owner.validate_effect_live(
                    &ack.session,
                    &ack.witness,
                    self.authorization.proposal()?,
                )?;
                ack.witness.validate_current_session(&ack.session)?;
            } else {
                self.authorization.validate_live()?;
            }
            Ok(())
        })()
        .inspect_err(|_| self.journal().quarantine())
    }
    fn next_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let turn = self.authorization.completed.owner.turn();
        match self.acks.len() {
            0 => {
                let (_, decision, _) = self
                    .authorization
                    .actual()?
                    .owner
                    .effect_facts(self.authorization.binding()?)
                    .ok_or(SourceJournalError::Binding)?;
                let scope = &self
                    .journal()
                    .context()
                    .registration()
                    .expected_facts()
                    .scope;
                let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
                let digest = wire::recipe_digest(
                    wire::RecipeV8::Decision,
                    &serde_json::json!({"scope":scope,"turn":turn,"attempt":0,"authorize":self.authorization.binding()?.authorize().function().id.as_str(),"decision":decision}),
                )?;
                Ok(EntryV8::Owned(
                    journal_model::OwnedBodyV8::OwnedAuthorizationReady {
                        turn,
                        attempt: 0,
                        staged: self.staged,
                        state_digest: self.authorization.state_digest.clone(),
                        decision_digest: digest,
                        grant_digest: self.commitments.grant_digest().to_owned(),
                    },
                ))
            }
            1 => Ok(EntryV8::Ordinary(
                SourceJournalEntry::AuthorizationConsumed {
                    turn,
                    attempt: 0,
                    grant_digest: self.commitments.grant_digest().to_owned(),
                },
            )),
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_next(
        self,
    ) -> Result<LiveOwnedContinuedEffectAppendV8<'j>, LiveContinuedEffectFailureV8<'j>> {
        match self.next_row() {
            Ok(selected) => Ok(LiveOwnedContinuedEffectAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveContinuedEffectFailureV8 { owner: self, error }),
        }
    }
    fn enter_promotion(self) -> Result<Self, LiveContinuedEffectFailureV8<'j>> {
        let Self {
            authorization,
            commitments,
            staged,
            acks,
        } = self;
        let LiveContinuedAuthorizationV8 {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks: aacks,
        } = authorization;
        let super::super::LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: ma,
            dispatched,
            proposal,
        } = completed;
        let current = acks.last().expect("real C ACK");
        let owner = if acks.len() == 1 {
            owner.promote_effect_actual(
                &current.session,
                &current.witness,
                proposal.as_ref().expect("checked actual K"),
            )
        } else {
            owner
        };
        let completed = super::super::LiveContinuedModelV8 {
            owner,
            request,
            ordinal,
            acks: ma,
            dispatched,
            proposal,
        };
        let authorization = LiveContinuedAuthorizationV8 {
            completed,
            state,
            state_digest,
            transfer_digest,
            acks: aacks,
        };
        let owner = Self {
            authorization,
            commitments,
            staged,
            acks,
        };
        match owner.validate_live() {
            Ok(()) => Ok(owner),
            Err(error) => Err(LiveContinuedEffectFailureV8 { owner, error }),
        }
    }
}
impl<'j> LiveOwnedContinuedEffectAppendV8<'j> {
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
        (self.owner.validate_live().and_then(|_| {
            if self.owner.next_row()? == self.selected {
                Ok(())
            } else {
                Err(SourceJournalError::Binding)
            }
        }))
        .inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
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
                .authorization
                .actual()?
                .owner
                .validate_effect_live(session, witness, self.owner.authorization.proposal()?)?;
            witness.validate_current_session(session)
        })()
        .inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedEffectAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedEffectAppendPermitV8 { owner: self })
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedEffectAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinuedEffectAppendV8<'j>,
}
impl<'j> FixedOwnedContinuedEffectAppendPermitV8<'_, 'j> {
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
            .authorization
            .actual()?
            .owner
            .validate_effect_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .authorization
            .actual()?
            .owner
            .advance_effect_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedEffectAcknowledgmentFailureV8<
    'j,
> {
    Acknowledged {
        owner: LiveOwnedContinuedEffectAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Entered(LiveContinuedEffectFailureV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_effect_v8<
    'j,
>(
    obligation: LiveOwnedContinuedEffectAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedEffectSuccessorV8<'j>,
) -> Result<LiveContinuedEffectV8<'j>, LiveContinuedEffectAcknowledgmentFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveContinuedEffectAcknowledgmentFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let mut owner = obligation.owner;
    owner.acks.push(EffectAckV8 { session, witness });
    owner
        .enter_promotion()
        .map_err(LiveContinuedEffectAcknowledgmentFailureV8::Entered)
}
#[cfg(all(test, unix))]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod prepared;
