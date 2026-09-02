//! Canonical compiler-side codecs for the private callable-v3 runtime wires.
//!
//! These encoders grant no loader, invocation, settlement, finalizer, receipt,
//! or ledger authority. The native host implements its parsers independently.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable-v3 runtime remains private and unwired")
)]

use sha2::{Digest, Sha256};

pub(super) const VERSION: u32 = 3;
pub(super) const HEADER_BYTES: u32 = 20;
pub(super) const MAX_WIRE_BYTES: u32 = 1024 * 1024;
pub(super) const HOST_RECEIPT_BYTES: u32 = 524;
pub(super) const PRE_EXECUTE_HOST_UNWIND_CODE: u32 = u32::MAX - 1;

pub(super) const REQUEST_FIXED_BYTES: u32 = 104;
pub(super) const REQUEST_I64_BYTES: u32 = 16;
pub(super) const REQUEST_BOOL_BYTES: u32 = 12;
pub(super) const REQUEST_OWNER_BYTES: u32 = 20;
pub(super) const EXECUTE_RESPONSE_FIXED_BYTES: u32 = 156;
pub(super) const EVENT_ORDINAL_BYTES: u32 = 4;
pub(super) const FRAME_FIXED_BYTES: u32 = 388;
pub(super) const FRAME_RESOURCE_CELL_BYTES: u32 = 12;
pub(super) const DECISION_BYTES: u32 = 172;
pub(super) const ACTION_EVIDENCE_BYTES: u32 = 196;
pub(super) const CANDIDATE_RECEIPT_FIXED_BYTES: u32 = 372;
pub(super) const CANDIDATE_DISPOSITION_BYTES: u32 = 12;

pub(super) const REQUEST_SCHEMA_STATEMENT: &[u8] = b"SPXNRQ03;v3;u32le;header20;total-exact;call32;invocation-u64;generation-u64;challenge32;argc;args[tag,index,payload];scalar-tag1;i64-8;bool-u32-0-or-1;owned-tag2-owner-u32-payload-u64;no-trailing";
pub(super) const EXECUTE_RESPONSE_SCHEMA_STATEMENT: &[u8] = b"SPXNEX03;v3;u32le;header20;total-declared;zero-tail-to-capacity;call32;invocation-u64;generation-u64;challenge32;request-digest32;checkpoint;outcome;detail;payload-u64;event-count;ordinals;outcomes1-scalar-2-semantic-3-owned";
pub(super) const FRAME_SCHEMA_STATEMENT: &[u8] = b"SPXNFR03;v3;u32le;header20;total-exact;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;return-tag;return-code;returns1-pending-2-returned-3-preexecute-host-unwind;preexecute-host-unwind-code-4294967294;checkpoint;phase;decision32;next-action;record-count;active-finalizers;resource-count;cells[state-u32,payload-u64];action-chain32;pre-candidate-frame32";
pub(super) const DECISION_SCHEMA_STATEMENT: &[u8] = b"SPXNDC03;v3;u32le;header20;total172;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;decision-tag;detail;tags1-scalar-2-semantic-3-owned-4-physical-5-malformed-6-trace-7-unwind";
pub(super) const ACTION_SCHEMA_STATEMENT: &[u8] = b"SPXNAC03;v3;u32le;header20;total196;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;action-index;boundary-tag;owner;payload-u64;before-state;after-state;checkpoint;tags1-start-2-complete-3-publish";
pub(super) const CANDIDATE_RECEIPT_SCHEMA_STATEMENT: &[u8] = b"SPXNCR03;v3;u32le;header20;total372-plus-12r;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;frame32;decision32;action32;outcome;detail;active-finalizers-zero;disposition-count;cells[disposition-u32,payload-u64]";
pub(super) const COMMITTED_RECEIPT_SCHEMA_STATEMENT: &[u8] = b"SPXHRP03;v3;u32le;header20;total524;host-only;instance32;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;frame32;decision32;action32;candidate32;ledger-before32;ledger-after32;publication;detail;hmac32;separate-receipt-key;atomic-ledger-and-cache";

pub(super) const CALL_ABI_STATEMENT: &[u8] = b"extern-C;getter=const-u8-ptr(void);execute=u32(const-u8-ptr,u32,u8-ptr,u32,u8-ptr,u32);settle=u32(u8-ptr,u32,const-u8-ptr,u32,u8-ptr,u32);windows-cdecl;synchronous;same-thread;no-unwind;no-longjmp;no-callbacks;no-retained-pointers;no-reentrancy";

const REQUEST_MAGIC: &[u8; 8] = b"SPXNRQ03";
const EXECUTE_RESPONSE_MAGIC: &[u8; 8] = b"SPXNEX03";
const FRAME_MAGIC: &[u8; 8] = b"SPXNFR03";
const DECISION_MAGIC: &[u8; 8] = b"SPXNDC03";
const ACTION_MAGIC: &[u8; 8] = b"SPXNAC03";
const CANDIDATE_MAGIC: &[u8; 8] = b"SPXNCR03";

const REQUEST_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-request-digest.v3\0";
const RESPONSE_STORAGE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.native-callable-execute-response-storage-digest.v3\0";
const DECISION_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-decision-digest.v3\0";
const ACTION_CHAIN_SEED_DOMAIN: &[u8] = b"semaprax.native-callable-action-chain-seed.v3\0";
const ACTION_CHAIN_STEP_DOMAIN: &[u8] = b"semaprax.native-callable-action-chain-step.v3\0";
const FRAME_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-pre-candidate-frame-digest.v3\0";
const CANDIDATE_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-candidate-digest.v3\0";

const ZERO_DIGEST: [u8; 32] = [0; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WireV3Error {
    Capacity,
    InvalidBinding,
    InvalidCount,
    InvalidTag,
    InvalidState,
    InvalidDigest,
    InvalidOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProviderBinding {
    pub(super) call_contract: [u8; 32],
    pub(super) recovery_contract: [u8; 32],
    pub(super) settlement_graph: [u8; 32],
    pub(super) invocation: u64,
    pub(super) frame_generation: u64,
    pub(super) provider_challenge: [u8; 32],
}

impl ProviderBinding {
    fn validate(self) -> Result<(), WireV3Error> {
        if self.invocation == 0
            || self.frame_generation == 0
            || is_zero(&self.provider_challenge)
            || is_zero(&self.call_contract)
            || is_zero(&self.recovery_contract)
            || is_zero(&self.settlement_graph)
        {
            return Err(WireV3Error::InvalidBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestArgument {
    I64 {
        index: u32,
        value: i64,
    },
    Bool {
        index: u32,
        value: bool,
    },
    Owned {
        index: u32,
        owner_ordinal: u32,
        payload: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecuteOutcome {
    Scalar(i64),
    SemanticFailure { selected_ordinal: u32 },
    Owned { owner_ordinal: u32, payload: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub(super) enum ResourceState {
    Live = 1,
    ProvisionalResult = 2,
    Finalizing = 3,
    Dead = 4,
    Published = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResourceCell {
    pub(super) state: ResourceState,
    pub(super) payload: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub(super) enum FramePhase {
    Executing = 1,
    DecisionLocked = 2,
    ActionInProgress = 3,
    ProviderSettled = 4,
    ReceiptCommitted = 5,
    Quarantined = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecuteReturn {
    Pending,
    Returned(u32),
    PreExecuteHostUnwind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RecoveryFrame<'a> {
    pub(super) binding: ProviderBinding,
    pub(super) request_digest: [u8; 32],
    pub(super) response_digest: [u8; 32],
    pub(super) semantic_trace_digest: [u8; 32],
    pub(super) execute_return: ExecuteReturn,
    pub(super) checkpoint: u32,
    pub(super) phase: FramePhase,
    pub(super) decision_digest: [u8; 32],
    pub(super) next_action_index: u32,
    pub(super) action_record_count: u32,
    pub(super) active_finalizers: u32,
    pub(super) resources: &'a [ResourceCell],
    pub(super) action_chain_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SettlementDecision {
    AcceptScalar,
    AcceptSemanticFailure,
    AcceptOwned { owner_ordinal: u32 },
    AbortPhysical { code: u32 },
    AbortMalformed,
    AbortTraceRejected,
    AbortHostUnwind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ActionBoundary {
    FinalizerStarted,
    FinalizerCompleted,
    Publish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ActionEvidence {
    pub(super) binding: ProviderBinding,
    pub(super) action_index: u32,
    pub(super) boundary: ActionBoundary,
    pub(super) owner_ordinal: u32,
    pub(super) payload: u64,
    pub(super) before: ResourceState,
    pub(super) after: ResourceState,
    pub(super) checkpoint: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CandidateOutcome {
    Scalar,
    SemanticFailure,
    Owned { owner_ordinal: u32 },
    Abort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub(super) enum TerminalDisposition {
    Dead = 1,
    Published = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DispositionCell {
    pub(super) disposition: TerminalDisposition,
    pub(super) payload: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CandidateReceipt<'a> {
    pub(super) binding: ProviderBinding,
    pub(super) request_digest: [u8; 32],
    pub(super) response_digest: [u8; 32],
    pub(super) semantic_trace_digest: [u8; 32],
    pub(super) pre_candidate_frame_digest: [u8; 32],
    pub(super) decision_digest: [u8; 32],
    pub(super) action_chain_digest: [u8; 32],
    pub(super) outcome: CandidateOutcome,
    pub(super) active_finalizers: u32,
    pub(super) dispositions: &'a [DispositionCell],
}

pub(super) fn encode_request(
    binding: ProviderBinding,
    arguments: &[RequestArgument],
) -> Result<Vec<u8>, WireV3Error> {
    binding.validate()?;
    let mut writer = Writer::new(REQUEST_MAGIC);
    writer.binding_request(binding);
    writer.u32(to_u32(arguments.len())?);
    let mut next_owner = 0_u32;
    for (expected, argument) in arguments.iter().enumerate() {
        let expected = to_u32(expected)?;
        match *argument {
            RequestArgument::I64 { index, value } if index == expected => {
                writer.u32(1);
                writer.u32(index);
                writer.i64(value);
            }
            RequestArgument::Bool { index, value } if index == expected => {
                writer.u32(1);
                writer.u32(index);
                writer.u32(u32::from(value));
            }
            RequestArgument::Owned {
                index,
                owner_ordinal,
                payload,
            } if index == expected && owner_ordinal == next_owner => {
                next_owner = next_owner.checked_add(1).ok_or(WireV3Error::Capacity)?;
                writer.u32(2);
                writer.u32(index);
                writer.u32(owner_ordinal);
                writer.u64(payload);
            }
            _ => return Err(WireV3Error::InvalidCount),
        }
    }
    writer.finish_exact()
}

pub(super) fn encode_execute_response(
    binding: ProviderBinding,
    request_digest: [u8; 32],
    checkpoint: u32,
    outcome: ExecuteOutcome,
    ordinals: &[u32],
    maximum_event_count: u32,
    dictionary_entries: u32,
) -> Result<Vec<u8>, WireV3Error> {
    binding.validate()?;
    require_digest(&request_digest)?;
    if checkpoint == 0
        || ordinals.is_empty()
        || ordinals.len() > maximum_event_count as usize
        || ordinals
            .iter()
            .any(|ordinal| *ordinal == 0 || *ordinal > dictionary_entries)
    {
        return Err(WireV3Error::InvalidCount);
    }
    let capacity = execute_response_capacity(maximum_event_count)?;
    let mut writer = Writer::new(EXECUTE_RESPONSE_MAGIC);
    writer.binding_request(binding);
    writer.bytes(&request_digest);
    writer.u32(checkpoint);
    match outcome {
        ExecuteOutcome::Scalar(value) => {
            writer.u32(1);
            writer.u32(0);
            writer.i64(value);
        }
        ExecuteOutcome::SemanticFailure { selected_ordinal }
            if selected_ordinal != 0 && selected_ordinal <= dictionary_entries =>
        {
            writer.u32(2);
            writer.u32(selected_ordinal);
            writer.u64(0);
        }
        ExecuteOutcome::Owned {
            owner_ordinal,
            payload,
        } => {
            writer.u32(3);
            writer.u32(owner_ordinal);
            writer.u64(payload);
        }
        ExecuteOutcome::SemanticFailure { .. } => return Err(WireV3Error::InvalidOutcome),
    }
    writer.u32(to_u32(ordinals.len())?);
    for ordinal in ordinals {
        writer.u32(*ordinal);
    }
    let declared = writer.finalize_total()?;
    if declared.len() > capacity as usize {
        return Err(WireV3Error::Capacity);
    }
    let mut storage = declared;
    storage.resize(capacity as usize, 0);
    Ok(storage)
}

pub(super) fn encode_decision(
    binding: ProviderBinding,
    decision: SettlementDecision,
) -> Result<Vec<u8>, WireV3Error> {
    binding.validate()?;
    let (tag, detail) = match decision {
        SettlementDecision::AcceptScalar => (1, 0),
        SettlementDecision::AcceptSemanticFailure => (2, 0),
        SettlementDecision::AcceptOwned { owner_ordinal } => (3, owner_ordinal),
        SettlementDecision::AbortPhysical { code } if code != 0 => (4, code),
        SettlementDecision::AbortPhysical { .. } => return Err(WireV3Error::InvalidOutcome),
        SettlementDecision::AbortMalformed => (5, 0),
        SettlementDecision::AbortTraceRejected => (6, 0),
        SettlementDecision::AbortHostUnwind => (7, 0),
    };
    let mut writer = Writer::new(DECISION_MAGIC);
    writer.binding_full(binding);
    writer.u32(tag);
    writer.u32(detail);
    let bytes = writer.finish_exact()?;
    if bytes.len() != DECISION_BYTES as usize {
        return Err(WireV3Error::Capacity);
    }
    Ok(bytes)
}

pub(super) fn encode_action_evidence(evidence: ActionEvidence) -> Result<Vec<u8>, WireV3Error> {
    evidence.binding.validate()?;
    if evidence.checkpoint == 0 {
        return Err(WireV3Error::InvalidState);
    }
    let tag = match (evidence.boundary, evidence.before, evidence.after) {
        (
            ActionBoundary::FinalizerStarted,
            ResourceState::Live | ResourceState::ProvisionalResult,
            ResourceState::Finalizing,
        ) => 1,
        (ActionBoundary::FinalizerCompleted, ResourceState::Finalizing, ResourceState::Dead) => 2,
        (ActionBoundary::Publish, ResourceState::ProvisionalResult, ResourceState::Published) => 3,
        _ => return Err(WireV3Error::InvalidState),
    };
    let mut writer = Writer::new(ACTION_MAGIC);
    writer.binding_full(evidence.binding);
    writer.u32(evidence.action_index);
    writer.u32(tag);
    writer.u32(evidence.owner_ordinal);
    writer.u64(evidence.payload);
    writer.u32(evidence.before as u32);
    writer.u32(evidence.after as u32);
    writer.u32(evidence.checkpoint);
    let bytes = writer.finish_exact()?;
    if bytes.len() != ACTION_EVIDENCE_BYTES as usize {
        return Err(WireV3Error::Capacity);
    }
    Ok(bytes)
}

pub(super) fn encode_frame(frame: &RecoveryFrame<'_>) -> Result<Vec<u8>, WireV3Error> {
    frame.binding.validate()?;
    require_digest(&frame.request_digest)?;
    if frame.checkpoint == 0 || frame.resources.is_empty() {
        return Err(WireV3Error::InvalidCount);
    }
    validate_frame_state(frame)?;
    let mut writer = Writer::new(FRAME_MAGIC);
    writer.binding_full(frame.binding);
    writer.bytes(&frame.request_digest);
    writer.bytes(&frame.response_digest);
    writer.bytes(&frame.semantic_trace_digest);
    match frame.execute_return {
        ExecuteReturn::Pending => {
            writer.u32(1);
            writer.u32(0);
        }
        ExecuteReturn::Returned(code) => {
            writer.u32(2);
            writer.u32(code);
        }
        ExecuteReturn::PreExecuteHostUnwind => {
            writer.u32(3);
            writer.u32(PRE_EXECUTE_HOST_UNWIND_CODE);
        }
    }
    writer.u32(frame.checkpoint);
    writer.u32(frame.phase as u32);
    writer.bytes(&frame.decision_digest);
    writer.u32(frame.next_action_index);
    writer.u32(frame.action_record_count);
    writer.u32(frame.active_finalizers);
    writer.u32(to_u32(frame.resources.len())?);
    for resource in frame.resources {
        writer.u32(resource.state as u32);
        writer.u64(resource.payload);
    }
    writer.bytes(&frame.action_chain_digest);
    writer.bytes(&ZERO_DIGEST);
    let mut bytes = writer.finish_exact()?;
    let expected = frame_capacity(to_u32(frame.resources.len())?)? as usize;
    if bytes.len() != expected {
        return Err(WireV3Error::Capacity);
    }
    let digest_offset = bytes.len() - 32;
    let digest = pre_candidate_frame_digest(&bytes)?;
    bytes[digest_offset..].copy_from_slice(&digest);
    Ok(bytes)
}

pub(super) fn encode_candidate_receipt(
    receipt: &CandidateReceipt<'_>,
) -> Result<Vec<u8>, WireV3Error> {
    receipt.binding.validate()?;
    for digest in [
        &receipt.request_digest,
        &receipt.response_digest,
        &receipt.pre_candidate_frame_digest,
        &receipt.decision_digest,
        &receipt.action_chain_digest,
    ] {
        require_digest(digest)?;
    }
    if receipt.active_finalizers != 0 || receipt.dispositions.is_empty() {
        return Err(WireV3Error::InvalidState);
    }
    let (tag, detail, require_trace, published) = match receipt.outcome {
        CandidateOutcome::Scalar => (1, 0, true, None),
        CandidateOutcome::SemanticFailure => (2, 0, true, None),
        CandidateOutcome::Owned { owner_ordinal } => (3, owner_ordinal, true, Some(owner_ordinal)),
        CandidateOutcome::Abort => (4, 0, false, None),
    };
    if require_trace == is_zero(&receipt.semantic_trace_digest) {
        return Err(WireV3Error::InvalidDigest);
    }
    let actual_published = receipt
        .dispositions
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            (cell.disposition == TerminalDisposition::Published).then_some(index as u32)
        })
        .collect::<Vec<_>>();
    match published {
        Some(owner) if actual_published.as_slice() == [owner] => {}
        None if actual_published.is_empty() => {}
        _ => return Err(WireV3Error::InvalidOutcome),
    }
    let mut writer = Writer::new(CANDIDATE_MAGIC);
    writer.binding_full(receipt.binding);
    writer.bytes(&receipt.request_digest);
    writer.bytes(&receipt.response_digest);
    writer.bytes(&receipt.semantic_trace_digest);
    writer.bytes(&receipt.pre_candidate_frame_digest);
    writer.bytes(&receipt.decision_digest);
    writer.bytes(&receipt.action_chain_digest);
    writer.u32(tag);
    writer.u32(detail);
    writer.u32(receipt.active_finalizers);
    writer.u32(to_u32(receipt.dispositions.len())?);
    for disposition in receipt.dispositions {
        writer.u32(disposition.disposition as u32);
        writer.u64(disposition.payload);
    }
    let bytes = writer.finish_exact()?;
    if bytes.len() != candidate_receipt_capacity(to_u32(receipt.dispositions.len())?)? as usize {
        return Err(WireV3Error::Capacity);
    }
    Ok(bytes)
}

pub(super) fn request_digest(bytes: &[u8]) -> Result<[u8; 32], WireV3Error> {
    require_exact_wire(bytes, REQUEST_MAGIC)?;
    Ok(framed_sha256(REQUEST_DIGEST_DOMAIN, bytes))
}

pub(super) fn response_storage_digest(
    execute_return: u32,
    storage: &[u8],
) -> Result<[u8; 32], WireV3Error> {
    if storage.is_empty() || storage.len() > MAX_WIRE_BYTES as usize {
        return Err(WireV3Error::Capacity);
    }
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_STORAGE_DIGEST_DOMAIN);
    hasher.update(execute_return.to_le_bytes());
    hash_field(&mut hasher, storage);
    Ok(hasher.finalize().into())
}

pub(super) fn decision_digest(bytes: &[u8]) -> Result<[u8; 32], WireV3Error> {
    require_exact_wire(bytes, DECISION_MAGIC)?;
    Ok(framed_sha256(DECISION_DIGEST_DOMAIN, bytes))
}

pub(super) fn initial_action_chain_digest(
    decision_digest: [u8; 32],
    expected_action_count: u64,
) -> Result<[u8; 32], WireV3Error> {
    require_digest(&decision_digest)?;
    let mut hasher = Sha256::new();
    hasher.update(ACTION_CHAIN_SEED_DOMAIN);
    hasher.update(decision_digest);
    hasher.update(expected_action_count.to_le_bytes());
    Ok(hasher.finalize().into())
}

pub(super) fn extend_action_chain_digest(
    previous: [u8; 32],
    record_index: u64,
    action_evidence: &[u8],
) -> Result<[u8; 32], WireV3Error> {
    require_digest(&previous)?;
    require_exact_wire(action_evidence, ACTION_MAGIC)?;
    if action_evidence.len() != ACTION_EVIDENCE_BYTES as usize {
        return Err(WireV3Error::Capacity);
    }
    let mut hasher = Sha256::new();
    hasher.update(ACTION_CHAIN_STEP_DOMAIN);
    hasher.update(previous);
    hasher.update(record_index.to_le_bytes());
    hash_field(&mut hasher, action_evidence);
    Ok(hasher.finalize().into())
}

pub(super) fn pre_candidate_frame_digest(bytes: &[u8]) -> Result<[u8; 32], WireV3Error> {
    require_exact_wire(bytes, FRAME_MAGIC)?;
    let prefix = bytes
        .get(..bytes.len().checked_sub(32).ok_or(WireV3Error::Capacity)?)
        .ok_or(WireV3Error::Capacity)?;
    Ok(framed_sha256(FRAME_DIGEST_DOMAIN, prefix))
}

pub(super) fn candidate_digest(bytes: &[u8]) -> Result<[u8; 32], WireV3Error> {
    require_exact_wire(bytes, CANDIDATE_MAGIC)?;
    Ok(framed_sha256(CANDIDATE_DIGEST_DOMAIN, bytes))
}

pub(super) fn execute_response_capacity(maximum_event_count: u32) -> Result<u32, WireV3Error> {
    checked_capacity(
        EXECUTE_RESPONSE_FIXED_BYTES,
        EVENT_ORDINAL_BYTES,
        maximum_event_count,
    )
}

pub(super) fn frame_capacity(resource_count: u32) -> Result<u32, WireV3Error> {
    checked_capacity(FRAME_FIXED_BYTES, FRAME_RESOURCE_CELL_BYTES, resource_count)
}

pub(super) fn candidate_receipt_capacity(resource_count: u32) -> Result<u32, WireV3Error> {
    checked_capacity(
        CANDIDATE_RECEIPT_FIXED_BYTES,
        CANDIDATE_DISPOSITION_BYTES,
        resource_count,
    )
}

fn checked_capacity(fixed: u32, per_item: u32, count: u32) -> Result<u32, WireV3Error> {
    let capacity = per_item
        .checked_mul(count)
        .and_then(|bytes| fixed.checked_add(bytes))
        .ok_or(WireV3Error::Capacity)?;
    if capacity == 0 || capacity > MAX_WIRE_BYTES {
        return Err(WireV3Error::Capacity);
    }
    Ok(capacity)
}

fn validate_frame_state(frame: &RecoveryFrame<'_>) -> Result<(), WireV3Error> {
    let finalizing = frame
        .resources
        .iter()
        .filter(|cell| cell.state == ResourceState::Finalizing)
        .count();
    match frame.execute_return {
        ExecuteReturn::Pending
            if !is_zero(&frame.response_digest) || !is_zero(&frame.semantic_trace_digest) =>
        {
            return Err(WireV3Error::InvalidState)
        }
        ExecuteReturn::Returned(_) if is_zero(&frame.response_digest) => {
            return Err(WireV3Error::InvalidDigest)
        }
        ExecuteReturn::PreExecuteHostUnwind
            if is_zero(&frame.response_digest) || !is_zero(&frame.semantic_trace_digest) =>
        {
            return Err(WireV3Error::InvalidState)
        }
        ExecuteReturn::Pending
        | ExecuteReturn::Returned(_)
        | ExecuteReturn::PreExecuteHostUnwind => {}
    }
    match frame.phase {
        FramePhase::Executing => {
            if !is_zero(&frame.decision_digest)
                || !is_zero(&frame.action_chain_digest)
                || frame.next_action_index != 0
                || frame.action_record_count != 0
                || frame.active_finalizers > 1
                || finalizing != frame.active_finalizers as usize
            {
                return Err(WireV3Error::InvalidState);
            }
        }
        FramePhase::DecisionLocked => {
            require_digest(&frame.decision_digest)?;
            require_digest(&frame.action_chain_digest)?;
            if frame.next_action_index != 0
                || frame.action_record_count != 0
                || frame.active_finalizers != 0
                || finalizing != 0
            {
                return Err(WireV3Error::InvalidState);
            }
        }
        FramePhase::ActionInProgress => {
            require_digest(&frame.decision_digest)?;
            require_digest(&frame.action_chain_digest)?;
            if frame.active_finalizers > 1 || finalizing != frame.active_finalizers as usize {
                return Err(WireV3Error::InvalidState);
            }
        }
        FramePhase::ProviderSettled => {
            require_digest(&frame.decision_digest)?;
            require_digest(&frame.action_chain_digest)?;
            if frame.active_finalizers != 0
                || finalizing != 0
                || frame.resources.iter().any(|cell| {
                    !matches!(cell.state, ResourceState::Dead | ResourceState::Published)
                })
            {
                return Err(WireV3Error::InvalidState);
            }
        }
        FramePhase::ReceiptCommitted | FramePhase::Quarantined => {
            return Err(WireV3Error::InvalidState);
        }
    }
    Ok(())
}

fn framed_sha256(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hash_field(&mut hasher, bytes);
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn require_digest(digest: &[u8; 32]) -> Result<(), WireV3Error> {
    if is_zero(digest) {
        Err(WireV3Error::InvalidDigest)
    } else {
        Ok(())
    }
}

fn is_zero(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn to_u32(value: usize) -> Result<u32, WireV3Error> {
    u32::try_from(value).map_err(|_| WireV3Error::Capacity)
}

fn require_exact_wire(bytes: &[u8], magic: &[u8; 8]) -> Result<(), WireV3Error> {
    if bytes.len() < HEADER_BYTES as usize
        || bytes.len() > MAX_WIRE_BYTES as usize
        || bytes.get(..8) != Some(magic.as_slice())
        || read_u32(bytes, 8)? != VERSION
        || read_u32(bytes, 12)? != HEADER_BYTES
        || read_u32(bytes, 16)? as usize != bytes.len()
    {
        return Err(WireV3Error::InvalidTag);
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, WireV3Error> {
    let field = bytes.get(offset..offset + 4).ok_or(WireV3Error::Capacity)?;
    Ok(u32::from_le_bytes(
        field.try_into().map_err(|_| WireV3Error::Capacity)?,
    ))
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new(magic: &[u8; 8]) -> Self {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(magic);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&HEADER_BYTES.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        Self { bytes }
    }

    fn binding_request(&mut self, binding: ProviderBinding) {
        self.bytes(&binding.call_contract);
        self.u64(binding.invocation);
        self.u64(binding.frame_generation);
        self.bytes(&binding.provider_challenge);
    }

    fn binding_full(&mut self, binding: ProviderBinding) {
        self.bytes(&binding.call_contract);
        self.bytes(&binding.recovery_contract);
        self.bytes(&binding.settlement_graph);
        self.u64(binding.invocation);
        self.u64(binding.frame_generation);
        self.bytes(&binding.provider_challenge);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn finalize_total(mut self) -> Result<Vec<u8>, WireV3Error> {
        if self.bytes.len() > MAX_WIRE_BYTES as usize {
            return Err(WireV3Error::Capacity);
        }
        let total = to_u32(self.bytes.len())?;
        self.bytes[16..20].copy_from_slice(&total.to_le_bytes());
        Ok(self.bytes)
    }

    fn finish_exact(self) -> Result<Vec<u8>, WireV3Error> {
        self.finalize_total()
    }
}

#[cfg(test)]
#[path = "native_callable_wire_v3/tests.rs"]
mod tests;
