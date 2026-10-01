//! Owner-bound continued wait Created and original Start funding. No evaluator
//! can enter until the actual Reserved successor is consumed by its fixed route.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedStartSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::later_carry::start::LiveOwnedLaterStartAppendV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedStartPhaseV8<'j> {
    owner: LiveContinuedWaitV8<'j>,
    acks: Vec<ContinuedStartAckV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedStartAckV8<'j> {
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
        FixedOwnedContinuedStartAppendPermitV8 {
            owner: StartPermitOwnerV8::First(self),
        }
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
    owner: StartPermitOwnerV8<'p, 'j>,
}
#[derive(Clone, Copy)]
enum StartPermitOwnerV8<'p, 'j> {
    First(&'p LiveOwnedContinuedStartAppendV8<'j>),
    Later(&'p LiveOwnedLaterStartAppendV8<'j>),
}
impl<'j> FixedOwnedContinuedStartAppendPermitV8<'_, 'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn later<'p>(
        owner: &'p LiveOwnedLaterStartAppendV8<'j>,
    ) -> FixedOwnedContinuedStartAppendPermitV8<'p, 'j> {
        FixedOwnedContinuedStartAppendPermitV8 {
            owner: StartPermitOwnerV8::Later(owner),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        match self.owner {
            StartPermitOwnerV8::First(owner) => owner.selected(),
            StartPermitOwnerV8::Later(owner) => owner.selected(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        match self.owner {
            StartPermitOwnerV8::First(owner) => {
                if !owner.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_live()
            }
            StartPermitOwnerV8::Later(owner) => {
                if !owner.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_live()
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        match self.owner {
            StartPermitOwnerV8::First(owner) => {
                if !owner.belongs_to(journal)
                    || inventory.sequence() != owner.sequence()
                    || inventory.acknowledged_bytes() != owner.acknowledged_bytes()
                {
                    return Err(SourceJournalError::Binding);
                }
                owner.owner.continued()?.validate_start_append_prefix(
                    journal,
                    inventory,
                    owner.selected(),
                )
            }
            StartPermitOwnerV8::Later(owner) => {
                if !owner.belongs_to(journal)
                    || inventory.sequence() != owner.sequence()
                    || inventory.acknowledged_bytes() != owner.acknowledged_bytes()
                {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_selected_prefix(journal, inventory)
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match self.owner {
            StartPermitOwnerV8::First(owner) => owner
                .owner
                .continued()?
                .advance_start_registry(witness, session),
            StartPermitOwnerV8::Later(owner) => owner.advance_registry(witness, session),
        }
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

#[cfg(all(test, unix))]
mod tests;

/// Actual source result comes first; all earlier ACKs are immutable lineage,
/// while the same accounting/held token remain nested in this result owner.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedStartedPhaseV8<'j> {
    owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedStartedWaitV8<'j>,
    observation: CheckedOwnedWaitObservationV8,
    _observe_acks: Vec<ObserveSettlementAckV8<'j>>,
    acks: Vec<ContinuedStartAckV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedSourceEntryFailureV8<
    'j,
> {
    Before {owner:LiveContinuedStartPhaseV8<'j>,error:SourceJournalError},
    Source {
        owner:crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedStartEntryFailureV8<'j>,
        _observation:CheckedOwnedWaitObservationV8,
        _observe_acks:Vec<ObserveSettlementAckV8<'j>>,
        _acks:Vec<ContinuedStartAckV8<'j>>,
    },
}
impl<'j> LiveContinuedStartPhaseV8<'j> {
    /// This consuming route cannot be called from Created alone. The sole
    /// evaluator receives the actual Reserved successor and physical owner.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn enter_actual_source(
        self,
    ) -> Result<LiveContinuedStartedPhaseV8<'j>, LiveContinuedSourceEntryFailureV8<'j>> {
        let before = (|| {
            if self.acks.len() != 2 {
                return Err(SourceJournalError::Order);
            }
            self.validate_live()?;
            if !matches!(
                self.acks
                    .last()
                    .expect("actual Start")
                    .witness
                    .selected_row(),
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved {
                    phase: journal_model::PhaseV8::Start,
                    replay_of: None,
                    ..
                })
            ) {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        if let Err(error) = before {
            return Err(LiveContinuedSourceEntryFailureV8::Before { owner: self, error });
        }
        let LiveContinuedStartPhaseV8 { owner, acks } = self;
        let LiveContinuedWaitV8 { owner, observation } = owner;
        let LiveSettledObserveV8 {
            owner,
            acks: observe_acks,
        } = owner;
        let LiveObserveSettlementOwnerV8::Continued(owner) = owner else {
            unreachable!("actual continued owner preflight")
        };
        let current = acks.last().expect("actual full-F Start ACK");
        let started = match owner.enter_actual_wait(&current.session, &current.witness) {
            Ok(owner) => owner,
            Err(owner) => {
                return Err(LiveContinuedSourceEntryFailureV8::Source {
                    owner,
                    _observation: observation,
                    _observe_acks: observe_acks,
                    _acks: acks,
                })
            }
        };
        Ok(LiveContinuedStartedPhaseV8 {
            owner: started,
            observation,
            _observe_acks: observe_acks,
            acks,
        })
    }
}
impl LiveContinuedStartedPhaseV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let current = self.acks.last().ok_or(SourceJournalError::Order)?;
        self.owner.validate_live(&current.session, &current.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn consumed(
        &self,
    ) -> Option<u64> {
        self.owner.consumed()
    }
}

impl LiveContinuedStartedPhaseV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &SourceOwnedWaitJournalV8 {
        self.owner.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.acks
            .last()
            .expect("actual Start ACK")
            .session
            .sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.acks
            .last()
            .expect("actual Start ACK")
            .session
            .acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn observation(
        &self,
    ) -> &CheckedOwnedWaitObservationV8 {
        &self.observation
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn encode_current_checkpoint(
        &self,
        session: &AppendSessionV8<'_>,
        key: &crate::resumable_effects::source_checkpoint::SourceCheckpointKey,
        expected: &crate::resumable_effects::owned_frame::v2::OwnedWaitCheckpointExpectationV8<'_>,
    ) -> Result<(Vec<u8>, String), SourceJournalError> {
        if !session.belongs_to(self.journal())
            || session.sequence() != self.sequence()
            || session.acknowledged_bytes() != self.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        let current = self.acks.last().ok_or(SourceJournalError::Order)?;
        self.owner
            .encode_checkpoint(session, &current.witness, key, expected)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod prepared;
