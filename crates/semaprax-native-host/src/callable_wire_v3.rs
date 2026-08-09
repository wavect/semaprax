//! Independent host codecs and replay gate for the private callable-v3 wires.
//!
//! Provider bytes are evidence only. This module owns no loader, ledger,
//! finalizer, capability authority, or publication authority.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable-v3 runtime admission remains private and unwired"
)]

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::descriptor_v3::{
    Action as GraphAction, Descriptor, Outcome as GraphOutcome, Parameter,
    ResourceState as GraphResourceState, ResultShape, TraceOutcome,
};

type HmacSha256 = Hmac<Sha256>;

const VERSION: u32 = 3;
const HEADER_SIZE: u32 = 20;
const REQUEST_MAGIC: &[u8; 8] = b"SPXNRQ03";
const RESPONSE_MAGIC: &[u8; 8] = b"SPXNEX03";
const FRAME_MAGIC: &[u8; 8] = b"SPXNFR03";
const DECISION_MAGIC: &[u8; 8] = b"SPXNDC03";
const ACTION_MAGIC: &[u8; 8] = b"SPXNAC03";
const CANDIDATE_MAGIC: &[u8; 8] = b"SPXNCR03";
const HOST_RECEIPT_MAGIC: &[u8; 8] = b"SPXHRP03";
pub(crate) const HOST_RECEIPT_BYTES: usize = 524;
const HOST_RECEIPT_BODY_BYTES: usize = 492;

const REQUEST_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-request-digest.v3\0";
const RESPONSE_STORAGE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.native-callable-execute-response-storage-digest.v3\0";
const DECISION_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-decision-digest.v3\0";
const ACTION_CHAIN_SEED_DOMAIN: &[u8] = b"semaprax.native-callable-action-chain-seed.v3\0";
const ACTION_CHAIN_STEP_DOMAIN: &[u8] = b"semaprax.native-callable-action-chain-step.v3\0";
const FRAME_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-pre-candidate-frame-digest.v3\0";
const CANDIDATE_DIGEST_DOMAIN: &[u8] = b"semaprax.native-callable-candidate-digest.v3\0";
const TRACE_EVIDENCE_DOMAIN: &[u8] = b"semaprax.native-recovery-trace-evidence.v1\0";
const RECEIPT_MAC_DOMAIN: &[u8] = b"semaprax.native-callable-host-receipt-auth.v3\0";
const PROVIDER_CHALLENGE_DOMAIN: &[u8] = b"semaprax.native-callable-provider-challenge.v3\0";
const LEDGER_BEFORE_DOMAIN: &[u8] = b"semaprax.native-callable-ledger-before.v3\0";
const LEDGER_AFTER_DOMAIN: &[u8] = b"semaprax.native-callable-ledger-after.v3\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WireError {
    Malformed,
    UnsupportedSchema,
    NonCanonical,
    CapacityMismatch,
    CrossBinding,
    DigestMismatch,
    ReplayMismatch,
    AuthenticationFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CallIdentity {
    pub(crate) call_contract: [u8; 32],
    pub(crate) invocation: NonZeroU64,
    pub(crate) frame_generation: NonZeroU64,
    pub(crate) provider_challenge: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryIdentity {
    pub(crate) call: CallIdentity,
    pub(crate) recovery_contract: [u8; 32],
    pub(crate) settlement_graph: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestArgument {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecuteRequest {
    pub(crate) identity: CallIdentity,
    pub(crate) arguments: Vec<RequestArgument>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecuteOutcome {
    Scalar { value: i64 },
    SemanticFailure { selected_ordinal: u32 },
    Owned { owner_ordinal: u32, payload: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecuteResponse {
    pub(crate) identity: CallIdentity,
    pub(crate) request_digest: [u8; 32],
    pub(crate) checkpoint: u32,
    pub(crate) outcome: ExecuteOutcome,
    pub(crate) event_ordinals: Vec<u32>,
    pub(crate) storage_capacity: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecuteReturn {
    Pending,
    Returned(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FramePhase {
    Executing,
    DecisionLocked,
    ActionInProgress,
    ProviderSettled,
    ReceiptCommitted,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellState {
    Live,
    ProvisionalResult,
    Finalizing,
    Dead,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResourceCell {
    pub(crate) state: CellState,
    pub(crate) payload: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryFrame {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) request_digest: [u8; 32],
    pub(crate) response_storage_digest: [u8; 32],
    pub(crate) semantic_trace_digest: [u8; 32],
    pub(crate) execute_return: ExecuteReturn,
    pub(crate) checkpoint: u32,
    pub(crate) phase: FramePhase,
    pub(crate) decision_digest: [u8; 32],
    pub(crate) next_action: u32,
    pub(crate) record_count: u32,
    pub(crate) active_finalizers: u32,
    pub(crate) cells: Vec<ResourceCell>,
    pub(crate) action_chain_digest: [u8; 32],
    pub(crate) pre_candidate_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Decision {
    AcceptScalar,
    AcceptSemanticFailure,
    AcceptOwned(u32),
    AbortPhysical(u32),
    AbortMalformed,
    AbortTraceRejected,
    AbortHostUnwind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettlementDecision {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) decision: Decision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionBoundary {
    Started,
    Completed,
    Publish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActionRecord {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) semantic_action_index: u32,
    pub(crate) boundary: ActionBoundary,
    pub(crate) owner_ordinal: u32,
    pub(crate) payload: u64,
    pub(crate) before: CellState,
    pub(crate) after: CellState,
    pub(crate) checkpoint: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateOutcome {
    Scalar,
    Failure,
    Owned(u32),
    Abort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Disposition {
    Dead,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DispositionCell {
    pub(crate) disposition: Disposition,
    pub(crate) payload: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CandidateReceipt {
    pub(crate) identity: RecoveryIdentity,
    pub(crate) request_digest: [u8; 32],
    pub(crate) response_storage_digest: [u8; 32],
    pub(crate) semantic_trace_digest: [u8; 32],
    pub(crate) frame_digest: [u8; 32],
    pub(crate) decision_digest: [u8; 32],
    pub(crate) action_evidence_digest: [u8; 32],
    pub(crate) outcome: CandidateOutcome,
    pub(crate) active_finalizers: u32,
    pub(crate) dispositions: Vec<DispositionCell>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Publication {
    NoOwned,
    Owned(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCommittedReceipt {
    pub(crate) instance_binding: [u8; 32],
    pub(crate) identity: RecoveryIdentity,
    pub(crate) request_digest: [u8; 32],
    pub(crate) response_storage_digest: [u8; 32],
    pub(crate) semantic_trace_digest: [u8; 32],
    pub(crate) frame_digest: [u8; 32],
    pub(crate) decision_digest: [u8; 32],
    pub(crate) action_evidence_digest: [u8; 32],
    pub(crate) candidate_digest: [u8; 32],
    pub(crate) ledger_before_digest: [u8; 32],
    pub(crate) ledger_after_digest: [u8; 32],
    pub(crate) publication: Publication,
    tag: [u8; 32],
}

/// Distinct host receipt key. It has no conversion to the owner-capability key.
pub(crate) struct ReceiptMacKey([u8; 32]);

impl ReceiptMacKey {
    pub(crate) fn from_runtime_bytes(bytes: [u8; 32]) -> Result<Self, WireError> {
        if bytes == [0; 32] {
            return Err(WireError::NonCanonical);
        }
        Ok(Self(bytes))
    }

    pub(crate) fn provider_challenge(
        &self,
        instance_binding: [u8; 32],
        instance_nonce: NonZeroU64,
        call_contract: [u8; 32],
        invocation: NonZeroU64,
        frame_generation: NonZeroU64,
    ) -> Result<[u8; 32], WireError> {
        if instance_binding == [0; 32] || call_contract == [0; 32] {
            return Err(WireError::NonCanonical);
        }
        let mut mac =
            HmacSha256::new_from_slice(&self.0).map_err(|_| WireError::AuthenticationFailed)?;
        mac.update(PROVIDER_CHALLENGE_DOMAIN);
        mac.update(&instance_binding);
        mac.update(&instance_nonce.get().to_le_bytes());
        mac.update(&call_contract);
        mac.update(&invocation.get().to_le_bytes());
        mac.update(&frame_generation.get().to_le_bytes());
        let challenge: [u8; 32] = mac.finalize().into_bytes().into();
        nonzero_digest(challenge)
    }
}

impl Drop for ReceiptMacKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LedgerState {
    InInvocation,
    Retired,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LedgerEntry {
    pub(crate) owner_ordinal: u32,
    pub(crate) slot: u64,
    pub(crate) generation: u64,
    pub(crate) state: LedgerState,
}

impl ExecuteRequest {
    pub(crate) fn parse(bytes: &[u8], descriptor: &Descriptor) -> Result<Self, WireError> {
        if bytes.len() != descriptor.capacities.request as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut reader = Reader::envelope(bytes, REQUEST_MAGIC)?;
        let identity = reader.call_identity()?;
        validate_call_identity(identity, descriptor)?;
        let count = reader.usize()?;
        if count != descriptor.parameters.len() {
            return Err(WireError::NonCanonical);
        }
        let mut arguments = Vec::with_capacity(count);
        for (expected_index, parameter) in descriptor.parameters.iter().enumerate() {
            let tag = reader.u32()?;
            let index = reader.u32()?;
            if usize::try_from(index).ok() != Some(expected_index) {
                return Err(WireError::NonCanonical);
            }
            let argument = match (tag, parameter) {
                (1, Parameter::Scalar { kind, .. }) => match kind {
                    crate::descriptor_v3::ScalarKind::I64 => RequestArgument::I64 {
                        index,
                        value: reader.i64()?,
                    },
                    crate::descriptor_v3::ScalarKind::Bool => {
                        let value = reader.u32()?;
                        if value > 1 {
                            return Err(WireError::NonCanonical);
                        }
                        RequestArgument::Bool {
                            index,
                            value: value == 1,
                        }
                    }
                },
                (2, Parameter::Owned { owner_ordinal, .. }) => {
                    let actual_owner = reader.u32()?;
                    if usize::try_from(actual_owner).ok() != Some(*owner_ordinal) {
                        return Err(WireError::NonCanonical);
                    }
                    RequestArgument::Owned {
                        index,
                        owner_ordinal: actual_owner,
                        payload: reader.u64()?,
                    }
                }
                _ => return Err(WireError::NonCanonical),
            };
            arguments.push(argument);
        }
        reader.finish()?;
        let value = Self {
            identity,
            arguments,
        };
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(REQUEST_MAGIC);
        writer.call_identity(self.identity);
        writer.u32(self.arguments.len() as u32);
        for argument in &self.arguments {
            match *argument {
                RequestArgument::I64 { index, value } => {
                    writer.u32(1);
                    writer.u32(index);
                    writer.i64(value);
                }
                RequestArgument::Bool { index, value } => {
                    writer.u32(1);
                    writer.u32(index);
                    writer.u32(u32::from(value));
                }
                RequestArgument::Owned {
                    index,
                    owner_ordinal,
                    payload,
                } => {
                    writer.u32(2);
                    writer.u32(index);
                    writer.u32(owner_ordinal);
                    writer.u64(payload);
                }
            }
        }
        writer.finish()
    }
}

impl ExecuteResponse {
    pub(crate) fn parse(bytes: &[u8], descriptor: &Descriptor) -> Result<Self, WireError> {
        if bytes.len() != descriptor.capacities.execute_response as usize {
            return Err(WireError::CapacityMismatch);
        }
        let declared = declared_total(bytes, RESPONSE_MAGIC)?;
        if declared < 156 || bytes[declared..].iter().any(|byte| *byte != 0) {
            return Err(WireError::NonCanonical);
        }
        let mut reader = Reader::envelope(&bytes[..declared], RESPONSE_MAGIC)?;
        let identity = reader.call_identity()?;
        validate_call_identity(identity, descriptor)?;
        let request_digest = nonzero_digest(reader.digest()?)?;
        let checkpoint = reader.u32()?;
        if checkpoint == 0 || checkpoint > descriptor.capacities.checkpoint_count {
            return Err(WireError::NonCanonical);
        }
        let tag = reader.u32()?;
        let detail = reader.u32()?;
        let payload = reader.u64()?;
        let outcome = match tag {
            1 if detail == 0 && descriptor.result == ResultShape::ScalarI64 => {
                ExecuteOutcome::Scalar {
                    value: i64::from_le_bytes(payload.to_le_bytes()),
                }
            }
            2 if detail != 0
                && detail <= descriptor.capacities.dictionary_entries
                && payload == 0 =>
            {
                ExecuteOutcome::SemanticFailure {
                    selected_ordinal: detail,
                }
            }
            3 if detail < descriptor.capacities.resource_count
                && matches!(
                    descriptor.result,
                    ResultShape::OwnedInput { owner_ordinal, .. }
                        if owner_ordinal == detail as usize
                ) =>
            {
                ExecuteOutcome::Owned {
                    owner_ordinal: detail,
                    payload,
                }
            }
            _ => return Err(WireError::NonCanonical),
        };
        let event_count = reader.usize()?;
        if event_count == 0 || event_count > descriptor.capacities.event_count as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut event_ordinals = Vec::with_capacity(event_count);
        for _ in 0..event_count {
            let ordinal = reader.u32()?;
            if ordinal == 0 || ordinal > descriptor.capacities.dictionary_entries {
                return Err(WireError::NonCanonical);
            }
            event_ordinals.push(ordinal);
        }
        reader.finish()?;
        let value = Self {
            identity,
            request_digest,
            checkpoint,
            outcome,
            event_ordinals,
            storage_capacity: bytes.len(),
        };
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(RESPONSE_MAGIC);
        writer.call_identity(self.identity);
        writer.digest(self.request_digest);
        writer.u32(self.checkpoint);
        match self.outcome {
            ExecuteOutcome::Scalar { value } => {
                writer.u32(1);
                writer.u32(0);
                writer.u64(u64::from_le_bytes(value.to_le_bytes()));
            }
            ExecuteOutcome::SemanticFailure { selected_ordinal } => {
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
        }
        writer.u32(self.event_ordinals.len() as u32);
        for ordinal in &self.event_ordinals {
            writer.u32(*ordinal);
        }
        let mut bytes = writer.finish();
        bytes.resize(self.storage_capacity, 0);
        bytes
    }
}

impl RecoveryFrame {
    pub(crate) fn parse(bytes: &[u8], descriptor: &Descriptor) -> Result<Self, WireError> {
        if bytes.len() != descriptor.capacities.frame as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut reader = Reader::envelope(bytes, FRAME_MAGIC)?;
        let identity = reader.recovery_identity()?;
        validate_recovery_identity(identity, descriptor)?;
        let request_digest = reader.digest()?;
        let response_storage_digest = reader.digest()?;
        let semantic_trace_digest = reader.digest()?;
        let return_tag = reader.u32()?;
        let execute_code = reader.u32()?;
        let execute_return = match (return_tag, execute_code) {
            (1, 0) => ExecuteReturn::Pending,
            (2, code) => ExecuteReturn::Returned(code),
            _ => return Err(WireError::NonCanonical),
        };
        let checkpoint = reader.u32()?;
        if checkpoint == 0 || checkpoint > descriptor.capacities.checkpoint_count {
            return Err(WireError::NonCanonical);
        }
        let phase = match reader.u32()? {
            1 => FramePhase::Executing,
            2 => FramePhase::DecisionLocked,
            3 => FramePhase::ActionInProgress,
            4 => FramePhase::ProviderSettled,
            5 => FramePhase::ReceiptCommitted,
            6 => FramePhase::Quarantined,
            _ => return Err(WireError::NonCanonical),
        };
        let decision_digest = reader.digest()?;
        let next_action = reader.u32()?;
        let record_count = reader.u32()?;
        let active_finalizers = reader.u32()?;
        let resource_count = reader.usize()?;
        if resource_count != descriptor.capacities.resource_count as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut cells = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            cells.push(ResourceCell {
                state: parse_cell_state(reader.u32()?)?,
                payload: reader.u64()?,
            });
        }
        let action_chain_digest = reader.digest()?;
        let pre_candidate_digest = reader.digest()?;
        reader.finish()?;
        if matches!(
            phase,
            FramePhase::ReceiptCommitted | FramePhase::Quarantined
        ) {
            return Err(WireError::NonCanonical);
        }
        let expected_frame = frame_digest(&bytes[..bytes.len() - 32]);
        if pre_candidate_digest != expected_frame {
            return Err(WireError::DigestMismatch);
        }
        let value = Self {
            identity,
            request_digest,
            response_storage_digest,
            semantic_trace_digest,
            execute_return,
            checkpoint,
            phase,
            decision_digest,
            next_action,
            record_count,
            active_finalizers,
            cells,
            action_chain_digest,
            pre_candidate_digest,
        };
        validate_frame_state(&value)?;
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(FRAME_MAGIC);
        writer.recovery_identity(self.identity);
        writer.digest(self.request_digest);
        writer.digest(self.response_storage_digest);
        writer.digest(self.semantic_trace_digest);
        match self.execute_return {
            ExecuteReturn::Pending => {
                writer.u32(1);
                writer.u32(0);
            }
            ExecuteReturn::Returned(code) => {
                writer.u32(2);
                writer.u32(code);
            }
        }
        writer.u32(self.checkpoint);
        writer.u32(match self.phase {
            FramePhase::Executing => 1,
            FramePhase::DecisionLocked => 2,
            FramePhase::ActionInProgress => 3,
            FramePhase::ProviderSettled => 4,
            FramePhase::ReceiptCommitted => 5,
            FramePhase::Quarantined => 6,
        });
        writer.digest(self.decision_digest);
        writer.u32(self.next_action);
        writer.u32(self.record_count);
        writer.u32(self.active_finalizers);
        writer.u32(self.cells.len() as u32);
        for cell in &self.cells {
            writer.u32(cell_state_tag(cell.state));
            writer.u64(cell.payload);
        }
        writer.digest(self.action_chain_digest);
        writer.digest(self.pre_candidate_digest);
        writer.finish()
    }
}

impl SettlementDecision {
    pub(crate) fn parse(bytes: &[u8], descriptor: &Descriptor) -> Result<Self, WireError> {
        if bytes.len() != descriptor.capacities.decision as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut reader = Reader::envelope(bytes, DECISION_MAGIC)?;
        let identity = reader.recovery_identity()?;
        validate_recovery_identity(identity, descriptor)?;
        let tag = reader.u32()?;
        let detail = reader.u32()?;
        let decision = match (tag, detail) {
            (1, 0) if descriptor.result == ResultShape::ScalarI64 => Decision::AcceptScalar,
            (2, 0) => Decision::AcceptSemanticFailure,
            (3, owner)
                if matches!(
                    descriptor.result,
                    ResultShape::OwnedInput { owner_ordinal, .. }
                        if owner_ordinal == owner as usize
                ) =>
            {
                Decision::AcceptOwned(owner)
            }
            (4, code) if code != 0 => Decision::AbortPhysical(code),
            (5, 0) => Decision::AbortMalformed,
            (6, 0) => Decision::AbortTraceRejected,
            (7, 0) => Decision::AbortHostUnwind,
            _ => return Err(WireError::NonCanonical),
        };
        reader.finish()?;
        let value = Self { identity, decision };
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(DECISION_MAGIC);
        writer.recovery_identity(self.identity);
        let (tag, detail) = match self.decision {
            Decision::AcceptScalar => (1, 0),
            Decision::AcceptSemanticFailure => (2, 0),
            Decision::AcceptOwned(owner) => (3, owner),
            Decision::AbortPhysical(code) => (4, code),
            Decision::AbortMalformed => (5, 0),
            Decision::AbortTraceRejected => (6, 0),
            Decision::AbortHostUnwind => (7, 0),
        };
        writer.u32(tag);
        writer.u32(detail);
        writer.finish()
    }
}

impl ActionRecord {
    pub(crate) fn parse(bytes: &[u8], descriptor: &Descriptor) -> Result<Self, WireError> {
        if bytes.len() != descriptor.capacities.action_evidence as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut reader = Reader::envelope(bytes, ACTION_MAGIC)?;
        let identity = reader.recovery_identity()?;
        validate_recovery_identity(identity, descriptor)?;
        let semantic_action_index = reader.u32()?;
        let boundary = match reader.u32()? {
            1 => ActionBoundary::Started,
            2 => ActionBoundary::Completed,
            3 => ActionBoundary::Publish,
            _ => return Err(WireError::NonCanonical),
        };
        let owner_ordinal = reader.u32()?;
        if owner_ordinal >= descriptor.capacities.resource_count {
            return Err(WireError::NonCanonical);
        }
        let payload = reader.u64()?;
        let before = parse_cell_state(reader.u32()?)?;
        let after = parse_cell_state(reader.u32()?)?;
        let checkpoint = reader.u32()?;
        if checkpoint == 0 || checkpoint > descriptor.capacities.checkpoint_count {
            return Err(WireError::NonCanonical);
        }
        reader.finish()?;
        match (boundary, before, after) {
            (
                ActionBoundary::Started,
                CellState::Live | CellState::ProvisionalResult,
                CellState::Finalizing,
            )
            | (ActionBoundary::Completed, CellState::Finalizing, CellState::Dead)
            | (ActionBoundary::Publish, CellState::ProvisionalResult, CellState::Published) => {}
            _ => return Err(WireError::NonCanonical),
        }
        let value = Self {
            identity,
            semantic_action_index,
            boundary,
            owner_ordinal,
            payload,
            before,
            after,
            checkpoint,
        };
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(ACTION_MAGIC);
        writer.recovery_identity(self.identity);
        writer.u32(self.semantic_action_index);
        writer.u32(match self.boundary {
            ActionBoundary::Started => 1,
            ActionBoundary::Completed => 2,
            ActionBoundary::Publish => 3,
        });
        writer.u32(self.owner_ordinal);
        writer.u64(self.payload);
        writer.u32(cell_state_tag(self.before));
        writer.u32(cell_state_tag(self.after));
        writer.u32(self.checkpoint);
        writer.finish()
    }
}

impl CandidateReceipt {
    pub(crate) fn parse(bytes: &[u8], descriptor: &Descriptor) -> Result<Self, WireError> {
        if bytes.len() != descriptor.capacities.candidate_receipt as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut reader = Reader::envelope(bytes, CANDIDATE_MAGIC)?;
        let identity = reader.recovery_identity()?;
        validate_recovery_identity(identity, descriptor)?;
        let request_digest = nonzero_digest(reader.digest()?)?;
        let response_storage_digest = nonzero_digest(reader.digest()?)?;
        let semantic_trace_digest = reader.digest()?;
        let frame_digest = nonzero_digest(reader.digest()?)?;
        let decision_digest = nonzero_digest(reader.digest()?)?;
        let action_evidence_digest = nonzero_digest(reader.digest()?)?;
        let tag = reader.u32()?;
        let detail = reader.u32()?;
        let outcome = match (tag, detail) {
            (1, 0) => CandidateOutcome::Scalar,
            (2, 0) => CandidateOutcome::Failure,
            (3, owner) => CandidateOutcome::Owned(owner),
            (4, 0) => CandidateOutcome::Abort,
            _ => return Err(WireError::NonCanonical),
        };
        let active_finalizers = reader.u32()?;
        if active_finalizers != 0 {
            return Err(WireError::NonCanonical);
        }
        let count = reader.usize()?;
        if count != descriptor.capacities.resource_count as usize {
            return Err(WireError::CapacityMismatch);
        }
        let mut dispositions = Vec::with_capacity(count);
        for _ in 0..count {
            dispositions.push(DispositionCell {
                disposition: match reader.u32()? {
                    1 => Disposition::Dead,
                    2 => Disposition::Published,
                    _ => return Err(WireError::NonCanonical),
                },
                payload: reader.u64()?,
            });
        }
        reader.finish()?;
        let published = dispositions
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| {
                (cell.disposition == Disposition::Published).then_some(index as u32)
            })
            .collect::<Vec<_>>();
        match outcome {
            CandidateOutcome::Owned(owner)
                if matches!(
                    descriptor.result,
                    ResultShape::OwnedInput { owner_ordinal, .. }
                        if owner_ordinal == owner as usize
                ) && published.as_slice() == [owner] => {}
            CandidateOutcome::Owned(_) => return Err(WireError::NonCanonical),
            CandidateOutcome::Scalar
                if descriptor.result == ResultShape::ScalarI64 && published.is_empty() => {}
            CandidateOutcome::Scalar => return Err(WireError::NonCanonical),
            CandidateOutcome::Failure | CandidateOutcome::Abort if published.is_empty() => {}
            CandidateOutcome::Failure | CandidateOutcome::Abort => {
                return Err(WireError::NonCanonical)
            }
        }
        if matches!(outcome, CandidateOutcome::Abort) != (semantic_trace_digest == [0; 32]) {
            return Err(WireError::NonCanonical);
        }
        let value = Self {
            identity,
            request_digest,
            response_storage_digest,
            semantic_trace_digest,
            frame_digest,
            decision_digest,
            action_evidence_digest,
            outcome,
            active_finalizers,
            dispositions,
        };
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(CANDIDATE_MAGIC);
        writer.recovery_identity(self.identity);
        writer.digest(self.request_digest);
        writer.digest(self.response_storage_digest);
        writer.digest(self.semantic_trace_digest);
        writer.digest(self.frame_digest);
        writer.digest(self.decision_digest);
        writer.digest(self.action_evidence_digest);
        let (tag, detail) = match self.outcome {
            CandidateOutcome::Scalar => (1, 0),
            CandidateOutcome::Failure => (2, 0),
            CandidateOutcome::Owned(owner) => (3, owner),
            CandidateOutcome::Abort => (4, 0),
        };
        writer.u32(tag);
        writer.u32(detail);
        writer.u32(self.active_finalizers);
        writer.u32(self.dispositions.len() as u32);
        for disposition in &self.dispositions {
            writer.u32(match disposition.disposition {
                Disposition::Dead => 1,
                Disposition::Published => 2,
            });
            writer.u64(disposition.payload);
        }
        writer.finish()
    }
}

impl HostCommittedReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn authenticate(
        key: &ReceiptMacKey,
        instance_binding: [u8; 32],
        candidate: &CandidateReceipt,
        ledger_before_digest: [u8; 32],
        ledger_after_digest: [u8; 32],
    ) -> Result<Self, WireError> {
        if instance_binding == [0; 32]
            || ledger_before_digest == [0; 32]
            || ledger_after_digest == [0; 32]
        {
            return Err(WireError::NonCanonical);
        }
        let candidate_digest = candidate_digest(&candidate.encode());
        let publication = match candidate.outcome {
            CandidateOutcome::Owned(owner) => Publication::Owned(owner),
            CandidateOutcome::Scalar | CandidateOutcome::Failure | CandidateOutcome::Abort => {
                Publication::NoOwned
            }
        };
        let mut value = Self {
            instance_binding,
            identity: candidate.identity,
            request_digest: candidate.request_digest,
            response_storage_digest: candidate.response_storage_digest,
            semantic_trace_digest: candidate.semantic_trace_digest,
            frame_digest: candidate.frame_digest,
            decision_digest: candidate.decision_digest,
            action_evidence_digest: candidate.action_evidence_digest,
            candidate_digest,
            ledger_before_digest,
            ledger_after_digest,
            publication,
            tag: [0; 32],
        };
        let unsigned = value.encode();
        value.tag = receipt_mac(key, &unsigned[..HOST_RECEIPT_BODY_BYTES])?;
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn parse_and_verify(
        bytes: &[u8],
        key: &ReceiptMacKey,
        descriptor: &Descriptor,
        expected_instance_binding: [u8; 32],
        candidate: &CandidateReceipt,
        expected_ledger_before: [u8; 32],
        expected_ledger_after: [u8; 32],
    ) -> Result<Self, WireError> {
        if bytes.len() != HOST_RECEIPT_BYTES {
            return Err(WireError::CapacityMismatch);
        }
        let mut reader = Reader::envelope(bytes, HOST_RECEIPT_MAGIC)?;
        let instance_binding = reader.digest()?;
        if instance_binding == [0; 32] || instance_binding != expected_instance_binding {
            return Err(WireError::CrossBinding);
        }
        let identity = reader.recovery_identity()?;
        validate_recovery_identity(identity, descriptor)?;
        let request_digest = reader.digest()?;
        let response_storage_digest = reader.digest()?;
        let semantic_trace_digest = reader.digest()?;
        let frame_digest = reader.digest()?;
        let decision_digest = reader.digest()?;
        let action_evidence_digest = reader.digest()?;
        let candidate_digest_value = reader.digest()?;
        let ledger_before_digest = reader.digest()?;
        let ledger_after_digest = reader.digest()?;
        let publication = match (reader.u32()?, reader.u32()?) {
            (1, 0) => Publication::NoOwned,
            (2, owner) => Publication::Owned(owner),
            _ => return Err(WireError::NonCanonical),
        };
        let tag = reader.digest()?;
        reader.finish()?;
        verify_receipt_mac(key, &bytes[..HOST_RECEIPT_BODY_BYTES], &tag)?;
        if identity != candidate.identity
            || request_digest != candidate.request_digest
            || response_storage_digest != candidate.response_storage_digest
            || semantic_trace_digest != candidate.semantic_trace_digest
            || frame_digest != candidate.frame_digest
            || decision_digest != candidate.decision_digest
            || action_evidence_digest != candidate.action_evidence_digest
            || candidate_digest_value != candidate_digest(&candidate.encode())
            || ledger_before_digest != expected_ledger_before
            || ledger_after_digest != expected_ledger_after
            || publication
                != match candidate.outcome {
                    CandidateOutcome::Owned(owner) => Publication::Owned(owner),
                    _ => Publication::NoOwned,
                }
        {
            return Err(WireError::CrossBinding);
        }
        let value = Self {
            instance_binding,
            identity,
            request_digest,
            response_storage_digest,
            semantic_trace_digest,
            frame_digest,
            decision_digest,
            action_evidence_digest,
            candidate_digest: candidate_digest_value,
            ledger_before_digest,
            ledger_after_digest,
            publication,
            tag,
        };
        require_exact(bytes, &value.encode())?;
        Ok(value)
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new(HOST_RECEIPT_MAGIC);
        writer.digest(self.instance_binding);
        writer.recovery_identity(self.identity);
        writer.digest(self.request_digest);
        writer.digest(self.response_storage_digest);
        writer.digest(self.semantic_trace_digest);
        writer.digest(self.frame_digest);
        writer.digest(self.decision_digest);
        writer.digest(self.action_evidence_digest);
        writer.digest(self.candidate_digest);
        writer.digest(self.ledger_before_digest);
        writer.digest(self.ledger_after_digest);
        match self.publication {
            Publication::NoOwned => {
                writer.u32(1);
                writer.u32(0);
            }
            Publication::Owned(owner) => {
                writer.u32(2);
                writer.u32(owner);
            }
        }
        writer.digest(self.tag);
        writer.finish()
    }
}

/// Independently replay provider evidence from the authenticated graph and
/// exact request payloads. Success is eligibility evidence, not ReceiptCommit.
struct ValidatedSuccessfulExecute {
    request_digest: [u8; 32],
    response_storage_digest: [u8; 32],
    semantic_trace_digest: [u8; 32],
    checkpoint_cells: Vec<ResourceCell>,
}

/// Validate the provider's returned-success evidence before any settlement
/// decision is committed or physical settlement is entered. This gate does
/// not trust a parseable/resealed response or frame: it reconstructs the exact
/// semantic witness and checkpoint cells from the admitted descriptor and
/// caller-owned request payloads.
pub(crate) fn validate_successful_execute_evidence(
    descriptor: &Descriptor,
    request: &ExecuteRequest,
    execute_return_code: u32,
    response_storage: &[u8],
    response: &ExecuteResponse,
    frame: &RecoveryFrame,
) -> Result<(), WireError> {
    let validated = validate_successful_execute_response(
        descriptor,
        request,
        execute_return_code,
        response_storage,
        response,
    )?;
    let identity = recovery_identity_from_call(request.identity, descriptor);
    if frame.identity != identity || response.checkpoint != frame.checkpoint {
        return Err(WireError::CrossBinding);
    }
    if frame.request_digest != validated.request_digest
        || frame.response_storage_digest != validated.response_storage_digest
        || frame.semantic_trace_digest != validated.semantic_trace_digest
    {
        return Err(WireError::DigestMismatch);
    }
    if frame.execute_return != ExecuteReturn::Returned(0)
        || frame.phase != FramePhase::Executing
        || frame.decision_digest != [0; 32]
        || frame.next_action != 0
        || frame.record_count != 0
        || frame.active_finalizers != 0
        || frame.cells != validated.checkpoint_cells
        || frame.action_chain_digest != [0; 32]
    {
        return Err(WireError::ReplayMismatch);
    }
    Ok(())
}

fn validate_successful_execute_response(
    descriptor: &Descriptor,
    request: &ExecuteRequest,
    execute_return_code: u32,
    response_storage: &[u8],
    response: &ExecuteResponse,
) -> Result<ValidatedSuccessfulExecute, WireError> {
    if execute_return_code != 0
        || response_storage.len() != descriptor.capacities.execute_response as usize
        || response.encode() != response_storage
        || response.identity != request.identity
    {
        return Err(WireError::CrossBinding);
    }
    let request_digest_value = request_digest(&request.encode());
    if response.request_digest != request_digest_value {
        return Err(WireError::DigestMismatch);
    }
    let checkpoint = descriptor
        .graph
        .checkpoints
        .get(
            response
                .checkpoint
                .checked_sub(1)
                .ok_or(WireError::ReplayMismatch)? as usize,
        )
        .filter(|checkpoint| checkpoint.id == response.checkpoint)
        .ok_or(WireError::ReplayMismatch)?;
    let semantic_trace_digest = semantic_trace_digest(descriptor, response)?;
    if semantic_trace_digest == [0; 32] {
        return Err(WireError::DigestMismatch);
    }

    let mut payloads = BTreeMap::new();
    for argument in &request.arguments {
        if let RequestArgument::Owned {
            owner_ordinal,
            payload,
            ..
        } = argument
        {
            if payloads.insert(*owner_ordinal, *payload).is_some() {
                return Err(WireError::ReplayMismatch);
            }
        }
    }
    if payloads.len() != descriptor.capacities.resource_count as usize {
        return Err(WireError::ReplayMismatch);
    }
    let checkpoint_cells = checkpoint
        .resources
        .iter()
        .enumerate()
        .map(|(owner, state)| {
            Ok(ResourceCell {
                state: graph_state_to_cell(*state)?,
                payload: *payloads
                    .get(&(owner as u32))
                    .ok_or(WireError::ReplayMismatch)?,
            })
        })
        .collect::<Result<Vec<_>, WireError>>()?;
    if let ExecuteOutcome::Owned {
        owner_ordinal,
        payload,
    } = response.outcome
    {
        if payloads.get(&owner_ordinal) != Some(&payload)
            || checkpoint_cells
                .get(owner_ordinal as usize)
                .is_none_or(|cell| cell.state != CellState::ProvisionalResult)
        {
            return Err(WireError::ReplayMismatch);
        }
    }
    Ok(ValidatedSuccessfulExecute {
        request_digest: request_digest_value,
        response_storage_digest: response_storage_digest(execute_return_code, response_storage),
        semantic_trace_digest,
        checkpoint_cells,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_candidate_replay(
    descriptor: &Descriptor,
    request: &ExecuteRequest,
    execute_return_code: u32,
    response_storage: &[u8],
    response: Option<&ExecuteResponse>,
    frame: &RecoveryFrame,
    decision: &SettlementDecision,
    actions: &[ActionRecord],
    candidate: &CandidateReceipt,
) -> Result<(), WireError> {
    let identity = recovery_identity_from_call(request.identity, descriptor);
    if frame.identity != identity
        || decision.identity != identity
        || candidate.identity != identity
        || actions.iter().any(|action| action.identity != identity)
    {
        return Err(WireError::CrossBinding);
    }
    if response_storage.len() != descriptor.capacities.execute_response as usize {
        return Err(WireError::CapacityMismatch);
    }
    let request_bytes = request.encode();
    let request_digest_value = request_digest(&request_bytes);
    let response_digest_value = response_storage_digest(execute_return_code, response_storage);
    if frame.request_digest != request_digest_value
        || candidate.request_digest != request_digest_value
        || frame.response_storage_digest != response_digest_value
        || candidate.response_storage_digest != response_digest_value
        || frame.execute_return != ExecuteReturn::Returned(execute_return_code)
        || frame.phase != FramePhase::ProviderSettled
        || frame.active_finalizers != 0
        || candidate.active_finalizers != 0
        || frame.record_count as usize != actions.len()
    {
        return Err(WireError::ReplayMismatch);
    }
    let decision_bytes = decision.encode();
    let decision_digest_value = decision_digest(&decision_bytes);
    if frame.decision_digest != decision_digest_value
        || candidate.decision_digest != decision_digest_value
    {
        return Err(WireError::DigestMismatch);
    }
    match decision.decision {
        Decision::AcceptScalar
        | Decision::AcceptSemanticFailure
        | Decision::AcceptOwned(_)
        | Decision::AbortMalformed
        | Decision::AbortTraceRejected
            if execute_return_code != 0 =>
        {
            return Err(WireError::ReplayMismatch)
        }
        Decision::AbortPhysical(expected) if expected != execute_return_code => {
            return Err(WireError::ReplayMismatch)
        }
        Decision::AcceptScalar
        | Decision::AcceptSemanticFailure
        | Decision::AcceptOwned(_)
        | Decision::AbortPhysical(_)
        | Decision::AbortMalformed
        | Decision::AbortTraceRejected
        | Decision::AbortHostUnwind => {}
    }
    let response = match decision.decision {
        Decision::AcceptScalar | Decision::AcceptSemanticFailure | Decision::AcceptOwned(_) => {
            response.ok_or(WireError::ReplayMismatch)?
        }
        Decision::AbortPhysical(_)
        | Decision::AbortMalformed
        | Decision::AbortTraceRejected
        | Decision::AbortHostUnwind => {
            if candidate.semantic_trace_digest != [0; 32]
                || frame.semantic_trace_digest != [0; 32]
                || candidate.outcome != CandidateOutcome::Abort
            {
                return Err(WireError::ReplayMismatch);
            }
            return replay_actions_and_dispositions(
                descriptor, request, frame, decision, actions, candidate,
            );
        }
    };
    if response.checkpoint != frame.checkpoint {
        return Err(WireError::CrossBinding);
    }
    let validated = validate_successful_execute_response(
        descriptor,
        request,
        execute_return_code,
        response_storage,
        response,
    )?;
    if frame.request_digest != validated.request_digest
        || candidate.request_digest != validated.request_digest
        || frame.response_storage_digest != validated.response_storage_digest
        || candidate.response_storage_digest != validated.response_storage_digest
        || frame.semantic_trace_digest != validated.semantic_trace_digest
        || candidate.semantic_trace_digest != validated.semantic_trace_digest
    {
        return Err(WireError::DigestMismatch);
    }
    match (decision.decision, response.outcome, candidate.outcome) {
        (Decision::AcceptScalar, ExecuteOutcome::Scalar { .. }, CandidateOutcome::Scalar)
        | (
            Decision::AcceptSemanticFailure,
            ExecuteOutcome::SemanticFailure { .. },
            CandidateOutcome::Failure,
        ) => {}
        (
            Decision::AcceptOwned(expected),
            ExecuteOutcome::Owned {
                owner_ordinal,
                payload,
            },
            CandidateOutcome::Owned(actual),
        ) if expected == owner_ordinal
            && expected == actual
            && frame
                .cells
                .get(expected as usize)
                .is_some_and(|cell| cell.payload == payload)
            && candidate
                .dispositions
                .get(expected as usize)
                .is_some_and(|cell| cell.payload == payload) => {}
        _ => return Err(WireError::ReplayMismatch),
    }
    replay_actions_and_dispositions(descriptor, request, frame, decision, actions, candidate)
}

fn replay_actions_and_dispositions(
    descriptor: &Descriptor,
    request: &ExecuteRequest,
    frame: &RecoveryFrame,
    decision: &SettlementDecision,
    actions: &[ActionRecord],
    candidate: &CandidateReceipt,
) -> Result<(), WireError> {
    let checkpoint = descriptor
        .graph
        .checkpoints
        .get(
            frame
                .checkpoint
                .checked_sub(1)
                .ok_or(WireError::ReplayMismatch)? as usize,
        )
        .filter(|checkpoint| checkpoint.id == frame.checkpoint)
        .ok_or(WireError::ReplayMismatch)?;
    let mut payloads = BTreeMap::new();
    for argument in &request.arguments {
        if let RequestArgument::Owned {
            owner_ordinal,
            payload,
            ..
        } = argument
        {
            payloads.insert(*owner_ordinal, *payload);
        }
    }
    if payloads.len() != descriptor.capacities.resource_count as usize {
        return Err(WireError::ReplayMismatch);
    }
    let mut cells = checkpoint
        .resources
        .iter()
        .enumerate()
        .map(|(owner, state)| {
            Ok(ResourceCell {
                state: graph_state_to_cell(*state)?,
                payload: *payloads
                    .get(&(owner as u32))
                    .ok_or(WireError::ReplayMismatch)?,
            })
        })
        .collect::<Result<Vec<_>, WireError>>()?;
    let cleanup = match decision.decision {
        Decision::AcceptScalar | Decision::AcceptSemanticFailure | Decision::AcceptOwned(_) => {
            &checkpoint.accept_order
        }
        _ => &checkpoint.abort_order,
    };
    let publish_owner = match decision.decision {
        Decision::AcceptOwned(owner) => Some(owner),
        _ => None,
    };
    let expected_records = cleanup
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(usize::from(publish_owner.is_some())))
        .ok_or(WireError::CapacityMismatch)?;
    if actions.len() != expected_records {
        return Err(WireError::ReplayMismatch);
    }
    let expected_semantic_actions = cleanup.len() + usize::from(publish_owner.is_some());
    let expected_action_digest = action_chain_digest(
        candidate.decision_digest,
        expected_semantic_actions,
        actions,
    )?;
    if frame.action_chain_digest != expected_action_digest
        || candidate.action_evidence_digest != expected_action_digest
        || frame.pre_candidate_digest != candidate.frame_digest
    {
        return Err(WireError::DigestMismatch);
    }
    let mut record_cursor = 0_usize;
    let mut semantic_cursor = 0_usize;
    for owner in cleanup {
        let owner_index = *owner as usize;
        let started = actions
            .get(record_cursor)
            .ok_or(WireError::ReplayMismatch)?;
        let completed = actions
            .get(record_cursor + 1)
            .ok_or(WireError::ReplayMismatch)?;
        let before = cells
            .get(owner_index)
            .copied()
            .ok_or(WireError::ReplayMismatch)?;
        if started.semantic_action_index as usize != semantic_cursor
            || completed.semantic_action_index as usize != semantic_cursor
            || started.checkpoint != frame.checkpoint
            || completed.checkpoint != frame.checkpoint
            || started.owner_ordinal != *owner
            || completed.owner_ordinal != *owner
            || started.payload != before.payload
            || completed.payload != before.payload
            || started.boundary != ActionBoundary::Started
            || started.before != before.state
            || started.after != CellState::Finalizing
            || completed.boundary != ActionBoundary::Completed
            || completed.before != CellState::Finalizing
            || completed.after != CellState::Dead
        {
            return Err(WireError::ReplayMismatch);
        }
        cells[owner_index].state = CellState::Dead;
        record_cursor += 2;
        semantic_cursor += 1;
    }
    if let Some(owner) = publish_owner {
        let record = actions
            .get(record_cursor)
            .ok_or(WireError::ReplayMismatch)?;
        let cell = cells
            .get_mut(owner as usize)
            .ok_or(WireError::ReplayMismatch)?;
        if record.semantic_action_index as usize != semantic_cursor
            || record.checkpoint != frame.checkpoint
            || record.owner_ordinal != owner
            || record.payload != cell.payload
            || record.boundary != ActionBoundary::Publish
            || record.before != CellState::ProvisionalResult
            || record.after != CellState::Published
            || cell.state != CellState::ProvisionalResult
        {
            return Err(WireError::ReplayMismatch);
        }
        cell.state = CellState::Published;
        record_cursor += 1;
        semantic_cursor += 1;
    }
    if record_cursor != actions.len()
        || semantic_cursor != expected_semantic_actions
        || frame.next_action as usize != expected_semantic_actions
        || frame.cells != cells
        || candidate.dispositions.len() != cells.len()
    {
        return Err(WireError::ReplayMismatch);
    }
    for (candidate_cell, frame_cell) in candidate.dispositions.iter().zip(cells) {
        let expected = match frame_cell.state {
            CellState::Dead => Disposition::Dead,
            CellState::Published => Disposition::Published,
            _ => return Err(WireError::ReplayMismatch),
        };
        if candidate_cell.disposition != expected || candidate_cell.payload != frame_cell.payload {
            return Err(WireError::ReplayMismatch);
        }
    }
    Ok(())
}

pub(crate) fn request_digest(bytes: &[u8]) -> [u8; 32] {
    hash_framed(REQUEST_DIGEST_DOMAIN, bytes)
}

pub(crate) fn response_storage_digest(code: u32, storage: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_STORAGE_DIGEST_DOMAIN);
    hasher.update(code.to_le_bytes());
    hash_field(&mut hasher, storage);
    hasher.finalize().into()
}

pub(crate) fn decision_digest(bytes: &[u8]) -> [u8; 32] {
    hash_framed(DECISION_DIGEST_DOMAIN, bytes)
}

pub(crate) fn action_chain_digest(
    decision: [u8; 32],
    expected_semantic_action_count: usize,
    records: &[ActionRecord],
) -> Result<[u8; 32], WireError> {
    let count =
        u64::try_from(expected_semantic_action_count).map_err(|_| WireError::CapacityMismatch)?;
    let mut seed = Sha256::new();
    seed.update(ACTION_CHAIN_SEED_DOMAIN);
    seed.update(decision);
    seed.update(count.to_le_bytes());
    let mut digest: [u8; 32] = seed.finalize().into();
    for (index, record) in records.iter().enumerate() {
        let bytes = record.encode();
        let mut step = Sha256::new();
        step.update(ACTION_CHAIN_STEP_DOMAIN);
        step.update(digest);
        step.update((index as u64).to_le_bytes());
        hash_field(&mut step, &bytes);
        digest = step.finalize().into();
    }
    Ok(digest)
}

pub(crate) fn frame_digest(prefix_without_digest: &[u8]) -> [u8; 32] {
    hash_framed(FRAME_DIGEST_DOMAIN, prefix_without_digest)
}

pub(crate) fn candidate_digest(bytes: &[u8]) -> [u8; 32] {
    hash_framed(CANDIDATE_DIGEST_DOMAIN, bytes)
}

fn ledger_before_digest(
    instance_binding: [u8; 32],
    call_contract: [u8; 32],
    invocation: NonZeroU64,
    generation: NonZeroU64,
    entries: &[LedgerEntry],
) -> Result<[u8; 32], WireError> {
    let mut hasher = Sha256::new();
    hasher.update(LEDGER_BEFORE_DOMAIN);
    hasher.update(instance_binding);
    hasher.update(call_contract);
    hasher.update(invocation.get().to_le_bytes());
    hasher.update(generation.get().to_le_bytes());
    hasher.update(
        u32::try_from(entries.len())
            .map_err(|_| WireError::CapacityMismatch)?
            .to_le_bytes(),
    );
    for (index, entry) in entries.iter().enumerate() {
        if entry.owner_ordinal as usize != index
            || entry.slot == 0
            || entry.generation == 0
            || entry.state != LedgerState::InInvocation
        {
            return Err(WireError::NonCanonical);
        }
        hasher.update(entry.owner_ordinal.to_le_bytes());
        hasher.update(entry.slot.to_le_bytes());
        hasher.update(entry.generation.to_le_bytes());
        hasher.update(1_u32.to_le_bytes());
    }
    Ok(hasher.finalize().into())
}

fn ledger_after_digest(
    before: [u8; 32],
    candidate: [u8; 32],
    entries: &[LedgerEntry],
) -> Result<[u8; 32], WireError> {
    let mut hasher = Sha256::new();
    hasher.update(LEDGER_AFTER_DOMAIN);
    hasher.update(before);
    hasher.update(candidate);
    hasher.update(
        u32::try_from(entries.len())
            .map_err(|_| WireError::CapacityMismatch)?
            .to_le_bytes(),
    );
    let mut published = 0;
    for (index, entry) in entries.iter().enumerate() {
        if entry.owner_ordinal as usize != index || entry.slot == 0 {
            return Err(WireError::NonCanonical);
        }
        let state = match entry.state {
            LedgerState::Retired if entry.generation == 0 => 1_u32,
            LedgerState::Published if entry.generation != 0 => {
                published += 1;
                2_u32
            }
            _ => return Err(WireError::NonCanonical),
        };
        hasher.update(entry.owner_ordinal.to_le_bytes());
        hasher.update(entry.slot.to_le_bytes());
        hasher.update(entry.generation.to_le_bytes());
        hasher.update(state.to_le_bytes());
    }
    if published > 1 {
        return Err(WireError::NonCanonical);
    }
    Ok(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn ledger_transition_digests(
    instance_binding: [u8; 32],
    call_contract: [u8; 32],
    invocation: NonZeroU64,
    generation: NonZeroU64,
    candidate: [u8; 32],
    before_entries: &[LedgerEntry],
    after_entries: &[LedgerEntry],
    published_owner: Option<u32>,
) -> Result<([u8; 32], [u8; 32]), WireError> {
    if instance_binding == [0; 32]
        || call_contract == [0; 32]
        || candidate == [0; 32]
        || before_entries.len() != after_entries.len()
    {
        return Err(WireError::NonCanonical);
    }
    for (before, after) in before_entries.iter().zip(after_entries) {
        if before.owner_ordinal != after.owner_ordinal || before.slot != after.slot {
            return Err(WireError::CrossBinding);
        }
        match (published_owner == Some(before.owner_ordinal), after.state) {
            (false, LedgerState::Retired) if after.generation == 0 => {}
            (true, LedgerState::Published)
                if before.generation.checked_add(1) == Some(after.generation) => {}
            _ => return Err(WireError::ReplayMismatch),
        }
    }
    if published_owner.is_some_and(|owner| owner as usize >= before_entries.len()) {
        return Err(WireError::ReplayMismatch);
    }
    let before = ledger_before_digest(
        instance_binding,
        call_contract,
        invocation,
        generation,
        before_entries,
    )?;
    let after = ledger_after_digest(before, candidate, after_entries)?;
    Ok((before, after))
}

fn semantic_trace_digest(
    descriptor: &Descriptor,
    response: &ExecuteResponse,
) -> Result<[u8; 32], WireError> {
    let outcome = match response.outcome {
        ExecuteOutcome::Scalar { .. } => TraceOutcome::ScalarSuccess,
        ExecuteOutcome::Owned { .. } => TraceOutcome::OwnedSuccess,
        ExecuteOutcome::SemanticFailure { selected_ordinal } => {
            if !response.event_ordinals.contains(&selected_ordinal) {
                return Err(WireError::ReplayMismatch);
            }
            TraceOutcome::Failure { selected_ordinal }
        }
    };
    let mut hasher = Sha256::new();
    hasher.update(TRACE_EVIDENCE_DOMAIN);
    hasher.update(descriptor.fingerprints.trace_path_certificate);
    hasher.update((response.event_ordinals.len() as u64).to_le_bytes());
    for ordinal in &response.event_ordinals {
        hasher.update(ordinal.to_le_bytes());
    }
    match outcome {
        TraceOutcome::ScalarSuccess => hasher.update([1]),
        TraceOutcome::OwnedSuccess => hasher.update([2]),
        TraceOutcome::Failure { selected_ordinal } => {
            hasher.update([3]);
            hasher.update(selected_ordinal.to_le_bytes());
        }
    }
    let digest: [u8; 32] = hasher.finalize().into();
    let checkpoint = descriptor
        .graph
        .checkpoints
        .get(
            response
                .checkpoint
                .checked_sub(1)
                .ok_or(WireError::ReplayMismatch)? as usize,
        )
        .ok_or(WireError::ReplayMismatch)?;
    let expected_outcome = match response.outcome {
        ExecuteOutcome::Scalar { .. } => GraphOutcome::ScalarSuccess,
        ExecuteOutcome::SemanticFailure { .. } => GraphOutcome::SemanticFailure,
        ExecuteOutcome::Owned { owner_ordinal, .. } => GraphOutcome::OwnedSuccess(owner_ordinal),
    };
    if checkpoint.outcome != Some(expected_outcome) {
        return Err(WireError::ReplayMismatch);
    }
    let mut exact_incoming = descriptor.graph.edges.iter().filter(|edge| {
        edge.to == checkpoint.id
            && matches!(
                &edge.action,
                GraphAction::CertifyOutcome(evidence)
                    if evidence.digest == digest
                        && evidence.ordinals == response.event_ordinals
                        && evidence.outcome == outcome
            )
    });
    if exact_incoming.next().is_none() || exact_incoming.next().is_some() {
        return Err(WireError::ReplayMismatch);
    }
    Ok(digest)
}

fn validate_frame_state(frame: &RecoveryFrame) -> Result<(), WireError> {
    if frame.request_digest == [0; 32] {
        return Err(WireError::NonCanonical);
    }
    let finalizing = frame
        .cells
        .iter()
        .filter(|cell| cell.state == CellState::Finalizing)
        .count();
    match frame.execute_return {
        ExecuteReturn::Pending
            if frame.response_storage_digest != [0; 32]
                || frame.semantic_trace_digest != [0; 32] =>
        {
            return Err(WireError::NonCanonical)
        }
        ExecuteReturn::Returned(_) if frame.response_storage_digest == [0; 32] => {
            return Err(WireError::NonCanonical)
        }
        ExecuteReturn::Pending | ExecuteReturn::Returned(_) => {}
    }
    match frame.phase {
        FramePhase::Executing => {
            if frame.decision_digest != [0; 32]
                || frame.action_chain_digest != [0; 32]
                || frame.next_action != 0
                || frame.record_count != 0
                || frame.active_finalizers > 1
                || finalizing != frame.active_finalizers as usize
            {
                return Err(WireError::NonCanonical);
            }
        }
        FramePhase::DecisionLocked => {
            if frame.decision_digest == [0; 32]
                || frame.action_chain_digest == [0; 32]
                || frame.next_action != 0
                || frame.record_count != 0
                || frame.active_finalizers != 0
                || finalizing != 0
            {
                return Err(WireError::NonCanonical);
            }
        }
        FramePhase::ActionInProgress => {
            if frame.decision_digest == [0; 32]
                || frame.action_chain_digest == [0; 32]
                || frame.active_finalizers > 1
                || finalizing != frame.active_finalizers as usize
            {
                return Err(WireError::NonCanonical);
            }
        }
        FramePhase::ProviderSettled => {
            if frame.decision_digest == [0; 32]
                || frame.action_chain_digest == [0; 32]
                || frame.active_finalizers != 0
                || finalizing != 0
                || frame
                    .cells
                    .iter()
                    .any(|cell| !matches!(cell.state, CellState::Dead | CellState::Published))
            {
                return Err(WireError::NonCanonical);
            }
        }
        FramePhase::ReceiptCommitted | FramePhase::Quarantined => {
            return Err(WireError::NonCanonical)
        }
    }
    Ok(())
}

fn verify_receipt_mac(key: &ReceiptMacKey, body: &[u8], tag: &[u8; 32]) -> Result<(), WireError> {
    let mut mac =
        HmacSha256::new_from_slice(&key.0).map_err(|_| WireError::AuthenticationFailed)?;
    mac.update(RECEIPT_MAC_DOMAIN);
    mac.update(&(body.len() as u64).to_be_bytes());
    mac.update(body);
    mac.verify_slice(tag)
        .map_err(|_| WireError::AuthenticationFailed)
}

fn receipt_mac(key: &ReceiptMacKey, body: &[u8]) -> Result<[u8; 32], WireError> {
    let mut mac =
        HmacSha256::new_from_slice(&key.0).map_err(|_| WireError::AuthenticationFailed)?;
    mac.update(RECEIPT_MAC_DOMAIN);
    mac.update(&(body.len() as u64).to_be_bytes());
    mac.update(body);
    Ok(mac.finalize().into_bytes().into())
}

fn validate_call_identity(
    identity: CallIdentity,
    descriptor: &Descriptor,
) -> Result<(), WireError> {
    if identity.call_contract != descriptor.fingerprints.call_contract
        || identity.provider_challenge == [0; 32]
    {
        return Err(WireError::CrossBinding);
    }
    Ok(())
}

fn validate_recovery_identity(
    identity: RecoveryIdentity,
    descriptor: &Descriptor,
) -> Result<(), WireError> {
    validate_call_identity(identity.call, descriptor)?;
    if identity.recovery_contract != descriptor.fingerprints.recovery_contract
        || identity.settlement_graph != descriptor.fingerprints.settlement_graph
    {
        return Err(WireError::CrossBinding);
    }
    Ok(())
}

fn recovery_identity_from_call(call: CallIdentity, descriptor: &Descriptor) -> RecoveryIdentity {
    RecoveryIdentity {
        call,
        recovery_contract: descriptor.fingerprints.recovery_contract,
        settlement_graph: descriptor.fingerprints.settlement_graph,
    }
}

fn graph_state_to_cell(state: GraphResourceState) -> Result<CellState, WireError> {
    match state {
        GraphResourceState::Live => Ok(CellState::Live),
        GraphResourceState::ProvisionalResult => Ok(CellState::ProvisionalResult),
        GraphResourceState::Dead => Ok(CellState::Dead),
        GraphResourceState::Finalizing | GraphResourceState::Published => {
            Err(WireError::ReplayMismatch)
        }
    }
}

fn parse_cell_state(tag: u32) -> Result<CellState, WireError> {
    match tag {
        1 => Ok(CellState::Live),
        2 => Ok(CellState::ProvisionalResult),
        3 => Ok(CellState::Finalizing),
        4 => Ok(CellState::Dead),
        5 => Ok(CellState::Published),
        _ => Err(WireError::NonCanonical),
    }
}

const fn cell_state_tag(state: CellState) -> u32 {
    match state {
        CellState::Live => 1,
        CellState::ProvisionalResult => 2,
        CellState::Finalizing => 3,
        CellState::Dead => 4,
        CellState::Published => 5,
    }
}

fn require_exact(actual: &[u8], expected: &[u8]) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::NonCanonical)
    }
}

fn declared_total(bytes: &[u8], magic: &[u8; 8]) -> Result<usize, WireError> {
    if bytes.len() < HEADER_SIZE as usize
        || bytes.get(..8) != Some(magic.as_slice())
        || u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| WireError::Malformed)?) != VERSION
        || u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| WireError::Malformed)?)
            != HEADER_SIZE
    {
        return Err(WireError::UnsupportedSchema);
    }
    let total = usize::try_from(u32::from_le_bytes(
        bytes[16..20].try_into().map_err(|_| WireError::Malformed)?,
    ))
    .map_err(|_| WireError::Malformed)?;
    if total < HEADER_SIZE as usize || total > bytes.len() {
        return Err(WireError::Malformed);
    }
    Ok(total)
}

fn hash_framed(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hash_field(&mut hasher, bytes);
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn envelope(bytes: &'a [u8], magic: &[u8; 8]) -> Result<Self, WireError> {
        if bytes.len() < HEADER_SIZE as usize {
            return Err(WireError::Malformed);
        }
        let mut reader = Self { bytes, cursor: 0 };
        if reader.take(8)? != magic
            || reader.u32()? != VERSION
            || reader.u32()? != HEADER_SIZE
            || reader.usize()? != bytes.len()
        {
            return Err(WireError::UnsupportedSchema);
        }
        Ok(reader)
    }

    fn call_identity(&mut self) -> Result<CallIdentity, WireError> {
        Ok(CallIdentity {
            call_contract: self.digest()?,
            invocation: NonZeroU64::new(self.u64()?).ok_or(WireError::NonCanonical)?,
            frame_generation: NonZeroU64::new(self.u64()?).ok_or(WireError::NonCanonical)?,
            provider_challenge: nonzero_digest(self.digest()?)?,
        })
    }

    fn recovery_identity(&mut self) -> Result<RecoveryIdentity, WireError> {
        let call_contract = self.digest()?;
        let recovery_contract = nonzero_digest(self.digest()?)?;
        let settlement_graph = nonzero_digest(self.digest()?)?;
        let invocation = NonZeroU64::new(self.u64()?).ok_or(WireError::NonCanonical)?;
        let frame_generation = NonZeroU64::new(self.u64()?).ok_or(WireError::NonCanonical)?;
        let provider_challenge = nonzero_digest(self.digest()?)?;
        Ok(RecoveryIdentity {
            call: CallIdentity {
                call_contract,
                invocation,
                frame_generation,
                provider_challenge,
            },
            recovery_contract,
            settlement_graph,
        })
    }

    fn digest(&mut self) -> Result<[u8; 32], WireError> {
        self.take(32)?.try_into().map_err(|_| WireError::Malformed)
    }

    fn i64(&mut self) -> Result<i64, WireError> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| WireError::Malformed)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| WireError::Malformed)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| WireError::Malformed)?,
        ))
    }

    fn usize(&mut self) -> Result<usize, WireError> {
        usize::try_from(self.u32()?).map_err(|_| WireError::Malformed)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WireError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(WireError::Malformed)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(WireError::Malformed)?;
        self.cursor = end;
        Ok(value)
    }

    fn finish(self) -> Result<(), WireError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(WireError::Malformed)
        }
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new(magic: &[u8; 8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(magic);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&HEADER_SIZE.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        Self { bytes }
    }

    fn call_identity(&mut self, identity: CallIdentity) {
        self.digest(identity.call_contract);
        self.u64(identity.invocation.get());
        self.u64(identity.frame_generation.get());
        self.digest(identity.provider_challenge);
    }

    fn recovery_identity(&mut self, identity: RecoveryIdentity) {
        self.digest(identity.call.call_contract);
        self.digest(identity.recovery_contract);
        self.digest(identity.settlement_graph);
        self.u64(identity.call.invocation.get());
        self.u64(identity.call.frame_generation.get());
        self.digest(identity.call.provider_challenge);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.bytes.extend_from_slice(&value);
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn finish(mut self) -> Vec<u8> {
        let length = u32::try_from(self.bytes.len()).expect("v3 wire is bounded below u32::MAX");
        self.bytes[16..20].copy_from_slice(&length.to_le_bytes());
        self.bytes
    }
}

fn nonzero_digest(value: [u8; 32]) -> Result<[u8; 32], WireError> {
    if value == [0; 32] {
        Err(WireError::NonCanonical)
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use semaprax::codegen::emit_native_callable_v3_descriptor;
    use semaprax::hir::DeclarationId;
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::*;
    use crate::descriptor_v3::{Action as GraphAction, Descriptor};

    struct Fixture {
        descriptor: Descriptor,
        request: ExecuteRequest,
        response_storage: Vec<u8>,
        response: ExecuteResponse,
        frame: RecoveryFrame,
        decision: SettlementDecision,
        actions: Vec<ActionRecord>,
        candidate: CandidateReceipt,
        candidate_bytes: Vec<u8>,
        receipt_bytes: Vec<u8>,
        key: ReceiptMacKey,
        instance: [u8; 32],
        ledger_before: [u8; 32],
        ledger_after: [u8; 32],
    }

    fn fixture() -> Fixture {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let artifact = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
        )
        .unwrap();
        let descriptor = Descriptor::parse(artifact.bytes()).unwrap();
        let call = CallIdentity {
            call_contract: descriptor.fingerprints.call_contract,
            invocation: NonZeroU64::new(0x0102_0304_0506_0708).unwrap(),
            frame_generation: NonZeroU64::new(0x1112_1314_1516_1718).unwrap(),
            provider_challenge: [0x44; 32],
        };
        let identity = recovery_identity_from_call(call, &descriptor);
        let request = ExecuteRequest {
            identity: call,
            arguments: vec![
                RequestArgument::Owned {
                    index: 0,
                    owner_ordinal: 0,
                    payload: 0,
                },
                RequestArgument::Owned {
                    index: 1,
                    owner_ordinal: 1,
                    payload: u64::MAX,
                },
            ],
        };
        let request_hash = request_digest(&request.encode());

        let scalar_checkpoint = descriptor
            .graph
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.outcome == Some(GraphOutcome::ScalarSuccess))
            .unwrap();
        let evidence = descriptor
            .graph
            .edges
            .iter()
            .find_map(|edge| match &edge.action {
                GraphAction::CertifyOutcome(evidence)
                    if edge.to == scalar_checkpoint.id
                        && evidence.outcome == TraceOutcome::ScalarSuccess =>
                {
                    Some(evidence)
                }
                _ => None,
            })
            .unwrap();
        let response = ExecuteResponse {
            identity: call,
            request_digest: request_hash,
            checkpoint: scalar_checkpoint.id,
            outcome: ExecuteOutcome::Scalar { value: 17 },
            event_ordinals: evidence.ordinals.clone(),
            storage_capacity: descriptor.capacities.execute_response as usize,
        };
        let response_storage = response.encode();
        assert_eq!(
            response_storage.len(),
            descriptor.capacities.execute_response as usize
        );
        let response_hash = response_storage_digest(9, &vec![0; response_storage.len()]);

        let decision = SettlementDecision {
            identity,
            decision: Decision::AbortHostUnwind,
        };
        let decision_hash = decision_digest(&decision.encode());
        let checkpoint = descriptor.graph.checkpoints.first().unwrap();
        assert_eq!(checkpoint.id, 1);
        let payloads = [0, u64::MAX];
        let mut actions = Vec::new();
        let mut final_cells = checkpoint
            .resources
            .iter()
            .enumerate()
            .map(|(owner, state)| ResourceCell {
                state: graph_state_to_cell(*state).unwrap(),
                payload: payloads[owner],
            })
            .collect::<Vec<_>>();
        for (semantic_index, owner) in checkpoint.abort_order.iter().copied().enumerate() {
            let cell = final_cells[owner as usize];
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index as u32,
                boundary: ActionBoundary::Started,
                owner_ordinal: owner,
                payload: cell.payload,
                before: cell.state,
                after: CellState::Finalizing,
                checkpoint: 1,
            });
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index as u32,
                boundary: ActionBoundary::Completed,
                owner_ordinal: owner,
                payload: cell.payload,
                before: CellState::Finalizing,
                after: CellState::Dead,
                checkpoint: 1,
            });
            final_cells[owner as usize].state = CellState::Dead;
        }
        let action_hash =
            action_chain_digest(decision_hash, checkpoint.abort_order.len(), &actions).unwrap();
        let mut frame = RecoveryFrame {
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [0; 32],
            execute_return: ExecuteReturn::Returned(9),
            checkpoint: 1,
            phase: FramePhase::ProviderSettled,
            decision_digest: decision_hash,
            next_action: checkpoint.abort_order.len() as u32,
            record_count: actions.len() as u32,
            active_finalizers: 0,
            cells: final_cells,
            action_chain_digest: action_hash,
            pre_candidate_digest: [0; 32],
        };
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        let frame_bytes = frame.encode();
        let frame = RecoveryFrame::parse(&frame_bytes, &descriptor).unwrap();
        let candidate = CandidateReceipt {
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [0; 32],
            frame_digest: frame.pre_candidate_digest,
            decision_digest: decision_hash,
            action_evidence_digest: action_hash,
            outcome: CandidateOutcome::Abort,
            active_finalizers: 0,
            dispositions: frame
                .cells
                .iter()
                .map(|cell| DispositionCell {
                    disposition: Disposition::Dead,
                    payload: cell.payload,
                })
                .collect(),
        };
        let candidate_bytes = candidate.encode();
        let candidate = CandidateReceipt::parse(&candidate_bytes, &descriptor).unwrap();
        validate_candidate_replay(
            &descriptor,
            &request,
            9,
            &vec![0; response_storage.len()],
            None,
            &frame,
            &decision,
            &actions,
            &candidate,
        )
        .unwrap();

        let candidate_hash = candidate_digest(&candidate_bytes);
        let before_entries = [
            LedgerEntry {
                owner_ordinal: 0,
                slot: 11,
                generation: 3,
                state: LedgerState::InInvocation,
            },
            LedgerEntry {
                owner_ordinal: 1,
                slot: 12,
                generation: 8,
                state: LedgerState::InInvocation,
            },
        ];
        let after_entries = [
            LedgerEntry {
                owner_ordinal: 0,
                slot: 11,
                generation: 0,
                state: LedgerState::Retired,
            },
            LedgerEntry {
                owner_ordinal: 1,
                slot: 12,
                generation: 0,
                state: LedgerState::Retired,
            },
        ];
        let instance = [0x77; 32];
        let (ledger_before, ledger_after) = ledger_transition_digests(
            instance,
            call.call_contract,
            call.invocation,
            call.frame_generation,
            candidate_hash,
            &before_entries,
            &after_entries,
            None,
        )
        .unwrap();
        let key = ReceiptMacKey::from_runtime_bytes([0x55; 32]).unwrap();
        let mut receipt = HostCommittedReceipt {
            instance_binding: instance,
            identity,
            request_digest: candidate.request_digest,
            response_storage_digest: candidate.response_storage_digest,
            semantic_trace_digest: candidate.semantic_trace_digest,
            frame_digest: candidate.frame_digest,
            decision_digest: candidate.decision_digest,
            action_evidence_digest: candidate.action_evidence_digest,
            candidate_digest: candidate_hash,
            ledger_before_digest: ledger_before,
            ledger_after_digest: ledger_after,
            publication: Publication::NoOwned,
            tag: [0; 32],
        };
        let unsigned = receipt.encode();
        receipt.tag = receipt_mac(&key, &unsigned[..HOST_RECEIPT_BODY_BYTES]).unwrap();
        let receipt_bytes = receipt.encode();
        assert_eq!(receipt_bytes.len(), HOST_RECEIPT_BYTES);

        Fixture {
            descriptor,
            request,
            response_storage,
            response,
            frame,
            decision,
            actions,
            candidate,
            candidate_bytes,
            receipt_bytes,
            key,
            instance,
            ledger_before,
            ledger_after,
        }
    }

    #[test]
    fn all_seven_wires_parse_reencode_and_replay_independently() {
        let fixture = fixture();
        let request_bytes = fixture.request.encode();
        assert_eq!(
            ExecuteRequest::parse(&request_bytes, &fixture.descriptor)
                .unwrap()
                .encode(),
            request_bytes
        );
        assert_eq!(
            ExecuteResponse::parse(&fixture.response_storage, &fixture.descriptor)
                .unwrap()
                .encode(),
            fixture.response_storage
        );
        let decision_bytes = fixture.decision.encode();
        assert_eq!(
            SettlementDecision::parse(&decision_bytes, &fixture.descriptor)
                .unwrap()
                .encode(),
            decision_bytes
        );
        for action in &fixture.actions {
            let bytes = action.encode();
            assert_eq!(
                ActionRecord::parse(&bytes, &fixture.descriptor)
                    .unwrap()
                    .encode(),
                bytes
            );
        }
        let frame_bytes = fixture.frame.encode();
        assert_eq!(
            RecoveryFrame::parse(&frame_bytes, &fixture.descriptor)
                .unwrap()
                .encode(),
            frame_bytes
        );
        assert_eq!(
            CandidateReceipt::parse(&fixture.candidate_bytes, &fixture.descriptor)
                .unwrap()
                .encode(),
            fixture.candidate_bytes
        );
        let receipt = HostCommittedReceipt::parse_and_verify(
            &fixture.receipt_bytes,
            &fixture.key,
            &fixture.descriptor,
            fixture.instance,
            &fixture.candidate,
            fixture.ledger_before,
            fixture.ledger_after,
        )
        .unwrap();
        assert_eq!(receipt.encode(), fixture.receipt_bytes);
    }

    #[test]
    fn host_receipt_rejects_every_prefix_trailing_byte_and_single_bit_mutation() {
        let fixture = fixture();
        let verify = |bytes: &[u8]| {
            HostCommittedReceipt::parse_and_verify(
                bytes,
                &fixture.key,
                &fixture.descriptor,
                fixture.instance,
                &fixture.candidate,
                fixture.ledger_before,
                fixture.ledger_after,
            )
        };
        for length in 0..fixture.receipt_bytes.len() {
            assert!(verify(&fixture.receipt_bytes[..length]).is_err());
        }
        let mut trailing = fixture.receipt_bytes.clone();
        trailing.push(0);
        assert!(verify(&trailing).is_err());
        for offset in 0..fixture.receipt_bytes.len() {
            for bit in 0..8 {
                let mut hostile = fixture.receipt_bytes.clone();
                hostile[offset] ^= 1 << bit;
                assert!(
                    verify(&hostile).is_err(),
                    "accepted byte {offset} bit {bit}"
                );
            }
        }
    }

    #[test]
    fn cross_binding_tail_action_and_key_conflicts_fail_closed() {
        let fixture = fixture();
        let wrong_key = ReceiptMacKey::from_runtime_bytes([0x56; 32]).unwrap();
        assert_eq!(
            HostCommittedReceipt::parse_and_verify(
                &fixture.receipt_bytes,
                &wrong_key,
                &fixture.descriptor,
                fixture.instance,
                &fixture.candidate,
                fixture.ledger_before,
                fixture.ledger_after,
            ),
            Err(WireError::AuthenticationFailed)
        );

        let mut hostile_response = fixture.response_storage.clone();
        *hostile_response.last_mut().unwrap() = 1;
        assert!(ExecuteResponse::parse(&hostile_response, &fixture.descriptor).is_err());

        let mut hostile_actions = fixture.actions.clone();
        hostile_actions[1].semantic_action_index += 1;
        assert!(validate_candidate_replay(
            &fixture.descriptor,
            &fixture.request,
            9,
            &vec![0; fixture.response_storage.len()],
            None,
            &fixture.frame,
            &fixture.decision,
            &hostile_actions,
            &fixture.candidate,
        )
        .is_err());

        let mut hostile_request = fixture.request.clone();
        let RequestArgument::Owned { payload, .. } = &mut hostile_request.arguments[0] else {
            unreachable!()
        };
        *payload ^= 1;
        assert!(validate_candidate_replay(
            &fixture.descriptor,
            &hostile_request,
            9,
            &vec![0; fixture.response_storage.len()],
            None,
            &fixture.frame,
            &fixture.decision,
            &fixture.actions,
            &fixture.candidate,
        )
        .is_err());
    }

    #[test]
    fn all_provider_wire_prefixes_trailing_bytes_and_single_byte_mutations_fail_closed() {
        let fixture = fixture();
        let request_bytes = fixture.request.encode();
        assert_boundaries(&request_bytes, |bytes| {
            ExecuteRequest::parse(bytes, &fixture.descriptor).is_ok()
        });
        for offset in 0..request_bytes.len() {
            let mut hostile = request_bytes.clone();
            hostile[offset] ^= 1;
            if let Ok(request) = ExecuteRequest::parse(&hostile, &fixture.descriptor) {
                assert!(validate_candidate_replay(
                    &fixture.descriptor,
                    &request,
                    9,
                    &vec![0; fixture.response_storage.len()],
                    None,
                    &fixture.frame,
                    &fixture.decision,
                    &fixture.actions,
                    &fixture.candidate,
                )
                .is_err());
            }
        }

        assert_boundaries(&fixture.response_storage, |bytes| {
            ExecuteResponse::parse(bytes, &fixture.descriptor).is_ok()
        });
        let response_digest = response_storage_digest(0, &fixture.response_storage);
        for offset in 0..fixture.response_storage.len() {
            let mut hostile = fixture.response_storage.clone();
            hostile[offset] ^= 1;
            if let Ok(response) = ExecuteResponse::parse(&hostile, &fixture.descriptor) {
                assert_ne!(response, fixture.response);
                assert_eq!(response.encode(), hostile);
                assert_ne!(response_storage_digest(0, &hostile), response_digest);
            }
        }

        let decision_bytes = fixture.decision.encode();
        assert_boundaries(&decision_bytes, |bytes| {
            SettlementDecision::parse(bytes, &fixture.descriptor).is_ok()
        });
        for offset in 0..decision_bytes.len() {
            let mut hostile = decision_bytes.clone();
            hostile[offset] ^= 1;
            if let Ok(decision) = SettlementDecision::parse(&hostile, &fixture.descriptor) {
                assert!(validate_candidate_replay(
                    &fixture.descriptor,
                    &fixture.request,
                    9,
                    &vec![0; fixture.response_storage.len()],
                    None,
                    &fixture.frame,
                    &decision,
                    &fixture.actions,
                    &fixture.candidate,
                )
                .is_err());
            }
        }

        for action_index in 0..fixture.actions.len() {
            let canonical = fixture.actions[action_index].encode();
            assert_boundaries(&canonical, |bytes| {
                ActionRecord::parse(bytes, &fixture.descriptor).is_ok()
            });
            for offset in 0..canonical.len() {
                let mut hostile = canonical.clone();
                hostile[offset] ^= 1;
                if let Ok(action) = ActionRecord::parse(&hostile, &fixture.descriptor) {
                    let mut actions = fixture.actions.clone();
                    actions[action_index] = action;
                    assert!(validate_candidate_replay(
                        &fixture.descriptor,
                        &fixture.request,
                        9,
                        &vec![0; fixture.response_storage.len()],
                        None,
                        &fixture.frame,
                        &fixture.decision,
                        &actions,
                        &fixture.candidate,
                    )
                    .is_err());
                }
            }
        }

        let frame_bytes = fixture.frame.encode();
        assert_boundaries(&frame_bytes, |bytes| {
            RecoveryFrame::parse(bytes, &fixture.descriptor).is_ok()
        });
        for offset in 0..frame_bytes.len() {
            let mut hostile = frame_bytes.clone();
            hostile[offset] ^= 1;
            assert!(RecoveryFrame::parse(&hostile, &fixture.descriptor).is_err());
        }

        assert_boundaries(&fixture.candidate_bytes, |bytes| {
            CandidateReceipt::parse(bytes, &fixture.descriptor).is_ok()
        });
        for offset in 0..fixture.candidate_bytes.len() {
            let mut hostile = fixture.candidate_bytes.clone();
            hostile[offset] ^= 1;
            if let Ok(candidate) = CandidateReceipt::parse(&hostile, &fixture.descriptor) {
                assert!(validate_candidate_replay(
                    &fixture.descriptor,
                    &fixture.request,
                    9,
                    &vec![0; fixture.response_storage.len()],
                    None,
                    &fixture.frame,
                    &fixture.decision,
                    &fixture.actions,
                    &candidate,
                )
                .is_err());
            }
        }
    }

    #[test]
    fn accepted_response_mutations_resealed_trace_payload_and_physical_code_fail_closed() {
        let fixture = fixture();
        let (storage, frame, decision, actions, candidate) = build_accepted(
            &fixture.descriptor,
            &fixture.request,
            &fixture.response,
            Decision::AcceptScalar,
        );
        validate_candidate_replay(
            &fixture.descriptor,
            &fixture.request,
            0,
            &storage,
            Some(&fixture.response),
            &frame,
            &decision,
            &actions,
            &candidate,
        )
        .unwrap();
        for offset in 0..storage.len() {
            let mut hostile = storage.clone();
            hostile[offset] ^= 1;
            if let Ok(response) = ExecuteResponse::parse(&hostile, &fixture.descriptor) {
                assert!(validate_candidate_replay(
                    &fixture.descriptor,
                    &fixture.request,
                    0,
                    &hostile,
                    Some(&response),
                    &frame,
                    &decision,
                    &actions,
                    &candidate,
                )
                .is_err());
            }
        }

        let mut wrong_trace = fixture.response.clone();
        let ordinal = wrong_trace.event_ordinals.first_mut().unwrap();
        *ordinal = if *ordinal == 1 { 2 } else { 1 };
        let (storage, frame, decision, actions, candidate) = build_accepted(
            &fixture.descriptor,
            &fixture.request,
            &wrong_trace,
            Decision::AcceptScalar,
        );
        assert!(validate_candidate_replay(
            &fixture.descriptor,
            &fixture.request,
            0,
            &storage,
            Some(&wrong_trace),
            &frame,
            &decision,
            &actions,
            &candidate,
        )
        .is_err());

        let mut physical_decision = fixture.decision;
        physical_decision.decision = Decision::AbortPhysical(8);
        let physical_digest = decision_digest(&physical_decision.encode());
        let physical_action = action_chain_digest(
            physical_digest,
            fixture.frame.next_action as usize,
            &fixture.actions,
        )
        .unwrap();
        let mut physical_frame = fixture.frame.clone();
        physical_frame.decision_digest = physical_digest;
        physical_frame.action_chain_digest = physical_action;
        physical_frame.pre_candidate_digest = [0; 32];
        let provisional = physical_frame.encode();
        physical_frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        let mut physical_candidate = fixture.candidate.clone();
        physical_candidate.decision_digest = physical_digest;
        physical_candidate.action_evidence_digest = physical_action;
        physical_candidate.frame_digest = physical_frame.pre_candidate_digest;
        assert!(validate_candidate_replay(
            &fixture.descriptor,
            &fixture.request,
            9,
            &vec![0; fixture.response_storage.len()],
            None,
            &physical_frame,
            &physical_decision,
            &fixture.actions,
            &physical_candidate,
        )
        .is_err());

        let corpus = build_owned_resource_corpus_v1().unwrap();
        let artifact = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.identity"),
        )
        .unwrap();
        let owned_descriptor = Descriptor::parse(artifact.bytes()).unwrap();
        let call = CallIdentity {
            call_contract: owned_descriptor.fingerprints.call_contract,
            invocation: NonZeroU64::new(21).unwrap(),
            frame_generation: NonZeroU64::new(22).unwrap(),
            provider_challenge: [23; 32],
        };
        let owned_request = ExecuteRequest {
            identity: call,
            arguments: vec![RequestArgument::Owned {
                index: 0,
                owner_ordinal: 0,
                payload: 42,
            }],
        };
        let checkpoint = owned_descriptor
            .graph
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.outcome == Some(GraphOutcome::OwnedSuccess(0)))
            .unwrap();
        let evidence = owned_descriptor
            .graph
            .edges
            .iter()
            .find_map(|edge| match &edge.action {
                GraphAction::CertifyOutcome(evidence) if edge.to == checkpoint.id => Some(evidence),
                _ => None,
            })
            .unwrap();
        let wrong_payload = ExecuteResponse {
            identity: call,
            request_digest: request_digest(&owned_request.encode()),
            checkpoint: checkpoint.id,
            outcome: ExecuteOutcome::Owned {
                owner_ordinal: 0,
                payload: 43,
            },
            event_ordinals: evidence.ordinals.clone(),
            storage_capacity: owned_descriptor.capacities.execute_response as usize,
        };
        let (storage, frame, decision, actions, candidate) = build_accepted(
            &owned_descriptor,
            &owned_request,
            &wrong_payload,
            Decision::AcceptOwned(0),
        );
        assert!(validate_candidate_replay(
            &owned_descriptor,
            &owned_request,
            0,
            &storage,
            Some(&wrong_payload),
            &frame,
            &decision,
            &actions,
            &candidate,
        )
        .is_err());
    }

    #[test]
    fn pre_settle_gate_rejects_resealed_wrong_witness_checkpoint_and_cell() {
        let fixture = fixture();
        let (storage, frame) =
            build_executed(&fixture.descriptor, &fixture.request, &fixture.response);
        validate_successful_execute_evidence(
            &fixture.descriptor,
            &fixture.request,
            0,
            &storage,
            &fixture.response,
            &frame,
        )
        .unwrap();

        let mut wrong_witness = fixture.response.clone();
        let ordinal = wrong_witness.event_ordinals.first_mut().unwrap();
        *ordinal = if *ordinal < fixture.descriptor.capacities.dictionary_entries {
            *ordinal + 1
        } else {
            *ordinal - 1
        };
        let wrong_witness_storage = wrong_witness.encode();
        let wrong_witness =
            ExecuteResponse::parse(&wrong_witness_storage, &fixture.descriptor).unwrap();
        let (_, wrong_witness_frame) =
            build_executed(&fixture.descriptor, &fixture.request, &wrong_witness);
        assert!(validate_successful_execute_evidence(
            &fixture.descriptor,
            &fixture.request,
            0,
            &wrong_witness_storage,
            &wrong_witness,
            &wrong_witness_frame,
        )
        .is_err());

        let mut wrong_checkpoint = fixture.response.clone();
        wrong_checkpoint.checkpoint = fixture
            .descriptor
            .graph
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.id != wrong_checkpoint.checkpoint)
            .unwrap()
            .id;
        let wrong_checkpoint_storage = wrong_checkpoint.encode();
        let wrong_checkpoint =
            ExecuteResponse::parse(&wrong_checkpoint_storage, &fixture.descriptor).unwrap();
        let (_, wrong_checkpoint_frame) =
            build_executed(&fixture.descriptor, &fixture.request, &wrong_checkpoint);
        assert!(validate_successful_execute_evidence(
            &fixture.descriptor,
            &fixture.request,
            0,
            &wrong_checkpoint_storage,
            &wrong_checkpoint,
            &wrong_checkpoint_frame,
        )
        .is_err());

        let mut wrong_cell = frame;
        wrong_cell.cells[0].payload ^= 1;
        reseal_frame(&mut wrong_cell);
        let wrong_cell_bytes = wrong_cell.encode();
        let wrong_cell = RecoveryFrame::parse(&wrong_cell_bytes, &fixture.descriptor).unwrap();
        assert!(validate_successful_execute_evidence(
            &fixture.descriptor,
            &fixture.request,
            0,
            &storage,
            &fixture.response,
            &wrong_cell,
        )
        .is_err());
    }

    #[test]
    fn independent_host_encoders_match_all_six_compiler_known_answers() {
        let call = CallIdentity {
            call_contract: [1; 32],
            invocation: NonZeroU64::new(0x0102_0304_0506_0708).unwrap(),
            frame_generation: NonZeroU64::new(0x1112_1314_1516_1718).unwrap(),
            provider_challenge: [4; 32],
        };
        let identity = RecoveryIdentity {
            call,
            recovery_contract: [2; 32],
            settlement_graph: [3; 32],
        };
        let request = ExecuteRequest {
            identity: call,
            arguments: vec![
                RequestArgument::I64 {
                    index: 0,
                    value: i64::MIN,
                },
                RequestArgument::Bool {
                    index: 1,
                    value: true,
                },
                RequestArgument::Owned {
                    index: 2,
                    owner_ordinal: 0,
                    payload: 0,
                },
                RequestArgument::Owned {
                    index: 3,
                    owner_ordinal: 1,
                    payload: u64::MAX,
                },
            ],
        };
        let request_hash = request_digest(&request.encode());
        assert_eq!(
            hex(&request_hash),
            "699565f407451aab7dbddf5a4788e99d6439f1c67ac045df721f805fec1ba135"
        );
        let response = ExecuteResponse {
            identity: call,
            request_digest: request_hash,
            checkpoint: 7,
            outcome: ExecuteOutcome::Owned {
                owner_ordinal: 1,
                payload: u64::MAX,
            },
            event_ordinals: vec![1, 7, 9],
            storage_capacity: 172,
        };
        let response_hash = response_storage_digest(0, &response.encode());
        assert_eq!(
            hex(&response_hash),
            "72bf589efdd016f29616f9a1448d563cff9ae0fd9fe231cc7706d867edba9fb7"
        );
        let decision = SettlementDecision {
            identity,
            decision: Decision::AcceptOwned(1),
        };
        let decision_hash = decision_digest(&decision.encode());
        assert_eq!(
            hex(&decision_hash),
            "cc7d67f6cacd3e1c80844e8c2f48c39e4833c892102a3b6ed02194e3eebe9e1f"
        );
        let action = ActionRecord {
            identity,
            semantic_action_index: 0,
            boundary: ActionBoundary::Publish,
            owner_ordinal: 1,
            payload: u64::MAX,
            before: CellState::ProvisionalResult,
            after: CellState::Published,
            checkpoint: 7,
        };
        let action_hash = action_chain_digest(decision_hash, 2, &[action]).unwrap();
        assert_eq!(
            hex(&action_hash),
            "606d8324103845e16657699ee014e3841adea671643c1522b1e64b2f75a58388"
        );
        let mut frame = RecoveryFrame {
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [5; 32],
            execute_return: ExecuteReturn::Returned(0),
            checkpoint: 7,
            phase: FramePhase::ProviderSettled,
            decision_digest: decision_hash,
            next_action: 2,
            record_count: 1,
            active_finalizers: 0,
            cells: vec![
                ResourceCell {
                    state: CellState::Dead,
                    payload: 0,
                },
                ResourceCell {
                    state: CellState::Published,
                    payload: u64::MAX,
                },
            ],
            action_chain_digest: action_hash,
            pre_candidate_digest: [0; 32],
        };
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        assert_eq!(
            hex(&frame.pre_candidate_digest),
            "b0013da9bf07b2be4dbcea3103360acae2b487ec7ce28fb264aaa5ac3fbad111"
        );
        let candidate = CandidateReceipt {
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [5; 32],
            frame_digest: frame.pre_candidate_digest,
            decision_digest: decision_hash,
            action_evidence_digest: action_hash,
            outcome: CandidateOutcome::Owned(1),
            active_finalizers: 0,
            dispositions: vec![
                DispositionCell {
                    disposition: Disposition::Dead,
                    payload: 0,
                },
                DispositionCell {
                    disposition: Disposition::Published,
                    payload: u64::MAX,
                },
            ],
        };
        let candidate_hash = candidate_digest(&candidate.encode());
        assert_eq!(
            hex(&candidate_hash),
            "4e82547b169ccd07d0c90e7ff3051265067b8a2de9312b53fcc8d96a8fa9b3bb"
        );

        let key = ReceiptMacKey::from_runtime_bytes([9; 32]).unwrap();
        let mut receipt = HostCommittedReceipt {
            instance_binding: [6; 32],
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [5; 32],
            frame_digest: frame.pre_candidate_digest,
            decision_digest: decision_hash,
            action_evidence_digest: action_hash,
            candidate_digest: candidate_hash,
            ledger_before_digest: [7; 32],
            ledger_after_digest: [8; 32],
            publication: Publication::Owned(1),
            tag: [0; 32],
        };
        let unsigned = receipt.encode();
        receipt.tag = receipt_mac(&key, &unsigned[..HOST_RECEIPT_BODY_BYTES]).unwrap();
        assert_eq!(
            hex(&Sha256::digest(receipt.encode())),
            "c3425663c2bf483492e4fc388ceebd9b89e7339a21a916f1ea6803447c0da5a4"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut value = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(value, "{byte:02x}").unwrap();
        }
        value
    }

    fn build_accepted(
        descriptor: &Descriptor,
        request: &ExecuteRequest,
        response: &ExecuteResponse,
        decision_value: Decision,
    ) -> (
        Vec<u8>,
        RecoveryFrame,
        SettlementDecision,
        Vec<ActionRecord>,
        CandidateReceipt,
    ) {
        let identity = recovery_identity_from_call(request.identity, descriptor);
        let decision = SettlementDecision {
            identity,
            decision: decision_value,
        };
        let decision_hash = decision_digest(&decision.encode());
        let checkpoint = &descriptor.graph.checkpoints[(response.checkpoint - 1) as usize];
        let payloads = request
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                RequestArgument::Owned {
                    owner_ordinal,
                    payload,
                    ..
                } => Some((*owner_ordinal, *payload)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut cells = checkpoint
            .resources
            .iter()
            .enumerate()
            .map(|(owner, state)| ResourceCell {
                state: graph_state_to_cell(*state).unwrap(),
                payload: payloads[&(owner as u32)],
            })
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        let mut semantic_index = 0_u32;
        for owner in &checkpoint.accept_order {
            let cell = cells[*owner as usize];
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index,
                boundary: ActionBoundary::Started,
                owner_ordinal: *owner,
                payload: cell.payload,
                before: cell.state,
                after: CellState::Finalizing,
                checkpoint: checkpoint.id,
            });
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index,
                boundary: ActionBoundary::Completed,
                owner_ordinal: *owner,
                payload: cell.payload,
                before: CellState::Finalizing,
                after: CellState::Dead,
                checkpoint: checkpoint.id,
            });
            cells[*owner as usize].state = CellState::Dead;
            semantic_index += 1;
        }
        if let Decision::AcceptOwned(owner) = decision_value {
            let cell = cells[owner as usize];
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index,
                boundary: ActionBoundary::Publish,
                owner_ordinal: owner,
                payload: cell.payload,
                before: CellState::ProvisionalResult,
                after: CellState::Published,
                checkpoint: checkpoint.id,
            });
            cells[owner as usize].state = CellState::Published;
            semantic_index += 1;
        }
        let action_hash =
            action_chain_digest(decision_hash, semantic_index as usize, &actions).unwrap();
        let storage = response.encode();
        let semantic = raw_trace_digest_for_test(descriptor, response);
        let mut frame = RecoveryFrame {
            identity,
            request_digest: request_digest(&request.encode()),
            response_storage_digest: response_storage_digest(0, &storage),
            semantic_trace_digest: semantic,
            execute_return: ExecuteReturn::Returned(0),
            checkpoint: checkpoint.id,
            phase: FramePhase::ProviderSettled,
            decision_digest: decision_hash,
            next_action: semantic_index,
            record_count: actions.len() as u32,
            active_finalizers: 0,
            cells,
            action_chain_digest: action_hash,
            pre_candidate_digest: [0; 32],
        };
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        let candidate = CandidateReceipt {
            identity,
            request_digest: frame.request_digest,
            response_storage_digest: frame.response_storage_digest,
            semantic_trace_digest: semantic,
            frame_digest: frame.pre_candidate_digest,
            decision_digest: decision_hash,
            action_evidence_digest: action_hash,
            outcome: match decision_value {
                Decision::AcceptScalar => CandidateOutcome::Scalar,
                Decision::AcceptSemanticFailure => CandidateOutcome::Failure,
                Decision::AcceptOwned(owner) => CandidateOutcome::Owned(owner),
                _ => unreachable!(),
            },
            active_finalizers: 0,
            dispositions: frame
                .cells
                .iter()
                .map(|cell| DispositionCell {
                    disposition: match cell.state {
                        CellState::Dead => Disposition::Dead,
                        CellState::Published => Disposition::Published,
                        _ => unreachable!(),
                    },
                    payload: cell.payload,
                })
                .collect(),
        };
        (storage, frame, decision, actions, candidate)
    }

    fn build_executed(
        descriptor: &Descriptor,
        request: &ExecuteRequest,
        response: &ExecuteResponse,
    ) -> (Vec<u8>, RecoveryFrame) {
        let identity = recovery_identity_from_call(request.identity, descriptor);
        let storage = response.encode();
        let checkpoint = &descriptor.graph.checkpoints[(response.checkpoint - 1) as usize];
        let payloads = request
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                RequestArgument::Owned {
                    owner_ordinal,
                    payload,
                    ..
                } => Some((*owner_ordinal, *payload)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let cells = checkpoint
            .resources
            .iter()
            .enumerate()
            .map(|(owner, state)| ResourceCell {
                state: graph_state_to_cell(*state).unwrap(),
                payload: payloads[&(owner as u32)],
            })
            .collect();
        let mut frame = RecoveryFrame {
            identity,
            request_digest: request_digest(&request.encode()),
            response_storage_digest: response_storage_digest(0, &storage),
            semantic_trace_digest: raw_trace_digest_for_test(descriptor, response),
            execute_return: ExecuteReturn::Returned(0),
            checkpoint: response.checkpoint,
            phase: FramePhase::Executing,
            decision_digest: [0; 32],
            next_action: 0,
            record_count: 0,
            active_finalizers: 0,
            cells,
            action_chain_digest: [0; 32],
            pre_candidate_digest: [0; 32],
        };
        reseal_frame(&mut frame);
        (storage, frame)
    }

    fn reseal_frame(frame: &mut RecoveryFrame) {
        frame.pre_candidate_digest = [0; 32];
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
    }

    fn raw_trace_digest_for_test(descriptor: &Descriptor, response: &ExecuteResponse) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(TRACE_EVIDENCE_DOMAIN);
        hasher.update(descriptor.fingerprints.trace_path_certificate);
        hasher.update((response.event_ordinals.len() as u64).to_le_bytes());
        for ordinal in &response.event_ordinals {
            hasher.update(ordinal.to_le_bytes());
        }
        match response.outcome {
            ExecuteOutcome::Scalar { .. } => hasher.update([1]),
            ExecuteOutcome::Owned { .. } => hasher.update([2]),
            ExecuteOutcome::SemanticFailure { selected_ordinal } => {
                hasher.update([3]);
                hasher.update(selected_ordinal.to_le_bytes());
            }
        }
        hasher.finalize().into()
    }

    fn assert_boundaries(bytes: &[u8], accepts: impl Fn(&[u8]) -> bool) {
        for length in 0..bytes.len() {
            assert!(
                !accepts(&bytes[..length]),
                "accepted prefix length {length}"
            );
        }
        for trailing in [0, 1, 0x7f, 0xff] {
            let mut hostile = bytes.to_vec();
            hostile.push(trailing);
            assert!(!accepts(&hostile), "accepted trailing byte {trailing}");
        }
    }
}
