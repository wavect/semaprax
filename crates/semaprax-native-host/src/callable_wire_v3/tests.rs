use std::collections::BTreeMap;

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
        decision: Decision::AbortPhysical(9),
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
        fixture.decision.encode_fixed().unwrap(),
        decision_bytes.as_slice()
    );
    assert_eq!(
        SettlementDecision::parse(&decision_bytes, &fixture.descriptor)
            .unwrap()
            .encode(),
        decision_bytes
    );
    for action in &fixture.actions {
        let bytes = action.encode();
        assert_eq!(action.encode_fixed().unwrap(), bytes.as_slice());
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
    assert_eq!(
        receipt.encode_fixed().unwrap(),
        fixture.receipt_bytes.as_slice()
    );
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
    let artifact =
        emit_native_callable_v3_descriptor(&corpus.program, &DeclarationId::new("token.identity"))
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
    let (storage, frame) = build_executed(&fixture.descriptor, &fixture.request, &fixture.response);
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
    let unwind_storage = vec![0; 160];
    let unwind_response_hash =
        response_storage_digest(PRE_EXECUTE_HOST_UNWIND_CODE, &unwind_storage);
    assert_eq!(
        hex(&unwind_response_hash),
        "bb1e191d800451376f7407935c9cf6771a61c8ea144befc62508d60845e63b70"
    );
    let mut unwind_frame = RecoveryFrame {
        identity,
        request_digest: request_hash,
        response_storage_digest: unwind_response_hash,
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::PreExecuteHostUnwind,
        checkpoint: 1,
        phase: FramePhase::Executing,
        decision_digest: [0; 32],
        next_action: 0,
        record_count: 0,
        active_finalizers: 0,
        cells: vec![ResourceCell {
            state: CellState::Live,
            payload: 0,
        }],
        action_chain_digest: [0; 32],
        pre_candidate_digest: [0; 32],
    };
    let provisional = unwind_frame.encode();
    unwind_frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
    let unwind_bytes = unwind_frame.encode();
    assert_eq!(&unwind_bytes[260..264], &3_u32.to_le_bytes());
    assert_eq!(
        &unwind_bytes[264..268],
        &PRE_EXECUTE_HOST_UNWIND_CODE.to_le_bytes()
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
