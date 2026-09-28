//! Pure successor settlement replay. These checked facts carry no grant,
//! dispatch permit, append ACK, cleanup authority or physical runtime owner.
use super::super::*;
use crate::agent_lifecycle::authorization::checked_owned_wait_ready_commitments_v8;
use crate::agent_lifecycle::iterative::effects::{plan_owned_effect_v8, CheckedOwnedEffectPlanV8};
use crate::execution_revision::typed::{AgentRuntimeV2, CheckedTypedOwnedWaitExecutionV8};
use crate::live_invocation::source_journal::{
    source_effect_digest, SourceEffectFailure, SourceJournalEntry, SourceJournalError as Error,
};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;

/// Borrowers of the genuine checked execution and its retained registry.
/// A caller-supplied digest cannot substitute for any of these checked inputs.
pub(crate) struct OwnedEffectSettlementInputsV8<'a> {
    pub(crate) runtime: &'a AgentRuntimeV2,
    pub(crate) execution: &'a CheckedTypedOwnedWaitExecutionV8,
    pub(crate) scope: &'a SourceCheckpointScope,
    pub(crate) turn: u32,
    pub(crate) attempt: u32,
    pub(crate) state: &'a serde_json::Value,
    pub(crate) decision: &'a serde_json::Value,
    pub(crate) proposal: &'a CheckedOwnedWaitProposalV8,
}
/// Exact checked request proof data. No TargetGrant or dispatch permit can be
/// recovered from it; retaining the plan borrows the actual effect registry.
pub(crate) struct CheckedOwnedEffectRequestV8<'a> {
    plan: CheckedOwnedEffectPlanV8<'a>,
    request: Vec<u8>,
}
impl CheckedOwnedEffectRequestV8<'_> {
    pub(crate) fn operation(&self) -> &TargetOperation {
        self.plan.operation()
    }
    pub(crate) fn request_wire(&self) -> &[u8] {
        &self.request
    }
    pub(crate) fn request_digest(&self) -> String {
        digest(REQUEST_DOMAIN, &self.request)
    }
    pub(crate) fn limits(&self) -> TargetLimits {
        self.plan.target_limits()
    }
}
pub(crate) fn checked_owned_effect_request_v8<'a>(
    inputs: &OwnedEffectSettlementInputsV8<'a>,
) -> Result<CheckedOwnedEffectRequestV8<'a>, Error> {
    if inputs.turn >= inputs.execution.ordinary().max_stages()
        || inputs.attempt >= inputs.execution.ordinary().max_attempts()
    {
        return Err(Error::Binding);
    }
    let commitments = checked_owned_wait_ready_commitments_v8(
        inputs.runtime,
        inputs.execution,
        inputs.scope,
        inputs.turn,
        inputs.attempt,
        inputs.state,
        inputs.decision,
        inputs.proposal,
    )?;
    let plan = plan_owned_effect_v8(
        inputs.runtime,
        inputs.execution,
        inputs.scope,
        inputs.proposal,
    )
    .map_err(|_| Error::Binding)?;
    let request = TargetHostRequest {
        grant_id: commitments.target_grant_digest().into(),
        authorization_binding: commitments.authorization_binding().into(),
        operation: plan.operation().clone(),
        turn: u64::from(inputs.turn),
        argument: plan.argument().clone(),
        fuel: 1,
    }
    .canonical_wire();
    Ok(CheckedOwnedEffectRequestV8 { plan, request })
}

pub(crate) struct CheckedOwnedEffectSettlementV8 {
    operation: TargetOperation,
    request: Vec<u8>,
    evidence: TargetEvidence,
    result_wire_limit: u64,
}
impl CheckedOwnedEffectSettlementV8 {
    pub(crate) fn operation(&self) -> &TargetOperation {
        &self.operation
    }
    pub(crate) fn request_wire(&self) -> &[u8] {
        &self.request
    }
    pub(crate) fn request_digest(&self) -> String {
        digest(REQUEST_DOMAIN, &self.request)
    }
    pub(crate) fn evidence(&self) -> &TargetEvidence {
        &self.evidence
    }
    /// The actual admitted target response sink ceiling, before host I/O.
    pub(crate) fn result_wire_limit(&self) -> u64 {
        self.result_wire_limit
    }
    /// Exact frozen framing with these checked identities, a present result
    /// digest, and the longest closed settlement tag. Fixed-width accounting
    /// numbers cannot enlarge it. This size bound carries no authority.
    pub(crate) fn evidence_wire_limit(&self) -> usize {
        let longest = [
            Settlement::Returned,
            Settlement::Cancelled,
            Settlement::CancelledAfterDispatch,
            Settlement::GrantBudget,
            Settlement::CallBudget,
            Settlement::RequestBudget,
            Settlement::FuelExhausted,
            Settlement::ResultBudget,
            Settlement::HostFailed,
            Settlement::HostPanicked,
            Settlement::ArgumentTypeMismatch,
            Settlement::ArgumentBindingMismatch,
            Settlement::MalformedResult,
            Settlement::ResultTypeMismatch,
        ]
        .iter()
        .map(|status| status.text().len())
        .max()
        .expect("closed settlements");
        self.evidence.canonical_wire().len() + SHA256_DIGEST_BYTES
            - self.evidence.result_digest.as_ref().map_or(0, String::len)
            + longest
            - self.evidence.settlement.text().len()
    }
}

pub(crate) fn checked_owned_effect_settlement_v8(
    inputs: OwnedEffectSettlementInputsV8<'_>,
    ordinary: &SourceJournalEntry,
    evidence_wire: &[u8],
    result_wire: Option<&[u8]>,
) -> Result<CheckedOwnedEffectSettlementV8, Error> {
    let checked = checked_owned_effect_request_v8(&inputs)?;
    let plan = &checked.plan;
    let request = &checked.request;
    let evidence = TargetEvidence::decode(evidence_wire).map_err(|_| Error::Malformed)?;
    evidence.replay_wire(request).map_err(|_| Error::Binding)?;
    let matches_phase = |turn: u32, attempt: u32, operation: &str| {
        turn == inputs.turn
            && attempt == inputs.attempt
            && operation == plan.operation().operation_id()
    };
    if evidence.settlement() == Settlement::Returned {
        let result_wire = result_wire.ok_or(Error::Malformed)?;
        if result_wire.len() as u64 > plan.target_limits().max_result_bytes {
            return Err(Error::Binding);
        }
        evidence
            .replay_exchange_wire(request, Some(result_wire))
            .map_err(|_| Error::Binding)?;
        let carrier = TypedCarrier::decode(result_wire, plan.operation().result_type())
            .map_err(|_| Error::Malformed)?;
        let accepted = plan.accepted_result(carrier.payload());
        let valid = match ordinary {
            SourceJournalEntry::EffectObserved {
                turn,
                attempt,
                operation,
                observation,
                observation_digest,
            } => {
                matches_phase(*turn, *attempt, operation)
                    && accepted.as_deref() == Some(observation.as_slice())
                    && carrier.payload() == observation.as_slice()
                    && *observation_digest == source_effect_digest(observation)
            }
            SourceJournalEntry::EffectFailed {
                turn,
                attempt,
                operation,
                reason,
            } => {
                matches_phase(*turn, *attempt, operation)
                    && *reason == SourceEffectFailure::HandlerFailed
                    && accepted.is_none()
            }
            _ => false,
        };
        if !valid {
            return Err(Error::Binding);
        }
    } else {
        if result_wire.is_some() {
            return Err(Error::Malformed);
        }
        evidence
            .replay_exchange_wire(request, None)
            .map_err(|_| Error::Binding)?;
        let expected = match evidence.settlement() {
            Settlement::Cancelled | Settlement::CancelledAfterDispatch => {
                SourceEffectFailure::Cancelled
            }
            Settlement::ResultBudget => SourceEffectFailure::ResultLimit,
            _ => SourceEffectFailure::HandlerFailed,
        };
        if !matches!(ordinary, SourceJournalEntry::EffectFailed { turn, attempt, operation, reason }
            if matches_phase(*turn,*attempt,operation) && *reason==expected)
        {
            return Err(Error::Binding);
        }
    }
    Ok(CheckedOwnedEffectSettlementV8 {
        operation: plan.operation().clone(),
        request: checked.request,
        evidence,
        result_wire_limit: plan.target_limits().max_result_bytes,
    })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::test_effect_exchange;
