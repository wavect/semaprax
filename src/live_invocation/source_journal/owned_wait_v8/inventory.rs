//! Authenticated, checked data only. This inventory grants no restoration or append authority.
use super::super::{SourceJournalEntry as Ordinary, SourceJournalError as Error};
use super::{
    checked_context::CheckedOwnedWaitJournalContextV8, fold, model::OwnedBodyV8 as Body, wire,
    EntryV8, ExpectedRowV8, ValidatedEntryV8,
};
use crate::interpreter::resumable::checkpoint;
use crate::resumable_effects::owned_frame::{
    v2::{self, CheckedOwnedWaitObservationV8},
    SourceOwnedWaitLeaseV8,
};
use crate::resumable_effects::source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope};
use serde_json::Value;
mod accounting;
pub(super) use accounting::CheckedAccountingPrefixV8;
mod cumulative;
pub(crate) use cumulative::CheckedCumulativeEffectPrefixV8;

pub(super) struct CheckedInventoryV8<'a> {
    entries: Vec<ValidatedEntryV8>,
    last_mac: String,
    accounting: Option<accounting::CheckedAccountingPrefixV8<'a>>,
}
impl<'a> CheckedInventoryV8<'a> {
    pub(super) fn into_authenticated_parts(
        self,
    ) -> (
        Vec<ValidatedEntryV8>,
        String,
        Option<CheckedAccountingPrefixV8<'a>>,
    ) {
        (self.entries, self.last_mac, self.accounting)
    }
    pub(super) fn into_parts(self) -> (Vec<ValidatedEntryV8>, String) {
        (self.entries, self.last_mac)
    }
    pub(super) fn entries(&self) -> &[ValidatedEntryV8] {
        &self.entries
    }
}
fn require(ok: bool) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::Binding)
    }
}
fn typed<T, E>(value: Result<T, E>) -> Result<T, Error> {
    value.map_err(|_| Error::Binding)
}

/// Preserve physical provenance even if external pins change again later.
pub(super) enum InventoryValidationErrorV8 {
    Physical(Error),
    Proof(Error),
}
impl InventoryValidationErrorV8 {
    pub(super) fn error(self) -> Error {
        match self {
            Self::Physical(error) | Self::Proof(error) => error,
        }
    }
}
pub(super) fn checked_inventory_v8<'a>(
    context: &'a CheckedOwnedWaitJournalContextV8,
    lease: &SourceOwnedWaitLeaseV8,
    key: &SourceCheckpointKey,
    bytes: &[u8],
) -> Result<CheckedInventoryV8<'a>, Error> {
    checked_inventory_tagged_v8(context, lease, key, bytes)
        .map_err(InventoryValidationErrorV8::error)
}
pub(super) fn checked_inventory_tagged_v8<'a>(
    context: &'a CheckedOwnedWaitJournalContextV8,
    lease: &SourceOwnedWaitLeaseV8,
    key: &SourceCheckpointKey,
    bytes: &[u8],
) -> Result<CheckedInventoryV8<'a>, InventoryValidationErrorV8> {
    context
        .validate_lease(lease)
        .map_err(InventoryValidationErrorV8::Physical)?;
    let zero = "0".repeat(64);
    let expected = ExpectedRowV8 {
        invocation: context.ordinary().invocation(),
        generation: context.generation(),
        seq: 0,
        prev_mac: &zero,
        ordinary: context.ordinary(),
    };
    let decoded =
        wire::decode_inventory(bytes, &expected, key).map_err(InventoryValidationErrorV8::Proof)?;
    let mut accounting = accounting::AccountingBuilderV8::authenticated(context, bytes)
        .map_err(InventoryValidationErrorV8::Proof)?;
    let mut result = check_entries_with_runtime(
        context.fold(),
        key,
        decoded,
        context.ready_runtime(),
        Some(&mut accounting),
    )
    .map_err(InventoryValidationErrorV8::Proof)?;
    result.last_mac = document_mac(bytes).map_err(InventoryValidationErrorV8::Proof)?;
    context
        .validate_lease(lease)
        .map_err(InventoryValidationErrorV8::Physical)?;
    result.accounting = Some(
        accounting
            .finish()
            .map_err(InventoryValidationErrorV8::Proof)?,
    );
    Ok(result)
}
fn scope(context: &super::FoldContextV8) -> Result<SourceCheckpointScope, Error> {
    let Body::OwnedRunCreated { scope, .. } = &context.created else {
        return Err(Error::Binding);
    };
    typed(SourceCheckpointScope::new(
        scope["program_root"].as_str().ok_or(Error::Binding)?,
        scope["invocation"].as_str().ok_or(Error::Binding)?,
        scope["policy_epoch"].as_u64().ok_or(Error::Binding)?,
    ))
}
fn observation(
    context: &super::FoldContextV8,
    scope: &SourceCheckpointScope,
    copy: &Value,
) -> Result<CheckedOwnedWaitObservationV8, Error> {
    let values = copy.as_array().ok_or(Error::Binding)?;
    require(values.len() == 1)?;
    let channel = typed(checkpoint::channel_from_json(&values[0]["value"]))?;
    let facts = typed(v2::bind_owned_wait_observation_v8(
        &context.checked_binding,
        scope,
        &channel,
    ))?;
    require(facts.copy_arguments() == copy)?;
    Ok(facts)
}
/// Pure prefixes have no retained runtime, so Ready remains refused.
#[cfg(test)]
fn check_entries<'a>(
    context: &super::FoldContextV8,
    key: &SourceCheckpointKey,
    decoded: Vec<EntryV8>,
) -> Result<CheckedInventoryV8<'a>, Error> {
    check_entries_with_runtime(context, key, decoded, None, None)
}
/// Only the joined cumulative failed-Observe State cleanup is replayable here.
/// Authenticated rows describe facts; fixed actual-owner permits authorize writes.
fn check_entries_with_runtime<'a>(
    context: &super::FoldContextV8,
    key: &SourceCheckpointKey,
    decoded: Vec<EntryV8>,
    ready: Option<(
        &crate::execution_revision::typed::AgentRuntimeV2,
        &crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8,
    )>,
    mut accounting: Option<&mut accounting::AccountingBuilderV8<'a>>,
) -> Result<CheckedInventoryV8<'a>, Error> {
    let b = &context.checked_binding;
    let scope = scope(context)?;
    let mut rows = Vec::new();
    let mut obs: Option<CheckedOwnedWaitObservationV8> = None;
    let mut raw: Option<(u32, u32, Vec<u8>)> = None;
    let mut state_digest: Option<String> = None;
    let mut state_value: Option<Value> = None;
    let mut proposal_facts: Option<(u32, u32, v2::CheckedOwnedWaitProposalV8)> = None;
    let mut staged_decision: Option<(u32, u32, usize, Value)> = None;
    // Retained only from this exact authenticated Recorded join. This proof
    // carries accepted bytes, never an Outcome owner or evaluator permission.
    let mut recorded_effect: Option<(u32, u32, usize,
        crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::CheckedOwnedEffectSettlementV8)> = None;
    for entry in decoded {
        let mut row_obs = None;
        match &entry {
            EntryV8::Owned(body) => match body {
                Body::OwnedInitializationCommitted { task, state, .. } => {
                    require(context.initialized_task.as_ref() == Some(task))?;
                    typed(v2::validate_owned_wait_state_v8(b, state))?;
                }
                Body::OwnedStateCommitted { state, .. }
                | Body::OwnedStateRearmed { state, .. }
                | Body::OwnedStateTransferCompleted { state, .. } => {
                    typed(v2::validate_owned_wait_state_v8(b, state))?;
                    state_value = Some(state.clone());
                    state_digest = Some(typed(v2::owned_wait_ordinary_state_digest_v8(b, state))?);
                }
                _ => {}
            },
            EntryV8::Ordinary(Ordinary::AttemptSettled {
                turn,
                attempt,
                response,
                ..
            }) => {
                raw = Some((*turn, *attempt, response.clone()));
            }
            _ => {}
        }
        match &entry {
            EntryV8::Ordinary(Ordinary::TurnObserved { state, .. }) => {
                require(state_digest.as_ref() == Some(state))?;
            }
            EntryV8::Owned(Body::OwnedWaitCreated { copy_arguments, .. }) => {
                obs = Some(observation(context, &scope, copy_arguments)?);
                row_obs = obs.clone();
                raw = None;
                proposal_facts = None;
                staged_decision = None;
            }
            EntryV8::Owned(Body::OwnedWaitPrepared {
                checkpoint: encoded,
                checkpoint_digest,
                consumed,
                ..
            }) => {
                let prior = fold::fold(context, &rows)?;
                let facts = obs.as_ref().ok_or(Error::Binding)?;
                let wait = rows
                    .iter()
                    .rev()
                    .find_map(|r| match &r.entry {
                        EntryV8::Owned(Body::OwnedWaitCreated {
                            argument_digest, ..
                        }) => Some(argument_digest),
                        _ => None,
                    })
                    .ok_or(Error::Binding)?;
                let bytes =
                    crate::live_invocation::identity::unhex(encoded).ok_or(Error::Malformed)?;
                let expected = v2::OwnedWaitCheckpointExpectationV8 {
                    scope: &scope,
                    argument_digest: wait,
                    observation: facts,
                    sequence: u64::try_from(rows.len()).map_err(|_| Error::Capacity)?,
                    reserved_total: prior.reserved_total,
                    consumed_total: prior
                        .consumed_recorded
                        .checked_add(*consumed)
                        .ok_or(Error::Capacity)?,
                };
                let checked = typed(v2::validate_owned_wait_checkpoint_v8(
                    b, key, &expected, &bytes,
                ))?;
                require(checked.outer_digest() == checkpoint_digest)?;
                row_obs = Some(facts.clone());
            }
            EntryV8::Owned(Body::OwnedWaitCompleted {
                turn,
                attempt,
                proposal,
                proposal_digest,
                result_digest,
                ..
            }) => {
                let (rt, ra, response) = raw.as_ref().ok_or(Error::Binding)?;
                require(rt == turn && ra == attempt)?;
                let text = std::str::from_utf8(response).map_err(|_| Error::Binding)?;
                let decoded = typed(b.lifecycle().proposal_schema().decode(text))?;
                let facts = typed(v2::bind_owned_wait_proposal_v8(b, &scope, &decoded))?;
                let argument = rows
                    .iter()
                    .rev()
                    .find_map(|r| match &r.entry {
                        EntryV8::Owned(Body::OwnedWaitCreated {
                            argument_digest, ..
                        }) => Some(argument_digest),
                        _ => None,
                    })
                    .ok_or(Error::Binding)?;
                require(
                    facts.value() == proposal
                        && facts.ordinary_digest() == proposal_digest
                        && typed(facts.result_digest(argument))? == *result_digest,
                )?;
                proposal_facts = Some((*turn, *attempt, facts));
            }
            EntryV8::Owned(Body::OwnedAuthorizationStaged {
                turn,
                attempt,
                decision,
                ..
            }) => {
                typed(v2::validate_owned_wait_decision_v8(b, decision))?;
                staged_decision = Some((*turn, *attempt, rows.len(), decision.clone()));
            }
            EntryV8::Owned(Body::OwnedAuthorizationReady {
                turn,
                attempt,
                staged,
                grant_digest,
                ..
            }) => {
                let (runtime, execution) = ready.ok_or(Error::Binding)?;
                let (st, sa, sequence, decision) =
                    staged_decision.as_ref().ok_or(Error::Binding)?;
                let (pt, pa, proposal) = proposal_facts.as_ref().ok_or(Error::Binding)?;
                require(
                    st == turn
                        && sa == attempt
                        && pt == turn
                        && pa == attempt
                        && u32::try_from(*sequence).ok() == Some(*staged),
                )?;
                let facts =
                    crate::agent_lifecycle::authorization::checked_owned_wait_ready_commitments_v8(
                        runtime,
                        execution,
                        &scope,
                        *turn,
                        *attempt,
                        state_value.as_ref().ok_or(Error::Binding)?,
                        decision,
                        proposal,
                    )?;
                require(facts.grant_digest() == grant_digest)?;
            }
            EntryV8::Ordinary(Ordinary::EffectIntent {
                turn,
                attempt,
                operation,
                request_digest,
            }) => {
                let inputs = effect_inputs(
                    ready,
                    &scope,
                    *turn,
                    *attempt,
                    &state_value,
                    &staged_decision,
                    &proposal_facts,
                )?;
                let facts = if context.cumulative_initialization {
                    let preceding = accounting.as_ref().and_then(|a| a.preceding()).or_else(|| {
                        recorded_effect
                            .as_ref()
                            .map(|(_, _, _, facts)| facts.accounting_proof())
                    });
                    let prefix = cumulative::checked_prefix(context, &rows, &inputs, preceding)?;
                    crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::checked_cumulative_owned_effect_request_v8(&inputs, &prefix)?
                } else {
                    crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::checked_owned_effect_request_v8(&inputs)?
                };
                require(
                    facts.operation().operation_id() == operation
                        && facts.request_digest() == *request_digest,
                )?;
            }
            EntryV8::Owned(Body::OwnedEffectSettlementRecorded {
                turn,
                attempt,
                intent,
                settlement,
                evidence,
                evidence_digest,
                result_wire,
            }) => {
                let inputs = effect_inputs(
                    ready,
                    &scope,
                    *turn,
                    *attempt,
                    &state_value,
                    &staged_decision,
                    &proposal_facts,
                )?;
                let evidence = effect_hex(evidence, 1475)?;
                let result = result_wire
                    .as_deref()
                    .map(|hex| effect_hex(hex, 65536))
                    .transpose()?;
                let ordinary = match rows.get(*settlement as usize).map(|r| &r.entry) {
                    Some(EntryV8::Ordinary(e)) => e,
                    _ => return Err(Error::Binding),
                };
                let preceding = accounting.as_ref().and_then(|a| a.preceding()).or_else(|| {
                    context
                        .cumulative_initialization
                        .then(|| {
                            recorded_effect
                                .as_ref()
                                .map(|(_, _, _, facts)| facts.accounting_proof())
                        })
                        .flatten()
                });
                let facts = if context.cumulative_initialization {
                    let prefix = cumulative::checked_prefix(context, &rows, &inputs, preceding)?;
                    crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::checked_cumulative_owned_effect_settlement_v8(inputs, &prefix, ordinary, &evidence, result.as_deref())?
                } else {
                    crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::checked_owned_effect_settlement_after_prefix_v8(inputs, preceding, ordinary, &evidence, result.as_deref())?
                };
                require(facts.evidence().digest() == evidence_digest)?;
                match rows.get(*intent as usize).map(|r| &r.entry) {
                    Some(EntryV8::Ordinary(Ordinary::EffectIntent {
                        turn: it,
                        attempt: ia,
                        operation,
                        request_digest,
                    })) => require(
                        it == turn
                            && ia == attempt
                            && facts.operation().operation_id() == operation
                            && facts.request_digest() == *request_digest,
                    )?,
                    _ => return Err(Error::Binding),
                }
                if let Some(a) = accounting.as_mut() {
                    a.record(rows.len(), *intent as usize, *settlement as usize, &facts)?;
                }
                recorded_effect = Some((*turn, *attempt, rows.len(), facts));
            }
            EntryV8::Ordinary(Ordinary::StageReservation {
                turn,
                attempt: Some(attempt),
                role: crate::live_invocation::source_journal::SourceStageRole::Reduce,
                ..
            }) => {
                let (rt, ra, recorded, facts) = recorded_effect.as_ref().ok_or(Error::Binding)?;
                require(rt == turn && ra == attempt)?;
                // The causal fold additionally requires the immediately prior
                // successful whole Decision receipt and exact reservation F.
                require(*recorded < rows.len())?;
                super::reduce_inventory::checked_outcome(b, facts)?;
            }
            EntryV8::Owned(Body::OwnedWaitFailed { status, .. }) => {
                typed(v2::validate_owned_wait_failure_v8(
                    b,
                    &b.helper().function().id,
                    status,
                ))?;
            }
            EntryV8::Owned(
                Body::OwnedCleanupStarted {
                    turn,
                    attempt,
                    wait,
                    owner,
                    ..
                }
                | Body::OwnedCleanupSettled {
                    turn,
                    attempt,
                    wait,
                    owner,
                    ..
                },
            ) => {
                // Authenticate and check the original failure prefix before
                // accepting this inert replay branch. The final fold checks
                // its exact compiler vector/receipt and causal row references.
                require(context.cumulative_initialization)?;
                let previous = fold::fold(context, &rows)?;
                let cleanup_turn = previous
                    .failed_observe_cleanup_turn()
                    .map_err(|_| Error::Binding)?;
                require(
                    cleanup_turn == *turn
                        && *owner == super::model::OwnerV8::State
                        && attempt.is_none()
                        && wait.is_none(),
                )?;
            }
            EntryV8::Ordinary(Ordinary::ProposalRefused { turn, attempt, .. }) => {
                let (rt, ra, response) = raw.as_ref().ok_or(Error::Binding)?;
                require(rt == turn && ra == attempt)?;
                let rejected = std::str::from_utf8(response).map_or(true, |text| {
                    b.lifecycle().proposal_schema().decode(text).is_err()
                });
                require(rejected)?;
            }
            _ => {}
        }
        rows.push(ValidatedEntryV8 {
            entry,
            observation: row_obs,
        });
        if let Some(a) = accounting.as_mut() {
            a.row_checked(rows.len())?;
        }
    }
    fold::fold(context, &rows)?;
    Ok(CheckedInventoryV8 {
        entries: rows,
        last_mac: "0".repeat(64),
        accounting: None,
    })
}

fn effect_hex(text: &str, max: usize) -> Result<Vec<u8>, Error> {
    if text.len() > max.checked_mul(2).ok_or(Error::Capacity)? {
        return Err(Error::Capacity);
    }
    let bytes = crate::live_invocation::identity::unhex(text).ok_or(Error::Malformed)?;
    require(crate::live_invocation::identity::hex(&bytes) == text)?;
    Ok(bytes)
}
fn effect_inputs<'a>(
    ready: Option<(&'a crate::execution_revision::typed::AgentRuntimeV2,
        &'a crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8)>,
    scope: &'a SourceCheckpointScope, turn: u32, attempt: u32,
    state: &'a Option<Value>, staged: &'a Option<(u32,u32,usize,Value)>,
    proposal: &'a Option<(u32,u32,v2::CheckedOwnedWaitProposalV8)>,
) -> Result<crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::OwnedEffectSettlementInputsV8<'a>, Error>{
    let (runtime, execution) = ready.ok_or(Error::Binding)?;
    let (st, sa, _, decision) = staged.as_ref().ok_or(Error::Binding)?;
    let (pt, pa, proposal) = proposal.as_ref().ok_or(Error::Binding)?;
    require(*st == turn && *sa == attempt && *pt == turn && *pa == attempt)?;
    Ok(crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::OwnedEffectSettlementInputsV8 {
        runtime, execution, scope, turn, attempt,
        state: state.as_ref().ok_or(Error::Binding)?, decision, proposal,
    })
}

fn document_mac(bytes: &[u8]) -> Result<String, Error> {
    let Some(row) = bytes.split_inclusive(|b| *b == b'\n').last() else {
        return Ok("0".repeat(64));
    };
    let value = wire::parse(&row[..row.len() - 1])?;
    Ok(value["authentication"]
        .as_str()
        .ok_or(Error::Malformed)?
        .to_owned())
}
pub(super) fn checked_candidate_inventory_v8<'a>(
    context: &'a CheckedOwnedWaitJournalContextV8,
    lease: &SourceOwnedWaitLeaseV8,
    key: &SourceCheckpointKey,
    prefix: &[u8],
    row: &[u8],
) -> Result<CheckedInventoryV8<'a>, Error> {
    checked_candidate_inventory_tagged_v8(context, lease, key, prefix, row)
        .map_err(InventoryValidationErrorV8::error)
}
pub(super) fn checked_candidate_inventory_tagged_v8<'a>(
    context: &'a CheckedOwnedWaitJournalContextV8,
    lease: &SourceOwnedWaitLeaseV8,
    key: &SourceCheckpointKey,
    prefix: &[u8],
    row: &[u8],
) -> Result<CheckedInventoryV8<'a>, InventoryValidationErrorV8> {
    let acknowledged = checked_inventory_tagged_v8(context, lease, key, prefix)?;
    let expected = ExpectedRowV8 {
        invocation: context.ordinary().invocation(),
        generation: context.generation(),
        seq: u32::try_from(acknowledged.entries.len())
            .map_err(|_| InventoryValidationErrorV8::Proof(Error::Capacity))?,
        prev_mac: &acknowledged.last_mac,
        ordinary: context.ordinary(),
    };
    wire::decode(row, &expected, key).map_err(InventoryValidationErrorV8::Proof)?;
    let length = prefix
        .len()
        .checked_add(row.len())
        .ok_or(InventoryValidationErrorV8::Proof(Error::Capacity))?;
    if length > super::super::MAX_SOURCE_DOCUMENT_BYTES {
        return Err(InventoryValidationErrorV8::Proof(Error::Capacity));
    }
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(row);
    checked_inventory_tagged_v8(context, lease, key, &bytes)
}
#[cfg(test)]
pub(super) mod tests;
