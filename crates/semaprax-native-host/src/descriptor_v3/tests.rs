use super::*;

fn fixture_graph() -> SettlementGraph {
    let trace_path_certificate = [0x37; 32];
    let ordinals = vec![7, 8];
    let trace_outcome = TraceOutcome::ScalarSuccess;
    SettlementGraph {
        function: "token.consume".to_owned(),
        recovery_contract: [0x38; 32],
        execution_cleanup: [0x35; 32],
        trace_path_certificate,
        resource_count: 1,
        checkpoints: vec![
            Checkpoint {
                id: 1,
                resources: vec![ResourceState::Live],
                outcome: None,
                abort_order: vec![0],
                accept_order: vec![],
            },
            Checkpoint {
                id: 2,
                resources: vec![ResourceState::Dead],
                outcome: None,
                abort_order: vec![],
                accept_order: vec![],
            },
            Checkpoint {
                id: 3,
                resources: vec![ResourceState::Dead],
                outcome: Some(Outcome::ScalarSuccess),
                abort_order: vec![],
                accept_order: vec![],
            },
        ],
        starts: vec![1],
        edges: vec![
            Edge {
                from: 1,
                to: 2,
                action: Action::Finalize(0),
            },
            Edge {
                from: 2,
                to: 3,
                action: Action::CertifyOutcome(TraceEvidence {
                    digest: trace_evidence_fingerprint(
                        &trace_path_certificate,
                        &ordinals,
                        trace_outcome,
                    ),
                    ordinals,
                    outcome: trace_outcome,
                }),
            },
        ],
    }
}

fn fixture_parameters() -> Vec<Parameter> {
    vec![
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
    ]
}

fn fixture_capacities() -> Capacities {
    Capacities {
        request: 140,
        execute_response: 220,
        frame: 400,
        decision: 172,
        action_evidence: 196,
        candidate_receipt: 384,
        event_count: 16,
        dictionary_bytes: 2048,
        dictionary_entries: 8,
        resource_count: 1,
        checkpoint_count: 3,
        graph_work_units: 3,
        active_frames: 256,
        quarantined_frames: 64,
        instance_reserved_bytes: 651_520,
    }
}

fn fixture() -> Descriptor {
    let target = current_target_tag().unwrap();
    let linkage = if cfg!(target_os = "ios") {
        Linkage::IosStatic
    } else {
        Linkage::Dynamic
    };
    let module = "test.callable_v3".to_owned();
    let function = "token.consume".to_owned();
    let graph = fixture_graph();
    let graph_bytes = encode_graph(&graph).unwrap();
    let schema = schema_fingerprint();
    let target_fingerprint = target_fingerprint(target.as_bytes());
    let semantic_module = [0x31; 32];
    let mut fingerprints = Fingerprints {
        schema,
        target: target_fingerprint,
        semantic_module,
        physical_module: physical_module_fingerprint(
            &schema,
            &target_fingerprint,
            &semantic_module,
            module.as_bytes(),
            linkage,
        ),
        function_template: [0x34; 32],
        execution_cleanup: graph.execution_cleanup,
        event_dictionary: [0x36; 32],
        trace_path_certificate: graph.trace_path_certificate,
        recovery_contract: graph.recovery_contract,
        settlement_graph: graph_fingerprint(&graph_bytes),
        request_schema: request_schema_fingerprint(),
        execute_response_schema: execute_response_schema_fingerprint(),
        frame_schema: frame_schema_fingerprint(),
        decision_schema: decision_schema_fingerprint(),
        action_schema: action_schema_fingerprint(),
        candidate_receipt_schema: candidate_receipt_schema_fingerprint(),
        committed_receipt_schema: committed_receipt_schema_fingerprint(),
        call_abi: call_abi_fingerprint(),
        call_contract: [0; 32],
    };
    let capacities = fixture_capacities();
    let parameters = fixture_parameters();
    let result = ResultShape::ScalarI64;
    fingerprints.call_contract = call_contract_fingerprint(
        &target,
        linkage,
        &fingerprints,
        &module,
        &function,
        &capacities,
        &parameters,
        &result,
    );
    let (getter_symbol, execute_symbol, settle_symbol) = derive_symbols(&fingerprints);
    Descriptor {
        target,
        linkage,
        fingerprints,
        module,
        function,
        getter_symbol,
        execute_symbol,
        settle_symbol,
        call_abi_tag: CALL_ABI_TAG,
        obligations: REQUIRED_OBLIGATIONS,
        capacities,
        parameters,
        result,
        graph,
    }
}

fn reseal(descriptor: &mut Descriptor) {
    descriptor.fingerprints.physical_module = physical_module_fingerprint(
        &descriptor.fingerprints.schema,
        &descriptor.fingerprints.target,
        &descriptor.fingerprints.semantic_module,
        descriptor.module.as_bytes(),
        descriptor.linkage,
    );
    descriptor.fingerprints.settlement_graph =
        graph_fingerprint(&encode_graph(&descriptor.graph).unwrap());
    descriptor.fingerprints.call_contract = call_contract_fingerprint(
        &descriptor.target,
        descriptor.linkage,
        &descriptor.fingerprints,
        &descriptor.module,
        &descriptor.function,
        &descriptor.capacities,
        &descriptor.parameters,
        &descriptor.result,
    );
    (
        descriptor.getter_symbol,
        descriptor.execute_symbol,
        descriptor.settle_symbol,
    ) = derive_symbols(&descriptor.fingerprints);
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(value, "{byte:02x}").unwrap();
    }
    value
}

#[test]
fn canonical_metadata_round_trips_and_binds_every_surface() {
    let expected = fixture();
    let bytes = encode_descriptor(&expected).unwrap();
    let parsed = Descriptor::parse(&bytes).unwrap();
    assert_eq!(parsed, expected);
    assert_eq!(&bytes[..8], MAGIC);
    assert_eq!(parsed.capacities, fixture_capacities());
    assert_eq!(parsed.graph.starts, [1]);
    assert_eq!(parsed.graph.edges.len(), 2);
    assert_ne!(
        parsed.fingerprints.candidate_receipt_schema,
        parsed.fingerprints.committed_receipt_schema
    );
    assert_ne!(parsed.getter_symbol, parsed.execute_symbol);
    assert_ne!(parsed.getter_symbol, parsed.settle_symbol);
    assert_ne!(parsed.execute_symbol, parsed.settle_symbol);
}

#[test]
fn schema_and_graph_known_answers_are_stable_and_symbols_are_deterministic() {
    let descriptor = fixture();
    assert_eq!(
        hex(&descriptor.fingerprints.schema),
        "e03477296a8f90b7544c340c4d41155eb7cbd860a40de40319b41b8a0249c5b5"
    );
    assert_eq!(
        hex(&descriptor.fingerprints.settlement_graph),
        "09f4e623e8345613ad82796fd5600c9b251ee16a1ad1d5ca6ed6432056770a74"
    );
    assert_eq!(
        descriptor.fingerprints.call_contract,
        call_contract_fingerprint(
            &descriptor.target,
            descriptor.linkage,
            &descriptor.fingerprints,
            &descriptor.module,
            &descriptor.function,
            &descriptor.capacities,
            &descriptor.parameters,
            &descriptor.result,
        )
    );
    assert_eq!(
        (
            descriptor.getter_symbol.clone(),
            descriptor.execute_symbol.clone(),
            descriptor.settle_symbol.clone()
        ),
        derive_symbols(&descriptor.fingerprints)
    );
    assert!(descriptor.getter_symbol.ends_with("_descriptor_v3"));
    assert!(descriptor.execute_symbol.ends_with("_execute_v3"));
    assert!(descriptor.settle_symbol.ends_with("_settle_v3"));
}

#[test]
fn rejects_every_prefix_trailing_byte_and_single_byte_mutation() {
    let bytes = encode_descriptor(&fixture()).unwrap();
    for length in 0..bytes.len() {
        assert!(
            Descriptor::parse(&bytes[..length]).is_err(),
            "accepted prefix length {length}"
        );
    }
    for trailing in [0_u8, 1, 0x7f, 0xff] {
        let mut hostile = bytes.clone();
        hostile.push(trailing);
        assert!(Descriptor::parse(&hostile).is_err());
    }
    for offset in 0..bytes.len() {
        let mut hostile = bytes.clone();
        hostile[offset] ^= 1;
        assert!(
            Descriptor::parse(&hostile).is_err(),
            "accepted mutation at byte {offset}"
        );
    }
}

#[test]
fn rejects_version_confusion_without_negotiation_or_fallback() {
    let canonical = encode_descriptor(&fixture()).unwrap();
    for (magic, version) in [
        (b"SPXNABI1".as_slice(), 1_u32),
        (b"SPXNABI2".as_slice(), 2_u32),
        (b"SPXNPRF1".as_slice(), 1_u32),
    ] {
        let mut hostile = canonical.clone();
        hostile[..8].copy_from_slice(magic);
        hostile[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::UnsupportedSchema)
        );
    }
}

#[test]
fn rejects_rehashed_hostile_capacities_and_cross_bindings() {
    let mut cases = Vec::new();
    let mut request = fixture();
    request.capacities.request += 1;
    cases.push(request);
    let mut work = fixture();
    work.capacities.graph_work_units += 1;
    cases.push(work);
    let mut resources = fixture();
    resources.capacities.resource_count += 1;
    cases.push(resources);
    let mut active = fixture();
    active.capacities.active_frames -= 1;
    cases.push(active);
    let mut reserved = fixture();
    reserved.capacities.instance_reserved_bytes += 1;
    cases.push(reserved);
    let mut function = fixture();
    function.graph.function = "token.other".to_owned();
    cases.push(function);
    let mut recovery = fixture();
    recovery.graph.recovery_contract = [0x99; 32];
    cases.push(recovery);
    let mut cleanup = fixture();
    cleanup.graph.execution_cleanup = [0x99; 32];
    cases.push(cleanup);
    let mut trace = fixture();
    trace.graph.trace_path_certificate = [0x99; 32];
    cases.push(trace);

    for mut hostile in cases {
        reseal(&mut hostile);
        assert!(Descriptor::parse(&encode_descriptor(&hostile).unwrap()).is_err());
    }
}

#[test]
fn rejects_rehashed_hostile_graph_topology_states_orders_and_tags() {
    let mut cases = Vec::new();
    let mut checkpoint_id = fixture();
    checkpoint_id.graph.checkpoints[0].id = 2;
    cases.push(checkpoint_id);
    let mut state = fixture();
    state.graph.checkpoints[0].resources[0] = ResourceState::Finalizing;
    cases.push(state);
    let mut order = fixture();
    order.graph.checkpoints[0].abort_order.clear();
    cases.push(order);
    let mut start = fixture();
    start.graph.starts = vec![2];
    cases.push(start);
    let mut backwards = fixture();
    backwards.graph.edges[0].to = backwards.graph.edges[0].from;
    cases.push(backwards);
    let mut zero_trace = fixture();
    let Action::CertifyOutcome(evidence) = &mut zero_trace.graph.edges[1].action else {
        unreachable!()
    };
    evidence.digest = [0; 32];
    cases.push(zero_trace);
    let mut nonzero_changed_digest = fixture();
    let Action::CertifyOutcome(evidence) = &mut nonzero_changed_digest.graph.edges[1].action else {
        unreachable!()
    };
    evidence.digest = [0x99; 32];
    cases.push(nonzero_changed_digest);
    let mut changed_witness = fixture();
    let Action::CertifyOutcome(evidence) = &mut changed_witness.graph.edges[1].action else {
        unreachable!()
    };
    evidence.ordinals[0] += 1;
    cases.push(changed_witness);
    let mut changed_witness_outcome = fixture();
    let Action::CertifyOutcome(evidence) = &mut changed_witness_outcome.graph.edges[1].action
    else {
        unreachable!()
    };
    evidence.outcome = TraceOutcome::OwnedSuccess;
    cases.push(changed_witness_outcome);

    for mut hostile in cases {
        reseal(&mut hostile);
        assert!(Descriptor::parse(&encode_descriptor(&hostile).unwrap()).is_err());
    }

    let mut unknown_tag = encode_descriptor(&fixture()).unwrap();
    let graph_start = graph_start(&unknown_tag);
    let first_state = graph_start + 4 + 4 + "token.consume".len() + 3 * 32 + 4 + 4 + 4 + 4;
    unknown_tag[first_state..first_state + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Descriptor::parse(&unknown_tag).is_err());
}

#[test]
fn rejects_hostile_text_counts_symbols_linkage_and_global_ceiling() {
    let bytes = encode_descriptor(&fixture()).unwrap();
    let target_length = HEADER_SIZE as usize;
    let target_start = target_length + 4;
    let mut nul = bytes.clone();
    nul[target_start] = 0;
    assert!(Descriptor::parse(&nul).is_err());
    let mut bad_utf8 = bytes.clone();
    bad_utf8[target_start] = 0xff;
    assert!(Descriptor::parse(&bad_utf8).is_err());
    let mut hostile_count = bytes.clone();
    hostile_count[target_length..target_length + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Descriptor::parse(&hostile_count).is_err());

    let mut wrong_linkage = fixture();
    wrong_linkage.linkage = if wrong_linkage.linkage == Linkage::Dynamic {
        Linkage::IosStatic
    } else {
        Linkage::Dynamic
    };
    reseal(&mut wrong_linkage);
    assert_eq!(
        Descriptor::parse(&encode_descriptor(&wrong_linkage).unwrap()),
        Err(DescriptorError::WrongTarget)
    );

    let mut duplicate_symbol = fixture();
    duplicate_symbol.execute_symbol = duplicate_symbol.getter_symbol.clone();
    assert!(Descriptor::parse(&encode_descriptor(&duplicate_symbol).unwrap()).is_err());
    assert_eq!(
        Descriptor::parse(&vec![0; MAX_DESCRIPTOR_BYTES + 1]),
        Err(DescriptorError::Malformed)
    );
}

#[test]
fn committed_receipt_schema_is_host_only_and_not_candidate_evidence() {
    assert_ne!(
        CANDIDATE_RECEIPT_SCHEMA_DOMAIN,
        COMMITTED_RECEIPT_SCHEMA_DOMAIN
    );
    let committed = std::str::from_utf8(COMMITTED_RECEIPT_SCHEMA_STATEMENT).unwrap();
    assert!(committed.contains("host-only"));
    assert!(committed.contains("hmac32"));
    assert!(committed.contains("separate-receipt-key"));
    assert!(!std::str::from_utf8(CANDIDATE_RECEIPT_SCHEMA_STATEMENT)
        .unwrap()
        .contains("host-only"));
}

fn graph_start(bytes: &[u8]) -> usize {
    let mut reader = Reader::new(bytes);
    reader.take(8).unwrap();
    reader.u32().unwrap();
    reader.u32().unwrap();
    reader.u32().unwrap();
    reader.text(MAX_TEXT_BYTES).unwrap();
    reader.u32().unwrap();
    reader.take(19 * 32).unwrap();
    for _ in 0..5 {
        reader.text(MAX_TEXT_BYTES).unwrap();
    }
    reader.u32().unwrap();
    reader.u32().unwrap();
    reader.take(15 * 4).unwrap();
    let parameter_count = reader.usize().unwrap();
    for _ in 0..parameter_count {
        match reader.u32().unwrap() {
            PARAMETER_SCALAR => {
                reader.u32().unwrap();
                reader.text(MAX_TEXT_BYTES).unwrap();
                reader.u32().unwrap();
            }
            PARAMETER_OWNED_RESOURCE => {
                reader.u32().unwrap();
                reader.text(MAX_TEXT_BYTES).unwrap();
                reader.u32().unwrap();
                reader.text(MAX_TEXT_BYTES).unwrap();
                reader.text(MAX_TEXT_BYTES).unwrap();
                reader.u32().unwrap();
            }
            _ => unreachable!(),
        }
    }
    match reader.u32().unwrap() {
        RESULT_SCALAR_I64 => {}
        RESULT_OWNED_INPUT => {
            reader.u32().unwrap();
            reader.text(MAX_TEXT_BYTES).unwrap();
            reader.u32().unwrap();
        }
        _ => unreachable!(),
    }
    let graph_len = reader.usize().unwrap();
    assert_eq!(graph_len, reader.remaining());
    reader.offset
}
