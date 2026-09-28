//! Private successor effect engine. The sealed held-Consumed handoff constructs
//! authorization and Intent ACKs; settlement and cleanup ACK producers remain
//! test-only until their actual fixed live append consumers are implemented.
use super::authorize::{
    HeldOwnedEffectAuthorizationV8, OwnedEffectDecisionReleaseV8, OwnedEffectReleasedRootsV8,
    ReadyOwnedAuthorizeV2,
};
use super::*;
use crate::agent_lifecycle::authorization::{
    checked_owned_wait_ready_commitments_v8,
    target_protocol::{
        self, Settlement, TargetAccounting, TargetDispatch, TargetHostHandler, TargetLimits,
        TargetOperation, TypedCarrier,
    },
};
use crate::agent_lifecycle::iterative::effects::{plan_owned_effect_v8, CheckedOwnedEffectPlanV8};
use crate::agent_runtime::AgentCancellation;
use crate::execution_revision::typed::{AgentRuntimeV2, CheckedTypedOwnedWaitExecutionV8};
use crate::live_invocation::source_journal::{HeldOwnedWaitStoreV8, SourceEffectFailure};
use crate::resumable_effects::capability::CapabilityPolicy;
use crate::resumable_effects::owned_frame::v2::{
    owned_wait_operations_v8, CheckedOwnedAgentWaitBindingV8, CheckedOwnedWaitProposalV8,
};

/// Inputs are actual checked borrowers. Inert facts cannot replace the sealed
/// store handle or the non-Clone authorization ACK.
pub(crate) struct OwnedEffectInputsV8<'a> {
    pub(crate) runtime: &'a AgentRuntimeV2,
    pub(crate) execution: &'a CheckedTypedOwnedWaitExecutionV8,
    pub(crate) proposal: CheckedOwnedWaitProposalV8,
    pub(crate) store: HeldOwnedWaitStoreV8<'a>,
    pub(crate) policy: &'a CapabilityPolicy,
    pub(crate) cancellation: &'a AgentCancellation,
    pub(crate) turn: u32,
    pub(crate) attempt: u32,
}
#[derive(Clone, Debug, PartialEq)]
struct AuthorizationBasisV8 {
    generation: String,
    execution: String,
    binding: String,
    invocation: String,
    epoch: u64,
    turn: u32,
    attempt: u32,
    state: serde_json::Value,
    decision: serde_json::Value,
    proposal: String,
    authorization: String,
    grant: String,
    target_grant: String,
    argument: String,
}
/// One-use authorization authority constructed only by the private live_append
/// child from the actual Ready owner and sealed held-Consumed permit. Intent
/// has its own sealed successor producer; later settlement/cleanup ACKs remain closed.
pub(crate) struct OwnedEffectAuthorizationAckV8 {
    basis: AuthorizationBasisV8,
    staged: u32,
    ready: u32,
    consumed: u32,
}
pub(crate) struct OwnedEffectIntentAckV8 {
    basis: AuthorizationBasisV8,
    authorization: u32,
    intent: u32,
    request: String,
    operation: String,
}
pub(crate) struct OwnedEffectSettlementAckV8 {
    basis: AuthorizationBasisV8,
    intent: u32,
    settlement: u32,
    evidence: String,
    operation: String,
    observation: Option<Vec<u8>>,
    reason: Option<SourceEffectFailure>,
}
pub(crate) struct OwnedEffectCleanupStartedAckV8 {
    basis: AuthorizationBasisV8,
    settlement: u32,
    recorded: u32,
    evidence: String,
    started: u32,
    operations: serde_json::Value,
    staged: u32,
    ready: u32,
    consumed: u32,
    intent: u32,
}
pub(crate) struct OwnedEffectCleanupSettledAckV8 {
    basis: AuthorizationBasisV8,
    started: u32,
    settled: u32,
    receipt: serde_json::Value,
}

/// Read-only phase data lets the trusted caller check its actual current tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedEffectPhaseV8 {
    Authorization(u32),
    Intent(u32),
    Settlement(u32),
    CleanupStarted(u32),
    CleanupSettled(u32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedEffectFailureV8 {
    Cancelled,
    AuthorityLost,
    Target(Settlement),
    ResultShape,
    ObservationFailed,
}

pub(crate) struct PreparedOwnedEffectV8<'a> {
    owner: HeldOwnedEffectAuthorizationV8,
    plan: CheckedOwnedEffectPlanV8<'a>,
    inputs: OwnedEffectInputsV8<'a>,
    basis: AuthorizationBasisV8,
    authorization_tail: u32,
    staged: u32,
    ready: u32,
    budget: i64,
    request: OwnedEffectTargetRequestV8,
    creator: u32,
}
pub(crate) struct OwnedEffectPreparationRejectionV8<'a> {
    pub(crate) ready: ReadyOwnedAuthorizeV2,
    pub(crate) inputs: OwnedEffectInputsV8<'a>,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct OwnedEffectDispatchRejectionV8<'a> {
    pub(crate) prepared: PreparedOwnedEffectV8<'a>,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct StagedOwnedEffectV8<'a> {
    prepared: PreparedOwnedEffectV8<'a>,
    intent: u32,
    dispatch: Option<TargetDispatch>,
    accepted: Option<Vec<u8>>,
    failure: Option<OwnedEffectFailureV8>,
    cleanup_started: bool,
    authority_lost: bool,
}
impl StagedOwnedEffectV8<'_> {
    pub(crate) fn failure(&self) -> Option<OwnedEffectFailureV8> {
        self.failure
    }
    pub(crate) fn cleanup_started(&self) -> bool {
        self.cleanup_started
    }
    pub(crate) fn observation(&self) -> Option<&[u8]> {
        self.accepted.as_deref()
    }
    pub(crate) fn reason(&self) -> Option<SourceEffectFailure> {
        match self.failure? {
            OwnedEffectFailureV8::Cancelled => Some(SourceEffectFailure::Cancelled),
            OwnedEffectFailureV8::Target(
                Settlement::Cancelled | Settlement::CancelledAfterDispatch,
            ) => Some(SourceEffectFailure::Cancelled),
            OwnedEffectFailureV8::Target(Settlement::ResultBudget) => {
                Some(SourceEffectFailure::ResultLimit)
            }
            OwnedEffectFailureV8::Target(_) | OwnedEffectFailureV8::ResultShape => {
                Some(SourceEffectFailure::HandlerFailed)
            }
            OwnedEffectFailureV8::AuthorityLost | OwnedEffectFailureV8::ObservationFailed => None,
        }
    }
    pub(crate) fn dispatch(&self) -> Option<&TargetDispatch> {
        self.dispatch.as_ref()
    }
    /// Inert existing target observation bytes for the future journal row.
    /// These bytes carry no ACK, dispatch permit or runtime owner.
    pub(crate) fn target_evidence_wire(&self) -> Option<Vec<u8>> {
        self.dispatch
            .as_ref()
            .map(|d| d.evidence().canonical_wire())
    }
    pub(crate) fn target_result_wire(&self) -> Option<Vec<u8>> {
        self.dispatch.as_ref()?.result().map(TypedCarrier::encode)
    }
}
/// Actual release has happened; this holder grants no reducer handoff until
/// the exact post-release receipt is acknowledged by a later causal row.
pub(crate) struct PendingOwnedEffectReceiptV8<'a> {
    released: OwnedEffectDecisionReleaseV8,
    plan: CheckedOwnedEffectPlanV8<'a>,
    inputs: OwnedEffectInputsV8<'a>,
    basis: AuthorizationBasisV8,
    started: u32,
    receipt: serde_json::Value,
    accepted: Option<Vec<u8>>,
    failure: Option<OwnedEffectFailureV8>,
    creator: u32,
}
impl PendingOwnedEffectReceiptV8<'_> {
    pub(crate) fn receipt(&self) -> &serde_json::Value {
        &self.receipt
    }
    pub(crate) fn failure(&self) -> Option<OwnedEffectFailureV8> {
        self.failure
    }
}
pub(crate) struct OwnedEffectReleaseRejectionV8<'a> {
    pub(crate) staged: StagedOwnedEffectV8<'a>,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct OwnedEffectCompletionRejectionV8<'a> {
    pub(crate) pending: PendingOwnedEffectReceiptV8<'a>,
    pub(crate) diagnostic: Diagnostic,
}
/// Roots precede the borrowed session handle, retaining backing under the same
/// pinned lease. No public owner, restore, claim or raw Outcome constructor.
pub(crate) struct ExecutedOwnedAgentTurnV2<'a> {
    roots: OwnedEffectReleasedRootsV8,
    binding: Arc<CheckedOwnedAgentWaitBindingV8>,
    inputs: OwnedEffectInputsV8<'a>,
    settled: u32,
}

impl<'a> ExecutedOwnedAgentTurnV2<'a> {
    /// A consuming next-stage handoff keeps the sealed store borrower beside
    /// the roots. It is not a public owner extraction or a restoration route.
    pub(super) fn into_reduce_parts(
        self,
        plan: &crate::resumable_effects::owned_frame::v2::CheckedOwnedReduceV2,
        mut check: impl FnMut(OwnedEffectPhaseV8) -> bool,
    ) -> Result<(OwnedEffectReleasedRootsV8, OwnedEffectInputsV8<'a>, u32), Self> {
        let Some(state) = self.roots.state.as_ref() else {
            return Err(self);
        };
        let Some(outcome) = self.roots.outcome.as_ref() else {
            return Err(self);
        };
        let operation = match plan_owned_effect_v8(
            self.inputs.runtime,
            self.inputs.execution,
            &self.inputs.store.registration().expected_facts().scope,
            &self.inputs.proposal,
        ) {
            Ok(plan) => plan,
            Err(_) => return Err(self),
        };
        if self.roots.creator != std::process::id()
            || plan.binding() != self.binding.binding()
            || !plan.helper().same_helper(&self.roots.helper)
            || !root_valid(plan.helper(), state)
            || !super::reduce::outcome_valid(plan, outcome)
            || !self.roots.allocations.validate(&[state, outcome])
            || !current(
                &self.inputs,
                self.roots.creator,
                OwnedEffectPhaseV8::CleanupSettled(self.settled),
                &mut check,
                operation.operation().effect_id(),
            )
        {
            return Err(self);
        }
        Ok((self.roots, self.inputs, self.settled))
    }
}

/// This target metadata is inert. Only the non-Clone dispatch permit grants
/// the target child access to its private metadata grant constructor.
pub(crate) struct OwnedEffectTargetRequestV8 {
    grant: String,
    authorization: String,
    operation: TargetOperation,
    argument: TypedCarrier,
    turn: u64,
    limits: TargetLimits,
}
impl OwnedEffectTargetRequestV8 {
    pub(crate) fn grant(&self) -> &str {
        &self.grant
    }
    pub(crate) fn authorization(&self) -> &str {
        &self.authorization
    }
    pub(crate) fn operation(&self) -> &TargetOperation {
        &self.operation
    }
    pub(crate) fn argument(&self) -> &TypedCarrier {
        &self.argument
    }
    pub(crate) fn turn(&self) -> u64 {
        self.turn
    }
    pub(crate) fn limits(&self) -> TargetLimits {
        self.limits
    }
}
pub(crate) struct OwnedEffectDispatchPermitV8 {
    request: OwnedEffectTargetRequestV8,
    argument_digest: String,
    budget: i64,
}
impl OwnedEffectDispatchPermitV8 {
    pub(crate) fn consume(self) -> (OwnedEffectTargetRequestV8, String, i64) {
        (self.request, self.argument_digest, self.budget)
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum LiveGuardV8 {
    Current,
    Cancelled,
    AuthorityLost,
}
fn guard_status(
    inputs: &OwnedEffectInputsV8<'_>,
    creator: u32,
    phase: OwnedEffectPhaseV8,
    check: &mut impl FnMut(OwnedEffectPhaseV8) -> bool,
    effect: &str,
) -> LiveGuardV8 {
    let cleanup = matches!(phase, OwnedEffectPhaseV8::CleanupStarted(_));
    if creator != std::process::id()
        || inputs.store.validate_guard().is_err()
        || !inputs.policy.allows(effect)
    {
        return LiveGuardV8::AuthorityLost;
    }
    if !cleanup && inputs.cancellation.is_cancelled() {
        return LiveGuardV8::Cancelled;
    }
    let allowed = check(phase);
    // Keep the callback's actual refusal independently of cancellation. A
    // simultaneous false+cancel can never be reclassified as cleanup permission.
    if !allowed
        || creator != std::process::id()
        || inputs.store.validate_guard().is_err()
        || !inputs.policy.allows(effect)
    {
        return LiveGuardV8::AuthorityLost;
    }
    if !cleanup && inputs.cancellation.is_cancelled() {
        LiveGuardV8::Cancelled
    } else {
        LiveGuardV8::Current
    }
}
fn current(
    inputs: &OwnedEffectInputsV8<'_>,
    creator: u32,
    phase: OwnedEffectPhaseV8,
    check: &mut impl FnMut(OwnedEffectPhaseV8) -> bool,
    effect: &str,
) -> bool {
    guard_status(inputs, creator, phase, check, effect) == LiveGuardV8::Current
}
pub(super) fn reducer_guard(
    inputs: &OwnedEffectInputsV8<'_>,
    creator: u32,
    settled: u32,
    check: &mut impl FnMut(OwnedEffectPhaseV8) -> bool,
) -> bool {
    let plan = match plan_owned_effect_v8(
        inputs.runtime,
        inputs.execution,
        &inputs.store.registration().expected_facts().scope,
        &inputs.proposal,
    ) {
        Ok(plan) => plan,
        Err(_) => return false,
    };
    current(
        inputs,
        creator,
        OwnedEffectPhaseV8::CleanupSettled(settled),
        check,
        plan.operation().effect_id(),
    )
}
fn checked_basis(
    inputs: &OwnedEffectInputsV8<'_>,
    owner: &HeldOwnedEffectAuthorizationV8,
) -> Option<(AuthorizationBasisV8, i64)> {
    checked_basis_facts(inputs, owner.facts()?)
}
fn checked_basis_facts(
    inputs: &OwnedEffectInputsV8<'_>,
    (state, decision, budget): (serde_json::Value, serde_json::Value, i64),
) -> Option<(AuthorizationBasisV8, i64)> {
    inputs.store.validate_guard().ok()?;
    let registration = inputs.store.registration();
    let facts = registration.expected_facts();
    let commitments = checked_owned_wait_ready_commitments_v8(
        inputs.runtime,
        inputs.execution,
        &facts.scope,
        inputs.turn,
        inputs.attempt,
        &state,
        &decision,
        &inputs.proposal,
    )
    .ok()?;
    if facts.execution != inputs.execution.ordinary().invocation()
        || facts.binding != inputs.execution.wait().binding()
        || inputs.turn >= inputs.execution.ordinary().max_stages()
        || inputs.attempt >= inputs.execution.ordinary().max_attempts()
    {
        return None;
    }
    Some((
        AuthorizationBasisV8 {
            generation: inputs.store.generation().into(),
            execution: facts.execution.clone(),
            binding: facts.binding.clone(),
            invocation: facts.scope.invocation_id().into(),
            epoch: facts.scope.policy_epoch(),
            turn: inputs.turn,
            attempt: inputs.attempt,
            state,
            decision,
            proposal: inputs.proposal.ordinary_digest().into(),
            authorization: commitments.authorization_binding().into(),
            grant: commitments.grant_digest().into(),
            target_grant: commitments.target_grant_digest().into(),
            argument: commitments.argument_digest().into(),
        },
        budget,
    ))
}
pub(crate) fn prepare_owned_effect_v8<'a>(
    inputs: OwnedEffectInputsV8<'a>,
    ready: ReadyOwnedAuthorizeV2,
    ack: OwnedEffectAuthorizationAckV8,
    mut check: impl FnMut(OwnedEffectPhaseV8) -> bool,
) -> Result<PreparedOwnedEffectV8<'a>, OwnedEffectPreparationRejectionV8<'a>> {
    let facts = inputs.store.registration().expected_facts();
    let plan = match plan_owned_effect_v8(
        inputs.runtime,
        inputs.execution,
        &facts.scope,
        &inputs.proposal,
    ) {
        Ok(plan) => plan,
        Err(diagnostic) => {
            return Err(OwnedEffectPreparationRejectionV8 {
                ready,
                inputs,
                diagnostic: diagnostic
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| rejected("effect plan differs")),
            })
        }
    };
    let creator = std::process::id();
    if ack.staged >= ack.ready
        || ack.ready >= ack.consumed
        || !current(
            &inputs,
            creator,
            OwnedEffectPhaseV8::Authorization(ack.consumed),
            &mut check,
            plan.operation().effect_id(),
        )
    {
        return Err(OwnedEffectPreparationRejectionV8 {
            ready,
            inputs,
            diagnostic: rejected("effect authorization ACK/current authority differs"),
        });
    }
    let owner = match ready.hold_for_effect(
        inputs.execution.wait(),
        &inputs.proposal,
        &facts.scope,
        || true,
    ) {
        Ok(owner) => owner,
        Err(ready) => {
            return Err(OwnedEffectPreparationRejectionV8 {
                ready,
                inputs,
                diagnostic: rejected("effect physical authorization differs"),
            })
        }
    };
    let Some((basis, budget)) = checked_basis(&inputs, &owner) else {
        return Err(OwnedEffectPreparationRejectionV8 {
            ready: owner.into_ready(),
            inputs,
            diagnostic: rejected("effect checked authorization facts differ"),
        });
    };
    if ack.basis != basis {
        return Err(OwnedEffectPreparationRejectionV8 {
            ready: owner.into_ready(),
            inputs,
            diagnostic: rejected("effect authorization ACK binding differs"),
        });
    }
    let request = OwnedEffectTargetRequestV8 {
        grant: basis.target_grant.clone(),
        authorization: basis.authorization.clone(),
        operation: plan.operation().clone(),
        argument: plan.argument().clone(),
        turn: u64::from(inputs.turn),
        limits: plan.target_limits(),
    };
    Ok(PreparedOwnedEffectV8 {
        owner,
        plan,
        inputs,
        basis,
        authorization_tail: ack.consumed,
        staged: ack.staged,
        ready: ack.ready,
        budget,
        request,
        creator,
    })
}
pub(crate) fn dispatch_owned_effect_v8<'a>(
    prepared: PreparedOwnedEffectV8<'a>,
    ack: OwnedEffectIntentAckV8,
    accounting: &mut TargetAccounting,
    check: impl FnMut(OwnedEffectPhaseV8) -> bool,
    handler: &mut dyn TargetHostHandler,
) -> Result<StagedOwnedEffectV8<'a>, OwnedEffectDispatchRejectionV8<'a>> {
    let activated = live_append::intent::activate_ack_owned_effect_v8(prepared, ack)?;
    Ok(
        live_append::intent::dispatch::dispatch_activated_owned_effect_v8(
            activated, accounting, check, handler,
        ),
    )
}

pub(crate) fn release_owned_effect_decision_v8<'a>(
    mut staged: StagedOwnedEffectV8<'a>,
    settlement: OwnedEffectSettlementAckV8,
    start: OwnedEffectCleanupStartedAckV8,
    mut check: impl FnMut(OwnedEffectPhaseV8) -> bool,
    observe: impl FnMut(&FinalizeAction),
) -> Result<PendingOwnedEffectReceiptV8<'a>, OwnedEffectReleaseRejectionV8<'a>> {
    let operations = match owned_wait_operations_v8(
        staged
            .prepared
            .inputs
            .execution
            .wait()
            .authorize()
            .disposal(),
    ) {
        Ok(value) => value,
        Err(_) => {
            return Err(OwnedEffectReleaseRejectionV8 {
                staged,
                diagnostic: rejected("effect compiler operations differ"),
            })
        }
    };
    let valid = !staged.cleanup_started
        && !staged.authority_lost
        && staged.failure != Some(OwnedEffectFailureV8::AuthorityLost)
        && settlement.basis == staged.prepared.basis
        && settlement.intent == staged.intent
        && settlement.settlement > staged.intent
        && staged
            .dispatch
            .as_ref()
            .is_some_and(|d| d.evidence().digest() == settlement.evidence)
        && settlement.operation == staged.prepared.plan.operation().operation_id()
        && settlement.observation.as_deref() == staged.observation()
        && settlement.reason == staged.reason()
        && start.staged == staged.prepared.staged
        && start.ready == staged.prepared.ready
        && start.consumed == staged.prepared.authorization_tail
        && start.intent == staged.intent
        && start.basis == staged.prepared.basis
        && start.settlement == settlement.settlement
        && start.settlement.checked_add(1) == Some(start.recorded)
        && start.recorded.checked_add(1) == Some(start.started)
        && start.evidence == settlement.evidence
        && start.operations == operations;
    if !valid
        || !current(
            &staged.prepared.inputs,
            staged.prepared.creator,
            OwnedEffectPhaseV8::CleanupStarted(start.started),
            &mut check,
            staged.prepared.plan.operation().effect_id(),
        )
    {
        return Err(OwnedEffectReleaseRejectionV8 {
            staged,
            diagnostic: rejected("effect settlement/cleanup-start ACK or authority differs"),
        });
    }
    staged.cleanup_started = true;
    let prepared = staged.prepared;
    let released = match prepared.owner.release_decision(
        || {
            current(
                &prepared.inputs,
                prepared.creator,
                OwnedEffectPhaseV8::CleanupStarted(start.started),
                &mut check,
                prepared.plan.operation().effect_id(),
            )
        },
        observe,
    ) {
        Ok(released) => released,
        Err(error) => {
            return Err(OwnedEffectReleaseRejectionV8 {
                staged: StagedOwnedEffectV8 {
                    prepared: PreparedOwnedEffectV8 {
                        owner: error.holder,
                        ..prepared
                    },
                    intent: staged.intent,
                    dispatch: staged.dispatch,
                    accepted: staged.accepted,
                    failure: staged.failure,
                    cleanup_started: true,
                    authority_lost: staged.authority_lost,
                },
                diagnostic: error.diagnostic,
            })
        }
    };
    let actual = owned_wait_operations_v8(&released.operations)
        .expect("checked compiler operations serialize");
    let entries=actual.as_array().expect("checked operations array").iter().map(|operation|serde_json::json!({"operation":operation,"outcome":if released.observations_succeeded {"completed"}else{"failed"}})).collect::<Vec<_>>();
    let receipt = serde_json::json!({"kind":"observed","settlement":if released.observations_succeeded {"completed"}else{"failed"},"operations":entries});
    let failure = staged
        .failure
        .or((!released.observations_succeeded).then_some(OwnedEffectFailureV8::ObservationFailed));
    Ok(PendingOwnedEffectReceiptV8 {
        released,
        plan: prepared.plan,
        inputs: prepared.inputs,
        basis: prepared.basis,
        started: start.started,
        receipt,
        accepted: staged.accepted,
        failure,
        creator: prepared.creator,
    })
}
pub(crate) fn ack_owned_effect_cleanup_v8<'a>(
    pending: PendingOwnedEffectReceiptV8<'a>,
    ack: OwnedEffectCleanupSettledAckV8,
    mut check: impl FnMut(OwnedEffectPhaseV8) -> bool,
) -> Result<ExecutedOwnedAgentTurnV2<'a>, OwnedEffectCompletionRejectionV8<'a>> {
    if ack.basis != pending.basis
        || ack.started != pending.started
        || ack.settled <= pending.started
        || ack.receipt != pending.receipt
        || pending.failure.is_some()
        || pending.accepted.is_none()
        || !current(
            &pending.inputs,
            pending.creator,
            OwnedEffectPhaseV8::CleanupSettled(ack.settled),
            &mut check,
            pending.plan.operation().effect_id(),
        )
    {
        return Err(OwnedEffectCompletionRejectionV8 {
            pending,
            diagnostic: rejected("effect post-release receipt ACK/handoff differs"),
        });
    }
    let binding = pending.inputs.execution.wait_arc();
    let mut pending = pending;
    let payload = pending.accepted.take().expect("checked accepted result");
    let roots = match pending.released.into_outcome(&binding, payload, || {
        current(
            &pending.inputs,
            pending.creator,
            OwnedEffectPhaseV8::CleanupSettled(ack.settled),
            &mut check,
            pending.plan.operation().effect_id(),
        )
    }) {
        Ok(roots) => roots,
        Err((released, payload, diagnostic)) => {
            return Err(OwnedEffectCompletionRejectionV8 {
                pending: PendingOwnedEffectReceiptV8 {
                    released,
                    accepted: Some(payload),
                    ..pending
                },
                diagnostic,
            })
        }
    };
    Ok(ExecutedOwnedAgentTurnV2 {
        roots,
        binding,
        inputs: pending.inputs,
        settled: ack.settled,
    })
}
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(super) use tests::{
    with_staged_complete_reduce_v2, with_staged_effect_reduce_v2, with_staged_task_zero_reduce_v2,
};

pub(super) mod live_append;

pub(crate) use live_append::intent::{
    activate_live_owned_effect_v8, ActivatedOwnedEffectV8, LiveEffectActivationRejectionV8,
};

pub(crate) use live_append::intent::dispatch::dispatch_live_owned_effect_v8;

pub(crate) use live_append::CheckedLiveOwnedEffectSettlementV8;

pub(crate) use live_append::settlement::cleanup::{
    ack_live_owned_effect_cleanup_v8, release_live_owned_effect_decision_v8,
    LiveEffectDecisionReleaseFailureV8, LiveEffectOutcomeFailureV8,
};

pub(crate) use live_append::settlement::cleanup::{
    release_live_failed_effect_state_v8, LiveFailedEffectStateReleaseFailureV8,
    ReleasedFailedEffectStateV8,
};
