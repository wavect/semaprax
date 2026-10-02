//! Private fresh live actor. Actual roots remain attached to the held container;
//! authenticated rows alone never mint an evaluator entry or live lineage.
use super::append::{AppendSessionV8, HeldOwnedWaitStoreV8, SourceOwnedWaitJournalV8};
use super::model as journal_model;
use super::*;
use crate::interpreter::resumable::owned_frame::{
    registered_stage::live_run::{
        initialize_live_owned_run_v8, LiveInitializeOutcomeV8, LiveInitializedStateV8,
    },
    OwnedFrameInput,
};
use crate::resumable_effects::owned_frame::v2::live_run_plan::{
    exact_runtime_task_v8, live_initializer_plan_v8,
};

/// Only an ACKed actual Initialize reservation creates this one-use right.
pub(crate) struct LiveInitializePermitV8<'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    fuel: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
impl LiveInitializePermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        self.held.validate_guard()
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
/// Only this actor's ACKed actual Observe reservation creates evaluator entry.
pub(crate) struct LiveObservePermitV8<'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    fuel: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
impl LiveObservePermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        self.held.validate_guard()
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
/// Actual first Start ACK only; inert checked history cannot create entry.
pub(crate) struct LiveWaitStartPermitV8<'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    fuel: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
impl LiveWaitStartPermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        self.held.validate_guard()
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
pub(super) mod authorize;
pub(super) mod effect;
pub(super) mod model;
mod runtime;
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod observe;
mod wait;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use wait::continue_recovered_first_turn_prepared_v8;
pub(super) use wait::restart_first_turn_prepared_v8;
pub(crate) use wait::{
    recover_first_turn_prepared_owner_v8, FirstTurnPreparedContinuationHostGrantV8,
    FirstTurnPreparedRecoveryHostGrantV8, RecoveredFirstTurnPreparedOwnerV8,
};
pub(super) struct InitializedLiveOwnedRunV8<'j> {
    // Backing roots must be disposed while the held container still exists.
    owner: LiveInitializedStateV8,
    journal: &'j SourceOwnedWaitJournalV8,
    session: AppendSessionV8<'j>,
    held: HeldOwnedWaitStoreV8<'j>,
    initialization: u32,
    state_commit: u32,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
pub(super) struct LiveRunAdmissionRefusalV8 {
    pub input: OwnedFrameInput,
    pub error: SourceJournalError,
}
pub(super) struct LiveRunFailureV8<'j> {
    pub owner: Option<LiveInitializeOutcomeV8>,
    pub held: HeldOwnedWaitStoreV8<'j>,
    pub error: SourceJournalError,
}
/// Fresh actor admission validates the exact typed Task before any write or
/// owned admission; input rejection preserves its original representation.
pub(super) fn initialize_live_actor_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    input: OwnedFrameInput,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
) -> Result<Result<InitializedLiveOwnedRunV8<'j>, LiveRunFailureV8<'j>>, LiveRunAdmissionRefusalV8>
{
    let reject = |input, error| LiveRunAdmissionRefusalV8 { input, error };
    if cancellation.is_cancelled() {
        return Err(reject(input, SourceJournalError::Binding));
    }
    let context = journal.context();
    let Some((runtime, execution)) = context.ready_runtime() else {
        return Err(reject(input, SourceJournalError::Binding));
    };
    if context.fold().initialized_task.is_none() {
        return Err(reject(input, SourceJournalError::Binding));
    }
    let task = match runtime.owned_wait_task_v8(execution) {
        Ok(task) => task,
        Err(error) => return Err(reject(input, error)),
    };
    if !exact_runtime_task_v8(&input, task, execution.wait()) {
        return Err(reject(input, SourceJournalError::Binding));
    }
    let plan = match live_initializer_plan_v8(execution.wait()) {
        Ok(plan) => plan,
        Err(_) => return Err(reject(input, SourceJournalError::Binding)),
    };
    if crate::interpreter::resumable::owned_frame::registered_stage::initialize::validate_owned_task_input_v2(&plan, &input).is_err() {
        return Err(reject(input, SourceJournalError::Binding));
    }
    let Some(fuel) = context.ordinary().max_steps_per_stage() else {
        return Err(reject(input, SourceJournalError::Binding));
    };
    let held = match journal.hold() {
        Ok(held) => held,
        Err(error) => return Err(reject(input, error)),
    };
    // Recover and authenticate the actual held prefix before the State owner
    // can be initialized. A first-turn entry cannot recreate an earlier turn.
    let session = match journal.begin_fresh_session() {
        Ok(session) => session,
        Err(error) => return Err(reject(input, error)),
    };
    let mut session = match session.append(EntryV8::Owned(context.fold().created.clone())) {
        Ok(session) => session,
        Err(_) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: None,
                held,
                error: SourceJournalError::Uncertain,
            }))
        }
    };
    if context.fold().cumulative_initialization {
        session = match session.append(EntryV8::Owned(fold::cumulative::profile_row(
            context.fold(),
        ))) {
            Ok(session) => session,
            Err(_) => {
                return Ok(Err(LiveRunFailureV8 {
                    owner: None,
                    held,
                    error: SourceJournalError::Uncertain,
                }))
            }
        };
    }
    session = match session.append(EntryV8::Ordinary(SourceJournalEntry::RunOpened)) {
        Ok(session) => session,
        Err(_) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: None,
                held,
                error: SourceJournalError::Uncertain,
            }))
        }
    };
    let reservation = session.sequence() as u32; // bounded actual combined index
    session = match session.append(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: None,
        role: super::super::SourceStageRole::Initialize,
        fuel,
    })) {
        Ok(session) => session,
        Err(_) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: None,
                held,
                error: SourceJournalError::Uncertain,
            }))
        }
    };
    // No raw ACK fields or caller reservation sequence enter the initializer.
    let permit_hold = match journal.hold() {
        Ok(held) => held,
        Err(error) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: None,
                held,
                error,
            }))
        }
    };
    let permit = LiveInitializePermitV8 {
        held: permit_hold,
        fuel,
        cancellation,
    };
    let outcome = match initialize_live_owned_run_v8(permit, &plan, input) {
        Ok(outcome) => outcome,
        Err(_) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: None,
                held,
                error: SourceJournalError::Binding,
            }))
        }
    };
    let LiveInitializeOutcomeV8::Initialized(owner) = outcome else {
        return Ok(Err(LiveRunFailureV8 {
            owner: Some(outcome),
            held,
            error: SourceJournalError::Order,
        }));
    };
    let initialization = session.sequence() as u32; // bounded authenticated inventory
    let state = owner.facts().clone();
    let state_digest = wire::record_argument_digest(&state);
    let task = context
        .fold()
        .initialized_task
        .as_ref()
        .expect("sealed initialized mode")
        .clone();
    let row = journal_model::OwnedBodyV8::OwnedInitializationCommitted {
        reservation,
        task_digest: wire::record_argument_digest(&task),
        task,
        state: state.clone(),
        state_digest: state_digest.clone(),
        consumed: owner.consumed(),
    };
    session = match session.append(EntryV8::Owned(row)) {
        Ok(session) => session,
        Err(_) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: Some(LiveInitializeOutcomeV8::Initialized(owner)),
                held,
                error: SourceJournalError::Uncertain,
            }))
        }
    };
    let state_commit = session.sequence() as u32; // already bounded by candidate inventory
    session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedStateCommitted {
            turn: 0,
            state,
            argument_digest: state_digest,
            cleanup_plan_digest: context.fold().cleanup_plan_digest.clone(),
        },
    )) {
        Ok(session) => session,
        Err(_) => {
            return Ok(Err(LiveRunFailureV8 {
                owner: Some(LiveInitializeOutcomeV8::Initialized(owner)),
                held,
                error: SourceJournalError::Uncertain,
            }))
        }
    };
    if let Err(error) = held.validate_guard().and_then(|()| {
        if cancellation.is_cancelled() {
            Err(SourceJournalError::Binding)
        } else {
            Ok(())
        }
    }) {
        return Ok(Err(LiveRunFailureV8 {
            owner: Some(LiveInitializeOutcomeV8::Initialized(owner)),
            held,
            error,
        }));
    }
    Ok(Ok(InitializedLiveOwnedRunV8 {
        owner,
        journal,
        session,
        held,
        initialization,
        state_commit,
        cancellation,
    }))
}

#[cfg(all(test, unix))]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::{
    advance_verified_observe_settlement_v8, FixedOwnedObserveSettlementAppendPermitV8,
    LiveObserveSettlementFailureV8, LiveOwnedObserveSettlementAppendV8, LiveSettledObserveV8,
};

#[cfg(all(test, unix))]
pub(crate) use observe::settlement::test_initial_observe_entry_v8;

pub(crate) use observe::settlement::failed_state::LiveFailedObserveStateCleanupPermitV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::failed_state::{FailedObserveCacheV8, FailedObserveContextV8, FailedObserveOwnerV8, FailedObserveSourceV8, FixedFailedObserveStateAppendPermitV8, LiveFailedObserveStateAppendV8, LiveFailedObserveStateFailureV8, LiveFailedObserveStateAcknowledgedV8, LiveFailedObserveStateStoppedV8, LiveFailedObserveStateQuarantinedV8, advance_verified_failed_observe_state_v8, stop_failed_observe_state_v8};

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::{advance_verified_continued_start_v8,FixedOwnedContinuedStartAppendPermitV8,LiveContinuedSourceEntryFailureV8,LiveContinuedStartFailureV8,LiveContinuedStartPhaseV8,LiveOwnedContinuedStartAppendV8};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::{
    advance_verified_later_start_v8, LiveLaterStartFailureV8, LiveLaterStartPhaseV8,
    LiveOwnedLaterStartAppendV8,
};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::LiveLaterStartedPhaseV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::prepared::{
    advance_verified_later_prepared_v8, LiveLaterPreparedFailureV8,
    LiveLaterPreparedPhaseV8, LiveOwnedLaterPreparedAppendV8,
};
pub(crate) use observe::settlement::later_carry::start::source::prepared::model::LiveLaterModelRequestOriginV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::prepared::model::{
    advance_verified_later_model_intent_v8, LiveLaterModelIntentFailureV8,
    LiveLaterModelIntentV8, LiveOwnedLaterModelIntentAppendV8,
};
pub(crate) use observe::settlement::later_carry::start::source::prepared::model::dispatch::LiveLaterModelIntentPermitV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::prepared::model::settlement::{
    advance_verified_later_model_settlement_v8, LiveLaterModelSettledV8,
    LiveLaterModelSettlementFailureV8, LiveOwnedLaterModelSettlementAppendV8,
};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::prepared::model::settlement::usage::{
    advance_verified_later_model_usage_v8, LiveLaterModelUsageFailureV8,
    LiveLaterModelUsageV8, LiveOwnedLaterModelUsageAppendV8,
};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::prepared::model::settlement::usage::resume::{
    advance_verified_later_model_resume_v8, LiveLaterModelResumeFailureV8,
    LiveLaterModelResumeReservedV8, LiveOwnedLaterModelResumeAppendV8,
};

impl LiveWaitStartPermitV8<'_> {
    pub(crate) fn matches_held_store(&self, held: &HeldOwnedWaitStoreV8<'_>) -> bool {
        self.held.same_container(held)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) use effect::authorization::step::r#continue::settlement::start::{ContinuedStartedWaitV8,ContinuedStartEntryFailureV8};

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::LiveContinuedStartedPhaseV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::{advance_verified_continued_prepared_v8,FixedOwnedContinuedPreparedAppendPermitV8,LiveContinuedPreparedFailureV8,LiveContinuedPreparedPhaseV8,LiveOwnedContinuedPreparedAppendV8};

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::{advance_verified_continued_model_v8,FixedOwnedContinuedModelAppendPermitV8,LiveContinuedModelAcknowledgmentFailureV8,LiveContinuedModelFailureV8,LiveContinuedModelPreparationFailureV8,LiveContinuedModelV8,LiveOwnedContinuedModelAppendV8};
pub(crate) use observe::settlement::carry::start::prepared::model::{LiveContinuedModelRequestOriginV8,LiveContinuedModelIntentPermitV8};
pub(crate) use effect::authorization::step::r#continue::settlement::start::model::LiveContinuedWaitResumePermitV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) use effect::authorization::step::r#continue::settlement::start::model::ContinuedResumedWaitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::{advance_verified_continued_authorize_v8,FixedOwnedContinuedAuthorizeAppendPermitV8,LiveContinuedAuthorizeAcknowledgmentFailureV8,LiveContinuedAuthorizationAdmissionFailureV8,LiveContinuedAuthorizationFailureV8,LiveContinuedAuthorizationV8,LiveOwnedContinuedAuthorizeAppendV8};
pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::{LiveContinuedStateTransferPermitV8,LiveContinuedAuthorizePermitV8};

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::{advance_verified_continued_effect_v8,FixedOwnedContinuedEffectAppendPermitV8,LiveContinuedEffectAcknowledgmentFailureV8,LiveContinuedEffectAdmissionFailureV8,LiveContinuedEffectFailureV8,LiveContinuedEffectV8,LiveOwnedContinuedEffectAppendV8};
pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::LiveContinuedReadyPromotionPermitV8;

pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::prepared::LiveContinuedEffectAuthorizationPermitV8;

#[cfg(all(test, unix))]
pub(in crate::live_invocation::source_journal::owned_wait_v8) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::prepared::test_continued_preparation_admissions;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::intent::{advance_verified_continued_intent_v8,FixedOwnedContinuedIntentAppendPermitV8,LiveContinuedIntentAcknowledgmentFailureV8,LiveActivatedContinuedEffectV8,LiveOwnedContinuedIntentAppendV8};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::{LiveContinuedEffectPreparationFailureV8,LivePreparedContinuedEffectV8};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::intent::LiveContinuedIntentSelectionFailureV8;
pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::prepared::intent::LiveContinuedIntentPermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::intent::dispatch::{LiveContinuedDispatchFailureV8,LiveDispatchedContinuedEffectV8};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::intent::dispatch::settlement::{advance_verified_continued_settlement_v8,FixedOwnedContinuedSettlementAppendPermitV8,LiveContinuedSettlementAcknowledgmentFailureV8,LiveContinuedSettlementAcknowledgedV8,LiveOwnedContinuedSettlementAppendV8,LiveRecordedContinuedEffectV8,LiveSettledContinuedEffectV8};
pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::prepared::intent::dispatch::settlement::LiveContinuedSettlementPermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::intent::dispatch::settlement::cleanup::{ContinuedDecisionCleanupRejectionV8, ContinuedDecisionReleaseFailureV8, ContinuedOutcomeFailureV8, PreparedContinuedDecisionCleanupV8, ReleasedContinuedDecisionCleanupV8, SettledContinuedDecisionCleanupV8, StartedContinuedDecisionCleanupV8};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::carry::start::prepared::model::authorize::effect::prepared::intent::dispatch::settlement::cleanup::continued_reduce::{advance_verified_continued_reduce_v8, ContinuedReduceReservationRejectionV8, FixedOwnedContinuedReduceReservationAppendPermitV8, LiveContinuedReduceAdvanceFailureV8, LiveContinuedReduceReservationAppendV8, LiveContinuedReduceReservedV8, LiveContinuedStagedStepV8, LiveContinuedTerminalDriverFailureV8, LiveContinuedTerminalPhaseV8};
pub(in crate::live_invocation::source_journal::owned_wait_v8) use effect::authorization::cleanup::{LiveCleanupAcknowledgedV8, LiveEffectCleanupFailureV8, LiveOwnedEffectCleanupAppendV8};

pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::prepared::intent::dispatch::settlement::cleanup::LiveContinuedDecisionCleanupPermitV8;
pub(crate) use effect::authorization::step::r#continue::settlement::start::model::authorize::effect::prepared::intent::dispatch::settlement::cleanup::LiveContinuedOutcomePermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) use observe::settlement::later_carry::start::source::prepared::model::settlement::usage::resume::completed::{
    advance_verified_later_model_completed_v8, LiveLaterModelCompletedV8, LiveLaterModelCompletedFailureV8, LiveOwnedLaterModelCompletedAppendV8,
};
