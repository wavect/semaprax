//! Owner-bound continued wait Created and original Start funding. No evaluator
//! can enter until the actual Reserved successor is consumed by its fixed route.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedStartSuccessorV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedStartPhaseV8<'j> {
    owner: LiveContinuedWaitV8<'j>,
    acks: Vec<ContinuedStartAckV8<'j>>,
}
struct ContinuedStartAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedStartSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedStartAppendV8<
    'j,
> {
    owner: LiveContinuedStartPhaseV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedStartFailureV8<'j> {
    Selection {
        owner: LiveContinuedStartPhaseV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedContinuedStartAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedStartSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveContinuedWaitV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_start_created(
        self,
    ) -> Result<LiveOwnedContinuedStartAppendV8<'j>, LiveContinuedStartFailureV8<'j>> {
        select(LiveContinuedStartPhaseV8 {
            owner: self,
            acks: Vec::new(),
        })
    }
}
fn select<'j>(
    owner: LiveContinuedStartPhaseV8<'j>,
) -> Result<LiveOwnedContinuedStartAppendV8<'j>, LiveContinuedStartFailureV8<'j>> {
    match owner.next_row() {
        Ok(selected) => Ok(LiveOwnedContinuedStartAppendV8 { owner, selected }),
        Err(error) => Err(LiveContinuedStartFailureV8::Selection { owner, error }),
    }
}
impl<'j> LiveContinuedStartPhaseV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.owner.owner.journal()
    }
    fn continued(&self) -> Result<&ContinuedObserveSettlementV8<'j>, SourceJournalError> {
        let LiveObserveSettlementOwnerV8::Continued(c) = &self.owner.owner.owner else {
            return Err(SourceJournalError::Binding);
        };
        Ok(c)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.acks.last().map_or_else(
            || {
                self.owner
                    .owner
                    .acks
                    .last()
                    .expect("actual TurnObserved")
                    .session
                    .sequence()
            },
            |a| a.session.sequence(),
        )
    }
    fn bytes(&self) -> usize {
        self.acks.last().map_or_else(
            || {
                self.owner
                    .owner
                    .acks
                    .last()
                    .expect("actual TurnObserved")
                    .session
                    .acknowledged_bytes()
            },
            |a| a.session.acknowledged_bytes(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            if let Some(current) = self.acks.last() {
                current.witness.validate_current_session(&current.session)?;
                self.continued()?
                    .guard_start_at(self.sequence(), self.bytes())?;
                current.witness.validate_current_session(&current.session)
            } else {
                self.owner.validate_live()
            }
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    fn next_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let journal = self.journal();
        let context = journal.context();
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let binding = execution.wait();
        let scope = &context.registration().expected_facts().scope;
        let turn = self.continued()?.turn();
        let wait = wire::recipe_digest(
            wire::RecipeV8::Attempt,
            &serde_json::json!({"invocation":scope.invocation_id(),"turn":turn,"attempt":0,"binding":binding.binding()}),
        )?;
        match self.acks.len() {
            0 => {
                let data = self.continued()?.data()?;
                let copy_arguments = self.owner.observation.copy_arguments().clone();
                Ok(EntryV8::Owned(
                    journal_model::OwnedBodyV8::OwnedWaitCreated {
                        turn,
                        attempt: 0,
                        wait,
                        plan_digest: binding.binding().into(),
                        cleanup_plan_digest: binding.cleanup_digest().into(),
                        signature: binding.signature().clone(),
                        argument_digest: wire::record_argument_digest(&data.state),
                        copy_arguments_digest: crate::live_invocation::identity::digest(
                            b"semaprax.source-owned-frame-copy-args.v2\0",
                            &wire::canonical(&copy_arguments),
                        ),
                        copy_arguments,
                    },
                ))
            }
            1 => {
                let fuel = execution.evaluation_fuel();
                if Some(fuel) != context.ordinary().max_steps_per_stage() {
                    return Err(SourceJournalError::Binding);
                }
                Ok(EntryV8::Owned(
                    journal_model::OwnedBodyV8::OwnedWaitReserved {
                        turn,
                        attempt: 0,
                        wait,
                        phase: journal_model::PhaseV8::Start,
                        replay_of: None,
                        fuel: fuel as u64,
                    },
                ))
            }
            _ => Err(SourceJournalError::Order),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_start_reservation(
        self,
    ) -> Result<LiveOwnedContinuedStartAppendV8<'j>, LiveContinuedStartFailureV8<'j>> {
        if self.acks.len() != 1 {
            return Err(LiveContinuedStartFailureV8::Selection {
                owner: self,
                error: SourceJournalError::Order,
            });
        }
        select(self)
    }
}
impl<'j> LiveOwnedContinuedStartAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.bytes()
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
        let result = (|| {
            if self.owner.next_row()? != self.selected {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedStartAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(self.fixed_permit())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_permit(
        &self,
    ) -> FixedOwnedContinuedStartAppendPermitV8<'_, 'j> {
        FixedOwnedContinuedStartAppendPermitV8 { owner: self }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            witness.validate_predecessor(
                self.owner.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            // Derive the selected facts without rechecking the retired prefix.
            let turn = self.owner.continued()?.turn();
            if !matches!(&self.selected,EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitCreated{turn:t,..}|journal_model::OwnedBodyV8::OwnedWaitReserved{turn:t,phase:journal_model::PhaseV8::Start,replay_of:None,..})if *t==turn)
            {
                return Err(SourceJournalError::Binding);
            }
            self.owner
                .continued()?
                .guard_start_at(session.sequence(), session.acknowledged_bytes())?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedStartAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedContinuedStartAppendV8<'j>,
}
impl<'j> FixedOwnedContinuedStartAppendPermitV8<'_, 'j> {
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
        self.owner.owner.continued()?.validate_start_append_prefix(
            journal,
            inventory,
            &self.owner.selected,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .continued()?
            .advance_start_registry(witness, session)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_start_v8<
    'j,
>(
    obligation: LiveOwnedContinuedStartAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedStartSuccessorV8<'j>,
) -> Result<LiveContinuedStartPhaseV8<'j>, LiveContinuedStartFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveContinuedStartFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let LiveOwnedContinuedStartAppendV8 { mut owner, .. } = obligation;
    owner.acks.push(ContinuedStartAckV8 { session, witness });
    Ok(owner)
}

#[cfg(test)]
mod tests;
