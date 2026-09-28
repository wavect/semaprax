//! First live model wait: actual source park follows its ACKed Start reservation.
use super::observe::ObservedLiveOwnedRunV8;
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    begin_live_owned_wait_v8, LiveParkedStateV8, LiveWaitStartOutcomeV8,
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
    let session = match session.append(EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCreated {
        turn: 0,
        attempt: 0,
        wait: wait.clone(),
        plan_digest: binding.binding().into(),
        cleanup_plan_digest: binding.cleanup_digest().into(),
        signature: binding.signature().clone(),
        argument_digest,
        copy_arguments,
        copy_arguments_digest,
    })) {
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
    let session = match session.append(EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved {
        turn: 0,
        attempt: 0,
        wait: wait.clone(),
        phase: model::PhaseV8::Start,
        replay_of: None,
        fuel: fuel as u64,
    })) {
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
    let session = match session.append(EntryV8::Owned(model::OwnedBodyV8::OwnedWaitPrepared {
        turn: 0,
        attempt: 0,
        wait: wait.clone(),
        reservation,
        observation_digest: observation.request_digest().into(),
        checkpoint_digest,
        checkpoint: crate::live_invocation::identity::hex(&checkpoint),
        consumed: owner.consumed(),
    })) {
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

#[cfg(test)]
mod tests;
