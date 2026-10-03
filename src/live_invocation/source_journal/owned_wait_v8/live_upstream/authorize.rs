//! Actual successful helper lineage through transfer and source authorization.
//! No data-only history, Ready permission, or target dispatch enters this route.
use super::super::append::AppendFailureV8;
use super::model::{check_clock_v8, CompletedLiveOwnedRunV8};
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    authorize_live_owned_state_v8, transfer_live_owned_state_v8, LiveAuthorizeOutcomeV8,
    LiveResumedStateV8, LiveStagedAuthorizationV8, LiveStateTransferOutcomeV8,
    LiveTransferredStateV8,
};
use crate::live_invocation::SourceInvocationClock;
use crate::resumable_effects::owned_frame::v2::{
    CheckedOwnedAgentWaitBindingV8, CheckedOwnedWaitProposalV8,
};

struct LiveStageAuthorityV8<'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    binding: &'j CheckedOwnedAgentWaitBindingV8,
    sequence: usize,
    bytes: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn SourceInvocationClock,
    domain: &'j str,
    initial: i64,
    deadline: i64,
}
impl LiveStageAuthorityV8<'_> {
    fn validate(&self) -> Result<(), SourceJournalError> {
        check_clock_v8(
            &self.held,
            self.sequence,
            self.bytes,
            self.cancellation,
            self.clock,
            self.domain,
            self.initial,
            self.deadline,
        )
    }
}
/// Only this actor's successful TransferReserved ACK creates this consuming right.
pub(crate) struct LiveStateTransferPermitV8<'j> {
    authority: LiveStageAuthorityV8<'j>,
}
impl LiveStateTransferPermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.authority.validate()
    }
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.authority.binding
    }
}
/// Consuming permit for the exact authenticated first-turn TransferCompleted
/// prefix. It grants structural reconstruction of one State owner only.
pub(crate) struct LiveRecoveredStateTransferPermitV8<'p, 'j> {
    held: &'p HeldOwnedWaitStoreV8<'j>,
    binding: &'j CheckedOwnedAgentWaitBindingV8,
    sequence: usize,
    bytes: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn SourceInvocationClock,
    domain: &'j str,
    initial: i64,
    deadline: i64,
    state: serde_json::Value,
    state_digest: String,
    proposal: CheckedOwnedWaitProposalV8,
}
impl LiveRecoveredStateTransferPermitV8<'_, '_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        let scope = self.held.registration().expected_facts().scope.clone();
        let scope = serde_json::json!({
            "program_root":scope.program_root(),
            "invocation":scope.invocation_id(),
            "policy_epoch":scope.policy_epoch()
        });
        if self.proposal.matches(self.binding.binding(), &scope)
            && wire::record_argument_digest(&self.state) == self.state_digest
        {
            check_clock_v8(
                self.held,
                self.sequence,
                self.bytes,
                self.cancellation,
                self.clock,
                self.domain,
                self.initial,
                self.deadline,
            )
        } else {
            Err(SourceJournalError::Binding)
        }
    }
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.binding
    }
    pub(crate) fn state(&self) -> &serde_json::Value {
        &self.state
    }
    pub(crate) fn proposal(&self) -> &CheckedOwnedWaitProposalV8 {
        &self.proposal
    }
}
impl<'p, 'j> LiveRecoveredStateTransferPermitV8<'p, 'j> {
    pub(super) fn new(
        held: &'p HeldOwnedWaitStoreV8<'j>,
        binding: &'j CheckedOwnedAgentWaitBindingV8,
        sequence: usize,
        bytes: usize,
        cancellation: &'j crate::agent_runtime::AgentCancellation,
        clock: &'j dyn SourceInvocationClock,
        domain: &'j str,
        initial: i64,
        deadline: i64,
        state: serde_json::Value,
        state_digest: String,
        proposal: CheckedOwnedWaitProposalV8,
    ) -> Self {
        Self {
            held,
            binding,
            sequence,
            bytes,
            cancellation,
            clock,
            domain,
            initial,
            deadline,
            state,
            state_digest,
            proposal,
        }
    }
}
/// Only this actor's original Authorize reservation ACK creates this right.
pub(crate) struct LiveAuthorizePermitV8<'j> {
    authority: LiveStageAuthorityV8<'j>,
    fuel: usize,
}
impl LiveAuthorizePermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.authority.validate()
    }
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.authority.binding
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
pub(super) enum LiveAuthorizeFailureOwnerV8 {
    Resumed(LiveResumedStateV8),
    Transfer(LiveStateTransferOutcomeV8),
    Transferred(LiveTransferredStateV8),
    Authorize(LiveAuthorizeOutcomeV8),
    Staged(LiveStagedAuthorizationV8),
}
pub(super) struct LiveAuthorizeFailureV8<'j> {
    owner: LiveAuthorizeFailureOwnerV8,
    held: HeldOwnedWaitStoreV8<'j>,
    error: SourceJournalError,
}
#[cfg(test)]
impl LiveAuthorizeFailureV8<'_> {
    pub(super) fn recovered_test_backings(&self) -> Vec<std::sync::Weak<[u8]>> {
        match &self.owner {
            LiveAuthorizeFailureOwnerV8::Transferred(owner) => owner.test_weak(),
            LiveAuthorizeFailureOwnerV8::Staged(owner) => owner.test_weak(),
            LiveAuthorizeFailureOwnerV8::Authorize(outcome) => match outcome {
                LiveAuthorizeOutcomeV8::Refused(owner) => owner.test_weak(),
                LiveAuthorizeOutcomeV8::Staged(owner)
                | LiveAuthorizeOutcomeV8::Failed(owner)
                | LiveAuthorizeOutcomeV8::GuardLost(owner) => owner.test_weak(),
            },
            _ => panic!("recovery never reconstructs an earlier helper phase"),
        }
    }
}
/// Full actual Decision is still staged, never an authorization permission.
pub(super) struct StagedLiveOwnedRunV8<'j> {
    pub(super) owner: LiveStagedAuthorizationV8,
    pub(super) session: AppendSessionV8<'j>,
    pub(super) held: HeldOwnedWaitStoreV8<'j>,
    pub(super) journal: &'j SourceOwnedWaitJournalV8,
    pub(super) proposal: CheckedOwnedWaitProposalV8,
    pub(super) transfer: u32,
    pub(super) reservation: u32,
    pub(super) staged: u32,
    pub(super) cancellation: &'j crate::agent_runtime::AgentCancellation,
    pub(super) clock: &'j dyn SourceInvocationClock,
    #[cfg(test)]
    transfer_digest: String,
}
fn append_failure_error(failure: &AppendFailureV8<'_>) -> SourceJournalError {
    match failure {
        AppendFailureV8::PhysicalBeforeCandidate { error, .. }
        | AppendFailureV8::CandidateRefused { error, .. }
        | AppendFailureV8::PrewriteRefused { error, .. } => *error,
        // Once a physical append was attempted, exact persistence is in doubt.
        AppendFailureV8::InDoubt { .. } => SourceJournalError::Uncertain,
    }
}
pub(super) fn authorize_live_actor_v8<'j>(
    completed: CompletedLiveOwnedRunV8<'j>,
) -> Result<StagedLiveOwnedRunV8<'j>, LiveAuthorizeFailureV8<'j>> {
    let CompletedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        proposal,
        completed,
        cancellation,
        wait,
        clock,
    } = completed;
    macro_rules! fail {
        ($owner:expr, $error:expr) => {
            LiveAuthorizeFailureV8 {
                owner: $owner,
                held,
                error: $error,
            }
        };
    }
    let context = journal.context();
    let Some((_, execution)) = context.ready_runtime() else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Resumed(owner),
            SourceJournalError::Binding
        ));
    };
    let binding = execution.wait();
    macro_rules! guard {
        ($session:expr, $owner:expr) => {
            if let Err(error) = check_clock_v8(
                &held,
                $session.sequence(),
                $session.acknowledged_bytes(),
                cancellation,
                clock,
                context.ordinary().clock_domain(),
                context.ordinary().initial_millis(),
                context.ordinary().deadline_millis(),
            ) {
                return Err(fail!($owner, error));
            }
        };
    }
    guard!(session, LiveAuthorizeFailureOwnerV8::Resumed(owner));
    let scope = &held.registration().expected_facts().scope;
    let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
    let Some(state) = owner.checked_facts(binding) else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Resumed(owner),
            SourceJournalError::Binding
        ));
    };
    if session.sequence() != completed as usize + 1
        || !proposal.matches(binding.binding(), &scope)
        || !owner.transfer_ready(binding, &proposal)
    {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Resumed(owner),
            SourceJournalError::Binding
        ));
    }
    let state_digest = wire::record_argument_digest(&state);
    let proposal_digest = proposal.ordinary_digest().to_owned();
    let from = binding.helper().function().id.as_str().to_owned();
    let to = binding.authorize().function().id.as_str().to_owned();
    let transfer_digest = match wire::recipe_digest(
        wire::RecipeV8::Transfer,
        &serde_json::json!({
            "scope":scope,"generation":held.generation(),"turn":0,"attempt":0,"wait":wait,
            "from":from,"to":to,"state_digest":state_digest,"proposal_digest":proposal_digest
        }),
    ) {
        Ok(digest) => digest,
        Err(error) => return Err(fail!(LiveAuthorizeFailureOwnerV8::Resumed(owner), error)),
    };
    #[cfg(test)]
    let selected_transfer_digest = transfer_digest.clone();
    let session = match session.append(EntryV8::Ordinary(SourceJournalEntry::ProposalAdmitted {
        turn: 0,
        attempt: 0,
        proposal_digest: proposal_digest.clone(),
    })) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Resumed(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Resumed(owner));
    let transfer_reservation = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedStateTransferReserved {
            turn: 0,
            attempt: 0,
            wait: wait.clone(),
            from,
            to,
            state_digest: state_digest.clone(),
            proposal_digest: proposal_digest.clone(),
            transfer_digest: transfer_digest.clone(),
        },
    )) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Resumed(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Resumed(owner));
    let authority = |held| LiveStageAuthorityV8 {
        held,
        binding,
        sequence: session.sequence(),
        bytes: session.acknowledged_bytes(),
        cancellation,
        clock,
        domain: context.ordinary().clock_domain(),
        initial: context.ordinary().initial_millis(),
        deadline: context.ordinary().deadline_millis(),
    };
    let permit_hold = match journal.hold() {
        Ok(held) => held,
        Err(error) => return Err(fail!(LiveAuthorizeFailureOwnerV8::Resumed(owner), error)),
    };
    let moved = transfer_live_owned_state_v8(
        LiveStateTransferPermitV8 {
            authority: authority(permit_hold),
        },
        owner,
        &proposal,
    );
    let LiveStateTransferOutcomeV8::Moved(owner) = moved else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transfer(moved),
            SourceJournalError::Binding
        ));
    };
    let Some(moved_state) = owner.checked_facts(binding, &proposal) else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    };
    if moved_state != state {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    }
    guard!(session, LiveAuthorizeFailureOwnerV8::Transferred(owner));
    let transfer = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedStateTransferCompleted {
            turn: 0,
            attempt: 0,
            wait,
            reservation: transfer_reservation,
            state: moved_state,
            state_digest: state_digest.clone(),
            proposal: proposal.value().clone(),
            proposal_digest: proposal_digest.clone(),
            transfer_digest,
        },
    )) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Transferred(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Transferred(owner));
    let Some(fuel) = context.ordinary().max_steps_per_stage() else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    };
    if fuel != execution.evaluation_fuel() {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    }
    let reservation = session.sequence() as u32;
    let session = match session.append(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: Some(0),
        role: super::super::super::SourceStageRole::Authorize,
        fuel,
    })) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Transferred(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Transferred(owner));
    let permit_hold = match journal.hold() {
        Ok(held) => held,
        Err(error) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Transferred(owner),
                error
            ))
        }
    };
    let authority = LiveStageAuthorityV8 {
        held: permit_hold,
        binding,
        sequence: session.sequence(),
        bytes: session.acknowledged_bytes(),
        cancellation,
        clock,
        domain: context.ordinary().clock_domain(),
        initial: context.ordinary().initial_millis(),
        deadline: context.ordinary().deadline_millis(),
    };
    let outcome = authorize_live_owned_state_v8(LiveAuthorizePermitV8 { authority, fuel }, owner);
    let LiveAuthorizeOutcomeV8::Staged(owner) = outcome else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Authorize(outcome),
            SourceJournalError::Binding
        ));
    };
    let Some((authorized_state, decision)) = owner.checked_facts(binding) else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Staged(owner),
            SourceJournalError::Binding
        ));
    };
    if authorized_state != state {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Staged(owner),
            SourceJournalError::Binding
        ));
    }
    guard!(session, LiveAuthorizeFailureOwnerV8::Staged(owner));
    let decision_digest = match wire::recipe_digest(
        wire::RecipeV8::Decision,
        &serde_json::json!({
            "scope":scope,"turn":0,"attempt":0,"authorize":binding.authorize().function().id.as_str(),"decision":decision
        }),
    ) {
        Ok(digest) => digest,
        Err(error) => return Err(fail!(LiveAuthorizeFailureOwnerV8::Staged(owner), error)),
    };
    let staged = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedAuthorizationStaged {
            turn: 0,
            attempt: 0,
            stage_reservation: reservation,
            transfer,
            state_digest,
            proposal_digest,
            decision,
            decision_digest,
            consumed: owner.consumed(),
        },
    )) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Staged(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Staged(owner));
    Ok(StagedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        proposal,
        transfer,
        reservation,
        staged,
        cancellation,
        clock,
        #[cfg(test)]
        transfer_digest: selected_transfer_digest,
    })
}

/// Resume only after the exact TransferCompleted tail has been authenticated
/// and its physical State has been reconstructed. The reservation is ordinary
/// Authorize fuel and this invokes the same checked evaluator as live flow.
pub(super) fn authorize_recovered_actor_v8<'j>(
    owner: LiveTransferredStateV8,
    session: AppendSessionV8<'j>,
    held: HeldOwnedWaitStoreV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    proposal: CheckedOwnedWaitProposalV8,
    transfer: u32,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn SourceInvocationClock,
) -> Result<StagedLiveOwnedRunV8<'j>, LiveAuthorizeFailureV8<'j>> {
    macro_rules! fail {
        ($owner:expr, $error:expr) => {
            LiveAuthorizeFailureV8 {
                owner: $owner,
                held,
                error: $error,
            }
        };
    }
    let context = journal.context();
    let Some((_, execution)) = context.ready_runtime() else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    };
    let binding = execution.wait();
    macro_rules! guard {
        ($session:expr, $owner:expr) => {
            if let Err(error) = check_clock_v8(
                &held,
                $session.sequence(),
                $session.acknowledged_bytes(),
                cancellation,
                clock,
                context.ordinary().clock_domain(),
                context.ordinary().initial_millis(),
                context.ordinary().deadline_millis(),
            ) {
                return Err(fail!($owner, error));
            }
        };
    }
    guard!(session, LiveAuthorizeFailureOwnerV8::Transferred(owner));
    let scope_ref = &held.registration().expected_facts().scope;
    let scope = serde_json::json!({"program_root":scope_ref.program_root(),"invocation":scope_ref.invocation_id(),"policy_epoch":scope_ref.policy_epoch()});
    let Some(state) = owner.checked_facts(binding, &proposal) else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    };
    if session.sequence() != transfer as usize + 1 || !proposal.matches(binding.binding(), &scope) {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    }
    let state_digest = wire::record_argument_digest(&state);
    let proposal_digest = proposal.ordinary_digest().to_owned();
    let Some(fuel) = context.ordinary().max_steps_per_stage() else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    };
    if fuel != execution.evaluation_fuel() {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Transferred(owner),
            SourceJournalError::Binding
        ));
    }
    let reservation = session.sequence() as u32;
    let session = match session.append(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: Some(0),
        role: super::super::super::SourceStageRole::Authorize,
        fuel,
    })) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Transferred(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Transferred(owner));
    let permit_hold = match journal.hold() {
        Ok(held) => held,
        Err(error) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Transferred(owner),
                error
            ))
        }
    };
    let authority = LiveStageAuthorityV8 {
        held: permit_hold,
        binding,
        sequence: session.sequence(),
        bytes: session.acknowledged_bytes(),
        cancellation,
        clock,
        domain: context.ordinary().clock_domain(),
        initial: context.ordinary().initial_millis(),
        deadline: context.ordinary().deadline_millis(),
    };
    let outcome = authorize_live_owned_state_v8(LiveAuthorizePermitV8 { authority, fuel }, owner);
    let LiveAuthorizeOutcomeV8::Staged(owner) = outcome else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Authorize(outcome),
            SourceJournalError::Binding
        ));
    };
    let Some((authorized_state, decision)) = owner.checked_facts(binding) else {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Staged(owner),
            SourceJournalError::Binding
        ));
    };
    if authorized_state != state {
        return Err(fail!(
            LiveAuthorizeFailureOwnerV8::Staged(owner),
            SourceJournalError::Binding
        ));
    }
    guard!(session, LiveAuthorizeFailureOwnerV8::Staged(owner));
    let decision_digest = match wire::recipe_digest(
        wire::RecipeV8::Decision,
        &serde_json::json!({
            "scope":scope,"turn":0,"attempt":0,"authorize":binding.authorize().function().id.as_str(),"decision":decision
        }),
    ) {
        Ok(digest) => digest,
        Err(error) => return Err(fail!(LiveAuthorizeFailureOwnerV8::Staged(owner), error)),
    };
    let staged = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedAuthorizationStaged {
            turn: 0,
            attempt: 0,
            stage_reservation: reservation,
            transfer,
            state_digest,
            proposal_digest,
            decision,
            decision_digest,
            consumed: owner.consumed(),
        },
    )) {
        Ok(session) => session,
        Err(failure) => {
            return Err(fail!(
                LiveAuthorizeFailureOwnerV8::Staged(owner),
                append_failure_error(&failure)
            ))
        }
    };
    guard!(session, LiveAuthorizeFailureOwnerV8::Staged(owner));
    Ok(StagedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        proposal,
        transfer,
        reservation,
        staged,
        cancellation,
        clock,
        #[cfg(test)]
        transfer_digest: String::new(),
    })
}

#[cfg(all(test, unix))]
mod tests;
