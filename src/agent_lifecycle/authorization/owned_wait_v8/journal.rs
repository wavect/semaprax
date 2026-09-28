//! Commitment consistency only; no proof of evaluation, ownership, or ACK.
use crate::agent_lifecycle::iterative::effects::plan_owned_effect_v8;
use crate::execution_revision::typed::{AgentRuntimeV2, CheckedTypedOwnedWaitExecutionV8};
use crate::live_invocation::source_journal::SourceJournalError as Error;
use crate::resumable_effects::owned_frame::v2::{self, CheckedOwnedWaitProposalV8};
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
use serde_json::Value;

/// These commitments are inert; they cannot be consumed as a physical grant.
pub(crate) struct CheckedOwnedWaitReadyCommitmentsV8 {
    authorization_binding: String,
    grant_digest: String,
    argument_digest: String,
}
impl CheckedOwnedWaitReadyCommitmentsV8 {
    pub(crate) fn authorization_binding(&self) -> &str {
        &self.authorization_binding
    }
    pub(crate) fn grant_digest(&self) -> &str {
        &self.grant_digest
    }
    pub(crate) fn argument_digest(&self) -> &str {
        &self.argument_digest
    }
}

pub(crate) fn checked_owned_wait_ready_commitments_v8(
    runtime: &AgentRuntimeV2,
    execution: &CheckedTypedOwnedWaitExecutionV8,
    scope: &SourceCheckpointScope,
    turn: u32,
    state: &Value,
    decision: &Value,
    proposal: &CheckedOwnedWaitProposalV8,
) -> Result<CheckedOwnedWaitReadyCommitmentsV8, Error> {
    let binding = execution.wait();
    v2::validate_owned_wait_decision_v8(binding, decision).map_err(|_| Error::Binding)?;
    let authorize = binding.authorize();
    if decision["case"] != authorize.granted().as_str() {
        return Err(Error::Binding);
    }
    let state = v2::ordinary_state_bytes(binding, state).map_err(|_| Error::Binding)?;
    let seal = decision["fields"]
        .as_array()
        .and_then(|fields| {
            fields
                .iter()
                .find(|field| field["identity"] == authorize.seal().as_str())
        })
        .and_then(|field| field["value"]["hex"].as_str())
        .and_then(crate::live_invocation::identity::unhex)
        .ok_or(Error::Binding)?;
    let plan =
        plan_owned_effect_v8(runtime, execution, scope, proposal).map_err(|_| Error::Binding)?;
    let policy = crate::live_invocation::identity::digest(
        b"semaprax.agent-iteration-policy.v2\0",
        format!("{}\0{turn}", binding.lifecycle().digest()).as_bytes(),
    );
    let authorization = super::binding_from_canonical_state(
        &policy,
        &state,
        proposal.canonical_proposal(),
        authorize.granted(),
        &seal,
    );
    let argument_digest =
        super::super::target_protocol::owned_wait_v8::argument_digest(plan.argument());
    let target = super::super::target_protocol::owned_wait_v8::grant_id(
        &authorization,
        &seal,
        scope.invocation_id(),
        Some(execution.ordinary().invocation()),
        u64::from(turn),
        plan.operation(),
        &argument_digest,
    );
    Ok(CheckedOwnedWaitReadyCommitmentsV8 {
        authorization_binding: authorization,
        grant_digest: target,
        argument_digest,
    })
}
