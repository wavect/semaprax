//! First live effect obligation, produced only from the actual source actor.
//! Inert Ready metadata remains legal; it cannot mint this lineage or an ACK.
use super::super::append::owned_effect::VerifiedOwnedEffectReadySuccessorV8;
use super::authorize::StagedLiveOwnedRunV8;
use super::model::check_clock_v8;
use super::*;
use crate::agent_lifecycle::authorization::{
    checked_owned_wait_ready_commitments_v8, CheckedOwnedWaitReadyCommitmentsV8,
};
use crate::agent_lifecycle::iterative::effects::plan_owned_effect_v8;
use crate::resumable_effects::CapabilityPolicy;

/// No raw Ready/facts/sequence constructor and no physical ACK or owner getter.
/// The future fixed adapter must keep this actual obligation inside its witness.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedEffectAppendV8<'j> {
    owner: StagedLiveOwnedRunV8<'j>,
    policy: &'j CapabilityPolicy,
    selected: EntryV8,
    commitments: CheckedOwnedWaitReadyCommitmentsV8,
}
pub(super) struct LiveEffectPreparationFailureV8<'j> {
    owner: StagedLiveOwnedRunV8<'j>,
    error: SourceJournalError,
}
impl LiveOwnedEffectAppendV8<'_> {
    /// Pure source facts do not create a grant or authorize generic append ACKs.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.owner.journal, journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        validate(&self.owner, self.policy)?;
        self.validate_facts()?;
        validate(&self.owner, self.policy)
    }
    /// Only the fixed adapter can mint this post-append witness. The original
    /// session remains unchanged; no caller prefix or snapshot can rebase it.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_ready_successor(
        &self,
        witness: &VerifiedOwnedEffectReadySuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.owner.journal,
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        validate_at(
            &self.owner,
            self.policy,
            witness.sequence(),
            witness.acknowledged_bytes(),
        )?;
        self.validate_facts()?;
        validate_at(
            &self.owner,
            self.policy,
            witness.sequence(),
            witness.acknowledged_bytes(),
        )?;
        witness.validate_predecessor(
            self.owner.journal,
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )
    }
    fn validate_facts(&self) -> Result<(), SourceJournalError> {
        let (selected, commitments) = ready_facts(&self.owner, self.policy)?;
        if selected != self.selected
            || commitments.authorization_binding() != self.commitments.authorization_binding()
            || commitments.grant_digest() != self.commitments.grant_digest()
            || commitments.target_grant_digest() != self.commitments.target_grant_digest()
            || commitments.argument_digest() != self.commitments.argument_digest()
            || commitments.budget() != self.commitments.budget()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
fn validate(
    owner: &StagedLiveOwnedRunV8<'_>,
    policy: &CapabilityPolicy,
) -> Result<(), SourceJournalError> {
    validate_at(
        owner,
        policy,
        owner.session.sequence(),
        owner.session.acknowledged_bytes(),
    )
}
fn validate_at(
    owner: &StagedLiveOwnedRunV8<'_>,
    policy: &CapabilityPolicy,
    sequence: usize,
    bytes: usize,
) -> Result<(), SourceJournalError> {
    let context = owner.journal.context();
    if owner.session.sequence() != owner.staged as usize + 1 {
        return Err(SourceJournalError::Order);
    }
    check_clock_v8(
        &owner.held,
        sequence,
        bytes,
        owner.cancellation,
        owner.clock,
        context.ordinary().clock_domain(),
        context.ordinary().initial_millis(),
        context.ordinary().deadline_millis(),
    )?;
    let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
    let scope = &owner.held.registration().expected_facts().scope;
    let plan = plan_owned_effect_v8(runtime, execution, scope, &owner.proposal)
        .map_err(|_| SourceJournalError::Binding)?;
    if !policy.allows(plan.operation().effect_id()) {
        return Err(SourceJournalError::Binding);
    }
    owner.held.validate_prefix(sequence, bytes)
}
fn ready_facts(
    owner: &StagedLiveOwnedRunV8<'_>,
    policy: &CapabilityPolicy,
) -> Result<(EntryV8, CheckedOwnedWaitReadyCommitmentsV8), SourceJournalError> {
    let context = owner.journal.context();
    let (runtime, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
    let binding = execution.wait();
    let (state, decision) = owner
        .owner
        .checked_facts(binding)
        .ok_or(SourceJournalError::Binding)?;
    let actions = binding.authorize().disposal();
    // This bounded producer has one actual Granted seal operation. Its future
    // receipt may use a single observed outcome, never invent a vector from a bool.
    if decision["case"].as_str() != Some(binding.authorize().granted().as_str())
        || actions.len() != 1
        || !actions[0]
            .active_case
            .as_ref()
            .is_some_and(|case| case.case == *binding.authorize().granted())
    {
        return Err(SourceJournalError::Binding);
    }
    let scope = &owner.held.registration().expected_facts().scope;
    let plan = plan_owned_effect_v8(runtime, execution, scope, &owner.proposal)
        .map_err(|_| SourceJournalError::Binding)?;
    if !policy.allows(plan.operation().effect_id()) {
        return Err(SourceJournalError::Binding);
    }
    let commitments = checked_owned_wait_ready_commitments_v8(
        runtime,
        execution,
        scope,
        0,
        0,
        &state,
        &decision,
        &owner.proposal,
    )?;
    if commitments.budget() < 1 {
        return Err(SourceJournalError::Binding);
    }
    let decision_digest = wire::recipe_digest(
        wire::RecipeV8::Decision,
        &serde_json::json!({
            "scope":{"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()},
            "turn":0,"attempt":0,"authorize":binding.authorize().function().id.as_str(),"decision":decision
        }),
    )?;
    Ok((
        EntryV8::Owned(journal_model::OwnedBodyV8::OwnedAuthorizationReady {
            turn: 0,
            attempt: 0,
            staged: owner.staged,
            state_digest: wire::record_argument_digest(&state),
            decision_digest,
            grant_digest: commitments.grant_digest().into(),
        }),
        commitments,
    ))
}
pub(super) fn prepare_live_effect_ready_v8<'j>(
    owner: StagedLiveOwnedRunV8<'j>,
    policy: &'j CapabilityPolicy,
) -> Result<LiveOwnedEffectAppendV8<'j>, LiveEffectPreparationFailureV8<'j>> {
    if let Err(error) = validate(&owner, policy) {
        return Err(LiveEffectPreparationFailureV8 { owner, error });
    }
    let (selected, commitments) = match ready_facts(&owner, policy) {
        Ok(facts) => facts,
        Err(error) => return Err(LiveEffectPreparationFailureV8 { owner, error }),
    };
    let obligation = LiveOwnedEffectAppendV8 {
        owner,
        policy,
        selected,
        commitments,
    };
    if let Err(error) = obligation.validate_live() {
        return Err(LiveEffectPreparationFailureV8 {
            owner: obligation.owner,
            error,
        });
    }
    Ok(obligation)
}
#[cfg(test)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod tests;
