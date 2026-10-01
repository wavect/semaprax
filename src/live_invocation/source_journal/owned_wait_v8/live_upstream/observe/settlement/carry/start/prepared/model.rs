//! Continued model requests borrow the actual Prepared owner. No inert state
//! facts, turn number or accounting value can create this sealed origin.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetAccounting;
use crate::interpreter::resumable::ResumableChannelValue;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedModelSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::later_carry::start::source::prepared::model::LiveOwnedLaterModelIntentAppendV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::later_carry::start::source::prepared::model::settlement::LiveOwnedLaterModelSettlementAppendV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::later_carry::start::source::prepared::model::settlement::usage::LiveOwnedLaterModelUsageAppendV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::later_carry::start::source::prepared::model::settlement::usage::resume::LiveOwnedLaterModelResumeAppendV8;
use crate::live_invocation::source_journal::SourceAttemptFailure;
use crate::provider_adapter_sdk::{CheckedOwnedModelRequestV8, OwnedModelSettlementV8};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;

pub(crate) struct LiveContinuedModelRequestOriginV8<'p, 'j> {
    owner: &'p LiveContinuedPreparedPhaseV8<'j>,
}
impl LiveContinuedModelRequestOriginV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.owner.validate_live()
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
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
struct ModelAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
}
/// Actual State-bearing owner is first. No source or model phase extracts it.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedModelV8<'j> {
    owner: ModelOwnerV8<'j>,
    request: CheckedOwnedModelRequestV8,
    ordinal: u32,
    acks: Vec<ModelAckV8<'j>>,
    dispatched: Option<OwnedModelSettlementV8>,
    proposal: Option<CheckedOwnedWaitProposalV8>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedContinuedModelAppendV8<
    'j,
> {
    owner: LiveContinuedModelV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedModelFailureV8<'j>
{
    owner: LiveContinuedModelV8<'j>,
    error: SourceJournalError,
}
impl<'j> LiveContinuedPreparedPhaseV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_model_intent(
        self,
        adapter: &crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) -> Result<LiveOwnedContinuedModelAppendV8<'j>, LiveContinuedModelPreparationFailureV8<'j>>
    {
        let built = (|| {
            let origin = LiveContinuedModelRequestOriginV8 { owner: &self };
            let (ordinal, _) = origin.coordinates()?;
            let journal = self.owner.journal();
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let request = adapter
                .checked_continued_model_request_v8(
                    runtime,
                    execution,
                    &journal.context().registration().expected_facts().scope,
                    &origin,
                )
                .map_err(|_| SourceJournalError::Binding)?;
            origin.validate_guard()?;
            Ok((request, ordinal))
        })();
        let (request, ordinal) = match built {
            Ok(v) => v,
            Err(error) => {
                return Err(LiveContinuedModelPreparationFailureV8::Prepared { owner: self, error })
            }
        };
        let owner = LiveContinuedModelV8 {
            owner: ModelOwnerV8::Parked(self),
            request,
            ordinal,
            acks: Vec::new(),
            dispatched: None,
            proposal: None,
        };
        owner
            .prepare_next()
            .map_err(LiveContinuedModelPreparationFailureV8::Selected)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedModelPreparationFailureV8<
    'j,
> {
    Prepared {
        owner: LiveContinuedPreparedPhaseV8<'j>,
        error: SourceJournalError,
    },
    Selected(LiveContinuedModelFailureV8<'j>),
}
impl<'j> LiveContinuedModelV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.journal()
    }
    fn session(&self) -> &AppendSessionV8<'j> {
        self.acks
            .last()
            .map_or(self.owner.prepared_session(), |a| &a.session)
    }
    fn turn(&self) -> u32 {
        self.owner.turn()
    }
    fn validate_at(&self, strict: bool) -> Result<(), SourceJournalError> {
        let result = (|| {
            let Some(ack) = self.acks.last() else {
                return self.owner.validate_initial();
            };
            self.owner
                .validate_model_live(&ack.session, &ack.witness, strict)?;
            let (_, _, turn, _) = ack.session.continued_model_facts()?;
            let total = ack.session.continued_model_accounting()?;
            if turn != self.turn() || total != *self.owner.accounting() {
                return Err(SourceJournalError::Binding);
            }
            ack.witness.validate_current_session(&ack.session)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    fn next_row(&self) -> Result<EntryV8, SourceJournalError> {
        let cancelled_failure = matches!(
            &self.dispatched,
            Some(OwnedModelSettlementV8::Failed { .. })
        );
        self.validate_at(!cancelled_failure)?;
        let turn = self.turn();
        Ok(match self.acks.len() {
            0 => {
                let id = self.request.identity();
                EntryV8::Ordinary(
                    self.journal()
                        .context()
                        .ordinary()
                        .attempt_intent_at_ordinal(
                            turn,
                            0,
                            id.request_digest.clone(),
                            id.prompt_digest.clone(),
                            id.request_bytes,
                            self.ordinal,
                        )?,
                )
            }
            1 => EntryV8::Ordinary(
                match self.dispatched.as_ref().ok_or(SourceJournalError::Order)? {
                    OwnedModelSettlementV8::Settled { response, .. } => {
                        SourceJournalEntry::AttemptSettled {
                            turn,
                            attempt: 0,
                            response_digest:
                                crate::live_invocation::source_journal::source_response_digest(
                                    response,
                                ),
                            response: response.clone(),
                        }
                    }
                    OwnedModelSettlementV8::Failed {
                        reason,
                        attempted_bytes,
                        ..
                    } => SourceJournalEntry::AttemptFailed {
                        turn,
                        attempt: 0,
                        reason: *reason,
                        attempted_bytes: *attempted_bytes,
                    },
                },
            ),
            2 => {
                let usage = match self.dispatched.as_ref().ok_or(SourceJournalError::Order)? {
                    OwnedModelSettlementV8::Settled { usage, .. }
                    | OwnedModelSettlementV8::Failed { usage, .. } => *usage,
                };
                EntryV8::Ordinary(SourceJournalEntry::AttemptUsage {
                    turn,
                    attempt: 0,
                    reported: reported_usage(usage),
                })
            }
            3 => {
                if self.proposal.is_none() {
                    return Err(SourceJournalError::Binding);
                }
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
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved {
                    turn,
                    attempt: 0,
                    wait: self.owner.wait()?.into(),
                    phase: journal_model::PhaseV8::Resume,
                    replay_of: None,
                    fuel: fuel as u64,
                })
            }
            4 => {
                let ModelOwnerV8::Resumed(owner) = &self.owner else {
                    return Err(SourceJournalError::Order);
                };
                let proposal = self.proposal.as_ref().ok_or(SourceJournalError::Binding)?;
                let execution = self
                    .journal()
                    .context()
                    .ready_runtime()
                    .ok_or(SourceJournalError::Binding)?
                    .1;
                let state = owner
                    .owner
                    .checked_model_facts(execution.wait())
                    .ok_or(SourceJournalError::Binding)?;
                let argument = wire::record_argument_digest(&state);
                let result_digest = proposal
                    .result_digest(&argument)
                    .map_err(|_| SourceJournalError::Binding)?;
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitCompleted {
                    turn,
                    attempt: 0,
                    wait: owner.wait()?.into(),
                    reservation: u32::try_from(self.session().sequence() - 1)
                        .map_err(|_| SourceJournalError::Capacity)?,
                    proposal: proposal.value().clone(),
                    proposal_digest: proposal.ordinary_digest().into(),
                    result_digest,
                    consumed: owner.owner.consumed().ok_or(SourceJournalError::Binding)?,
                })
            }
            _ => return Err(SourceJournalError::Order),
        })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_next(
        mut self,
    ) -> Result<LiveOwnedContinuedModelAppendV8<'j>, LiveContinuedModelFailureV8<'j>> {
        if self.acks.len() == 3 && self.proposal.is_none() {
            let built = (|| {
                self.validate_at(true)?;
                let Some(OwnedModelSettlementV8::Settled { decoded, .. }) = &self.dispatched else {
                    return Err(SourceJournalError::Binding);
                };
                let journal = self.journal();
                let execution = journal
                    .context()
                    .ready_runtime()
                    .ok_or(SourceJournalError::Binding)?
                    .1;
                crate::resumable_effects::owned_frame::v2::bind_owned_wait_proposal_v8(
                    execution.wait(),
                    &journal.context().registration().expected_facts().scope,
                    decoded,
                )
                .map_err(|_| SourceJournalError::Binding)
            })();
            match built {
                Ok(proposal) => self.proposal = Some(proposal),
                Err(error) => return Err(LiveContinuedModelFailureV8 { owner: self, error }),
            }
        }
        match self.next_row() {
            Ok(selected) => Ok(LiveOwnedContinuedModelAppendV8 {
                owner: self,
                selected,
            }),
            Err(error) => Err(LiveContinuedModelFailureV8 { owner: self, error }),
        }
    }
}

fn reported_usage(
    value: Option<(u64, u64, i64)>,
) -> Option<crate::live_invocation::source_journal::SourceReportedUsage> {
    value.map(
        |(input, output, _)| crate::live_invocation::source_journal::SourceReportedUsage {
            total: input.checked_add(output),
            input: Some(input),
            output: Some(output),
            reasoning: None,
            cache_read: None,
            cache_write: None,
        },
    )
}

/// Borrowed only by the fixed SDK call after this owner's actual Intent ACK.
pub(crate) struct LiveContinuedModelIntentPermitV8<'p, 'j> {
    owner: &'p LiveContinuedModelV8<'j>,
    admission: std::cell::Cell<Option<SourceAttemptFailure>>,
}
impl LiveContinuedModelIntentPermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.owner.acks.len() != 1 || self.owner.dispatched.is_some() {
            return Err(SourceJournalError::Order);
        }
        let checked = (|| {
            let ModelOwnerV8::Parked(parked) = &self.owner.owner else {
                return Err(SourceJournalError::Order);
            };
            let ack = self.owner.acks.last().ok_or(SourceJournalError::Order)?;
            let admission = parked
                .owner
                .owner
                .validate_sdk_live(&ack.session, &ack.witness)?;
            let (_, _, turn, _) = ack.session.continued_model_facts()?;
            let actual = ack.session.continued_model_accounting()?;
            if turn != self.owner.turn() || actual != *self.owner.owner.accounting() {
                return Err(SourceJournalError::Binding);
            }
            ack.witness.validate_current_session(&ack.session)?;
            Ok(admission)
        })();
        match checked {
            Err(error) => {
                self.owner.journal().quarantine();
                Err(error)
            }
            Ok(admission) => {
                if let Some(failure) = admission.failure() {
                    if self.admission.get().is_none() {
                        self.admission.set(Some(failure));
                    }
                }
                admission.error().map_or(Ok(()), Err)
            }
        }
    }
    pub(crate) fn validate_store(&self) -> Result<(), SourceJournalError> {
        if self.owner.acks.len() != 1 {
            return Err(SourceJournalError::Order);
        }
        self.owner.validate_at(false)
    }
    pub(crate) fn request(&self) -> &CheckedOwnedModelRequestV8 {
        &self.owner.request
    }
    pub(crate) fn clock(&self) -> &dyn crate::live_invocation::SourceInvocationClock {
        self.owner.owner.clock()
    }
    pub(crate) fn guard_failure(&self) -> SourceAttemptFailure {
        self.admission.get().unwrap_or_else(|| {
            if self.owner.owner.cancelled() {
                SourceAttemptFailure::Cancelled
            } else {
                SourceAttemptFailure::Refused
            }
        })
    }
    pub(crate) fn quarantine(&self) {
        self.owner.journal().quarantine()
    }
}
impl<'j> LiveContinuedModelV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn dispatch_model(
        mut self,
        adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    ) -> Result<Self, LiveContinuedModelFailureV8<'j>> {
        if self.acks.len() != 1 || self.dispatched.is_some() {
            return Err(LiveContinuedModelFailureV8 {
                owner: self,
                error: SourceJournalError::Order,
            });
        }
        if let Err(error) = self.validate_at(true) {
            return Err(LiveContinuedModelFailureV8 { owner: self, error });
        }
        self.owner.configure_adapter(adapter);
        let result = {
            let permit = LiveContinuedModelIntentPermitV8 {
                owner: &self,
                admission: std::cell::Cell::new(None),
            };
            adapter.dispatch_continued_wait_v8(&permit)
        };
        // SDK's guarded panic bracket preserves its already selected source
        // primary. Actual State stays outside that borrowed callback closure.
        self.dispatched = Some(result);
        if let Err(error) = self.validate_at(false) {
            return Err(LiveContinuedModelFailureV8 { owner: self, error });
        }
        Ok(self)
    }
}
impl<'j> LiveOwnedContinuedModelAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.session().sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session().acknowledged_bytes()
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
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
            let strict = !matches!(
                &self.owner.dispatched,
                Some(OwnedModelSettlementV8::Failed { .. })
            );
            self.owner
                .owner
                .validate_model_live(session, witness, strict)?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedModelAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedContinuedModelAppendPermitV8 {
            owner: ModelPermitOwnerV8::First(self),
        })
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedModelAppendPermitV8<
    'p,
    'j,
> {
    owner: ModelPermitOwnerV8<'p, 'j>,
}
#[derive(Clone, Copy)]
enum ModelPermitOwnerV8<'p, 'j> {
    First(&'p LiveOwnedContinuedModelAppendV8<'j>),
    Later(&'p LiveOwnedLaterModelIntentAppendV8<'j>),
    LaterSettlement(&'p LiveOwnedLaterModelSettlementAppendV8<'j>),
    LaterUsage(&'p LiveOwnedLaterModelUsageAppendV8<'j>),
    LaterResume(&'p LiveOwnedLaterModelResumeAppendV8<'j>),
}
impl FixedOwnedContinuedModelAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn later<'p, 'j>(
        owner: &'p LiveOwnedLaterModelIntentAppendV8<'j>,
    ) -> FixedOwnedContinuedModelAppendPermitV8<'p, 'j> {
        FixedOwnedContinuedModelAppendPermitV8 {
            owner: ModelPermitOwnerV8::Later(owner),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn later_settlement<'p, 'j>(
        owner: &'p LiveOwnedLaterModelSettlementAppendV8<'j>,
    ) -> FixedOwnedContinuedModelAppendPermitV8<'p, 'j> {
        FixedOwnedContinuedModelAppendPermitV8 {
            owner: ModelPermitOwnerV8::LaterSettlement(owner),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn later_usage<'p, 'j>(
        owner: &'p LiveOwnedLaterModelUsageAppendV8<'j>,
    ) -> FixedOwnedContinuedModelAppendPermitV8<'p, 'j> {
        FixedOwnedContinuedModelAppendPermitV8 {
            owner: ModelPermitOwnerV8::LaterUsage(owner),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn later_resume<'p, 'j>(
        owner: &'p LiveOwnedLaterModelResumeAppendV8<'j>,
    ) -> FixedOwnedContinuedModelAppendPermitV8<'p, 'j> {
        FixedOwnedContinuedModelAppendPermitV8 {
            owner: ModelPermitOwnerV8::LaterResume(owner),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        match self.owner {
            ModelPermitOwnerV8::First(owner) => owner.selected(),
            ModelPermitOwnerV8::Later(owner) => owner.selected(),
            ModelPermitOwnerV8::LaterSettlement(owner) => owner.selected(),
            ModelPermitOwnerV8::LaterUsage(owner) => owner.selected(),
            ModelPermitOwnerV8::LaterResume(owner) => owner.selected(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        match self.owner {
            ModelPermitOwnerV8::First(owner) => {
                if !owner.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_live()
            }
            ModelPermitOwnerV8::Later(owner) => {
                if !owner.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_live()
            }
            ModelPermitOwnerV8::LaterSettlement(owner) => {
                if !owner.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_live()
            }
            ModelPermitOwnerV8::LaterUsage(owner) => {
                if !owner.belongs_to(journal) {
                    return Err(SourceJournalError::Binding);
                }
                owner.validate_live()
            }
            ModelPermitOwnerV8::LaterResume(owner) => {
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
            ModelPermitOwnerV8::First(owner) => {
                if !owner.belongs_to(journal)
                    || inventory.sequence() != owner.sequence()
                    || inventory.acknowledged_bytes() != owner.acknowledged_bytes()
                {
                    return Err(SourceJournalError::Binding);
                }
                owner
                    .owner
                    .owner
                    .validate_append_prefix(journal, inventory, &owner.selected)
            }
            ModelPermitOwnerV8::Later(owner) => owner.validate_selected_prefix(journal, inventory),
            ModelPermitOwnerV8::LaterSettlement(owner) => {
                owner.validate_selected_prefix(journal, inventory)
            }
            ModelPermitOwnerV8::LaterUsage(owner) => {
                owner.validate_selected_prefix(journal, inventory)
            }
            ModelPermitOwnerV8::LaterResume(owner) => {
                owner.validate_selected_prefix(journal, inventory)
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedContinuedModelSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match self.owner {
            ModelPermitOwnerV8::First(owner) => {
                owner.owner.owner.advance_registry(witness, session)
            }
            ModelPermitOwnerV8::Later(owner) => owner.advance_registry(witness, session),
            ModelPermitOwnerV8::LaterSettlement(owner) => owner.advance_registry(witness, session),
            ModelPermitOwnerV8::LaterUsage(owner) => owner.advance_registry(witness, session),
            ModelPermitOwnerV8::LaterResume(owner) => owner.advance_registry(witness, session),
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedModelAcknowledgmentFailureV8<
    'j,
> {
    Acknowledged {
        owner: LiveOwnedContinuedModelAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_model_v8<
    'j,
>(
    obligation: LiveOwnedContinuedModelAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
) -> Result<LiveContinuedModelV8<'j>, LiveContinuedModelAcknowledgmentFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveContinuedModelAcknowledgmentFailureV8::Acknowledged {
            owner: obligation,
            session,
            witness,
            error,
        });
    }
    let mut owner = obligation.owner;
    owner.acks.push(ModelAckV8 { session, witness });
    Ok(owner)
}

mod resume;
use resume::ModelOwnerV8;

#[cfg(all(test, unix))]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod authorize;
