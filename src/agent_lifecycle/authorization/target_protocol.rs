//! Target-neutral, authority-free Agent host-call protocol (#182).
//!
//! This module is deliberately below `authorization`: construction takes the
//! real, opaque [`super::Authorized`] value, so source code, a decoded
//! proposal, and a C/Wasm adapter cannot forge a turn grant.  The iterative
//! typed-effect adapter transfers the freshly minted `Authorized` into
//! [`TargetGrant::bind`] and routes its injected handler through [`dispatch`];
//! it never deserializes a grant or calls the handler directly.

use std::panic::{catch_unwind, AssertUnwindSafe};

use sha2::{Digest, Sha256};

use super::{Authorized, AuthorizedRequest};
pub(crate) mod owned_wait_v8;
use crate::agent_runtime::AgentCancellation;

/// Closed, length-framed schema for an admitted target result carrier.
pub const CARRIER_SCHEMA: &str = "semaprax.agent-target-carrier.v1";
/// Closed, length-framed schema for an authority-free target request. Version
/// two adds the authorization-binding commitment used by independent replay.
pub const REQUEST_SCHEMA: &str = "semaprax.agent-target-host-request.v2";
/// Schema for an authority-free target execution observation.
pub const EVIDENCE_SCHEMA: &str = "semaprax.agent-target-host-evidence.v1";

const GRANT_DOMAIN: &[u8] = b"semaprax.agent-target-host.grant.v1\0";
const ARGUMENT_DOMAIN: &[u8] = b"semaprax.agent-target-host.argument.v1\0";
const REQUEST_DOMAIN: &[u8] = b"semaprax.agent-target-host.request.v2\0";
const RESULT_DOMAIN: &[u8] = b"semaprax.agent-target-host.result.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.agent-target-host.evidence.v1\0";
const MAX_IDENTIFIER_BYTES: usize = 240;
const MAX_CARRIER_BYTES: usize = 65_536;
const SHA256_DIGEST_BYTES: usize = 71;
const MAX_REQUEST_BYTES: usize = (10 * std::mem::size_of::<u64>())
    + REQUEST_SCHEMA.len()
    + (2 * SHA256_DIGEST_BYTES)
    + (4 * MAX_IDENTIFIER_BYTES)
    + (2 * std::mem::size_of::<u64>())
    + MAX_CARRIER_BYTES;

/// Exact source-owned operation facts.  A grant is bound to both identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetOperation {
    operation_id: String,
    effect_id: String,
    argument_type: String,
    result_type: String,
}

impl TargetOperation {
    pub fn new(
        operation_id: impl Into<String>,
        effect_id: impl Into<String>,
        argument_type: impl Into<String>,
        result_type: impl Into<String>,
    ) -> Result<Self, ProtocolError> {
        let value = Self {
            operation_id: operation_id.into(),
            effect_id: effect_id.into(),
            argument_type: argument_type.into(),
            result_type: result_type.into(),
        };
        for identifier in [
            &value.operation_id,
            &value.effect_id,
            &value.argument_type,
            &value.result_type,
        ] {
            validate_identifier(identifier)?;
        }
        Ok(value)
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    pub fn argument_type(&self) -> &str {
        &self.argument_type
    }

    pub fn result_type(&self) -> &str {
        &self.result_type
    }

    fn canonical(&self, out: &mut Vec<u8>) {
        frame(out, self.operation_id.as_bytes());
        frame(out, self.effect_id.as_bytes());
        frame(out, self.argument_type.as_bytes());
        frame(out, self.result_type.as_bytes());
    }
}

/// A bounded typed carrier.  Its wire is length framed, never a host-selected
/// pointer or a source scalar standing in for nominal type identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedCarrier {
    type_id: String,
    payload: Vec<u8>,
}

impl TypedCarrier {
    pub fn new(type_id: impl Into<String>, payload: Vec<u8>) -> Result<Self, ProtocolError> {
        let value = Self {
            type_id: type_id.into(),
            payload,
        };
        validate_identifier(&value.type_id)?;
        let encoded_len = 3usize
            .checked_mul(std::mem::size_of::<u64>())
            .and_then(|length| length.checked_add(CARRIER_SCHEMA.len()))
            .and_then(|length| length.checked_add(value.type_id.len()))
            .and_then(|length| length.checked_add(value.payload.len()))
            .ok_or(ProtocolError::CarrierTooLarge)?;
        if encoded_len > MAX_CARRIER_BYTES {
            return Err(ProtocolError::CarrierTooLarge);
        }
        Ok(value)
    }

    pub fn type_id(&self) -> &str {
        &self.type_id
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity(CARRIER_SCHEMA.len() + self.type_id.len() + self.payload.len() + 16);
        frame(&mut bytes, CARRIER_SCHEMA.as_bytes());
        frame(&mut bytes, self.type_id.as_bytes());
        frame(&mut bytes, &self.payload);
        bytes
    }

    /// Independently validates the entire carrier before a result is exposed.
    pub fn decode(bytes: &[u8], expected_type: &str) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_CARRIER_BYTES {
            return Err(ProtocolError::CarrierTooLarge);
        }
        let mut cursor = 0;
        let schema = take_frame(bytes, &mut cursor)?;
        let type_id = take_frame(bytes, &mut cursor)?;
        let payload = take_frame(bytes, &mut cursor)?;
        if cursor != bytes.len() || schema != CARRIER_SCHEMA.as_bytes() {
            return Err(ProtocolError::MalformedCarrier);
        }
        let type_id = std::str::from_utf8(type_id).map_err(|_| ProtocolError::MalformedCarrier)?;
        validate_identifier(type_id)?;
        if type_id != expected_type {
            return Err(ProtocolError::ResultTypeMismatch);
        }
        let value = Self::new(type_id, payload.to_vec())?;
        if value.encode() != bytes {
            return Err(ProtocolError::MalformedCarrier);
        }
        Ok(value)
    }
}

/// Per-invocation ceilings.  These are checked and reserved before dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetLimits {
    pub max_calls: u64,
    pub max_request_bytes: u64,
    pub max_result_bytes: u64,
    pub max_total_bytes: u64,
    pub max_fuel: u64,
}

/// Cumulative accounting held by the lifecycle driver, never by a target
/// adapter.  A dispatched request remains charged after host/result failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TargetAccounting {
    calls: u64,
    request_bytes: u64,
    result_bytes: u64,
    fuel: u64,
}

impl TargetAccounting {
    pub fn calls(&self) -> u64 {
        self.calls
    }
    pub fn request_bytes(&self) -> u64 {
        self.request_bytes
    }
    pub fn result_bytes(&self) -> u64 {
        self.result_bytes
    }
    pub fn fuel(&self) -> u64 {
        self.fuel
    }

    fn reserve(
        &mut self,
        request_bytes: u64,
        fuel: u64,
        limits: TargetLimits,
    ) -> Result<(), Settlement> {
        let calls = self.calls.checked_add(1).ok_or(Settlement::CallBudget)?;
        let bytes = self
            .request_bytes
            .checked_add(request_bytes)
            .ok_or(Settlement::RequestBudget)?;
        let total_fuel = self
            .fuel
            .checked_add(fuel)
            .ok_or(Settlement::FuelExhausted)?;
        if calls > limits.max_calls {
            return Err(Settlement::CallBudget);
        }
        if bytes > limits.max_request_bytes {
            return Err(Settlement::RequestBudget);
        }
        if bytes
            .checked_add(self.result_bytes)
            .is_none_or(|total| total > limits.max_total_bytes)
        {
            return Err(Settlement::RequestBudget);
        }
        if total_fuel > limits.max_fuel {
            return Err(Settlement::FuelExhausted);
        }
        self.calls = calls;
        self.request_bytes = bytes;
        self.fuel = total_fuel;
        Ok(())
    }

    fn charge_result(&mut self, raw_bytes: usize, limits: TargetLimits) -> Result<u64, Settlement> {
        let charged = u64::try_from(raw_bytes)
            .unwrap_or(u64::MAX)
            .min(limits.max_result_bytes.saturating_add(1));
        self.result_bytes = self.result_bytes.saturating_add(charged);
        if raw_bytes > usize::try_from(limits.max_result_bytes).unwrap_or(usize::MAX)
            || self.result_bytes > limits.max_result_bytes
            || self
                .request_bytes
                .checked_add(self.result_bytes)
                .is_none_or(|total| total > limits.max_total_bytes)
        {
            Err(Settlement::ResultBudget)
        } else {
            Ok(charged)
        }
    }
}

/// A target-private, move-only grant.  It cannot be constructed from a
/// binding string, copied into a checkpoint, or reused after `dispatch`.
pub struct TargetGrant {
    grant_id: String,
    authorization_binding: String,
    operation: TargetOperation,
    argument_digest: String,
    turn: u64,
    granted_budget: i64,
}

impl TargetGrant {
    /// Consumes the only real lifecycle grant and binds it to one operation
    /// and invocation turn.  The source-level seal never leaves this module.
    pub(in crate::agent_lifecycle) fn bind(
        authorization: Authorized,
        invocation_root: &str,
        execution_binding: Option<&str>,
        turn: u64,
        operation: TargetOperation,
        argument: &TypedCarrier,
    ) -> Result<Self, ProtocolError> {
        validate_digest(invocation_root)?;
        if let Some(binding) = execution_binding {
            validate_digest(binding)?;
        }
        let request = authorization.consume();
        Ok(Self::bind_request(
            request,
            invocation_root,
            execution_binding,
            turn,
            operation,
            argument,
        ))
    }

    fn bind_request(
        authorization: AuthorizedRequest,
        invocation_root: &str,
        execution_binding: Option<&str>,
        turn: u64,
        operation: TargetOperation,
        argument: &TypedCarrier,
    ) -> Self {
        let argument_digest = digest(ARGUMENT_DOMAIN, &argument.encode());
        let grant_id = owned_wait_v8::grant_id(
            authorization.binding(),
            authorization.seal(),
            invocation_root,
            execution_binding,
            turn,
            &operation,
            &argument_digest,
        );
        Self {
            grant_id,
            authorization_binding: authorization.binding().to_owned(),
            operation,
            argument_digest,
            turn,
            granted_budget: authorization.budget(),
        }
    }
}

/// Exact request passed to a target adapter. Its authorization-binding digest
/// anchors retained evidence to the same spent lifecycle authorization, but
/// carries no ambient handle, source pointer, seal, capability, or mutable
/// accounting authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetHostRequest {
    grant_id: String,
    authorization_binding: String,
    operation: TargetOperation,
    turn: u64,
    argument: TypedCarrier,
    fuel: u64,
}

impl TargetHostRequest {
    pub fn grant_id(&self) -> &str {
        &self.grant_id
    }
    pub fn operation(&self) -> &TargetOperation {
        &self.operation
    }
    pub fn turn(&self) -> u64 {
        self.turn
    }
    pub fn argument(&self) -> &TypedCarrier {
        &self.argument
    }
    pub fn fuel(&self) -> u64 {
        self.fuel
    }

    /// Canonical authority-free request bytes.  These bytes are diagnostic or
    /// replay input only: this type has no public constructor and no decode
    /// route, so they cannot be turned into a handler dispatch capability.
    pub fn canonical_wire(&self) -> Vec<u8> {
        let argument = self.argument.encode();
        let mut bytes = Vec::with_capacity(argument.len() + 512);
        frame(&mut bytes, REQUEST_SCHEMA.as_bytes());
        frame(&mut bytes, self.grant_id.as_bytes());
        frame(&mut bytes, self.authorization_binding.as_bytes());
        self.operation.canonical(&mut bytes);
        frame(&mut bytes, &self.turn.to_be_bytes());
        frame(&mut bytes, &self.fuel.to_be_bytes());
        frame(&mut bytes, &argument);
        bytes
    }

    fn decode_for_replay(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(ProtocolError::MalformedRequest);
        }
        let mut cursor = 0;
        let schema = take_frame_request(bytes, &mut cursor)?;
        let grant_id = take_frame_request(bytes, &mut cursor)?;
        let authorization_binding = take_frame_request(bytes, &mut cursor)?;
        let operation_id = take_frame_request(bytes, &mut cursor)?;
        let effect_id = take_frame_request(bytes, &mut cursor)?;
        let argument_type = take_frame_request(bytes, &mut cursor)?;
        let result_type = take_frame_request(bytes, &mut cursor)?;
        let turn = take_u64_request(bytes, &mut cursor)?;
        let fuel = take_u64_request(bytes, &mut cursor)?;
        let argument = take_frame_request(bytes, &mut cursor)?;
        if cursor != bytes.len() || schema != REQUEST_SCHEMA.as_bytes() {
            return Err(ProtocolError::MalformedRequest);
        }
        let grant_id =
            std::str::from_utf8(grant_id).map_err(|_| ProtocolError::MalformedRequest)?;
        validate_digest(grant_id).map_err(|_| ProtocolError::MalformedRequest)?;
        let authorization_binding = std::str::from_utf8(authorization_binding)
            .map_err(|_| ProtocolError::MalformedRequest)?;
        validate_digest(authorization_binding).map_err(|_| ProtocolError::MalformedRequest)?;
        let operation = TargetOperation::new(
            decode_identifier(operation_id, ProtocolError::MalformedRequest)?,
            decode_identifier(effect_id, ProtocolError::MalformedRequest)?,
            decode_identifier(argument_type, ProtocolError::MalformedRequest)?,
            decode_identifier(result_type, ProtocolError::MalformedRequest)?,
        )
        .map_err(|_| ProtocolError::MalformedRequest)?;
        let argument = TypedCarrier::decode(argument, operation.argument_type())
            .map_err(|_| ProtocolError::MalformedRequest)?;
        let request = Self {
            grant_id: grant_id.to_owned(),
            authorization_binding: authorization_binding.to_owned(),
            operation,
            turn,
            argument,
            fuel,
        };
        if request.canonical_wire() != bytes {
            return Err(ProtocolError::MalformedRequest);
        }
        Ok(request)
    }
}

/// The only target-specific capability.  Implementations receive a closed
/// request and may write untrusted carrier bytes only through the bounded
/// response sink. The host can allocate its own memory, but it cannot force
/// the protocol boundary to allocate or hash beyond the declared ceiling.
pub trait TargetHostHandler {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        response: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError>;
}

/// Protocol-owned bounded response buffer. Once any write crosses the ceiling,
/// the sink stays overflowed and retains no bytes beyond that ceiling.
pub struct TargetResponseSink {
    bytes: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl TargetResponseSink {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(4096)),
            limit,
            overflowed: false,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), TargetResponseOverflow> {
        if self.overflowed
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|length| length > self.limit)
        {
            self.overflowed = true;
            return Err(TargetResponseOverflow);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetResponseOverflow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetHostError {
    Unavailable,
    Failed,
}

/// Stable terminal meaning shared by the retained, C11, and Wasm adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Settlement {
    Returned,
    Cancelled,
    /// The target call had already crossed the host boundary when cancellation
    /// was observed. The call remains charged, but its result is not published.
    CancelledAfterDispatch,
    GrantBudget,
    CallBudget,
    RequestBudget,
    FuelExhausted,
    ResultBudget,
    HostFailed,
    HostPanicked,
    ArgumentTypeMismatch,
    ArgumentBindingMismatch,
    MalformedResult,
    ResultTypeMismatch,
}

impl Settlement {
    pub(in crate::agent_lifecycle) fn text(self) -> &'static str {
        match self {
            Self::Returned => "returned",
            Self::Cancelled => "cancelled",
            Self::CancelledAfterDispatch => "cancelled_after_dispatch",
            Self::GrantBudget => "grant_budget",
            Self::CallBudget => "call_budget",
            Self::RequestBudget => "request_budget",
            Self::FuelExhausted => "fuel_exhausted",
            Self::ResultBudget => "result_budget",
            Self::HostFailed => "host_failed",
            Self::HostPanicked => "host_panicked",
            Self::ArgumentTypeMismatch => "argument_type_mismatch",
            Self::ArgumentBindingMismatch => "argument_binding_mismatch",
            Self::MalformedResult => "malformed_result",
            Self::ResultTypeMismatch => "result_type_mismatch",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "returned" => Self::Returned,
            "cancelled" => Self::Cancelled,
            "cancelled_after_dispatch" => Self::CancelledAfterDispatch,
            "grant_budget" => Self::GrantBudget,
            "call_budget" => Self::CallBudget,
            "request_budget" => Self::RequestBudget,
            "fuel_exhausted" => Self::FuelExhausted,
            "result_budget" => Self::ResultBudget,
            "host_failed" => Self::HostFailed,
            "host_panicked" => Self::HostPanicked,
            "argument_type_mismatch" => Self::ArgumentTypeMismatch,
            "argument_binding_mismatch" => Self::ArgumentBindingMismatch,
            "malformed_result" => Self::MalformedResult,
            "result_type_mismatch" => Self::ResultTypeMismatch,
            _ => return None,
        })
    }
}

/// The settled turn result.  A failure never carries a typed result.
#[derive(Debug, Eq, PartialEq)]
pub struct TargetDispatch {
    result: Option<TypedCarrier>,
    evidence: TargetEvidence,
}

impl TargetDispatch {
    pub fn result(&self) -> Option<&TypedCarrier> {
        self.result.as_ref()
    }
    pub fn evidence(&self) -> &TargetEvidence {
        &self.evidence
    }
    pub(in crate::agent_lifecycle) fn authorization_binding(&self) -> &str {
        &self.evidence.authorization_binding
    }
}

/// Common, replayable semantic evidence; it is observational and grants no
/// handler, target execution, checkpoint, or publication authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetEvidence {
    grant_id: String,
    authorization_binding: String,
    operation: TargetOperation,
    turn: u64,
    request_digest: String,
    result_digest: Option<String>,
    accounting: TargetAccounting,
    dispatched: bool,
    settlement: Settlement,
    digest: String,
}

impl TargetEvidence {
    pub fn settlement(&self) -> Settlement {
        self.settlement
    }
    pub fn dispatched(&self) -> bool {
        self.dispatched
    }
    pub fn accounting(&self) -> TargetAccounting {
        self.accounting
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Canonical, bounded observation bytes for independent no-dispatch
    /// replay.  The opaque grant identity remains data: decoding this wire
    /// never creates a [`TargetGrant`] or an adapter capability.
    pub fn canonical_wire(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        frame(&mut bytes, EVIDENCE_SCHEMA.as_bytes());
        frame(&mut bytes, self.grant_id.as_bytes());
        frame(&mut bytes, self.authorization_binding.as_bytes());
        self.operation.canonical(&mut bytes);
        frame(&mut bytes, &self.turn.to_be_bytes());
        frame(&mut bytes, self.request_digest.as_bytes());
        frame(
            &mut bytes,
            self.result_digest.as_deref().unwrap_or("").as_bytes(),
        );
        frame(&mut bytes, &self.accounting.calls.to_be_bytes());
        frame(&mut bytes, &self.accounting.request_bytes.to_be_bytes());
        frame(&mut bytes, &self.accounting.result_bytes.to_be_bytes());
        frame(&mut bytes, &self.accounting.fuel.to_be_bytes());
        frame(&mut bytes, &[u8::from(self.dispatched)]);
        frame(&mut bytes, self.settlement.text().as_bytes());
        bytes
    }

    /// Decodes an exact canonical target observation.  The reconstructed value
    /// is descriptive only and must still be paired with an exact request for
    /// replay; it has no route to host dispatch.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(ProtocolError::MalformedEvidence);
        }
        let mut cursor = 0;
        let schema = take_frame_evidence(bytes, &mut cursor)?;
        let grant_id = take_frame_evidence(bytes, &mut cursor)?;
        let authorization_binding = take_frame_evidence(bytes, &mut cursor)?;
        let operation_id = take_frame_evidence(bytes, &mut cursor)?;
        let effect_id = take_frame_evidence(bytes, &mut cursor)?;
        let argument_type = take_frame_evidence(bytes, &mut cursor)?;
        let result_type = take_frame_evidence(bytes, &mut cursor)?;
        let turn = take_u64_evidence(bytes, &mut cursor)?;
        let request_digest = take_frame_evidence(bytes, &mut cursor)?;
        let result_digest = take_frame_evidence(bytes, &mut cursor)?;
        let calls = take_u64_evidence(bytes, &mut cursor)?;
        let request_bytes = take_u64_evidence(bytes, &mut cursor)?;
        let result_bytes = take_u64_evidence(bytes, &mut cursor)?;
        let fuel = take_u64_evidence(bytes, &mut cursor)?;
        let dispatched = take_frame_evidence(bytes, &mut cursor)?;
        let settlement = take_frame_evidence(bytes, &mut cursor)?;
        if cursor != bytes.len() || schema != EVIDENCE_SCHEMA.as_bytes() || dispatched.len() != 1 {
            return Err(ProtocolError::MalformedEvidence);
        }
        let grant_id = decode_digest(grant_id, ProtocolError::MalformedEvidence)?;
        let authorization_binding =
            decode_digest(authorization_binding, ProtocolError::MalformedEvidence)?;
        let request_digest = decode_digest(request_digest, ProtocolError::MalformedEvidence)?;
        let result_digest = if result_digest.is_empty() {
            None
        } else {
            Some(decode_digest(
                result_digest,
                ProtocolError::MalformedEvidence,
            )?)
        };
        let operation = TargetOperation::new(
            decode_identifier(operation_id, ProtocolError::MalformedEvidence)?,
            decode_identifier(effect_id, ProtocolError::MalformedEvidence)?,
            decode_identifier(argument_type, ProtocolError::MalformedEvidence)?,
            decode_identifier(result_type, ProtocolError::MalformedEvidence)?,
        )
        .map_err(|_| ProtocolError::MalformedEvidence)?;
        let dispatched = match dispatched[0] {
            0 => false,
            1 => true,
            _ => return Err(ProtocolError::MalformedEvidence),
        };
        let settlement = Settlement::parse(
            std::str::from_utf8(settlement).map_err(|_| ProtocolError::MalformedEvidence)?,
        )
        .ok_or(ProtocolError::MalformedEvidence)?;
        let mut evidence = Self {
            grant_id,
            authorization_binding,
            operation,
            turn,
            request_digest,
            result_digest,
            accounting: TargetAccounting {
                calls,
                request_bytes,
                result_bytes,
                fuel,
            },
            dispatched,
            settlement,
            digest: String::new(),
        };
        evidence
            .validate_observation()
            .map_err(|_| ProtocolError::MalformedEvidence)?;
        evidence.digest = evidence.compute_digest();
        if evidence.canonical_wire() != bytes {
            return Err(ProtocolError::MalformedEvidence);
        }
        Ok(evidence)
    }

    /// Parses an independently retained request wire and verifies the same
    /// observation without invoking a host handler.
    pub fn replay_wire(&self, request_wire: &[u8]) -> Result<(), ProtocolError> {
        let request = TargetHostRequest::decode_for_replay(request_wire)?;
        self.replay(&request)
    }

    /// Independently replay both sides of one observed host exchange without
    /// acquiring a grant or invoking a handler.
    ///
    /// `replay_wire` verifies only the host-visible request because some
    /// callers retain request and result bytes separately. This stronger form
    /// also requires the exact result bytes whenever the observation carries
    /// a result commitment, rejects unexpected bytes for a no-result
    /// settlement, and rechecks the settlement-specific carrier shape.
    pub fn replay_exchange_wire(
        &self,
        request_wire: &[u8],
        result_wire: Option<&[u8]>,
    ) -> Result<(), ProtocolError> {
        self.replay_wire(request_wire)?;
        match (self.result_digest.as_deref(), result_wire) {
            (None, None) => return Ok(()),
            (Some(expected), Some(result))
                if result.len() <= MAX_CARRIER_BYTES
                    && digest(RESULT_DOMAIN, result) == expected => {}
            _ => return Err(ProtocolError::ReplayMismatch),
        }

        let result = result_wire.expect("matched committed result bytes");
        let decoded = TypedCarrier::decode(result, self.operation.result_type());
        let shape_matches = match self.settlement {
            Settlement::Returned => decoded.is_ok(),
            Settlement::ResultTypeMismatch => decoded == Err(ProtocolError::ResultTypeMismatch),
            Settlement::MalformedResult => {
                matches!(
                    decoded,
                    Err(ProtocolError::MalformedCarrier)
                        | Err(ProtocolError::CarrierTooLarge)
                        | Err(ProtocolError::InvalidIdentifier)
                )
            }
            Settlement::HostFailed | Settlement::HostPanicked => true,
            _ => false,
        };
        if shape_matches {
            Ok(())
        } else {
            Err(ProtocolError::ReplayMismatch)
        }
    }

    /// Independent no-dispatch replay.  It rederives the request commitment
    /// from exact host-visible data and verifies the sealed observation.
    pub fn replay(&self, request: &TargetHostRequest) -> Result<(), ProtocolError> {
        if request.grant_id != self.grant_id
            || request.authorization_binding != self.authorization_binding
            || request.operation != self.operation
            || request.turn != self.turn
        {
            return Err(ProtocolError::ReplayMismatch);
        }
        if digest(REQUEST_DOMAIN, &request.canonical_wire()) != self.request_digest
            || self.digest != self.compute_digest()
        {
            return Err(ProtocolError::ReplayMismatch);
        }
        self.validate_observation()
            .map_err(|_| ProtocolError::ReplayMismatch)?;
        Ok(())
    }

    fn compute_digest(&self) -> String {
        digest(EVIDENCE_DOMAIN, &self.canonical_wire())
    }

    fn validate_observation(&self) -> Result<(), ProtocolError> {
        let pre_dispatch = matches!(
            self.settlement,
            Settlement::Cancelled
                | Settlement::GrantBudget
                | Settlement::CallBudget
                | Settlement::RequestBudget
                | Settlement::FuelExhausted
                | Settlement::ArgumentTypeMismatch
                | Settlement::ArgumentBindingMismatch
        );
        let result_required = matches!(
            self.settlement,
            Settlement::Returned | Settlement::MalformedResult | Settlement::ResultTypeMismatch
        );
        let result_forbidden = pre_dispatch
            || matches!(
                self.settlement,
                Settlement::ResultBudget | Settlement::CancelledAfterDispatch
            );
        if pre_dispatch != !self.dispatched
            || (result_forbidden && self.result_digest.is_some())
            || (result_required && self.result_digest.is_none())
        {
            return Err(ProtocolError::ReplayMismatch);
        }
        Ok(())
    }
}

/// Runs pre-dispatch checks, reserves accounting, performs exactly one injected
/// target call, and normalizes every post-dispatch failure to sticky settlement.
pub(in crate::agent_lifecycle) fn dispatch(
    grant: TargetGrant,
    argument: TypedCarrier,
    fuel: u64,
    limits: TargetLimits,
    accounting: &mut TargetAccounting,
    cancellation: &AgentCancellation,
    handler: &mut dyn TargetHostHandler,
) -> TargetDispatch {
    let request = TargetHostRequest {
        grant_id: grant.grant_id.clone(),
        authorization_binding: grant.authorization_binding.clone(),
        operation: grant.operation.clone(),
        turn: grant.turn,
        argument,
        fuel,
    };
    let request_digest = digest(REQUEST_DOMAIN, &request.canonical_wire());
    let baseline = *accounting;
    if cancellation.is_cancelled() {
        return settled(
            grant,
            request_digest,
            baseline,
            false,
            Settlement::Cancelled,
            None,
            None,
        );
    }
    if request.argument.type_id != request.operation.argument_type {
        return settled(
            grant,
            request_digest,
            baseline,
            false,
            Settlement::ArgumentTypeMismatch,
            None,
            None,
        );
    }
    if digest(ARGUMENT_DOMAIN, &request.argument.encode()) != grant.argument_digest {
        return settled(
            grant,
            request_digest,
            baseline,
            false,
            Settlement::ArgumentBindingMismatch,
            None,
            None,
        );
    }
    if grant.granted_budget < 0 || u64::try_from(grant.granted_budget).unwrap_or(0) < fuel {
        return settled(
            grant,
            request_digest,
            baseline,
            false,
            Settlement::GrantBudget,
            None,
            None,
        );
    }
    if let Err(settlement) = accounting.reserve(request.canonical_wire().len() as u64, fuel, limits)
    {
        return settled(
            grant,
            request_digest,
            baseline,
            false,
            settlement,
            None,
            None,
        );
    }
    let remaining_total = limits
        .max_total_bytes
        .saturating_sub(accounting.request_bytes);
    let response_limit = limits
        .max_result_bytes
        .min(remaining_total)
        .min(MAX_CARRIER_BYTES as u64);
    let response_limit = usize::try_from(response_limit).unwrap_or(MAX_CARRIER_BYTES);
    let mut response = TargetResponseSink::new(response_limit);
    let host_outcome = catch_unwind(AssertUnwindSafe(|| {
        handler.dispatch(&request, &mut response)
    }));
    // A target handler is synchronous. If cancellation becomes visible after
    // it returns, its one permitted call has already happened and all observed
    // response bytes must be charged before cancellation wins settlement. Do
    // not recast that effect as pre-dispatch cancellation or publish a result.
    if response.overflowed {
        let charged_bytes = response_limit.saturating_add(1);
        let _ = accounting.charge_result(charged_bytes, limits);
        if cancellation.is_cancelled() {
            return settled(
                grant,
                request_digest,
                *accounting,
                true,
                Settlement::CancelledAfterDispatch,
                None,
                None,
            );
        }
        return settled(
            grant,
            request_digest,
            *accounting,
            true,
            Settlement::ResultBudget,
            None,
            None,
        );
    }
    let raw = response.bytes;
    let result_charge = accounting.charge_result(raw.len(), limits);
    // A handler failure is already selected when the call returns. A
    // simultaneous cancellation cannot replace that physical outcome.
    if cancellation.is_cancelled() && matches!(&host_outcome, Ok(Ok(()))) {
        return settled(
            grant,
            request_digest,
            *accounting,
            true,
            Settlement::CancelledAfterDispatch,
            None,
            None,
        );
    }
    if let Err(settlement) = result_charge {
        return settled(
            grant,
            request_digest,
            *accounting,
            true,
            settlement,
            None,
            None,
        );
    }
    let result_digest = (!raw.is_empty()).then(|| digest(RESULT_DOMAIN, &raw));
    match host_outcome {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            return settled(
                grant,
                request_digest,
                *accounting,
                true,
                Settlement::HostFailed,
                None,
                result_digest,
            )
        }
        Err(_) => {
            return settled(
                grant,
                request_digest,
                *accounting,
                true,
                Settlement::HostPanicked,
                None,
                result_digest,
            )
        }
    }
    let result_digest = result_digest.or_else(|| Some(digest(RESULT_DOMAIN, &raw)));
    match TypedCarrier::decode(&raw, request.operation.result_type()) {
        Ok(result) => settled(
            grant,
            request_digest,
            *accounting,
            true,
            Settlement::Returned,
            Some(result),
            result_digest,
        ),
        Err(ProtocolError::ResultTypeMismatch) => settled(
            grant,
            request_digest,
            *accounting,
            true,
            Settlement::ResultTypeMismatch,
            None,
            result_digest,
        ),
        Err(_) => settled(
            grant,
            request_digest,
            *accounting,
            true,
            Settlement::MalformedResult,
            None,
            result_digest,
        ),
    }
}

fn settled(
    grant: TargetGrant,
    request_digest: String,
    accounting: TargetAccounting,
    dispatched: bool,
    settlement: Settlement,
    result: Option<TypedCarrier>,
    result_digest: Option<String>,
) -> TargetDispatch {
    let mut evidence = TargetEvidence {
        grant_id: grant.grant_id,
        authorization_binding: grant.authorization_binding,
        operation: grant.operation,
        turn: grant.turn,
        request_digest,
        result_digest,
        accounting,
        dispatched,
        settlement,
        digest: String::new(),
    };
    evidence.digest = evidence.compute_digest();
    TargetDispatch { result, evidence }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidIdentifier,
    InvalidDigest,
    CarrierTooLarge,
    MalformedCarrier,
    MalformedRequest,
    MalformedEvidence,
    ResultTypeMismatch,
    ReplayMismatch,
}

#[cfg(test)]
#[path = "target_protocol_authorization_tests.rs"]
mod authorization_tests;

fn validate_identifier(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        Err(ProtocolError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

fn validate_digest(value: &str) -> Result<(), ProtocolError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(ProtocolError::InvalidDigest)
    }
}

fn frame(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u64).to_be_bytes());
    out.extend_from_slice(value);
}

fn take_frame<'a>(input: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], ProtocolError> {
    let end = cursor
        .checked_add(8)
        .ok_or(ProtocolError::MalformedCarrier)?;
    let length = input
        .get(*cursor..end)
        .ok_or(ProtocolError::MalformedCarrier)?;
    *cursor = end;
    let size = usize::try_from(u64::from_be_bytes(
        length
            .try_into()
            .map_err(|_| ProtocolError::MalformedCarrier)?,
    ))
    .map_err(|_| ProtocolError::MalformedCarrier)?;
    let end = cursor
        .checked_add(size)
        .ok_or(ProtocolError::MalformedCarrier)?;
    let value = input
        .get(*cursor..end)
        .ok_or(ProtocolError::MalformedCarrier)?;
    *cursor = end;
    Ok(value)
}

fn take_frame_request<'a>(input: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], ProtocolError> {
    take_frame(input, cursor).map_err(|_| ProtocolError::MalformedRequest)
}

fn take_frame_evidence<'a>(input: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], ProtocolError> {
    take_frame(input, cursor).map_err(|_| ProtocolError::MalformedEvidence)
}

fn take_u64_request(input: &[u8], cursor: &mut usize) -> Result<u64, ProtocolError> {
    let bytes = take_frame_request(input, cursor)?;
    bytes
        .try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| ProtocolError::MalformedRequest)
}

fn take_u64_evidence(input: &[u8], cursor: &mut usize) -> Result<u64, ProtocolError> {
    let bytes = take_frame_evidence(input, cursor)?;
    bytes
        .try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| ProtocolError::MalformedEvidence)
}

fn decode_identifier(bytes: &[u8], error: ProtocolError) -> Result<String, ProtocolError> {
    let value = std::str::from_utf8(bytes).map_err(|_| error)?;
    validate_identifier(value).map_err(|_| error)?;
    Ok(value.to_owned())
}

fn decode_digest(bytes: &[u8], error: ProtocolError) -> Result<String, ProtocolError> {
    let value = std::str::from_utf8(bytes).map_err(|_| error)?;
    validate_digest(value).map_err(|_| error)?;
    Ok(value.to_owned())
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation() -> TargetOperation {
        TargetOperation::new(
            "fixture.agent.effect.read",
            "read",
            "fixture.Argument",
            "fixture.Result",
        )
        .unwrap()
    }

    fn carrier(type_id: &str, payload: &[u8]) -> TypedCarrier {
        TypedCarrier::new(type_id, payload.to_vec()).unwrap()
    }

    fn limits() -> TargetLimits {
        TargetLimits {
            max_calls: 1,
            max_request_bytes: 4096,
            max_result_bytes: 1024,
            max_total_bytes: 5120,
            max_fuel: 20,
        }
    }

    fn request() -> AuthorizedRequest {
        AuthorizedRequest {
            binding: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            budget: 10,
            seal: b"seal".to_vec(),
        }
    }

    fn grant() -> TargetGrant {
        TargetGrant::bind_request(
            request(),
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            None,
            3,
            operation(),
            &carrier("fixture.Argument", b"request"),
        )
    }

    struct Handler {
        calls: usize,
        response: Result<Vec<u8>, TargetHostError>,
    }
    impl TargetHostHandler for Handler {
        fn dispatch(
            &mut self,
            _: &TargetHostRequest,
            sink: &mut TargetResponseSink,
        ) -> Result<(), TargetHostError> {
            self.calls += 1;
            match &self.response {
                Ok(bytes) => {
                    let _ = sink.write(bytes);
                    Ok(())
                }
                Err(error) => Err(*error),
            }
        }
    }

    #[test]
    fn exact_typed_result_is_dispatched_once_and_evidence_replays_without_host_work() {
        let response = carrier("fixture.Result", b"ok").encode();
        let mut handler = Handler {
            calls: 0,
            response: Ok(response),
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.Argument", b"request"),
            4,
            limits(),
            &mut accounting,
            &AgentCancellation::new(),
            &mut handler,
        );
        assert_eq!(handler.calls, 1);
        assert_eq!(run.evidence().settlement(), Settlement::Returned);
        assert_eq!(run.result().unwrap().payload(), b"ok");
        assert_eq!(run.evidence().accounting().calls(), 1);
        let request = TargetHostRequest {
            grant_id: run.evidence().grant_id.clone(),
            authorization_binding: run.evidence().authorization_binding.clone(),
            operation: operation(),
            turn: 3,
            argument: carrier("fixture.Argument", b"request"),
            fuel: 4,
        };
        run.evidence().replay(&request).unwrap();
        assert_eq!(
            handler.calls, 1,
            "evidence replay has no dispatch authority"
        );
    }

    #[test]
    fn cancellation_call_budget_fuel_and_grant_budget_refuse_before_host_dispatch() {
        let cases = [
            (true, limits(), 4, 10, Settlement::Cancelled),
            (
                false,
                TargetLimits {
                    max_calls: 0,
                    ..limits()
                },
                4,
                10,
                Settlement::CallBudget,
            ),
            (
                false,
                TargetLimits {
                    max_fuel: 3,
                    ..limits()
                },
                4,
                10,
                Settlement::FuelExhausted,
            ),
            (
                false,
                TargetLimits {
                    max_request_bytes: 1,
                    ..limits()
                },
                4,
                10,
                Settlement::RequestBudget,
            ),
            (
                false,
                TargetLimits {
                    max_total_bytes: 1,
                    ..limits()
                },
                4,
                10,
                Settlement::RequestBudget,
            ),
            (false, limits(), 11, 10, Settlement::GrantBudget),
        ];
        for (cancelled, limits, fuel, budget, expected) in cases {
            let mut handler = Handler {
                calls: 0,
                response: Ok(carrier("fixture.Result", b"ok").encode()),
            };
            let mut accounting = TargetAccounting::default();
            let cancellation = AgentCancellation::new();
            if cancelled {
                cancellation.cancel();
            }
            let mut grant = grant();
            grant.granted_budget = budget;
            let run = dispatch(
                grant,
                carrier("fixture.Argument", b"request"),
                fuel,
                limits,
                &mut accounting,
                &cancellation,
                &mut handler,
            );
            assert_eq!(run.evidence().settlement(), expected);
            assert!(!run.evidence().dispatched());
            assert_eq!(handler.calls, 0);
        }

        let mut handler = Handler {
            calls: 0,
            response: Ok(carrier("fixture.Result", b"ok").encode()),
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.OtherArgument", b"request"),
            4,
            limits(),
            &mut accounting,
            &AgentCancellation::new(),
            &mut handler,
        );
        assert_eq!(
            run.evidence().settlement(),
            Settlement::ArgumentTypeMismatch
        );
        assert_eq!(handler.calls, 0);
    }

    #[test]
    fn cancellation_observed_after_host_dispatch_is_charged_and_never_publishes_a_result() {
        struct CancellingHandler<'a> {
            cancellation: &'a AgentCancellation,
            calls: usize,
        }
        impl TargetHostHandler for CancellingHandler<'_> {
            fn dispatch(
                &mut self,
                request: &TargetHostRequest,
                response: &mut TargetResponseSink,
            ) -> Result<(), TargetHostError> {
                self.calls += 1;
                self.cancellation.cancel();
                let result = TypedCarrier::new(request.operation.result_type(), b"ok".to_vec())
                    .map_err(|_| TargetHostError::Failed)?;
                response
                    .write(&result.encode())
                    .map_err(|_| TargetHostError::Failed)
            }
        }

        let cancellation = AgentCancellation::new();
        let mut handler = CancellingHandler {
            cancellation: &cancellation,
            calls: 0,
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.Argument", b"request"),
            4,
            limits(),
            &mut accounting,
            &cancellation,
            &mut handler,
        );
        assert_eq!(handler.calls, 1);
        assert_eq!(
            run.evidence().settlement(),
            Settlement::CancelledAfterDispatch
        );
        assert!(run.evidence().dispatched());
        assert!(run.result().is_none());
        assert_eq!(run.evidence().accounting(), accounting);
        assert_eq!(accounting.calls(), 1);
        assert!(accounting.fuel() > 0);
    }

    #[test]
    fn same_typed_argument_substitution_is_refused_before_dispatch() {
        let mut handler = Handler {
            calls: 0,
            response: Ok(carrier("fixture.Result", b"ok").encode()),
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.Argument", b"substituted"),
            4,
            limits(),
            &mut accounting,
            &AgentCancellation::new(),
            &mut handler,
        );
        assert_eq!(handler.calls, 0);
        assert_eq!(accounting, TargetAccounting::default());
        assert_eq!(
            run.evidence().settlement(),
            Settlement::ArgumentBindingMismatch
        );
    }

    #[test]
    fn malformed_wrong_type_and_oversized_results_are_sticky_charged_and_stop_at_host_boundary() {
        let malformed = vec![0, 1, 2];
        let wrong_type = carrier("fixture.Other", b"wrong").encode();
        let oversized = vec![42; 1025];
        for (response, expected) in [
            (malformed, Settlement::MalformedResult),
            (wrong_type, Settlement::ResultTypeMismatch),
            (oversized, Settlement::ResultBudget),
        ] {
            let mut handler = Handler {
                calls: 0,
                response: Ok(response),
            };
            let mut accounting = TargetAccounting::default();
            let run = dispatch(
                grant(),
                carrier("fixture.Argument", b"request"),
                4,
                limits(),
                &mut accounting,
                &AgentCancellation::new(),
                &mut handler,
            );
            assert_eq!(run.evidence().settlement(), expected);
            assert!(run.evidence().dispatched());
            assert!(run.result().is_none());
            assert_eq!(handler.calls, 1);
            assert_eq!(
                run.evidence().accounting().calls(),
                1,
                "host work remains charged"
            );
        }
    }

    #[test]
    fn aggregate_wire_ceiling_rejects_a_result_that_fits_its_individual_ceiling() {
        let response = carrier("fixture.Result", b"ok").encode();
        let request_bytes = TargetHostRequest {
            grant_id: grant().grant_id.clone(),
            authorization_binding: grant().authorization_binding.clone(),
            operation: operation(),
            turn: 3,
            argument: carrier("fixture.Argument", b"request"),
            fuel: 4,
        }
        .canonical_wire()
        .len() as u64;
        let mut handler = Handler {
            calls: 0,
            response: Ok(response.clone()),
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.Argument", b"request"),
            4,
            TargetLimits {
                max_total_bytes: request_bytes + response.len() as u64 - 1,
                ..limits()
            },
            &mut accounting,
            &AgentCancellation::new(),
            &mut handler,
        );
        assert_eq!(handler.calls, 1);
        assert_eq!(run.evidence().settlement(), Settlement::ResultBudget);
        assert!(run.result().is_none());
    }

    #[test]
    fn hostile_carrier_frames_and_forged_evidence_do_not_replay() {
        let valid = carrier("fixture.Result", b"ok").encode();
        let mut duplicate = valid.clone();
        duplicate.extend_from_slice(&valid);
        assert_eq!(
            TypedCarrier::decode(&duplicate, "fixture.Result"),
            Err(ProtocolError::MalformedCarrier)
        );

        let mut handler = Handler {
            calls: 0,
            response: Ok(valid),
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.Argument", b"request"),
            4,
            limits(),
            &mut accounting,
            &AgentCancellation::new(),
            &mut handler,
        );
        let mut forged = run.evidence().clone();
        forged.dispatched = false;
        let request = TargetHostRequest {
            grant_id: forged.grant_id.clone(),
            authorization_binding: forged.authorization_binding.clone(),
            operation: operation(),
            turn: 3,
            argument: carrier("fixture.Argument", b"request"),
            fuel: 4,
        };
        assert_eq!(forged.replay(&request), Err(ProtocolError::ReplayMismatch));
        assert_eq!(handler.calls, 1);
    }

    #[test]
    fn carrier_limit_applies_to_the_complete_wire_not_only_its_payload() {
        let type_id = "app.types.Result";
        let overhead = 3 * std::mem::size_of::<u64>() + CARRIER_SCHEMA.len() + type_id.len();
        let exact = TypedCarrier::new(type_id, vec![0; MAX_CARRIER_BYTES - overhead]).unwrap();
        assert_eq!(exact.encode().len(), MAX_CARRIER_BYTES);
        assert_eq!(
            TypedCarrier::new(type_id, vec![0; MAX_CARRIER_BYTES - overhead + 1]),
            Err(ProtocolError::CarrierTooLarge)
        );
    }

    #[test]
    fn host_failure_is_sticky_after_reservation() {
        let mut handler = Handler {
            calls: 0,
            response: Err(TargetHostError::Unavailable),
        };
        let mut accounting = TargetAccounting::default();
        let run = dispatch(
            grant(),
            carrier("fixture.Argument", b"request"),
            4,
            limits(),
            &mut accounting,
            &AgentCancellation::new(),
            &mut handler,
        );
        assert_eq!(run.evidence().settlement(), Settlement::HostFailed);
        assert!(run.evidence().dispatched());
        assert_eq!(run.evidence().accounting().calls(), 1);
        assert_eq!(run.evidence().accounting().result_bytes(), 0);
    }
}
