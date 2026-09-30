//! A true Resume reservation consumes the actual continued park once. Earlier
//! ACKs remain inert history; the current model witness alone guards this phase.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedResumedWaitV8;

pub(super) enum ModelOwnerV8<'j> {
    Parked(LiveContinuedPreparedPhaseV8<'j>),
    Resumed(ResumedModelOwnerV8<'j>),
}
pub(super) struct ResumedModelOwnerV8<'j> {
    pub(super) owner: ContinuedResumedWaitV8<'j>,
    history: PreparedHistoryV8<'j>,
}
struct PreparedHistoryV8<'j> {
    observation: CheckedOwnedWaitObservationV8,
    _observe_acks: Vec<ObserveSettlementAckV8<'j>>,
    _start_acks: Vec<ContinuedStartAckV8<'j>>,
    session: AppendSessionV8<'j>,
    _witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
    wait: String,
}
impl<'j> ModelOwnerV8<'j> {
    pub(super) fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        match self {
            Self::Parked(o) => o.owner.owner.model_journal(),
            Self::Resumed(o) => o.owner.model_journal(),
        }
    }
    pub(super) fn prepared_session(&self) -> &AppendSessionV8<'j> {
        match self {
            Self::Parked(o) => &o.session,
            Self::Resumed(o) => &o.history.session,
        }
    }
    pub(super) fn wait(&self) -> Result<&str, SourceJournalError> {
        match self {
            Self::Parked(o) => match o.witness.selected_row() {
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitPrepared { wait, .. }) => {
                    Ok(wait)
                }
                _ => Err(SourceJournalError::Binding),
            },
            Self::Resumed(o) => Ok(&o.history.wait),
        }
    }
    pub(super) fn turn(&self) -> u32 {
        match self {
            Self::Parked(o) => o.owner.owner.turn(),
            Self::Resumed(o) => o.owner.turn(),
        }
    }
    pub(super) fn accounting(&self) -> &TargetAccounting {
        match self {
            Self::Parked(o) => o.owner.owner.model_accounting(),
            Self::Resumed(o) => o.owner.model_accounting(),
        }
    }
    pub(super) fn clock(&self) -> &dyn crate::live_invocation::SourceInvocationClock {
        match self {
            Self::Parked(o) => o.owner.owner.model_clock(),
            Self::Resumed(o) => o.owner.model_clock(),
        }
    }
    pub(super) fn cancelled(&self) -> bool {
        match self {
            Self::Parked(o) => o.owner.owner.model_cancelled(),
            Self::Resumed(o) => o.owner.model_cancelled(),
        }
    }
    pub(super) fn configure_adapter(
        &self,
        adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) {
        match self {
            Self::Parked(o) => o.owner.owner.configure_model_adapter(adapter),
            Self::Resumed(o) => o.owner.configure_model_adapter(adapter),
        }
    }
    pub(super) fn validate_initial(&self) -> Result<(), SourceJournalError> {
        match self {
            Self::Parked(o) => o.validate_live(),
            Self::Resumed(_) => Err(SourceJournalError::Order),
        }
    }
    pub(super) fn validate_model_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        strict: bool,
    ) -> Result<(), SourceJournalError> {
        match self {
            Self::Parked(o) => o.owner.owner.validate_model_live(session, witness, strict),
            Self::Resumed(o) => o.owner.validate_model_live(session, witness, strict),
        }
    }
    pub(super) fn validate_append_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        match self {
            Self::Parked(o) => o
                .owner
                .owner
                .validate_model_append_prefix(journal, inventory, selected),
            Self::Resumed(o) => o
                .owner
                .validate_model_append_prefix(journal, inventory, selected),
        }
    }
    pub(super) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match self {
            Self::Parked(o) => o.owner.owner.advance_model_registry(witness, session),
            Self::Resumed(o) => o.owner.advance_model_registry(witness, session),
        }
    }
}
impl ResumedModelOwnerV8<'_> {
    pub(super) fn wait(&self) -> Result<&str, SourceJournalError> {
        Ok(&self.history.wait)
    }
}
impl<'j> LiveContinuedModelV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn resume_actual(
        self,
    ) -> Result<Self, LiveContinuedModelFailureV8<'j>> {
        let before = (|| {
            if self.acks.len() != 4 || !matches!(&self.owner, ModelOwnerV8::Parked(_)) {
                return Err(SourceJournalError::Order);
            }
            self.validate_at(true)?;
            let current = self.acks.last().ok_or(SourceJournalError::Order)?;
            let execution = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            if !matches!(current.witness.selected_row(),EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved{turn,attempt:0,phase:journal_model::PhaseV8::Resume,replay_of:None,fuel,..})if *turn==self.turn()&&*fuel==execution.evaluation_fuel() as u64)
            {
                return Err(SourceJournalError::Binding);
            }
            self.proposal.as_ref().ok_or(SourceJournalError::Binding)?;
            Ok(self.owner.wait()?.to_owned())
        })();
        let wait = match before {
            Ok(v) => v,
            Err(error) => return Err(LiveContinuedModelFailureV8 { owner: self, error }),
        };
        let Self {
            owner,
            request,
            ordinal,
            acks,
            dispatched,
            proposal,
        } = self;
        let ModelOwnerV8::Parked(parked) = owner else {
            unreachable!("checked actual park")
        };
        let LiveContinuedPreparedPhaseV8 {
            owner,
            session,
            witness,
        } = parked;
        let LiveContinuedStartedPhaseV8 {
            owner,
            observation,
            _observe_acks,
            acks: start_acks,
        } = owner;
        let history = PreparedHistoryV8 {
            observation,
            _observe_acks,
            _start_acks: start_acks,
            session,
            _witness: witness,
            wait,
        };
        let current = acks.last().expect("actual Resume ACK");
        let resumed = owner.resume_actual(
            &current.session,
            &current.witness,
            proposal.as_ref().expect("checked decoded K"),
        );
        let owner = Self {
            owner: ModelOwnerV8::Resumed(ResumedModelOwnerV8 {
                owner: resumed,
                history,
            }),
            request,
            ordinal,
            acks,
            dispatched,
            proposal,
        };
        if let Err(error) = owner.validate_at(true) {
            return Err(LiveContinuedModelFailureV8 { owner, error });
        }
        let ModelOwnerV8::Resumed(resumed) = &owner.owner else {
            unreachable!()
        };
        let binding = owner
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)
            .map(|x| x.1.wait());
        if binding
            .ok()
            .and_then(|b| resumed.owner.checked_model_facts(b))
            .is_none()
        {
            return Err(LiveContinuedModelFailureV8 {
                owner,
                error: SourceJournalError::Binding,
            });
        }
        Ok(owner)
    }
}

impl<'j> ModelOwnerV8<'j> {
    pub(super) fn transfer_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Self {
        match self {
            Self::Resumed(o) => Self::Resumed(ResumedModelOwnerV8 {
                owner: o
                    .owner
                    .transfer_authorize_actual(session, witness, proposal),
                history: o.history,
            }),
            Self::Parked(o) => Self::Parked(o),
        }
    }
    pub(super) fn authorize_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedAuthorizeSuccessorV8<'j>,
    ) -> Self {
        match self {
            Self::Resumed(o) => Self::Resumed(ResumedModelOwnerV8 {
                owner: o.owner.evaluate_authorize_actual(session, witness),
                history: o.history,
            }),
            Self::Parked(o) => Self::Parked(o),
        }
    }
}

impl<'j> ModelOwnerV8<'j> {
    pub(super) fn promote_effect_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness:&crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Self {
        match self {
            Self::Resumed(owner) => {
                let ResumedModelOwnerV8 { owner, history } = owner;
                Self::Resumed(ResumedModelOwnerV8 {
                    owner: owner.promote_effect_actual(session, witness, proposal),
                    history,
                })
            }
            owner => owner,
        }
    }
}

impl<'j> ModelOwnerV8<'j> {
    pub(super) fn prepare_effect_actual(
        self,
        session: &AppendSessionV8<'j>,
        witness:&crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &crate::agent_lifecycle::authorization::CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<Self, (Self, SourceJournalError)> {
        match self {
            Self::Resumed(owner) => {
                let ResumedModelOwnerV8 { owner, history } = owner;
                match owner.prepare_effect_actual(
                    session,
                    witness,
                    proposal,
                    commitments,
                    references,
                ) {
                    Ok(owner) => Ok(Self::Resumed(ResumedModelOwnerV8 { owner, history })),
                    Err((owner, error)) => {
                        Err((Self::Resumed(ResumedModelOwnerV8 { owner, history }), error))
                    }
                }
            }
            owner => Err((owner, SourceJournalError::Order)),
        }
    }
}

impl<'j> ModelOwnerV8<'j> {
    pub(super) fn activate_continued_intent(
        self,
        prior: &AppendSessionV8<'j>,
        consumed:&crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness:&crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &crate::agent_lifecycle::authorization::CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
    ) -> Result<Self, (Self, SourceJournalError)> {
        match self {
            Self::Resumed(actual) => {
                let ResumedModelOwnerV8 { owner, history } = actual;
                match owner.activate_continued_intent(
                    prior,
                    consumed,
                    session,
                    witness,
                    proposal,
                    commitments,
                    references,
                ) {
                    Ok(owner) => Ok(Self::Resumed(ResumedModelOwnerV8 { owner, history })),
                    Err((owner, error)) => {
                        Err((Self::Resumed(ResumedModelOwnerV8 { owner, history }), error))
                    }
                }
            }
            owner => Err((owner, SourceJournalError::Order)),
        }
    }
}

impl<'j> ModelOwnerV8<'j> {
    pub(super) fn dispatch_continued_effect(
        self,
        prior: &AppendSessionV8<'j>,
        consumed:&crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedEffectSuccessorV8<'j>,
        session: &AppendSessionV8<'j>,
        witness:&crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedIntentSuccessorV8<'j>,
        proposal: &CheckedOwnedWaitProposalV8,
        commitments: &crate::agent_lifecycle::authorization::CheckedOwnedWaitReadyCommitmentsV8,
        references: (u32, u32, u32),
        handler: &mut dyn crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler,
    ) -> Result<Self, (Self, SourceJournalError)> {
        match self {
            Self::Resumed(actual) => {
                let ResumedModelOwnerV8 { owner, history } = actual;
                match owner.dispatch_continued_effect(
                    prior,
                    consumed,
                    session,
                    witness,
                    proposal,
                    commitments,
                    references,
                    handler,
                ) {
                    Ok(owner) => Ok(Self::Resumed(ResumedModelOwnerV8 { owner, history })),
                    Err((owner, error)) => {
                        Err((Self::Resumed(ResumedModelOwnerV8 { owner, history }), error))
                    }
                }
            }
            owner => Err((owner, SourceJournalError::Order)),
        }
    }
}
