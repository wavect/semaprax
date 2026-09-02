//! Canonical host-side byte codec for native callable ABI v2.
//!
//! This module only serializes requests and validates completed response
//! storage. It neither changes ownership state nor invokes native code.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "the callable wire remains staged behind SPX-B104")
)]

use std::num::NonZeroU64;

use crate::descriptor_v2::{Descriptor, Parameter, ResultShape, ScalarKind};

const REQUEST_MAGIC: &[u8; 8] = b"SPXNREQ1";
const RESPONSE_MAGIC: &[u8; 8] = b"SPXNRSP1";
const WIRE_VERSION: u32 = 1;
const HEADER_SIZE: u32 = 20;
const FIXED_REQUEST_BYTES: usize = 64;
const FIXED_RESPONSE_BYTES: usize = 68;

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED: u32 = 2;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

pub(crate) const OUTCOME_SUCCESS: u32 = 1;
pub(crate) const OUTCOME_FAILURE: u32 = 2;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum RequestArgument {
    I64(i64),
    Bool(bool),
    OwnedPayload(u64),
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct DecodedRequest {
    pub(crate) invocation: NonZeroU64,
    pub(crate) arguments: Vec<RequestArgument>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResponseOutcome {
    ScalarSuccess(i64),
    OwnedSuccess { owner_ordinal: usize },
    Failure { selected_ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedResponse {
    pub(crate) outcome: ResponseOutcome,
    pub(crate) semantic_ordinals: Vec<u32>,
    pub(crate) declared_len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DecodedResponseHead {
    pub(crate) outcome: ResponseOutcome,
    pub(crate) declared_len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WireError {
    WrongStorageCapacity,
    Malformed,
    UnsupportedSchema,
    WrongCallContract,
    WrongInvocation,
    ArgumentCountMismatch,
    ArgumentKindMismatch,
    NonCanonicalArgument,
    OutcomeMismatch,
    EventCountOutOfBounds,
    UnknownSemanticOrdinal,
    InsufficientOutputCapacity,
    AllocationFailed,
}

pub(crate) fn encode_request(
    descriptor: &Descriptor,
    invocation: NonZeroU64,
    arguments: &[RequestArgument],
) -> Result<Vec<u8>, WireError> {
    validate_request_arguments(descriptor, arguments)?;
    let expected = expected_request_len(descriptor)?;
    if expected != descriptor.capacities.max_request_bytes as usize {
        return Err(WireError::WrongStorageCapacity);
    }

    let mut writer = Writer::with_capacity(expected);
    writer.bytes(REQUEST_MAGIC);
    writer.u32(WIRE_VERSION);
    writer.u32(HEADER_SIZE);
    writer.u32(u32::try_from(expected).map_err(|_| WireError::WrongStorageCapacity)?);
    writer.bytes(&descriptor.fingerprints.call_contract);
    writer.u64(invocation.get());
    writer.u32(u32::try_from(arguments.len()).map_err(|_| WireError::ArgumentCountMismatch)?);
    for (parameter, argument) in descriptor.parameters.iter().zip(arguments) {
        match (parameter, argument) {
            (Parameter::Scalar { index, .. }, RequestArgument::I64(value)) => {
                writer.u32(PARAMETER_SCALAR);
                writer.index(*index)?;
                writer.i64(*value);
            }
            (Parameter::Scalar { index, .. }, RequestArgument::Bool(value)) => {
                writer.u32(PARAMETER_SCALAR);
                writer.index(*index)?;
                writer.u32(u32::from(*value));
            }
            (
                Parameter::Owned {
                    index,
                    owner_ordinal,
                    ..
                },
                RequestArgument::OwnedPayload(payload),
            ) => {
                writer.u32(PARAMETER_OWNED);
                writer.index(*index)?;
                writer.index(*owner_ordinal)?;
                writer.u64(*payload);
            }
            _ => return Err(WireError::ArgumentKindMismatch),
        }
    }
    if writer.output.len() != expected {
        return Err(WireError::WrongStorageCapacity);
    }
    Ok(writer.output)
}

pub(crate) fn decode_request(
    descriptor: &Descriptor,
    bytes: &[u8],
) -> Result<DecodedRequest, WireError> {
    let expected = expected_request_len(descriptor)?;
    if expected != descriptor.capacities.max_request_bytes as usize || bytes.len() != expected {
        return Err(WireError::WrongStorageCapacity);
    }
    let mut reader = Reader::new(bytes);
    read_envelope(&mut reader, REQUEST_MAGIC, bytes.len())?;
    if reader.fingerprint()? != descriptor.fingerprints.call_contract {
        return Err(WireError::WrongCallContract);
    }
    let invocation = NonZeroU64::new(reader.u64()?).ok_or(WireError::WrongInvocation)?;
    let count = reader.usize()?;
    if count != descriptor.parameters.len() {
        return Err(WireError::ArgumentCountMismatch);
    }
    let mut arguments = Vec::with_capacity(count);
    for (expected_index, parameter) in descriptor.parameters.iter().enumerate() {
        let tag = reader.u32()?;
        let index = reader.usize()?;
        if index != expected_index {
            return Err(WireError::NonCanonicalArgument);
        }
        match parameter {
            Parameter::Scalar {
                index,
                kind: ScalarKind::I64,
                ..
            } if tag == PARAMETER_SCALAR && *index == expected_index => {
                arguments.push(RequestArgument::I64(reader.i64()?));
            }
            Parameter::Scalar {
                index,
                kind: ScalarKind::Bool,
                ..
            } if tag == PARAMETER_SCALAR && *index == expected_index => {
                let value = match reader.u32()? {
                    0 => false,
                    1 => true,
                    _ => return Err(WireError::NonCanonicalArgument),
                };
                arguments.push(RequestArgument::Bool(value));
            }
            Parameter::Owned {
                index,
                owner_ordinal,
                ..
            } if tag == PARAMETER_OWNED && *index == expected_index => {
                if reader.usize()? != *owner_ordinal {
                    return Err(WireError::NonCanonicalArgument);
                }
                arguments.push(RequestArgument::OwnedPayload(reader.u64()?));
            }
            Parameter::Scalar { .. } | Parameter::Owned { .. } => {
                return Err(WireError::ArgumentKindMismatch)
            }
        }
    }
    if reader.offset != bytes.len() {
        return Err(WireError::Malformed);
    }
    Ok(DecodedRequest {
        invocation,
        arguments,
    })
}

pub(crate) fn encode_response(
    descriptor: &Descriptor,
    invocation: NonZeroU64,
    outcome: ResponseOutcome,
    semantic_ordinals: &[u32],
) -> Result<Vec<u8>, WireError> {
    validate_outcome(descriptor, &outcome)?;
    validate_semantic_ordinals(descriptor, semantic_ordinals)?;
    let declared_len = response_declared_len(&outcome, semantic_ordinals.len())?;
    let capacity = descriptor.capacities.max_response_bytes as usize;
    if declared_len > capacity || expected_response_capacity(descriptor)? != capacity {
        return Err(WireError::WrongStorageCapacity);
    }

    let mut writer = Writer::with_capacity(capacity);
    writer.bytes(RESPONSE_MAGIC);
    writer.u32(WIRE_VERSION);
    writer.u32(HEADER_SIZE);
    writer.u32(u32::try_from(declared_len).map_err(|_| WireError::WrongStorageCapacity)?);
    writer.bytes(&descriptor.fingerprints.call_contract);
    writer.u64(invocation.get());
    writer.u32(match outcome {
        ResponseOutcome::ScalarSuccess(_) | ResponseOutcome::OwnedSuccess { .. } => OUTCOME_SUCCESS,
        ResponseOutcome::Failure { .. } => OUTCOME_FAILURE,
    });
    writer
        .u32(u32::try_from(semantic_ordinals.len()).map_err(|_| WireError::EventCountOutOfBounds)?);
    match outcome {
        ResponseOutcome::ScalarSuccess(value) => {
            writer.u32(RESULT_SCALAR_I64);
            writer.i64(value);
        }
        ResponseOutcome::OwnedSuccess { owner_ordinal } => {
            writer.u32(RESULT_OWNED_INPUT);
            writer.index(owner_ordinal)?;
        }
        ResponseOutcome::Failure { selected_ordinal } => writer.u32(selected_ordinal),
    }
    for ordinal in semantic_ordinals {
        writer.u32(*ordinal);
    }
    if writer.output.len() != declared_len {
        return Err(WireError::Malformed);
    }
    writer.output.resize(capacity, 0);
    Ok(writer.output)
}

pub(crate) fn decode_response(
    descriptor: &Descriptor,
    expected_invocation: NonZeroU64,
    storage: &[u8],
) -> Result<DecodedResponse, WireError> {
    let mut semantic_ordinals = Vec::new();
    semantic_ordinals
        .try_reserve_exact(descriptor.capacities.max_event_count as usize)
        .map_err(|_| WireError::AllocationFailed)?;
    let head = decode_response_into(
        descriptor,
        expected_invocation,
        storage,
        &mut semantic_ordinals,
    )?;
    Ok(DecodedResponse {
        outcome: head.outcome,
        semantic_ordinals,
        declared_len: head.declared_len,
    })
}

pub(crate) fn decode_response_into(
    descriptor: &Descriptor,
    expected_invocation: NonZeroU64,
    storage: &[u8],
    semantic_ordinals: &mut Vec<u32>,
) -> Result<DecodedResponseHead, WireError> {
    semantic_ordinals.clear();
    if semantic_ordinals.capacity() < descriptor.capacities.max_event_count as usize {
        return Err(WireError::InsufficientOutputCapacity);
    }
    let result = decode_response_into_preallocated(
        descriptor,
        expected_invocation,
        storage,
        semantic_ordinals,
    );
    if result.is_err() {
        semantic_ordinals.clear();
    }
    result
}

fn decode_response_into_preallocated(
    descriptor: &Descriptor,
    expected_invocation: NonZeroU64,
    storage: &[u8],
    semantic_ordinals: &mut Vec<u32>,
) -> Result<DecodedResponseHead, WireError> {
    let capacity = descriptor.capacities.max_response_bytes as usize;
    if storage.len() != capacity || expected_response_capacity(descriptor)? != capacity {
        return Err(WireError::WrongStorageCapacity);
    }
    let mut reader = Reader::new(storage);
    let declared_len = read_response_envelope(&mut reader, storage.len())?;
    if reader.fingerprint()? != descriptor.fingerprints.call_contract {
        return Err(WireError::WrongCallContract);
    }
    if reader.u64()? != expected_invocation.get() {
        return Err(WireError::WrongInvocation);
    }
    let outcome_tag = reader.u32()?;
    let event_count = reader.usize()?;
    if event_count == 0 || event_count > descriptor.capacities.max_event_count as usize {
        return Err(WireError::EventCountOutOfBounds);
    }
    let outcome = match outcome_tag {
        OUTCOME_SUCCESS => match descriptor.result {
            ResultShape::ScalarI64 => {
                if reader.u32()? != RESULT_SCALAR_I64 {
                    return Err(WireError::OutcomeMismatch);
                }
                ResponseOutcome::ScalarSuccess(reader.i64()?)
            }
            ResultShape::OwnedInput { owner_ordinal, .. } => {
                if reader.u32()? != RESULT_OWNED_INPUT || reader.usize()? != owner_ordinal {
                    return Err(WireError::OutcomeMismatch);
                }
                ResponseOutcome::OwnedSuccess { owner_ordinal }
            }
        },
        OUTCOME_FAILURE => ResponseOutcome::Failure {
            selected_ordinal: read_semantic_ordinal(&mut reader, descriptor)?,
        },
        _ => return Err(WireError::OutcomeMismatch),
    };
    let expected_declared = response_declared_len(&outcome, event_count)?;
    if declared_len != expected_declared {
        return Err(WireError::Malformed);
    }
    for _ in 0..event_count {
        semantic_ordinals.push(read_semantic_ordinal(&mut reader, descriptor)?);
    }
    if reader.offset != declared_len {
        return Err(WireError::Malformed);
    }
    Ok(DecodedResponseHead {
        outcome,
        declared_len,
    })
}

fn validate_request_arguments(
    descriptor: &Descriptor,
    arguments: &[RequestArgument],
) -> Result<(), WireError> {
    if arguments.len() != descriptor.parameters.len() {
        return Err(WireError::ArgumentCountMismatch);
    }
    for (parameter, argument) in descriptor.parameters.iter().zip(arguments) {
        if !matches!(
            (parameter, argument),
            (
                Parameter::Scalar {
                    kind: ScalarKind::I64,
                    ..
                },
                RequestArgument::I64(_)
            ) | (
                Parameter::Scalar {
                    kind: ScalarKind::Bool,
                    ..
                },
                RequestArgument::Bool(_)
            ) | (Parameter::Owned { .. }, RequestArgument::OwnedPayload(_))
        ) {
            return Err(WireError::ArgumentKindMismatch);
        }
    }
    Ok(())
}

fn validate_outcome(descriptor: &Descriptor, outcome: &ResponseOutcome) -> Result<(), WireError> {
    match (descriptor.result, outcome) {
        (ResultShape::ScalarI64, ResponseOutcome::ScalarSuccess(_)) => Ok(()),
        (
            ResultShape::OwnedInput { owner_ordinal, .. },
            ResponseOutcome::OwnedSuccess {
                owner_ordinal: actual,
            },
        ) if owner_ordinal == *actual => Ok(()),
        (_, ResponseOutcome::Failure { selected_ordinal }) => {
            validate_semantic_ordinal(descriptor, *selected_ordinal)
        }
        _ => Err(WireError::OutcomeMismatch),
    }
}

fn validate_semantic_ordinals(descriptor: &Descriptor, ordinals: &[u32]) -> Result<(), WireError> {
    if ordinals.is_empty() || ordinals.len() > descriptor.capacities.max_event_count as usize {
        return Err(WireError::EventCountOutOfBounds);
    }
    for ordinal in ordinals {
        validate_semantic_ordinal(descriptor, *ordinal)?;
    }
    Ok(())
}

fn validate_semantic_ordinal(descriptor: &Descriptor, ordinal: u32) -> Result<(), WireError> {
    if ordinal == 0 || ordinal > descriptor.capacities.dictionary_entries {
        Err(WireError::UnknownSemanticOrdinal)
    } else {
        Ok(())
    }
}

fn read_semantic_ordinal(
    reader: &mut Reader<'_>,
    descriptor: &Descriptor,
) -> Result<u32, WireError> {
    let ordinal = reader.u32()?;
    validate_semantic_ordinal(descriptor, ordinal)?;
    Ok(ordinal)
}

fn expected_request_len(descriptor: &Descriptor) -> Result<usize, WireError> {
    let mut length = FIXED_REQUEST_BYTES;
    for parameter in &descriptor.parameters {
        let bytes = match parameter {
            Parameter::Scalar {
                kind: ScalarKind::I64,
                ..
            } => 16,
            Parameter::Scalar {
                kind: ScalarKind::Bool,
                ..
            } => 12,
            Parameter::Owned { .. } => 20,
        };
        length = length
            .checked_add(bytes)
            .ok_or(WireError::WrongStorageCapacity)?;
    }
    Ok(length)
}

fn expected_response_capacity(descriptor: &Descriptor) -> Result<usize, WireError> {
    let success_bytes = match descriptor.result {
        ResultShape::ScalarI64 => 12_usize,
        ResultShape::OwnedInput { .. } => 8,
    };
    let events = (descriptor.capacities.max_event_count as usize)
        .checked_mul(4)
        .ok_or(WireError::WrongStorageCapacity)?;
    FIXED_RESPONSE_BYTES
        .checked_add(success_bytes.max(4))
        .and_then(|length| length.checked_add(events))
        .ok_or(WireError::WrongStorageCapacity)
}

fn response_declared_len(
    outcome: &ResponseOutcome,
    event_count: usize,
) -> Result<usize, WireError> {
    let outcome_bytes = match outcome {
        ResponseOutcome::ScalarSuccess(_) => 12_usize,
        ResponseOutcome::OwnedSuccess { .. } => 8,
        ResponseOutcome::Failure { .. } => 4,
    };
    FIXED_RESPONSE_BYTES
        .checked_add(outcome_bytes)
        .and_then(|length| length.checked_add(event_count.checked_mul(4)?))
        .ok_or(WireError::Malformed)
}

fn read_envelope(
    reader: &mut Reader<'_>,
    magic: &[u8; 8],
    exact_len: usize,
) -> Result<(), WireError> {
    if reader.take(8)? != magic || reader.u32()? != WIRE_VERSION || reader.u32()? != HEADER_SIZE {
        return Err(WireError::UnsupportedSchema);
    }
    if reader.usize()? != exact_len {
        return Err(WireError::Malformed);
    }
    Ok(())
}

fn read_response_envelope(reader: &mut Reader<'_>, capacity: usize) -> Result<usize, WireError> {
    if reader.take(8)? != RESPONSE_MAGIC
        || reader.u32()? != WIRE_VERSION
        || reader.u32()? != HEADER_SIZE
    {
        return Err(WireError::UnsupportedSchema);
    }
    let declared = reader.usize()?;
    if declared < FIXED_RESPONSE_BYTES || declared > capacity {
        return Err(WireError::Malformed);
    }
    Ok(declared)
}

struct Writer {
    output: Vec<u8>,
}

impl Writer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            output: Vec::with_capacity(capacity),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.output.extend_from_slice(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    fn index(&mut self, value: usize) -> Result<(), WireError> {
        self.u32(u32::try_from(value).map_err(|_| WireError::NonCanonicalArgument)?);
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WireError> {
        let end = self.offset.checked_add(count).ok_or(WireError::Malformed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WireError::Malformed)?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| WireError::Malformed)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| WireError::Malformed)?,
        ))
    }

    fn i64(&mut self) -> Result<i64, WireError> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| WireError::Malformed)?,
        ))
    }

    fn usize(&mut self) -> Result<usize, WireError> {
        usize::try_from(self.u32()?).map_err(|_| WireError::Malformed)
    }

    fn fingerprint(&mut self) -> Result<[u8; 32], WireError> {
        self.take(32)?.try_into().map_err(|_| WireError::Malformed)
    }
}

#[cfg(test)]
#[path = "callable_wire/tests.rs"]
mod tests;
