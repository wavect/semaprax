//! Pure successor settlement replay. These checked facts carry no grant,
//! dispatch permit, append ACK, cleanup authority or physical runtime owner.
use super::super::*;
pub(crate) mod accounting;
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
    first_dispatch: accounting::ReservedTargetAccountingV8,
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
    if inputs.turn != 0 {
        return Err(Error::Binding);
    }
    request_after_prefix(inputs, None)
}
fn request_after_prefix<'a>(
    inputs: &OwnedEffectSettlementInputsV8<'a>,
    previous: Option<&accounting::CheckedTargetAccountingV8>,
) -> Result<CheckedOwnedEffectRequestV8<'a>, Error> {
    if inputs.attempt >= inputs.execution.ordinary().max_attempts() {
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
    if commitments.budget() < 1 {
        return Err(Error::Binding);
    }
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
    // Proof of this closed first-effect request only. This does not reset or
    // replace a live invocation's cumulative target accounting owner.
    let first_dispatch = accounting::reserve(previous, &request, plan.target_limits())?;
    Ok(CheckedOwnedEffectRequestV8 {
        plan,
        request,
        first_dispatch,
    })
}

pub(crate) struct CheckedOwnedEffectSettlementV8 {
    operation: TargetOperation,
    request: Vec<u8>,
    evidence: TargetEvidence,
    accounting: accounting::CheckedTargetAccountingV8,
    accepted_payload: Option<Vec<u8>>,
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
    pub(crate) fn accounting_proof(&self) -> &accounting::CheckedTargetAccountingV8 {
        &self.accounting
    }
    pub(crate) fn evidence(&self) -> &TargetEvidence {
        &self.evidence
    }
    /// Exact inert payload matched to the successful ordinary observation.
    /// This carries no runtime owner, evaluator or dispatch authority.
    pub(crate) fn accepted_payload(&self) -> Option<&[u8]> {
        self.accepted_payload.as_deref()
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
    checked_owned_effect_settlement_after_prefix_v8(
        inputs,
        None,
        ordinary,
        evidence_wire,
        result_wire,
    )
}
/// Current closed source profile cannot consume a second Recorded exchange.
/// Retaining the preceding checked proof makes reset attempts explicit.
pub(crate) fn checked_owned_effect_settlement_after_prefix_v8(
    inputs: OwnedEffectSettlementInputsV8<'_>,
    previous: Option<&accounting::CheckedTargetAccountingV8>,
    ordinary: &SourceJournalEntry,
    evidence_wire: &[u8],
    result_wire: Option<&[u8]>,
) -> Result<CheckedOwnedEffectSettlementV8, Error> {
    if previous.is_some() {
        return Err(Error::Binding);
    }
    let checked = checked_owned_effect_request_v8(&inputs)?;
    settle_checked(inputs, checked, ordinary, evidence_wire, result_wire)
}
/// Additive inert verifier entry. Only the source inventory's exact checked
/// profile/prefix join can supply the sealed proof; it grants no live dispatch.
pub(crate) fn checked_cumulative_owned_effect_request_v8<'a>(
    inputs: &OwnedEffectSettlementInputsV8<'a>,
    prefix: &crate::live_invocation::source_journal::CheckedCumulativeEffectPrefixV8<'_>,
) -> Result<CheckedOwnedEffectRequestV8<'a>, Error> {
    prefix.validate(inputs)?;
    request_after_prefix(inputs, prefix.previous())
}
pub(crate) fn checked_cumulative_owned_effect_settlement_v8(
    inputs: OwnedEffectSettlementInputsV8<'_>,
    prefix: &crate::live_invocation::source_journal::CheckedCumulativeEffectPrefixV8<'_>,
    ordinary: &SourceJournalEntry,
    evidence_wire: &[u8],
    result_wire: Option<&[u8]>,
) -> Result<CheckedOwnedEffectSettlementV8, Error> {
    let checked = checked_cumulative_owned_effect_request_v8(&inputs, prefix)?;
    settle_checked(inputs, checked, ordinary, evidence_wire, result_wire)
}
fn settle_checked(
    inputs: OwnedEffectSettlementInputsV8<'_>,
    checked: CheckedOwnedEffectRequestV8<'_>,
    ordinary: &SourceJournalEntry,
    evidence_wire: &[u8],
    result_wire: Option<&[u8]>,
) -> Result<CheckedOwnedEffectSettlementV8, Error> {
    let plan = &checked.plan;
    let request = &checked.request;
    let evidence = TargetEvidence::decode(evidence_wire).map_err(|_| Error::Malformed)?;
    evidence.replay_wire(request).map_err(|_| Error::Binding)?;
    if !evidence.dispatched() {
        return Err(Error::Binding);
    }
    let accounting = accounting::verify(
        checked.first_dispatch,
        plan.target_limits(),
        &evidence,
        result_wire,
    )?;
    let mut accepted_payload = None;
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
        accepted_payload = accepted;
    } else {
        if result_wire.is_some() {
            return Err(Error::Malformed);
        }
        evidence
            .replay_exchange_wire(request, None)
            .map_err(|_| Error::Binding)?;
        let expected_reason = match evidence.settlement() {
            Settlement::HostFailed | Settlement::HostPanicked => SourceEffectFailure::HandlerFailed,
            Settlement::ResultBudget => SourceEffectFailure::ResultLimit,
            // CancelledAfterDispatch discards its raw charge basis; all
            // pre-dispatch and other raw-result forms remain outside §21.
            _ => return Err(Error::Binding),
        };
        if !matches!(ordinary, SourceJournalEntry::EffectFailed { turn, attempt, operation, reason }
            if matches_phase(*turn,*attempt,operation) && *reason==expected_reason)
        {
            return Err(Error::Binding);
        }
    }
    Ok(CheckedOwnedEffectSettlementV8 {
        operation: plan.operation().clone(),
        request: checked.request,
        evidence,
        accounting,
        accepted_payload,
        result_wire_limit: plan.target_limits().max_result_bytes,
    })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::{test_effect_exchange, test_failed_effect_exchange};
