//! First live model wait: actual source park follows its ACKed Start reservation.
use super::model::CompletedLiveOwnedRunV8;
use super::observe::ObservedLiveOwnedRunV8;
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    begin_live_owned_wait_v8, restore_live_parked_state_v8, LiveParkedStateV8,
    LiveWaitStartOutcomeV8,
};
pub(super) struct ParkedLiveOwnedRunV8<'j> {
    pub(super) owner: LiveParkedStateV8,
    pub(super) session: AppendSessionV8<'j>,
    pub(super) held: HeldOwnedWaitStoreV8<'j>,
    pub(super) journal: &'j SourceOwnedWaitJournalV8,
    pub(super) observation:
        crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8,
    pub(super) wait: String,
    pub(super) reservation: u32,
    pub(super) prepared: u32,
    pub(super) cancellation: &'j crate::agent_runtime::AgentCancellation,
}
/// One held restart owner for the exact first-turn Prepared tail. It can enter
/// only the reviewed original-model continuation when a separate host grant
/// consumes it; it has no effect or finalization entry point.
pub(crate) struct RecoveredFirstTurnPreparedOwnerV8<'j> {
    owner: LiveParkedStateV8,
    held: HeldOwnedWaitStoreV8<'j>,
    session: AppendSessionV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    observation: crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8,
    wait: String,
    reservation: u32,
    prepared: u32,
    authentication: String,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
impl RecoveredFirstTurnPreparedOwnerV8<'_> {
    #[cfg(test)]
    pub(crate) fn test_metadata(&self) -> (u32, u32, usize, usize, String) {
        (
            self.reservation,
            self.prepared,
            self.session.sequence(),
            self.session.acknowledged_bytes(),
            self.wait.clone(),
        )
    }
    #[cfg(test)]
    pub(crate) fn test_fresh_backings(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.owner.test_weak()
    }
    #[cfg(test)]
    pub(crate) fn test_guard(&self) -> Result<(), SourceJournalError> {
        self.held
            .validate_prefix(self.session.sequence(), self.session.acknowledged_bytes())
    }
}
/// Explicit trusted-host authority for one restart materialization. It is not
/// derived from a checkpoint, journal row, registration or source fact.
pub(crate) struct FirstTurnPreparedRecoveryHostGrantV8 {
    creator: u32,
}
impl FirstTurnPreparedRecoveryHostGrantV8 {
    pub(crate) fn for_trusted_host(
        protected_checkpoint_key_available: bool,
    ) -> Result<Self, SourceJournalError> {
        if !protected_checkpoint_key_available {
            return Err(SourceJournalError::Binding);
        }
        Ok(Self {
            creator: std::process::id(),
        })
    }
    fn check(&self) -> Result<(), SourceJournalError> {
        if self.creator != std::process::id() {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}

/// Explicit trusted-host authority for one continuation of an already restored
/// first-turn Prepared owner. It has no checkpoint, row, registration or
/// source-derived constructor and is consumed by the continuation call.
pub(crate) struct FirstTurnPreparedContinuationHostGrantV8 {
    creator: u32,
}
impl FirstTurnPreparedContinuationHostGrantV8 {
    pub(crate) fn for_trusted_host(
        protected_checkpoint_key_available: bool,
    ) -> Result<Self, SourceJournalError> {
        if !protected_checkpoint_key_available {
            return Err(SourceJournalError::Binding);
        }
        Ok(Self {
            creator: std::process::id(),
        })
    }
    fn check(&self) -> Result<(), SourceJournalError> {
        if self.creator != std::process::id() {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}

/// Refusal before the one-way append transition retains the fresh physical
/// parked owner. A model failure is the existing opaque model failure owner;
/// both paths quarantine the recovered journal after the transition.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum RecoveredFirstTurnPreparedContinuationFailureV8<
    'j,
> {
    Refused {
        owner: RecoveredFirstTurnPreparedOwnerV8<'j>,
        error: SourceJournalError,
    },
    Model(super::model::LiveModelFailureV8<'j>),
}

fn validate_recovered_first_turn_prepared_continuation_v8(
    owner: &RecoveredFirstTurnPreparedOwnerV8<'_>,
) -> Result<(), SourceJournalError> {
    if owner.cancellation.is_cancelled() {
        return Err(SourceJournalError::Binding);
    }
    owner
        .held
        .validate_prefix(owner.session.sequence(), owner.session.acknowledged_bytes())?;
    if owner.held.registration() != owner.journal.context().registration()
        || owner.held.generation() != owner.journal.context().generation()
    {
        return Err(SourceJournalError::Binding);
    }
    let current = owner.journal.begin_session()?;
    let facts = current.inventory.first_turn_prepared_recovery()?;
    if facts.sequence != owner.session.sequence()
        || facts.bytes != owner.session.acknowledged_bytes()
        || facts.authentication != owner.authentication
        || facts.wait != owner.wait
        || facts.reservation != owner.reservation
        || facts.prepared != owner.prepared
        || facts.observation.ordinary_digest() != owner.observation.ordinary_digest()
        || facts.observation.ordinary_bytes() != owner.observation.ordinary_bytes()
        || facts.observation.request_digest() != owner.observation.request_digest()
        || facts.observation.copy_arguments() != owner.observation.copy_arguments()
        || current.inventory.authentication_tail() != owner.authentication
    {
        return Err(SourceJournalError::Binding);
    }
    owner.held.validate_prefix(facts.sequence, facts.bytes)
}

/// Consume one recovered first-turn Prepared owner into the original model
/// path. The authenticated prefix contains no AttemptIntent, so this is the
/// single original dispatch, never a redispatch. No cleanup/finalizer path is
/// reachable here because the resulting model path still carries the owner.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continue_recovered_first_turn_prepared_v8<
    'j,
>(
    owner: RecoveredFirstTurnPreparedOwnerV8<'j>,
    grant: FirstTurnPreparedContinuationHostGrantV8,
    adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
) -> Result<
    super::model::CompletedLiveOwnedRunV8<'j>,
    RecoveredFirstTurnPreparedContinuationFailureV8<'j>,
> {
    if let Err(error) = grant
        .check()
        .and_then(|_| validate_recovered_first_turn_prepared_continuation_v8(&owner))
    {
        owner.journal.quarantine();
        return Err(RecoveredFirstTurnPreparedContinuationFailureV8::Refused { owner, error });
    }
    let registration = owner.held.registration().clone();
    if owner
        .journal
        .lease
        .try_borrow_mut()
        .map_err(|_| SourceJournalError::Order)
        .and_then(|mut lease| {
            lease
                .authorize_recovered_first_prepared_continuation(&registration)
                .map_err(|_| SourceJournalError::Binding)
        })
        .is_err()
    {
        owner.journal.quarantine();
        return Err(RecoveredFirstTurnPreparedContinuationFailureV8::Refused {
            owner,
            error: SourceJournalError::Binding,
        });
    }
    let RecoveredFirstTurnPreparedOwnerV8 {
        owner,
        held,
        session,
        journal,
        observation,
        wait,
        reservation,
        prepared,
        cancellation,
        ..
    } = owner;
    let parked = ParkedLiveOwnedRunV8 {
        owner,
        held,
        session,
        journal,
        observation,
        wait,
        reservation,
        prepared,
        cancellation,
    };
    super::model::model_live_actor_v8(parked, adapter, clock).map_err(|failure| {
        journal.quarantine();
        RecoveredFirstTurnPreparedContinuationFailureV8::Model(failure)
    })
}

/// Failure from the closed first-turn Prepared restart entry.
///
/// `Recovery` has no materialized physical owner. `Continuation` retains the
/// reached owner in its existing sealed failure shape; callers can retain it
/// for the prescribed private cleanup without extracting a State or lease.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum FirstTurnPreparedRestartFailureV8<
    'j,
> {
    Recovery(SourceJournalError),
    Continuation(RecoveredFirstTurnPreparedContinuationFailureV8<'j>),
}

/// Restore and consume the sole admitted first-turn Prepared restart tail.
///
/// Both host grants are one-use values supplied independently by the trusted
/// host. Authenticated rows, retained registration data and checkpoint bytes
/// cannot replace either grant. The continuation reaches the original model
/// path exactly once; it never opens a fresh State or redispatches an intent.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn restart_first_turn_prepared_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    recovery: FirstTurnPreparedRecoveryHostGrantV8,
    continuation: FirstTurnPreparedContinuationHostGrantV8,
    adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
) -> Result<CompletedLiveOwnedRunV8<'j>, FirstTurnPreparedRestartFailureV8<'j>> {
    let owner = recover_first_turn_prepared_owner_v8(journal, recovery, cancellation)
        .map_err(FirstTurnPreparedRestartFailureV8::Recovery)?;
    continue_recovered_first_turn_prepared_v8(owner, continuation, adapter, clock)
        .map_err(FirstTurnPreparedRestartFailureV8::Continuation)
}

pub(crate) fn recover_first_turn_prepared_owner_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    grant: FirstTurnPreparedRecoveryHostGrantV8,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
) -> Result<RecoveredFirstTurnPreparedOwnerV8<'j>, SourceJournalError> {
    grant.check()?;
    if cancellation.is_cancelled() {
        return Err(SourceJournalError::Binding);
    }
    {
        let lease = journal
            .lease
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        lease
            .validate_recovery_read_only(journal.context().registration())
            .map_err(|_| SourceJournalError::Binding)?;
    }
    let held = journal.hold()?;
    let session = journal.begin_session()?;
    let facts = session.inventory.first_turn_prepared_recovery()?;
    held.validate_prefix(facts.sequence, facts.bytes)?;
    if cancellation.is_cancelled()
        || facts.sequence != session.sequence()
        || facts.bytes != session.acknowledged_bytes()
        || facts.authentication != session.inventory.authentication_tail()
    {
        journal.quarantine();
        return Err(SourceJournalError::Binding);
    }
    let context = journal.context();
    let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
    if held.registration() != context.registration() || held.generation() != context.generation() {
        journal.quarantine();
        return Err(SourceJournalError::Binding);
    }
    let observation = facts
        .observation
        .copy_arguments()
        .as_array()
        .and_then(|values| values.first())
        .and_then(|value| value.get("value"))
        .ok_or(SourceJournalError::Binding)
        .and_then(|value| {
            crate::interpreter::resumable::checkpoint::channel_from_json(value)
                .map_err(|_| SourceJournalError::Binding)
        })?;
    let owner = restore_live_parked_state_v8(
        execution.wait(),
        facts
            .checkpoint
            .fresh_owned_input()
            .map_err(|_| SourceJournalError::Binding)?,
        observation,
        facts.consumed,
    )
    .map_err(|_| SourceJournalError::Binding)?;
    held.validate_prefix(facts.sequence, facts.bytes)?;
    if cancellation.is_cancelled() {
        journal.quarantine();
        return Err(SourceJournalError::Binding);
    }
    Ok(RecoveredFirstTurnPreparedOwnerV8 {
        owner,
        held,
        session,
        journal,
        observation: facts.observation,
        wait: facts.wait,
        reservation: facts.reservation,
        prepared: facts.prepared,
        authentication: facts.authentication,
        cancellation,
    })
}
pub(super) enum LiveWaitFailureOwnerV8 {
    Observed(
        crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveObservedStateV8,
    ),
    Start(LiveWaitStartOutcomeV8),
}
pub(super) struct LiveWaitFailureV8<'j> {
    owner: LiveWaitFailureOwnerV8,
    held: HeldOwnedWaitStoreV8<'j>,
    error: SourceJournalError,
}
pub(super) fn start_live_actor_v8<'j>(
    observed: ObservedLiveOwnedRunV8<'j>,
) -> Result<ParkedLiveOwnedRunV8<'j>, LiveWaitFailureV8<'j>> {
    let ObservedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        observation,
        cancellation,
        ..
    } = observed;
    macro_rules! fail {
        ($owner:expr,$error:expr $(,)?) => {
            LiveWaitFailureV8 {
                owner: $owner,
                held,
                error: $error,
            }
        };
    }
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Observed(owner),
            SourceJournalError::Binding
        ));
    }
    if let Err(error) = held.validate_guard() {
        return Err(fail!(LiveWaitFailureOwnerV8::Observed(owner), error));
    }
    let context = journal.context();
    let Some((_, execution)) = context.ready_runtime() else {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Observed(owner),
            SourceJournalError::Binding
        ));
    };
    let binding = execution.wait();
    let scope = &held.registration().expected_facts().scope;
    let wait = match wire::recipe_digest(
        wire::RecipeV8::Attempt,
        &serde_json::json!({"invocation":scope.invocation_id(),"turn":0,"attempt":0,"binding":binding.binding()}),
    ) {
        Ok(wait) => wait,
        Err(error) => return Err(fail!(LiveWaitFailureOwnerV8::Observed(owner), error)),
    };
    let argument_digest = wire::record_argument_digest(owner.facts());
    let copy_arguments = observation.copy_arguments().clone();
    let copy_arguments_digest = crate::live_invocation::identity::digest(
        b"semaprax.source-owned-frame-copy-args.v2\0",
        &wire::canonical(&copy_arguments),
    );
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedWaitCreated {
            turn: 0,
            attempt: 0,
            wait: wait.clone(),
            plan_digest: binding.binding().into(),
            cleanup_plan_digest: binding.cleanup_digest().into(),
            signature: binding.signature().clone(),
            argument_digest,
            copy_arguments,
            copy_arguments_digest,
        },
    )) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveWaitFailureOwnerV8::Observed(owner),
                SourceJournalError::Uncertain
            ))
        }
    };
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Observed(owner),
            SourceJournalError::Binding
        ));
    }
    if let Err(error) = held.validate_guard() {
        return Err(fail!(LiveWaitFailureOwnerV8::Observed(owner), error));
    }
    let fuel = execution.evaluation_fuel();
    if context.ordinary().max_steps_per_stage() != Some(fuel) {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Observed(owner),
            SourceJournalError::Binding
        ));
    }
    let reservation = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedWaitReserved {
            turn: 0,
            attempt: 0,
            wait: wait.clone(),
            phase: journal_model::PhaseV8::Start,
            replay_of: None,
            fuel: fuel as u64,
        },
    )) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveWaitFailureOwnerV8::Observed(owner),
                SourceJournalError::Uncertain
            ))
        }
    };
    let permit_hold = match journal.hold() {
        Ok(hold) => hold,
        Err(error) => return Err(fail!(LiveWaitFailureOwnerV8::Observed(owner), error)),
    };
    let outcome = begin_live_owned_wait_v8(
        LiveWaitStartPermitV8 {
            held: permit_hold,
            fuel,
            cancellation,
        },
        owner,
    );
    let LiveWaitStartOutcomeV8::Parked(owner) = outcome else {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Start(outcome),
            SourceJournalError::Binding
        ));
    };
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Start(LiveWaitStartOutcomeV8::GuardLost(owner)),
            SourceJournalError::Binding
        ));
    }
    let (checkpoint, checkpoint_digest) = match session.live_parked_checkpoint(&owner, &observation)
    {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            return Err(fail!(
                LiveWaitFailureOwnerV8::Start(LiveWaitStartOutcomeV8::GuardLost(owner)),
                error
            ))
        }
    };
    let prepared = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedWaitPrepared {
            turn: 0,
            attempt: 0,
            wait: wait.clone(),
            reservation,
            observation_digest: observation.request_digest().into(),
            checkpoint_digest,
            checkpoint: crate::live_invocation::identity::hex(&checkpoint),
            consumed: owner.consumed(),
        },
    )) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveWaitFailureOwnerV8::Start(LiveWaitStartOutcomeV8::GuardLost(owner)),
                SourceJournalError::Uncertain
            ))
        }
    };
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Start(LiveWaitStartOutcomeV8::GuardLost(owner)),
            SourceJournalError::Binding
        ));
    }
    if let Err(error) = held.validate_guard() {
        return Err(fail!(
            LiveWaitFailureOwnerV8::Start(LiveWaitStartOutcomeV8::GuardLost(owner)),
            error
        ));
    }
    Ok(ParkedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        observation,
        wait,
        reservation,
        prepared,
        cancellation,
    })
}

#[cfg(all(test, unix))]
mod tests;
