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
    let head = decode_response_into(&descriptor, invocation(), &storage, &mut ordinals).unwrap();
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
