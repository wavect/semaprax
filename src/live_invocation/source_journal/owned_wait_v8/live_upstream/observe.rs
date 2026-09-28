//! Actual same-owner Observe, after the exact ordinary reservation ACK.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    observe_live_owned_run_v8, LiveObserveOutcomeV8, LiveObservedStateV8,
};
use crate::resumable_effects::owned_frame::v2::{
    bind_owned_wait_observation_v8, owned_wait_ordinary_state_digest_v8,
    CheckedOwnedWaitObservationV8,
};
pub(super) struct ObservedLiveOwnedRunV8<'j> {
    pub(super) owner: LiveObservedStateV8,
    pub(super) session: AppendSessionV8<'j>,
    pub(super) held: HeldOwnedWaitStoreV8<'j>,
    pub(super) journal: &'j SourceOwnedWaitJournalV8,
    pub(super) observation: CheckedOwnedWaitObservationV8,
    pub(super) reservation: u32,
    pub(super) observed: u32,
    pub(super) cancellation: &'j crate::agent_runtime::AgentCancellation,
}
pub(super) struct LiveObserveFailureV8<'j> {
    owner: LiveObserveOutcomeV8,
    held: HeldOwnedWaitStoreV8<'j>,
    error: SourceJournalError,
}
pub(super) fn observe_live_actor_v8<'j>(
    initialized: InitializedLiveOwnedRunV8<'j>,
) -> Result<ObservedLiveOwnedRunV8<'j>, LiveObserveFailureV8<'j>> {
    let InitializedLiveOwnedRunV8 {
        owner,
        journal,
        session,
        held,
        cancellation,
        ..
    } = initialized;
    macro_rules! fail {
        ($owner:expr, $error:expr $(,)?) => {
            LiveObserveFailureV8 {
                owner: $owner,
                held,
                error: $error,
            }
        };
    }
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveObserveOutcomeV8::Refused(owner),
            SourceJournalError::Binding,
        ));
    }
    if let Err(error) = held.validate_guard() {
        return Err(fail!(LiveObserveOutcomeV8::Refused(owner), error));
    }
    let context = journal.context();
    let Some((_, execution)) = context.ready_runtime() else {
        return Err(fail!(
            LiveObserveOutcomeV8::Refused(owner),
            SourceJournalError::Binding
        ));
    };
    let Some(fuel) = context.ordinary().max_steps_per_stage() else {
        return Err(fail!(
            LiveObserveOutcomeV8::Refused(owner),
            SourceJournalError::Binding,
        ));
    };
    let reservation = session.sequence() as u32;
    let session = match session.append(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: None,
        role: super::super::super::SourceStageRole::Observe,
        fuel,
    })) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveObserveOutcomeV8::Refused(owner),
                SourceJournalError::Uncertain,
            ))
        }
    };
    let permit_hold = match journal.hold() {
        Ok(hold) => hold,
        Err(error) => return Err(fail!(LiveObserveOutcomeV8::Refused(owner), error)),
    };
    let permit = LiveObservePermitV8 {
        held: permit_hold,
        fuel,
        cancellation,
    };
    let outcome = observe_live_owned_run_v8(permit, owner, execution.wait().observe());
    let LiveObserveOutcomeV8::Observed(owner) = outcome else {
        return Err(fail!(outcome, SourceJournalError::Binding));
    };
    let binding = execution.wait();
    let scope = &held.registration().expected_facts().scope;
    let observation = match bind_owned_wait_observation_v8(binding, scope, owner.observation()) {
        Ok(observation) => observation,
        Err(_) => {
            return Err(fail!(
                LiveObserveOutcomeV8::GuardLost(owner),
                SourceJournalError::Binding,
            ))
        }
    };
    let state = match owned_wait_ordinary_state_digest_v8(binding, owner.facts()) {
        Ok(state) => state,
        Err(_) => {
            return Err(fail!(
                LiveObserveOutcomeV8::GuardLost(owner),
                SourceJournalError::Binding,
            ))
        }
    };
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveObserveOutcomeV8::GuardLost(owner),
            SourceJournalError::Binding,
        ));
    }
    if let Err(error) = held.validate_guard() {
        return Err(fail!(LiveObserveOutcomeV8::GuardLost(owner), error));
    }
    let observed = session.sequence() as u32;
    let session = match session.append(EntryV8::Ordinary(SourceJournalEntry::TurnObserved {
        turn: 0,
        state,
        observation: observation.ordinary_digest().into(),
        feedback: crate::live_invocation::identity::digest(
            b"semaprax.source-feedback.v2\0",
            b"none",
        ),
    })) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveObserveOutcomeV8::GuardLost(owner),
                SourceJournalError::Uncertain,
            ))
        }
    };
    if cancellation.is_cancelled() {
        return Err(fail!(
            LiveObserveOutcomeV8::GuardLost(owner),
            SourceJournalError::Binding,
        ));
    }
    if let Err(error) = held.validate_guard() {
        return Err(fail!(LiveObserveOutcomeV8::GuardLost(owner), error));
    }
    Ok(ObservedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        observation,
        reservation,
        observed,
        cancellation,
    })
}

#[cfg(test)]
mod tests;
