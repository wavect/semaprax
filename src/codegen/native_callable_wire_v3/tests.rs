use std::fmt::Write as _;

use super::*;

fn binding() -> ProviderBinding {
    ProviderBinding {
        call_contract: [1; 32],
        recovery_contract: [2; 32],
        settlement_graph: [3; 32],
        invocation: 0x0102_0304_0506_0708,
        frame_generation: 0x1112_1314_1516_1718,
        provider_challenge: [4; 32],
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        },
    )
}

#[test]
fn all_six_wires_have_exact_lengths_and_stable_digests() {
    let request = encode_request(
        binding(),
        &[
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
    )
    .unwrap();
    assert_eq!(request.len(), (104 + 16 + 12 + 20 + 20) as usize);
    let request_hash = request_digest(&request).unwrap();

    let response = encode_execute_response(
        binding(),
        request_hash,
        7,
        ExecuteOutcome::Owned {
            owner_ordinal: 1,
            payload: u64::MAX,
        },
        &[1, 7, 9],
        4,
        9,
    )
    .unwrap();
    assert_eq!(
        response.len(),
        execute_response_capacity(4).unwrap() as usize
    );
    assert_eq!(read_u32(&response, 16).unwrap(), 168);
    assert_eq!(&response[168..], &[0; 4]);
    let response_hash = response_storage_digest(0, &response).unwrap();

    let decision = encode_decision(
        binding(),
        SettlementDecision::AcceptOwned { owner_ordinal: 1 },
    )
    .unwrap();
    let decision_hash = decision_digest(&decision).unwrap();
    let chain0 = initial_action_chain_digest(decision_hash, 2).unwrap();
    let action = encode_action_evidence(ActionEvidence {
        binding: binding(),
        action_index: 0,
        boundary: ActionBoundary::Publish,
        owner_ordinal: 1,
        payload: u64::MAX,
        before: ResourceState::ProvisionalResult,
        after: ResourceState::Published,
        checkpoint: 7,
    })
    .unwrap();
    let chain1 = extend_action_chain_digest(chain0, 0, &action).unwrap();

    let resources = [
        ResourceCell {
            state: ResourceState::Dead,
            payload: 0,
        },
        ResourceCell {
            state: ResourceState::Published,
            payload: u64::MAX,
        },
    ];
    let frame = encode_frame(&RecoveryFrame {
        binding: binding(),
        request_digest: request_hash,
        response_digest: response_hash,
        semantic_trace_digest: [5; 32],
        execute_return: ExecuteReturn::Returned(0),
        checkpoint: 7,
        phase: FramePhase::ProviderSettled,
        decision_digest: decision_hash,
        next_action_index: 2,
        action_record_count: 1,
        active_finalizers: 0,
        resources: &resources,
        action_chain_digest: chain1,
    })
    .unwrap();
    assert_eq!(frame.len(), frame_capacity(2).unwrap() as usize);
    let frame_hash: [u8; 32] = frame[frame.len() - 32..].try_into().unwrap();
    assert_eq!(pre_candidate_frame_digest(&frame).unwrap(), frame_hash);

    let dispositions = [
        DispositionCell {
            disposition: TerminalDisposition::Dead,
            payload: 0,
        },
        DispositionCell {
            disposition: TerminalDisposition::Published,
            payload: u64::MAX,
        },
    ];
    let candidate = encode_candidate_receipt(&CandidateReceipt {
        binding: binding(),
        request_digest: request_hash,
        response_digest: response_hash,
        semantic_trace_digest: [5; 32],
        pre_candidate_frame_digest: frame_hash,
        decision_digest: decision_hash,
        action_chain_digest: chain1,
        outcome: CandidateOutcome::Owned { owner_ordinal: 1 },
        active_finalizers: 0,
        dispositions: &dispositions,
    })
    .unwrap();
    assert_eq!(
        candidate.len(),
        candidate_receipt_capacity(2).unwrap() as usize
    );
    assert_eq!(
        hex(&request_hash),
        "699565f407451aab7dbddf5a4788e99d6439f1c67ac045df721f805fec1ba135"
    );
    assert_eq!(
        hex(&response_hash),
        "72bf589efdd016f29616f9a1448d563cff9ae0fd9fe231cc7706d867edba9fb7"
    );
    assert_eq!(
        hex(&decision_hash),
        "cc7d67f6cacd3e1c80844e8c2f48c39e4833c892102a3b6ed02194e3eebe9e1f"
    );
    assert_eq!(
        hex(&chain1),
        "606d8324103845e16657699ee014e3841adea671643c1522b1e64b2f75a58388"
    );
    assert_eq!(
        hex(&frame_hash),
        "b0013da9bf07b2be4dbcea3103360acae2b487ec7ce28fb264aaa5ac3fbad111"
    );
    assert_eq!(
        hex(&candidate_digest(&candidate).unwrap()),
        "4e82547b169ccd07d0c90e7ff3051265067b8a2de9312b53fcc8d96a8fa9b3bb"
    );
}

#[test]
fn tags_bounds_and_phase_invariants_fail_closed() {
    assert!(encode_decision(binding(), SettlementDecision::AbortPhysical { code: 0 }).is_err());
    assert!(execute_response_capacity(u32::MAX).is_err());
    assert!(frame_capacity(u32::MAX).is_err());
    assert!(candidate_receipt_capacity(u32::MAX).is_err());

    let resources = [ResourceCell {
        state: ResourceState::Finalizing,
        payload: 0,
    }];
    let invalid = RecoveryFrame {
        binding: binding(),
        request_digest: [1; 32],
        response_digest: [0; 32],
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::Pending,
        checkpoint: 1,
        phase: FramePhase::Executing,
        decision_digest: [0; 32],
        next_action_index: 0,
        action_record_count: 0,
        active_finalizers: 0,
        resources: &resources,
        action_chain_digest: [0; 32],
    };
    assert_eq!(encode_frame(&invalid), Err(WireV3Error::InvalidState));

    let response = vec![0; execute_response_capacity(1).unwrap() as usize];
    let unwind_digest = response_storage_digest(PRE_EXECUTE_HOST_UNWIND_CODE, &response).unwrap();
    let unwind_resources = [ResourceCell {
        state: ResourceState::Live,
        payload: 0,
    }];
    let unwind = encode_frame(&RecoveryFrame {
        resources: &unwind_resources,
        response_digest: unwind_digest,
        execute_return: ExecuteReturn::PreExecuteHostUnwind,
        active_finalizers: 0,
        ..invalid
    })
    .unwrap();
    assert_eq!(read_u32(&unwind, 260).unwrap(), 3);
    assert_eq!(
        read_u32(&unwind, 264).unwrap(),
        PRE_EXECUTE_HOST_UNWIND_CODE
    );
    assert_eq!(
        hex(&unwind_digest),
        "bb1e191d800451376f7407935c9cf6771a61c8ea144befc62508d60845e63b70"
    );
}

#[test]
fn digest_transcripts_reject_wrong_wire_or_self_inclusion() {
    let request = encode_request(binding(), &[]).unwrap();
    assert!(decision_digest(&request).is_err());
    let mut trailing = request.clone();
    trailing.push(0);
    assert!(request_digest(&trailing).is_err());

    let resources = [ResourceCell {
        state: ResourceState::Live,
        payload: 42,
    }];
    let frame = encode_frame(&RecoveryFrame {
        binding: binding(),
        request_digest: request_digest(&request).unwrap(),
        response_digest: [0; 32],
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::Pending,
        checkpoint: 1,
        phase: FramePhase::Executing,
        decision_digest: [0; 32],
        next_action_index: 0,
        action_record_count: 0,
        active_finalizers: 0,
        resources: &resources,
        action_chain_digest: [0; 32],
    })
    .unwrap();
    let digest = pre_candidate_frame_digest(&frame).unwrap();
    let self_including = framed_sha256(FRAME_DIGEST_DOMAIN, &frame);
    assert_ne!(digest, self_including);
}

#[test]
fn every_closed_outcome_decision_action_and_phase_has_one_canonical_encoding() {
    let request = encode_request(binding(), &[]).unwrap();
    let request_hash = request_digest(&request).unwrap();
    for outcome in [
        ExecuteOutcome::Scalar(i64::MAX),
        ExecuteOutcome::SemanticFailure {
            selected_ordinal: 2,
        },
        ExecuteOutcome::Owned {
            owner_ordinal: 0,
            payload: 0,
        },
    ] {
        let bytes =
            encode_execute_response(binding(), request_hash, 1, outcome, &[1, 2], 2, 2).unwrap();
        assert_eq!(bytes.len(), execute_response_capacity(2).unwrap() as usize);
    }

    for decision in [
        SettlementDecision::AcceptScalar,
        SettlementDecision::AcceptSemanticFailure,
        SettlementDecision::AcceptOwned { owner_ordinal: 0 },
        SettlementDecision::AbortPhysical { code: u32::MAX },
        SettlementDecision::AbortMalformed,
        SettlementDecision::AbortTraceRejected,
        SettlementDecision::AbortHostUnwind,
    ] {
        assert_eq!(
            encode_decision(binding(), decision).unwrap().len(),
            DECISION_BYTES as usize
        );
    }

    for evidence in [
        ActionEvidence {
            binding: binding(),
            action_index: 0,
            boundary: ActionBoundary::FinalizerStarted,
            owner_ordinal: 0,
            payload: 7,
            before: ResourceState::Live,
            after: ResourceState::Finalizing,
            checkpoint: 1,
        },
        ActionEvidence {
            binding: binding(),
            action_index: 0,
            boundary: ActionBoundary::FinalizerCompleted,
            owner_ordinal: 0,
            payload: 7,
            before: ResourceState::Finalizing,
            after: ResourceState::Dead,
            checkpoint: 1,
        },
        ActionEvidence {
            binding: binding(),
            action_index: 1,
            boundary: ActionBoundary::Publish,
            owner_ordinal: 1,
            payload: 9,
            before: ResourceState::ProvisionalResult,
            after: ResourceState::Published,
            checkpoint: 1,
        },
    ] {
        assert_eq!(
            encode_action_evidence(evidence).unwrap().len(),
            ACTION_EVIDENCE_BYTES as usize
        );
    }

    let returned_response = response_storage_digest(0, &[0; 32]).unwrap();
    let decision_hash = [8; 32];
    let action_hash = [9; 32];
    let cases = [
        (FramePhase::DecisionLocked, ResourceState::Live, 0, 0, 0),
        (
            FramePhase::ActionInProgress,
            ResourceState::Finalizing,
            0,
            1,
            1,
        ),
    ];
    for (phase, state, next, records, active) in cases {
        let resources = [ResourceCell { state, payload: 7 }];
        let bytes = encode_frame(&RecoveryFrame {
            binding: binding(),
            request_digest: request_hash,
            response_digest: returned_response,
            semantic_trace_digest: [7; 32],
            execute_return: ExecuteReturn::Returned(0),
            checkpoint: 1,
            phase,
            decision_digest: decision_hash,
            next_action_index: next,
            action_record_count: records,
            active_finalizers: active,
            resources: &resources,
            action_chain_digest: action_hash,
        })
        .unwrap();
        assert_eq!(bytes.len(), frame_capacity(1).unwrap() as usize);
    }

    for phase in [FramePhase::ReceiptCommitted, FramePhase::Quarantined] {
        let resources = [ResourceCell {
            state: ResourceState::Dead,
            payload: 7,
        }];
        let host_only = RecoveryFrame {
            binding: binding(),
            request_digest: request_hash,
            response_digest: returned_response,
            semantic_trace_digest: [7; 32],
            execute_return: ExecuteReturn::Returned(0),
            checkpoint: 1,
            phase,
            decision_digest: decision_hash,
            next_action_index: 1,
            action_record_count: 2,
            active_finalizers: 0,
            resources: &resources,
            action_chain_digest: action_hash,
        };
        assert_eq!(encode_frame(&host_only), Err(WireV3Error::InvalidState));
    }

    let dead = [DispositionCell {
        disposition: TerminalDisposition::Dead,
        payload: 7,
    }];
    for outcome in [
        CandidateOutcome::Scalar,
        CandidateOutcome::SemanticFailure,
        CandidateOutcome::Abort,
    ] {
        let semantic = if outcome == CandidateOutcome::Abort {
            [0; 32]
        } else {
            [7; 32]
        };
        let bytes = encode_candidate_receipt(&CandidateReceipt {
            binding: binding(),
            request_digest: request_hash,
            response_digest: returned_response,
            semantic_trace_digest: semantic,
            pre_candidate_frame_digest: [6; 32],
            decision_digest: decision_hash,
            action_chain_digest: action_hash,
            outcome,
            active_finalizers: 0,
            dispositions: &dead,
        })
        .unwrap();
        assert_eq!(bytes.len(), candidate_receipt_capacity(1).unwrap() as usize);
    }
}
