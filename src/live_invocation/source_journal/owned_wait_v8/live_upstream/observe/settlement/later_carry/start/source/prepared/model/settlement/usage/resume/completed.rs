//! Completion derives from the actual successful Resume State and consumption.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::carry::start::prepared::model::join::LaterModelHistoryV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::later::settlement::start::resume::LaterResumedWaitV8;
use crate::live_invocation::source_journal::owned_wait_v8::wire;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterModelResumedV8<'j> {
    owner: LaterResumedWaitV8<'j>,
    history: LaterModelHistoryV8<'j>,
    proposal: CheckedOwnedWaitProposalV8,
    wait: String,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterModelActualResumeFailureV8<
    'j,
> {
    Before {
        owner: LiveLaterModelResumeReservedV8<'j>,
        error: SourceJournalError,
    },
    After {
        owner: LiveLaterModelResumedV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveLaterModelResumeReservedV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn resume_actual(
        self,
    ) -> Result<LiveLaterModelResumedV8<'j>, LiveLaterModelActualResumeFailureV8<'j>> {
        let checked = (|| {
            self.validate_live()?;
            let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved {
                turn,
                attempt: 0,
                wait,
                phase: journal_model::PhaseV8::Resume,
                replay_of: None,
                fuel,
            }) = self.witness.selected_row()
            else {
                return Err(SourceJournalError::Binding);
            };
            let execution = self
                .owner
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            if *turn != self.owner.turn() || *fuel != execution.evaluation_fuel() as u64 {
                return Err(SourceJournalError::Binding);
            }
            Ok(wait.clone())
        })();
        let wait = match checked {
            Ok(wait) => wait,
            Err(error) => {
                return Err(LiveLaterModelActualResumeFailureV8::Before { owner: self, error })
            }
        };
        let Self {
            owner,
            proposal,
            session,
            witness,
        } = self;
        let LiveLaterModelUsageV8 {
            owner,
            session: usage_session,
            witness: usage_witness,
        } = owner;
        let LiveLaterModelSettledV8 {
            owner,
            session: settled_session,
            witness: settled_witness,
        } = owner;
        let LiveLaterModelIntentV8 {
            owner,
            request,
            ordinal,
            session: intent_session,
            witness: intent_witness,
            dispatched,
        } = owner;
        let LiveLaterPreparedPhaseV8 {
            owner,
            session: prepared_session,
            witness: prepared_witness,
        } = owner;
        let LiveLaterStartedPhaseV8 {
            owner,
            observation,
            _observe_acks,
            acks,
        } = owner;
        let history = LaterModelHistoryV8::new(
            observation,
            _observe_acks,
            acks.into_iter()
                .map(|ack| (ack.session, ack.witness))
                .collect(),
            prepared_session,
            prepared_witness,
            wait.clone(),
            request,
            ordinal,
            dispatched,
            vec![
                (intent_session, intent_witness),
                (settled_session, settled_witness),
                (usage_session, usage_witness),
            ],
        );
        let actual = owner.resume_actual(&session, &witness, &proposal);
        let resumed = LiveLaterModelResumedV8 {
            owner: actual,
            history,
            proposal,
            wait,
            session,
            witness,
        };
        if let Err(error) = resumed.selected_completed() {
            return Err(LiveLaterModelActualResumeFailureV8::After {
                owner: resumed,
                error,
            });
        }
        Ok(resumed)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedLaterModelCompletedAppendV8<
    'j,
> {
    owner: LiveLaterModelResumedV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterModelCompletedV8<'j> {
    owner: LiveLaterModelResumedV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterModelCompletedFailureV8<
    'j,
> {
    Selection {
        owner: LiveLaterModelResumedV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveOwnedLaterModelCompletedAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveLaterModelResumedV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.journal()
    }
    fn turn(&self) -> u32 {
        self.owner.turn()
    }
    fn accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.accounting()
    }
    fn validate_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .validate_append_prefix(journal, inventory, selected)
    }
    fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner.advance_registry(witness, session)
    }
    fn selected_completed(&self) -> Result<EntryV8, SourceJournalError> {
        self.owner.validate_live(&self.session, &self.witness)?;
        self.completed_facts()
    }
    fn completed_facts(&self) -> Result<EntryV8, SourceJournalError> {
        let execution = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?
            .1;
        let state = self
            .owner
            .checked_facts(execution.wait())
            .ok_or(SourceJournalError::Binding)?;
        let result_digest = self
            .proposal
            .result_digest(&wire::record_argument_digest(&state))
            .map_err(|_| SourceJournalError::Binding)?;
        let consumed = self.owner.consumed().ok_or(SourceJournalError::Binding)?;
        Ok(EntryV8::Owned(
            journal_model::OwnedBodyV8::OwnedWaitCompleted {
                turn: self.turn(),
                attempt: 0,
                wait: self.wait.clone(),
                reservation: u32::try_from(
                    self.session
                        .sequence()
                        .checked_sub(1)
                        .ok_or(SourceJournalError::Order)?,
                )
                .map_err(|_| SourceJournalError::Capacity)?,
                proposal: self.proposal.value().clone(),
                proposal_digest: self.proposal.ordinary_digest().into(),
                result_digest,
                consumed,
            },
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_completed(
        self,
    ) -> Result<LiveOwnedLaterModelCompletedAppendV8<'j>, LiveLaterModelCompletedFailureV8<'j>>
    {
        match self.selected_completed() {
            Ok(selected) => Ok(LiveOwnedLaterModelCompletedAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveLaterModelCompletedFailureV8::Selection { owner: self, error }),
        }
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_result(
        &self,
    ) -> (serde_json::Value, u64, String) {
        let execution = self.journal().context().ready_runtime().unwrap().1;
        let state = self
            .owner
            .checked_facts(execution.wait())
            .expect("actual successful Resume State");
        let digest = self
            .proposal
            .result_digest(&wire::record_argument_digest(&state))
            .unwrap();
        (state, self.owner.consumed().unwrap(), digest)
    }
}
impl<'j> LiveOwnedLaterModelCompletedAppendV8<'j> {
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
        std::ptr::eq(self.owner.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.journal();
        let result = (|| {
            if self.owner.selected_completed()? != self.selected {
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
        Ok(FixedOwnedContinuedModelAppendPermitV8::later_completed(
            self,
        ))
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
            .validate_append_prefix(journal, inventory, &self.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner.advance_registry(witness, session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let journal = self.owner.journal();
        let result = (|| {
            witness.validate_predecessor(
                journal,
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_current_session(session)?;
            self.owner.owner.validate_live(session, witness)?;
            let (_, _, turn, selected) = session.continued_model_facts()?;
            if turn != self.owner.turn()
                || selected != &self.selected
                || session.continued_model_accounting()? != *self.owner.accounting()
            {
                return Err(SourceJournalError::Binding);
            }
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| journal.quarantine())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_later_model_completed_v8<
    'j,
>(
    obligation: LiveOwnedLaterModelCompletedAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
) -> Result<LiveLaterModelCompletedV8<'j>, LiveLaterModelCompletedFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveLaterModelCompletedFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    Ok(LiveLaterModelCompletedV8 {
        owner: obligation.owner,
        session,
        witness,
    })
}
impl LiveLaterModelCompletedV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner.owner.validate_live(&self.session, &self.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.session.sequence()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &crate::agent_lifecycle::authorization::target_protocol::TargetAccounting {
        self.owner.accounting()
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod join;
