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
mod tests {
    use super::*;
    use crate::descriptor_v2::{Capacities, Fingerprints};

    fn descriptor(result: ResultShape, max_events: u32, dictionary_entries: u32) -> Descriptor {
        let parameters = vec![
            Parameter::Owned {
                index: 0,
                value: "token.value".to_owned(),
                owner_ordinal: 0,
                resource: "token.type".to_owned(),
                lifecycle: "token.drop".to_owned(),
                payload_wire_kind: 1,
            },
            Parameter::Scalar {
                index: 1,
                value: "enabled.value".to_owned(),
                kind: ScalarKind::Bool,
            },
            Parameter::Scalar {
                index: 2,
                value: "count.value".to_owned(),
                kind: ScalarKind::I64,
            },
        ];
        let response =
            68 + match result {
                ResultShape::ScalarI64 => 12,
                ResultShape::OwnedInput { .. } => 8,
            } + 4 * max_events;
        Descriptor {
            target: "test-target".to_owned(),
            fingerprints: Fingerprints {
                schema: [1; 32],
                target: [2; 32],
                semantic_module: [3; 32],
                physical_module: [4; 32],
                function_template: [5; 32],
                execution_cleanup: [6; 32],
                event_dictionary: [7; 32],
                trace_path_certificate: [8; 32],
                request_schema: [8; 32],
                response_schema: [9; 32],
                call_abi: [10; 32],
                call_contract: [11; 32],
            },
            module: "test.module".to_owned(),
            function: "test.call".to_owned(),
            getter_symbol: "spx_getter".to_owned(),
            callable_symbol: "spx_call".to_owned(),
            call_abi_tag: 1,
            obligations: 0x0f,
            capacities: Capacities {
                max_request_bytes: 112,
                max_response_bytes: response,
                max_event_count: max_events,
                dictionary_bytes: 100,
                dictionary_entries,
            },
            parameters,
            result,
        }
    }

    fn invocation() -> NonZeroU64 {
        NonZeroU64::new(0x0102_0304_0506_0708).unwrap()
    }

    fn request_arguments() -> [RequestArgument; 3] {
        [
            RequestArgument::OwnedPayload(u64::MAX),
            RequestArgument::Bool(true),
            RequestArgument::I64(i64::MIN),
        ]
    }

    fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn request_round_trip_is_byte_exact_at_scalar_and_payload_boundaries() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let bytes = encode_request(&descriptor, invocation(), &request_arguments()).unwrap();
        assert_eq!(bytes.len(), 112);
        assert_eq!(&bytes[..8], REQUEST_MAGIC);
        assert_eq!(&bytes[20..52], &[11; 32]);
        assert_eq!(
            u64::from_le_bytes(bytes[52..60].try_into().unwrap()),
            invocation().get()
        );
        let decoded = decode_request(&descriptor, &bytes).unwrap();
        assert_eq!(decoded.invocation, invocation());
        assert!(matches!(
            decoded.arguments.as_slice(),
            [
                RequestArgument::OwnedPayload(u64::MAX),
                RequestArgument::Bool(true),
                RequestArgument::I64(i64::MIN)
            ]
        ));

        let zero_payload = [
            RequestArgument::OwnedPayload(0),
            RequestArgument::Bool(false),
            RequestArgument::I64(i64::MAX),
        ];
        let bytes = encode_request(&descriptor, invocation(), &zero_payload).unwrap();
        assert!(decode_request(&descriptor, &bytes).unwrap().arguments == zero_payload);
    }

    #[test]
    fn request_rejects_every_truncation_trailing_byte_and_wrong_capacity() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let bytes = encode_request(&descriptor, invocation(), &request_arguments()).unwrap();
        for length in 0..bytes.len() {
            assert!(decode_request(&descriptor, &bytes[..length]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_request(&descriptor, &trailing),
            Err(WireError::WrongStorageCapacity)
        ));
        let mut wrong = descriptor.clone();
        wrong.capacities.max_request_bytes += 1;
        assert_eq!(
            encode_request(&wrong, invocation(), &request_arguments()),
            Err(WireError::WrongStorageCapacity)
        );
    }

    #[test]
    fn request_structural_fields_and_canonical_bool_fail_closed() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let canonical = encode_request(&descriptor, invocation(), &request_arguments()).unwrap();
        for offset in [0, 8, 12, 16, 20, 60, 64, 68, 72, 84, 88, 96, 100] {
            let mut hostile = canonical.clone();
            hostile[offset] ^= 1;
            assert!(
                decode_request(&descriptor, &hostile).is_err(),
                "accepted structural request mutation at {offset}"
            );
        }
        let mut zero_invocation = canonical.clone();
        zero_invocation[52..60].fill(0);
        assert!(matches!(
            decode_request(&descriptor, &zero_invocation),
            Err(WireError::WrongInvocation)
        ));
        let mut noncanonical_bool = canonical;
        replace_u32(&mut noncanonical_bool, 92, 2);
        assert!(matches!(
            decode_request(&descriptor, &noncanonical_bool),
            Err(WireError::NonCanonicalArgument)
        ));
    }

    #[test]
    fn every_request_byte_is_either_validated_or_typed_payload() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let canonical = encode_request(&descriptor, invocation(), &request_arguments()).unwrap();
        for offset in 0..canonical.len() {
            let mut mutated = canonical.clone();
            mutated[offset] ^= 1;
            let accepted = decode_request(&descriptor, &mutated).is_ok();
            let typed_payload = (52..60).contains(&offset)
                || (76..84).contains(&offset)
                || offset == 92
                || (104..112).contains(&offset);
            assert_eq!(
                accepted, typed_payload,
                "unexpected request mutation classification at {offset}"
            );
        }
    }

    #[test]
    fn response_round_trips_scalar_owned_and_failure_outcomes() {
        let scalar = descriptor(ResultShape::ScalarI64, 4, 9);
        let storage = encode_response(
            &scalar,
            invocation(),
            ResponseOutcome::ScalarSuccess(i64::MIN),
            &[1, 9],
        )
        .unwrap();
        let decoded = decode_response(&scalar, invocation(), &storage).unwrap();
        assert_eq!(decoded.outcome, ResponseOutcome::ScalarSuccess(i64::MIN));
        assert_eq!(decoded.semantic_ordinals, [1, 9]);
        assert_eq!(decoded.declared_len, 88);

        let owned = descriptor(
            ResultShape::OwnedInput {
                parameter_index: 0,
                owner_ordinal: 0,
            },
            4,
            9,
        );
        let storage = encode_response(
            &owned,
            invocation(),
            ResponseOutcome::OwnedSuccess { owner_ordinal: 0 },
            &[2],
        )
        .unwrap();
        assert_eq!(
            decode_response(&owned, invocation(), &storage)
                .unwrap()
                .outcome,
            ResponseOutcome::OwnedSuccess { owner_ordinal: 0 }
        );

        let storage = encode_response(
            &owned,
            invocation(),
            ResponseOutcome::Failure {
                selected_ordinal: 3,
            },
            &[3, 4],
        )
        .unwrap();
        assert_eq!(
            decode_response(&owned, invocation(), &storage)
                .unwrap()
                .outcome,
            ResponseOutcome::Failure {
                selected_ordinal: 3
            }
        );
    }

    #[test]
    fn response_ignores_poison_after_declared_length_but_rejects_trailing_storage() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let mut storage = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(7),
            &[1],
        )
        .unwrap();
        let declared = u32::from_le_bytes(storage[16..20].try_into().unwrap()) as usize;
        storage[declared..].fill(0xa5);
        let decoded = decode_response(&descriptor, invocation(), &storage).unwrap();
        assert_eq!(decoded.declared_len, declared);
        assert_eq!(decoded.outcome, ResponseOutcome::ScalarSuccess(7));

        storage.push(0);
        assert_eq!(
            decode_response(&descriptor, invocation(), &storage),
            Err(WireError::WrongStorageCapacity)
        );
    }

    #[test]
    fn response_rejects_every_truncation_and_hostile_structural_field() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let canonical = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(7),
            &[1, 2],
        )
        .unwrap();
        for length in 0..canonical.len() {
            assert!(decode_response(&descriptor, invocation(), &canonical[..length]).is_err());
        }
        for offset in [0, 8, 12, 16, 20, 52, 60, 64, 68] {
            let mut hostile = canonical.clone();
            hostile[offset] ^= 1;
            assert!(
                decode_response(&descriptor, invocation(), &hostile).is_err(),
                "accepted structural response mutation at {offset}"
            );
        }
        let mut unknown_outcome = canonical.clone();
        replace_u32(&mut unknown_outcome, 60, 99);
        assert_eq!(
            decode_response(&descriptor, invocation(), &unknown_outcome),
            Err(WireError::OutcomeMismatch)
        );
        let mut zero_event = canonical.clone();
        replace_u32(&mut zero_event, 80, 0);
        assert_eq!(
            decode_response(&descriptor, invocation(), &zero_event),
            Err(WireError::UnknownSemanticOrdinal)
        );
        let mut unknown_event = canonical;
        replace_u32(&mut unknown_event, 84, 10);
        assert_eq!(
            decode_response(&descriptor, invocation(), &unknown_event),
            Err(WireError::UnknownSemanticOrdinal)
        );
    }

    #[test]
    fn every_response_byte_is_either_validated_payload_or_ignored_tail() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let canonical = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(7),
            &[1, 2],
        )
        .unwrap();
        let declared = u32::from_le_bytes(canonical[16..20].try_into().unwrap()) as usize;
        for offset in 0..canonical.len() {
            let mut mutated = canonical.clone();
            mutated[offset] ^= 1;
            let accepted = decode_response(&descriptor, invocation(), &mutated).is_ok();
            let typed_or_ignored = (72..80).contains(&offset)
                || offset == 84
                || (declared..canonical.len()).contains(&offset);
            assert_eq!(
                accepted, typed_or_ignored,
                "unexpected response mutation classification at {offset}"
            );
        }
    }

    #[test]
    fn response_outcome_result_and_failure_ordinals_are_exact() {
        let owned = descriptor(
            ResultShape::OwnedInput {
                parameter_index: 0,
                owner_ordinal: 0,
            },
            2,
            3,
        );
        assert_eq!(
            encode_response(
                &owned,
                invocation(),
                ResponseOutcome::OwnedSuccess { owner_ordinal: 1 },
                &[1]
            ),
            Err(WireError::OutcomeMismatch)
        );
        assert_eq!(
            encode_response(
                &owned,
                invocation(),
                ResponseOutcome::Failure {
                    selected_ordinal: 0
                },
                &[1]
            ),
            Err(WireError::UnknownSemanticOrdinal)
        );
        assert_eq!(
            encode_response(
                &owned,
                invocation(),
                ResponseOutcome::Failure {
                    selected_ordinal: 4
                },
                &[1]
            ),
            Err(WireError::UnknownSemanticOrdinal)
        );
        assert_eq!(
            encode_response(
                &owned,
                invocation(),
                ResponseOutcome::Failure {
                    selected_ordinal: 1
                },
                &[]
            ),
            Err(WireError::EventCountOutOfBounds)
        );
    }

    #[test]
    fn maximum_event_count_round_trips_and_one_over_fails_before_allocation() {
        let descriptor = descriptor(ResultShape::ScalarI64, 65_536, 65_536);
        let events = vec![65_536; 65_536];
        let storage = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(i64::MAX),
            &events,
        )
        .unwrap();
        assert_eq!(
            decode_response(&descriptor, invocation(), &storage)
                .unwrap()
                .semantic_ordinals
                .len(),
            65_536
        );
        let mut one_over = events;
        one_over.push(1);
        assert_eq!(
            encode_response(
                &descriptor,
                invocation(),
                ResponseOutcome::ScalarSuccess(0),
                &one_over
            ),
            Err(WireError::EventCountOutOfBounds)
        );
    }

    #[test]
    fn response_into_requires_full_capacity_and_never_grows_supplied_storage() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let storage = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(7),
            &[1, 2],
        )
        .unwrap();

        let mut undersized = Vec::with_capacity(3);
        undersized.push(9);
        assert_eq!(
            decode_response_into(&descriptor, invocation(), &storage, &mut undersized),
            Err(WireError::InsufficientOutputCapacity)
        );
        assert!(undersized.is_empty());

        let mut ordinals = Vec::with_capacity(4);
        let pointer = ordinals.as_ptr();
        let capacity = ordinals.capacity();
        let head =
            decode_response_into(&descriptor, invocation(), &storage, &mut ordinals).unwrap();
        assert_eq!(head.outcome, ResponseOutcome::ScalarSuccess(7));
        assert_eq!(ordinals, [1, 2]);
        assert_eq!(ordinals.as_ptr(), pointer);
        assert_eq!(ordinals.capacity(), capacity);
    }

    #[test]
    fn response_into_clears_partially_decoded_ordinals_on_error() {
        let descriptor = descriptor(ResultShape::ScalarI64, 4, 9);
        let mut storage = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(7),
            &[1, 2],
        )
        .unwrap();
        replace_u32(&mut storage, 84, 10);
        let mut ordinals = Vec::with_capacity(4);
        ordinals.push(9);

        assert_eq!(
            decode_response_into(&descriptor, invocation(), &storage, &mut ordinals),
            Err(WireError::UnknownSemanticOrdinal)
        );
        assert!(ordinals.is_empty());
        assert_eq!(ordinals.capacity(), 4);
    }
}
