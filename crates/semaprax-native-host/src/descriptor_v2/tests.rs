use super::*;

struct Offsets {
    declared: usize,
    target_bytes: usize,
    physical_module: usize,
    execution_cleanup: usize,
    getter_bytes: usize,
    callable_bytes: usize,
    call_abi_tag: usize,
    obligations: usize,
    max_request: usize,
    dictionary_entries: usize,
    parameter_count: usize,
    first_index: usize,
    first_payload_kind: usize,
    second_kind: usize,
    result_parameter: usize,
    result_value: usize,
    result_ordinal: usize,
}

fn push_u32(output: &mut Vec<u8>, value: u32) -> usize {
    let offset = output.len();
    output.extend_from_slice(&value.to_le_bytes());
    offset
}

fn push_text(output: &mut Vec<u8>, value: &str) -> (usize, usize) {
    let length = push_u32(output, value.len().try_into().unwrap());
    let bytes = output.len();
    output.extend_from_slice(value.as_bytes());
    (length, bytes)
}

fn fixture_fingerprints(target: &str, module: &str) -> Fingerprints {
    let schema = schema_fingerprint();
    let target_fingerprint = target_fingerprint(target.as_bytes());
    let semantic_module = [0x31; FINGERPRINT_BYTES];
    Fingerprints {
        schema,
        target: target_fingerprint,
        semantic_module,
        physical_module: physical_module_fingerprint(
            &schema,
            &target_fingerprint,
            &semantic_module,
            module.as_bytes(),
        ),
        function_template: [0x32; FINGERPRINT_BYTES],
        execution_cleanup: [0x33; FINGERPRINT_BYTES],
        event_dictionary: [0x34; FINGERPRINT_BYTES],
        trace_path_certificate: [0x35; FINGERPRINT_BYTES],
        request_schema: request_schema_fingerprint(),
        response_schema: response_schema_fingerprint(),
        call_abi: call_abi_fingerprint(),
        call_contract: [0; FINGERPRINT_BYTES],
    }
}

fn canonical_wire() -> (Vec<u8>, Offsets) {
    canonical_wire_with_limits(16, 2048, 8)
}

fn canonical_wire_with_limits(
    max_event_count: u32,
    dictionary_bytes: u32,
    dictionary_entries: u32,
) -> (Vec<u8>, Offsets) {
    let target = current_target_tag();
    let module = "test.module";
    let function = "test.select";
    let parameters = vec![
        Parameter::Owned {
            index: 0,
            value: "token.value".to_owned(),
            owner_ordinal: 0,
            resource: "token.type".to_owned(),
            lifecycle: "token.drop".to_owned(),
            payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
        },
        Parameter::Scalar {
            index: 1,
            value: "delta.value".to_owned(),
            kind: ScalarKind::I64,
        },
    ];
    let result = ResultShape::OwnedInput {
        parameter_index: 0,
        owner_ordinal: 0,
    };
    let capacities = Capacities {
        max_request_bytes: request_capacity(&parameters).unwrap(),
        max_response_bytes: response_capacity(&result, max_event_count).unwrap(),
        max_event_count,
        dictionary_bytes,
        dictionary_entries,
    };
    let mut fingerprints = fixture_fingerprints(&target, module);
    fingerprints.call_contract = call_contract_fingerprint(
        &target,
        &fingerprints,
        module,
        function,
        &capacities,
        &parameters,
        &result,
    );
    let (getter, callable) = derive_symbols(&fingerprints);

    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    push_u32(&mut output, VERSION);
    push_u32(&mut output, HEADER_SIZE);
    let declared = push_u32(&mut output, 0);
    let (_, target_bytes) = push_text(&mut output, &target);
    output.extend_from_slice(&fingerprints.schema);
    output.extend_from_slice(&fingerprints.target);
    output.extend_from_slice(&fingerprints.semantic_module);
    let physical_module = output.len();
    output.extend_from_slice(&fingerprints.physical_module);
    output.extend_from_slice(&fingerprints.function_template);
    let execution_cleanup = output.len();
    output.extend_from_slice(&fingerprints.execution_cleanup);
    output.extend_from_slice(&fingerprints.event_dictionary);
    output.extend_from_slice(&fingerprints.trace_path_certificate);
    output.extend_from_slice(&fingerprints.request_schema);
    output.extend_from_slice(&fingerprints.response_schema);
    output.extend_from_slice(&fingerprints.call_abi);
    output.extend_from_slice(&fingerprints.call_contract);
    push_text(&mut output, module);
    push_text(&mut output, function);
    let (_, getter_bytes) = push_text(&mut output, &getter);
    let (_, callable_bytes) = push_text(&mut output, &callable);
    let call_abi_tag = push_u32(&mut output, CALL_ABI_TAG);
    let obligations = push_u32(&mut output, REQUIRED_OBLIGATIONS);
    let max_request = push_u32(&mut output, capacities.max_request_bytes);
    push_u32(&mut output, capacities.max_response_bytes);
    push_u32(&mut output, capacities.max_event_count);
    push_u32(&mut output, capacities.dictionary_bytes);
    let dictionary_entries = push_u32(&mut output, capacities.dictionary_entries);
    let parameter_count = push_u32(&mut output, 2);

    push_u32(&mut output, PARAMETER_OWNED_RESOURCE);
    let first_index = push_u32(&mut output, 0);
    push_text(&mut output, "token.value");
    push_u32(&mut output, 0);
    push_text(&mut output, "token.type");
    push_text(&mut output, "token.drop");
    let first_payload_kind = push_u32(&mut output, OWNED_PAYLOAD_WIRE_KIND);

    push_u32(&mut output, PARAMETER_SCALAR);
    push_u32(&mut output, 1);
    push_text(&mut output, "delta.value");
    let second_kind = push_u32(&mut output, SCALAR_I64);

    push_u32(&mut output, RESULT_OWNED_INPUT);
    let result_parameter = push_u32(&mut output, 0);
    let (_, result_value) = push_text(&mut output, "token.value");
    let result_ordinal = push_u32(&mut output, 0);
    let total = u32::try_from(output.len()).unwrap();
    replace_u32(&mut output, declared, total);
    (
        output,
        Offsets {
            declared,
            target_bytes,
            physical_module,
            execution_cleanup,
            getter_bytes,
            callable_bytes,
            call_abi_tag,
            obligations,
            max_request,
            dictionary_entries,
            parameter_count,
            first_index,
            first_payload_kind,
            second_kind,
            result_parameter,
            result_value,
            result_ordinal,
        },
    )
}

fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn canonical_callable_descriptor_round_trips_every_bound_field() {
    let (wire, _) = canonical_wire();
    let descriptor = Descriptor::parse(&wire).unwrap();
    assert_eq!(descriptor.target, current_target_tag());
    assert_eq!(descriptor.module, "test.module");
    assert_eq!(descriptor.function, "test.select");
    assert_eq!(descriptor.call_abi_tag, CALL_ABI_TAG);
    assert_eq!(descriptor.obligations, REQUIRED_OBLIGATIONS);
    assert_eq!(descriptor.capacities.max_request_bytes, 100);
    assert_eq!(descriptor.capacities.max_response_bytes, 140);
    assert_eq!(descriptor.capacities.max_event_count, 16);
    assert_eq!(descriptor.capacities.dictionary_bytes, 2048);
    assert_eq!(descriptor.capacities.dictionary_entries, 8);
    assert_eq!(descriptor.parameters.len(), 2);
    assert!(matches!(
        descriptor.parameters[0],
        Parameter::Owned {
            owner_ordinal: 0,
            payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
            ..
        }
    ));
    assert_eq!(
        descriptor.result,
        ResultShape::OwnedInput {
            parameter_index: 0,
            owner_ordinal: 0,
        }
    );
}

#[test]
fn envelope_target_and_trailing_data_fail_closed() {
    let (wire, offsets) = canonical_wire();

    let mut wrong_magic = wire.clone();
    wrong_magic[7] = b'9';
    assert_eq!(
        Descriptor::parse(&wrong_magic),
        Err(DescriptorError::UnsupportedSchema)
    );
    let mut wrong_target = wire.clone();
    wrong_target[offsets.target_bytes] ^= 1;
    assert_eq!(
        Descriptor::parse(&wrong_target),
        Err(DescriptorError::WrongTarget)
    );
    let mut wrong_length = wire.clone();
    replace_u32(&mut wrong_length, offsets.declared, 20);
    assert_eq!(
        Descriptor::parse(&wrong_length),
        Err(DescriptorError::Malformed)
    );
    let mut trailing = wire;
    trailing.push(0);
    assert_eq!(
        Descriptor::parse(&trailing),
        Err(DescriptorError::Malformed)
    );
    let trailing_length = u32::try_from(trailing.len()).unwrap();
    replace_u32(&mut trailing, offsets.declared, trailing_length);
    assert_eq!(
        Descriptor::parse(&trailing),
        Err(DescriptorError::Malformed)
    );
}

#[test]
fn fingerprints_symbols_abi_and_obligations_are_exact() {
    let (wire, offsets) = canonical_wire();
    for offset in [offsets.physical_module, offsets.execution_cleanup] {
        let mut hostile = wire.clone();
        hostile[offset] ^= 1;
        assert_eq!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::NonCanonical)
        );
    }
    for offset in [offsets.getter_bytes, offsets.callable_bytes] {
        let mut hostile = wire.clone();
        hostile[offset] = b'x';
        assert_eq!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::NonCanonical)
        );
    }
    let mut abi = wire.clone();
    replace_u32(&mut abi, offsets.call_abi_tag, 2);
    assert_eq!(
        Descriptor::parse(&abi),
        Err(DescriptorError::UnsupportedSchema)
    );
    let mut obligations = wire;
    replace_u32(&mut obligations, offsets.obligations, 0x07);
    assert_eq!(
        Descriptor::parse(&obligations),
        Err(DescriptorError::NonCanonical)
    );
}

#[test]
fn capacities_and_counts_are_bounded_before_allocation() {
    let (wire, offsets) = canonical_wire();
    for (offset, value) in [
        (offsets.max_request, 0),
        (offsets.max_request, MAX_CALL_WIRE_BYTES + 1),
        (offsets.dictionary_entries, 0),
        (offsets.parameter_count, u32::MAX),
    ] {
        let mut hostile = wire.clone();
        replace_u32(&mut hostile, offset, value);
        assert_eq!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::NonCanonical)
        );
    }
}

#[test]
fn protocol_limits_accept_the_boundary_and_reject_one_over() {
    let (boundary, _) = canonical_wire_with_limits(
        MAX_EVENT_COUNT,
        MAX_DICTIONARY_BYTES,
        MAX_DICTIONARY_ENTRIES,
    );
    let parsed = Descriptor::parse(&boundary).unwrap();
    assert_eq!(parsed.capacities.max_event_count, MAX_EVENT_COUNT);
    assert_eq!(parsed.capacities.dictionary_bytes, MAX_DICTIONARY_BYTES);
    assert_eq!(parsed.capacities.dictionary_entries, MAX_DICTIONARY_ENTRIES);

    for limits in [
        (
            MAX_EVENT_COUNT + 1,
            MAX_DICTIONARY_BYTES,
            MAX_DICTIONARY_ENTRIES,
        ),
        (
            MAX_EVENT_COUNT,
            MAX_DICTIONARY_BYTES + 1,
            MAX_DICTIONARY_ENTRIES,
        ),
        (
            MAX_EVENT_COUNT,
            MAX_DICTIONARY_BYTES,
            MAX_DICTIONARY_ENTRIES + 1,
        ),
    ] {
        let (hostile, _) = canonical_wire_with_limits(limits.0, limits.1, limits.2);
        assert_eq!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::NonCanonical)
        );
    }

    assert_eq!(
        Descriptor::parse(&vec![0; MAX_DESCRIPTOR_BYTES]),
        Err(DescriptorError::UnsupportedSchema)
    );
    assert_eq!(
        Descriptor::parse(&vec![0; MAX_DESCRIPTOR_BYTES + 1]),
        Err(DescriptorError::Malformed)
    );
}

#[test]
fn parameter_and_owned_result_mappings_are_canonical() {
    let (wire, offsets) = canonical_wire();
    for (offset, value) in [
        (offsets.first_index, 1),
        (offsets.first_payload_kind, 2),
        (offsets.second_kind, 99),
        (offsets.result_parameter, 1),
        (offsets.result_ordinal, 1),
    ] {
        let mut hostile = wire.clone();
        replace_u32(&mut hostile, offset, value);
        assert!(matches!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::NonCanonical | DescriptorError::UnsupportedSchema)
        ));
    }
    let mut wrong_value = wire;
    wrong_value[offsets.result_value] ^= 1;
    assert_eq!(
        Descriptor::parse(&wrong_value),
        Err(DescriptorError::NonCanonical)
    );
}

#[test]
fn truncated_invalid_utf8_nul_and_unknown_tags_are_rejected() {
    let (wire, offsets) = canonical_wire();
    for length in 0..wire.len() {
        assert!(
            Descriptor::parse(&wire[..length]).is_err(),
            "accepted truncated prefix of length {length}"
        );
    }
    let mut invalid_utf8 = wire.clone();
    invalid_utf8[offsets.target_bytes] = 0xff;
    assert_eq!(
        Descriptor::parse(&invalid_utf8),
        Err(DescriptorError::Malformed)
    );
    let mut nul_symbol = wire.clone();
    nul_symbol[offsets.getter_bytes] = 0;
    assert_eq!(
        Descriptor::parse(&nul_symbol),
        Err(DescriptorError::NonCanonical)
    );
    let mut unknown_parameter = wire;
    replace_u32(&mut unknown_parameter, offsets.first_index - 4, 99);
    assert_eq!(
        Descriptor::parse(&unknown_parameter),
        Err(DescriptorError::NonCanonical)
    );
}

#[test]
fn every_encoded_byte_is_structural_or_authenticated() {
    let (wire, _) = canonical_wire();
    Descriptor::parse(&wire).unwrap();
    for offset in 0..wire.len() {
        let mut hostile = wire.clone();
        hostile[offset] ^= 1;
        assert!(
            Descriptor::parse(&hostile).is_err(),
            "accepted single-byte mutation at offset {offset}"
        );
    }
}
