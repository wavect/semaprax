
use super::*;

macro_rules! assert_not_impl {
    ($type:ty, $trait:path) => {{
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
        struct Implemented;
        impl<T: ?Sized + $trait> AmbiguousIfImplemented<Implemented> for T {}
        let _ = <$type as AmbiguousIfImplemented<_>>::probe;
    }};
}

const CONTRACT: [u8; 32] = [0x5a; 32];

#[derive(Debug, Eq, PartialEq)]
struct FrameSnapshot {
    function: DeclarationId,
    recovery_contract: [u8; 32],
    certificate_fingerprint: [u8; 32],
    invocation: NonZeroU64,
    checkpoint: u32,
    resources: Vec<SettlementResourceState>,
    terminal: Option<(SettlementDecision, String)>,
}

fn snapshot(frame: &NativeSettlementFrame) -> FrameSnapshot {
    FrameSnapshot {
        function: frame.function.clone(),
        recovery_contract: frame.recovery_contract,
        certificate_fingerprint: frame.certificate_fingerprint,
        invocation: frame.invocation,
        checkpoint: frame.checkpoint,
        resources: frame.resources.clone(),
        terminal: frame
            .terminal
            .as_ref()
            .map(|terminal| (terminal.decision, terminal.receipt.canonical_json())),
    }
}

fn certificate(checkpoints: Vec<SettlementCheckpointSpec>) -> NativeSettlementCertificate {
    let resource_count = checkpoints[0].resources.len();
    NativeSettlementCertificate::try_new(
        DeclarationId::new("token.settlement"),
        CONTRACT,
        resource_count,
        checkpoints,
    )
    .unwrap()
}

fn reverse_non_dead(states: &[SettlementResourceState]) -> Vec<u32> {
    states
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, state)| **state != SettlementResourceState::Dead)
        .map(|(ordinal, _)| ordinal as u32)
        .collect()
}

fn reverse_live(states: &[SettlementResourceState]) -> Vec<u32> {
    states
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, state)| **state == SettlementResourceState::Live)
        .map(|(ordinal, _)| ordinal as u32)
        .collect()
}

#[test]
fn abort_exhaustively_finalizes_every_non_dead_resource_once() {
    for resource_count in 1..=6_usize {
        let mut checkpoint_id = 1_u32;
        let mut specs = Vec::new();
        let combinations = 3_usize.pow(resource_count as u32);
        for mut encoded in 0..combinations {
            let mut states = Vec::new();
            let mut provisional_count = 0;
            for _ in 0..resource_count {
                let state = match encoded % 3 {
                    0 => SettlementResourceState::Live,
                    1 => SettlementResourceState::Dead,
                    _ => {
                        provisional_count += 1;
                        SettlementResourceState::ProvisionalResult
                    }
                };
                encoded /= 3;
                states.push(state);
            }
            if provisional_count > 1 {
                continue;
            }
            specs.push(SettlementCheckpointSpec::new(
                checkpoint_id,
                states.clone(),
                None,
                reverse_non_dead(&states),
                Vec::new(),
            ));
            checkpoint_id += 1;
        }
        let certificate = certificate(specs);
        for checkpoint in 1..checkpoint_id {
            for reason in [
                AdapterAbortReason::PhysicalResult(1),
                AdapterAbortReason::PhysicalResult(u32::MAX),
                AdapterAbortReason::MalformedResponse,
                AdapterAbortReason::TraceRejected,
                AdapterAbortReason::HostUnwind,
            ] {
                let mut frame = certificate
                    .prepare_frame(NonZeroU64::new(checkpoint as u64).unwrap(), checkpoint)
                    .unwrap();
                let initial = frame.resources.clone();
                let decision = SettlementDecision::Abort(reason);
                let first = certificate.settle(&mut frame, decision).unwrap();
                assert!(frame.is_terminal());
                assert!(frame
                    .resources()
                    .iter()
                    .all(|state| *state == SettlementResourceState::Dead));
                let finalized = first
                    .performed_actions()
                    .iter()
                    .map(|action| match action {
                        SettlementAction::Finalize { owner_ordinal } => *owner_ordinal,
                        SettlementAction::Publish { .. } => panic!("abort cannot publish"),
                    })
                    .collect::<Vec<_>>();
                assert_eq!(finalized, reverse_non_dead(&initial));
                assert_eq!(
                    finalized.iter().copied().collect::<BTreeSet<_>>().len(),
                    finalized.len()
                );
                certificate
                    .validate_receipt(frame.invocation(), first.receipt())
                    .unwrap();

                let replay = certificate.settle(&mut frame, decision).unwrap();
                assert_eq!(replay.receipt(), first.receipt());
                assert!(replay.performed_actions().is_empty());
            }
        }
    }
}

#[test]
fn accepted_outcomes_are_exact_and_owned_publication_is_unique() {
    let scalar_states = vec![
        SettlementResourceState::Live,
        SettlementResourceState::Dead,
        SettlementResourceState::Live,
    ];
    let failure_states = vec![
        SettlementResourceState::Dead,
        SettlementResourceState::Live,
        SettlementResourceState::Live,
    ];
    let owned_states = vec![
        SettlementResourceState::Live,
        SettlementResourceState::ProvisionalResult,
        SettlementResourceState::Dead,
    ];
    let certificate = certificate(vec![
        SettlementCheckpointSpec::new(
            1,
            scalar_states.clone(),
            Some(SettlementOutcome::ScalarSuccess),
            reverse_non_dead(&scalar_states),
            reverse_live(&scalar_states),
        ),
        SettlementCheckpointSpec::new(
            2,
            failure_states.clone(),
            Some(SettlementOutcome::SemanticFailure),
            reverse_non_dead(&failure_states),
            reverse_live(&failure_states),
        ),
        SettlementCheckpointSpec::new(
            3,
            owned_states.clone(),
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            reverse_non_dead(&owned_states),
            reverse_live(&owned_states),
        ),
    ]);

    for (checkpoint, outcome) in [
        (1, SettlementOutcome::ScalarSuccess),
        (2, SettlementOutcome::SemanticFailure),
        (3, SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
    ] {
        let mut frame = certificate
            .prepare_frame(NonZeroU64::new(checkpoint as u64).unwrap(), checkpoint)
            .unwrap();
        let application = certificate
            .settle(&mut frame, SettlementDecision::Accept(outcome))
            .unwrap();
        let published = application
            .receipt()
            .dispositions()
            .iter()
            .filter(|disposition| **disposition == SettlementDisposition::Published)
            .count();
        assert_eq!(published, usize::from(checkpoint == 3));
        assert_eq!(application.receipt().active_finalizers(), 0);
    }

    let mut wrong = certificate
        .prepare_frame(NonZeroU64::new(9).unwrap(), 3)
        .unwrap();
    let before = snapshot(&wrong);
    assert_eq!(
        certificate.settle(
            &mut wrong,
            SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 0 })
        ),
        Err(SettlementError::DecisionNotAdmitted)
    );
    assert_eq!(snapshot(&wrong), before);
}

#[test]
fn accepted_outcomes_exhaust_every_owner_liveness_combination() {
    for resource_count in 1..=6_usize {
        let mut specs = Vec::new();
        let mut outcomes = Vec::new();
        let combinations = 2_usize.pow(resource_count as u32);
        for mut encoded in 0..combinations {
            let mut states = Vec::new();
            for _ in 0..resource_count {
                states.push(if encoded.is_multiple_of(2) {
                    SettlementResourceState::Live
                } else {
                    SettlementResourceState::Dead
                });
                encoded /= 2;
            }
            for outcome in [
                SettlementOutcome::ScalarSuccess,
                SettlementOutcome::SemanticFailure,
            ] {
                let checkpoint = u32::try_from(specs.len() + 1).unwrap();
                specs.push(SettlementCheckpointSpec::new(
                    checkpoint,
                    states.clone(),
                    Some(outcome),
                    reverse_non_dead(&states),
                    reverse_live(&states),
                ));
                outcomes.push(outcome);
            }
        }
        for result_ordinal in 0..resource_count {
            let other_combinations = 2_usize.pow((resource_count - 1) as u32);
            for mut encoded in 0..other_combinations {
                let mut states = Vec::new();
                for ordinal in 0..resource_count {
                    if ordinal == result_ordinal {
                        states.push(SettlementResourceState::ProvisionalResult);
                    } else {
                        states.push(if encoded.is_multiple_of(2) {
                            SettlementResourceState::Live
                        } else {
                            SettlementResourceState::Dead
                        });
                        encoded /= 2;
                    }
                }
                let outcome = SettlementOutcome::OwnedSuccess {
                    owner_ordinal: result_ordinal as u32,
                };
                let checkpoint = u32::try_from(specs.len() + 1).unwrap();
                specs.push(SettlementCheckpointSpec::new(
                    checkpoint,
                    states.clone(),
                    Some(outcome),
                    reverse_non_dead(&states),
                    reverse_live(&states),
                ));
                outcomes.push(outcome);
            }
        }

        let certificate = certificate(specs);
        for (index, outcome) in outcomes.iter().copied().enumerate() {
            let checkpoint = u32::try_from(index + 1).unwrap();
            let mut frame = certificate
                .prepare_frame(NonZeroU64::new((index + 1) as u64).unwrap(), checkpoint)
                .unwrap();
            let application = certificate
                .settle(&mut frame, SettlementDecision::Accept(outcome))
                .unwrap();
            let expected_published = match outcome {
                SettlementOutcome::OwnedSuccess { owner_ordinal } => Some(owner_ordinal),
                SettlementOutcome::ScalarSuccess | SettlementOutcome::SemanticFailure => None,
            };
            let actual_published = application
                .receipt()
                .dispositions()
                .iter()
                .enumerate()
                .filter(|(_, disposition)| **disposition == SettlementDisposition::Published)
                .map(|(ordinal, _)| ordinal as u32)
                .collect::<Vec<_>>();
            assert_eq!(
                actual_published,
                expected_published.into_iter().collect::<Vec<_>>()
            );
            assert!(frame.resources().iter().all(|state| matches!(
                state,
                SettlementResourceState::Dead | SettlementResourceState::Published
            )));
            assert_eq!(application.receipt().active_finalizers(), 0);
        }
    }
}

#[test]
fn conflicting_terminal_decision_is_nonmutating() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        Some(SettlementOutcome::ScalarSuccess),
        reverse_non_dead(&states),
        reverse_live(&states),
    )]);
    let mut frame = certificate
        .prepare_frame(NonZeroU64::new(1).unwrap(), 1)
        .unwrap();
    certificate
        .settle(
            &mut frame,
            SettlementDecision::Abort(AdapterAbortReason::MalformedResponse),
        )
        .unwrap();
    let terminal = snapshot(&frame);
    assert_eq!(
        certificate.settle(
            &mut frame,
            SettlementDecision::Accept(SettlementOutcome::ScalarSuccess)
        ),
        Err(SettlementError::ConflictingTerminalDecision)
    );
    assert_eq!(snapshot(&frame), terminal);
}

#[test]
fn certificate_builder_rejects_every_structural_ambiguity() {
    let valid = SettlementCheckpointSpec::new(
        1,
        vec![SettlementResourceState::Live],
        Some(SettlementOutcome::ScalarSuccess),
        vec![0],
        vec![0],
    );
    for function in ["", "token\0settlement"] {
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new(function),
                CONTRACT,
                1,
                vec![valid.clone()]
            ),
            Err(SettlementError::InvalidFunctionIdentity)
        );
    }
    assert_eq!(
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            [0; 32],
            1,
            vec![valid.clone()]
        ),
        Err(SettlementError::ZeroRecoveryContract)
    );
    assert_eq!(
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            CONTRACT,
            0,
            vec![valid.clone()]
        ),
        Err(SettlementError::ResourceCountOutOfBounds)
    );
    let mut noncanonical = valid.clone();
    noncanonical.checkpoint = 2;
    assert_eq!(
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            CONTRACT,
            1,
            vec![noncanonical]
        ),
        Err(SettlementError::NonCanonicalCheckpoint)
    );
    let duplicate = SettlementCheckpointSpec::new(
        1,
        vec![SettlementResourceState::Live],
        None,
        vec![0, 0],
        Vec::new(),
    );
    assert_eq!(
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            CONTRACT,
            1,
            vec![duplicate]
        ),
        Err(SettlementError::InvalidCleanupOrder)
    );
    let terminal = SettlementCheckpointSpec::new(
        1,
        vec![SettlementResourceState::Published],
        None,
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            CONTRACT,
            1,
            vec![terminal]
        ),
        Err(SettlementError::InvalidCheckpointState)
    );
    let multiple = SettlementCheckpointSpec::new(
        1,
        vec![
            SettlementResourceState::ProvisionalResult,
            SettlementResourceState::ProvisionalResult,
        ],
        None,
        vec![1, 0],
        Vec::new(),
    );
    assert_eq!(
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            CONTRACT,
            2,
            vec![multiple]
        ),
        Err(SettlementError::MultipleProvisionalResults)
    );
}

#[test]
fn malformed_receipts_fail_independent_validation() {
    let states = vec![
        SettlementResourceState::Live,
        SettlementResourceState::ProvisionalResult,
    ];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
        reverse_non_dead(&states),
        reverse_live(&states),
    )]);
    let invocation = NonZeroU64::new(7).unwrap();
    let mut frame = certificate.prepare_frame(invocation, 1).unwrap();
    let valid = certificate
        .settle(
            &mut frame,
            SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
        )
        .unwrap()
        .receipt;

    let mut mutations = Vec::new();
    let mut receipt = valid.clone();
    receipt.schema = "wrong";
    mutations.push((receipt, SettlementError::ReceiptSchemaMismatch));
    let mut receipt = valid.clone();
    receipt.recovery_contract[0] ^= 1;
    mutations.push((receipt, SettlementError::ReceiptBindingMismatch));
    let mut receipt = valid.clone();
    receipt.certificate_fingerprint[0] ^= 1;
    mutations.push((receipt, SettlementError::ReceiptBindingMismatch));
    let mut receipt = valid.clone();
    receipt.invocation = NonZeroU64::new(8).unwrap();
    mutations.push((receipt, SettlementError::ReceiptBindingMismatch));
    let mut receipt = valid.clone();
    receipt.actions.swap(0, 1);
    mutations.push((receipt, SettlementError::ReceiptActionMismatch));
    let mut receipt = valid.clone();
    receipt.dispositions[1] = SettlementDisposition::Dead;
    mutations.push((receipt, SettlementError::ReceiptDispositionMismatch));
    let mut receipt = valid.clone();
    receipt.active_finalizers = 1;
    mutations.push((receipt, SettlementError::NotQuiescent));

    for (receipt, expected) in mutations {
        assert_eq!(
            certificate.validate_receipt(invocation, &receipt),
            Err(expected)
        );
    }
}

#[test]
fn canonical_certificate_and_receipt_are_deterministic_and_domain_separated() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        Some(SettlementOutcome::ScalarSuccess),
        reverse_non_dead(&states),
        reverse_live(&states),
    )]);
    assert_eq!(certificate.canonical_json(), certificate.canonical_json());
    assert_eq!(certificate.fingerprint(), certificate.fingerprint());

    let mut frame = certificate
        .prepare_frame(NonZeroU64::new(1).unwrap(), 1)
        .unwrap();
    let receipt = certificate
        .settle(
            &mut frame,
            SettlementDecision::Accept(SettlementOutcome::ScalarSuccess),
        )
        .unwrap()
        .receipt;
    assert_eq!(receipt.canonical_json(), receipt.canonical_json());
    assert_eq!(receipt.fingerprint(), receipt.fingerprint());
    assert_ne!(certificate.fingerprint(), receipt.fingerprint());
}

#[test]
fn physical_result_zero_is_never_an_abort_reason() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        None,
        reverse_non_dead(&states),
        Vec::new(),
    )]);
    let mut frame = certificate
        .prepare_frame(NonZeroU64::new(1).unwrap(), 1)
        .unwrap();
    let before = snapshot(&frame);
    assert_eq!(
        certificate.settle(
            &mut frame,
            SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(0))
        ),
        Err(SettlementError::InvalidAbortReason)
    );
    assert_eq!(snapshot(&frame), before);
}

#[test]
fn deterministic_frame_preparation_is_not_a_uniqueness_reservation() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        None,
        reverse_non_dead(&states),
        Vec::new(),
    )]);
    let invocation = NonZeroU64::new(77).unwrap();
    let first = certificate.prepare_frame(invocation, 1).unwrap();
    let second = certificate.prepare_frame(invocation, 1).unwrap();
    assert_eq!(snapshot(&first), snapshot(&second));
    assert!(!first.is_terminal());
    assert!(!second.is_terminal());
}

#[test]
fn strict_progress_graph_rejects_bad_starts_edges_transitions_and_orphans() {
    let live = SettlementCheckpointSpec::new(
        1,
        vec![SettlementResourceState::Live],
        None,
        vec![0],
        Vec::new(),
    );
    let dead = SettlementCheckpointSpec::new(
        2,
        vec![SettlementResourceState::Dead],
        None,
        Vec::new(),
        Vec::new(),
    );
    let terminal = SettlementCheckpointSpec::new(
        3,
        vec![SettlementResourceState::Dead],
        Some(SettlementOutcome::ScalarSuccess),
        Vec::new(),
        Vec::new(),
    );
    let finalize = SettlementProgressEdge::new(
        1,
        2,
        SettlementProgressAction::Finalize { owner_ordinal: 0 },
    );
    let certify = SettlementProgressEdge::new(
        2,
        3,
        SettlementProgressAction::CertifyOutcome {
            trace_evidence: [7; 32],
        },
    );
    let build = |starts, edges| {
        NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new("token.progress"),
            CONTRACT,
            1,
            vec![live.clone(), dead.clone(), terminal.clone()],
            starts,
            edges,
        )
    };
    assert!(build(vec![1], vec![finalize, certify]).is_ok());
    assert_eq!(
        build(vec![1, 2], vec![finalize, certify]),
        Err(SettlementError::InvalidProgressStart)
    );
    assert_eq!(
        build(vec![1], vec![certify]),
        Err(SettlementError::NonCanonicalProgressEdge)
    );
    assert_eq!(
        build(vec![1], vec![finalize]),
        Err(SettlementError::UnreachableCheckpoint)
    );
    assert_eq!(
        build(vec![1], vec![finalize, finalize, certify]),
        Err(SettlementError::NonCanonicalProgressEdge)
    );
    assert_eq!(
        build(
            vec![1],
            vec![
                SettlementProgressEdge::new(
                    1,
                    2,
                    SettlementProgressAction::StageOwnedResult { owner_ordinal: 0 },
                ),
                certify,
            ],
        ),
        Err(SettlementError::InvalidProgressTransition)
    );
    assert_eq!(
        build(
            vec![1],
            vec![
                finalize,
                SettlementProgressEdge::new(
                    2,
                    3,
                    SettlementProgressAction::Finalize { owner_ordinal: 0 },
                ),
            ],
        ),
        Err(SettlementError::InvalidProgressTransition)
    );

    let finalize_order_counterexample = NativeSettlementCertificate::try_new_with_progress(
        DeclarationId::new("token.progress.finalize-order"),
        CONTRACT,
        3,
        vec![
            SettlementCheckpointSpec::new(
                1,
                vec![SettlementResourceState::Live; 3],
                None,
                vec![2, 1, 0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                2,
                vec![
                    SettlementResourceState::Live,
                    SettlementResourceState::Live,
                    SettlementResourceState::Dead,
                ],
                None,
                vec![0, 1],
                Vec::new(),
            ),
        ],
        vec![1],
        vec![SettlementProgressEdge::new(
            1,
            2,
            SettlementProgressAction::Finalize { owner_ordinal: 2 },
        )],
    );
    assert_eq!(
        finalize_order_counterexample,
        Err(SettlementError::InvalidProgressTransition)
    );

    let skipped_live_counterexample = NativeSettlementCertificate::try_new_with_progress(
        DeclarationId::new("token.progress.finalize-skips-live"),
        CONTRACT,
        2,
        vec![
            SettlementCheckpointSpec::new(
                1,
                vec![SettlementResourceState::Live; 2],
                None,
                vec![0, 1],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                2,
                vec![SettlementResourceState::Live, SettlementResourceState::Dead],
                None,
                vec![0],
                Vec::new(),
            ),
        ],
        vec![1],
        vec![SettlementProgressEdge::new(
            1,
            2,
            SettlementProgressAction::Finalize { owner_ordinal: 1 },
        )],
    );
    assert_eq!(
        skipped_live_counterexample,
        Err(SettlementError::InvalidProgressTransition)
    );

    let stage_order_counterexample = NativeSettlementCertificate::try_new_with_progress(
        DeclarationId::new("token.progress.stage-order"),
        CONTRACT,
        2,
        vec![
            SettlementCheckpointSpec::new(
                1,
                vec![SettlementResourceState::Live; 2],
                None,
                vec![1, 0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                2,
                vec![
                    SettlementResourceState::Live,
                    SettlementResourceState::ProvisionalResult,
                ],
                None,
                vec![0, 1],
                Vec::new(),
            ),
        ],
        vec![1],
        vec![SettlementProgressEdge::new(
            1,
            2,
            SettlementProgressAction::StageOwnedResult { owner_ordinal: 1 },
        )],
    );
    assert_eq!(
        stage_order_counterexample,
        Err(SettlementError::InvalidProgressTransition)
    );

    let certify_accept_counterexample = NativeSettlementCertificate::try_new_with_progress(
        DeclarationId::new("token.progress.certify-order"),
        CONTRACT,
        2,
        vec![
            SettlementCheckpointSpec::new(
                1,
                vec![SettlementResourceState::Live; 2],
                None,
                vec![1, 0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                2,
                vec![SettlementResourceState::Live; 2],
                Some(SettlementOutcome::ScalarSuccess),
                vec![1, 0],
                vec![0, 1],
            ),
        ],
        vec![1],
        vec![SettlementProgressEdge::new(
            1,
            2,
            SettlementProgressAction::CertifyOutcome {
                trace_evidence: [9; 32],
            },
        )],
    );
    assert_eq!(
        certify_accept_counterexample,
        Err(SettlementError::InvalidProgressTransition)
    );
}

fn complete_phase_transaction(
    certificate: &NativeSettlementCertificate,
    transaction: &mut NativeSettlementTransaction,
    decision: SettlementDecision,
) -> NativeSettlementReceipt {
    certificate
        .lock_transaction_decision(transaction, decision)
        .unwrap();
    certificate
        .lock_transaction_decision(transaction, decision)
        .unwrap();
    loop {
        match transaction.actions.get(transaction.next_action).copied() {
            Some(SettlementAction::Finalize { owner_ordinal }) => {
                let ticket = certificate.begin_next_finalizer(transaction).unwrap();
                assert_eq!(
                    ticket.action(),
                    SettlementAction::Finalize { owner_ordinal }
                );
                assert_eq!(
                    transaction.resources[owner_ordinal as usize],
                    SettlementResourceState::Finalizing
                );
                certificate.complete_finalizer(transaction, ticket).unwrap();
                assert_eq!(
                    transaction.resources[owner_ordinal as usize],
                    SettlementResourceState::Dead
                );
            }
            Some(SettlementAction::Publish { owner_ordinal }) => {
                assert!(transaction.resources.iter().all(|state| {
                    !matches!(
                        state,
                        SettlementResourceState::Live
                            | SettlementResourceState::Finalizing
                            | SettlementResourceState::Published
                    )
                }));
                certificate.publish_owned_candidate(transaction).unwrap();
                assert_eq!(
                    transaction.resources[owner_ordinal as usize],
                    SettlementResourceState::Published
                );
            }
            None => break,
        }
    }
    let candidate = certificate.finish_provider_settlement(transaction).unwrap();
    assert!(candidate.performed_actions().is_empty());
    assert_eq!(
        transaction.phase(),
        SettlementTransactionPhase::ProviderSettled
    );
    let provider_replay = certificate
        .replay_provider_candidate(transaction, decision)
        .unwrap();
    assert_eq!(provider_replay.receipt(), candidate.receipt());
    assert!(provider_replay.performed_actions().is_empty());
    let receipt = candidate.receipt().clone();
    let committed = certificate
        .commit_provider_receipt(transaction, &receipt)
        .unwrap();
    assert_eq!(committed.receipt(), &receipt);
    assert!(committed.performed_actions().is_empty());
    assert_eq!(
        transaction.phase(),
        SettlementTransactionPhase::ReceiptCommitted
    );
    let duplicate = certificate
        .commit_provider_receipt(transaction, &receipt)
        .unwrap();
    assert_eq!(
        duplicate.receipt().canonical_json(),
        receipt.canonical_json()
    );
    assert!(duplicate.performed_actions().is_empty());
    let replay = certificate
        .replay_committed_receipt(transaction, decision)
        .unwrap();
    assert_eq!(replay.receipt().canonical_json(), receipt.canonical_json());
    assert!(replay.performed_actions().is_empty());
    receipt
}

#[test]
fn phase_machine_exhausts_decisions_and_preserves_receipt_kats() {
    let states = vec![
        SettlementResourceState::Live,
        SettlementResourceState::ProvisionalResult,
        SettlementResourceState::Dead,
    ];
    let owned_certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
        reverse_non_dead(&states),
        reverse_live(&states),
    )]);
    let decisions = [
        SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
        SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(1)),
        SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(u32::MAX)),
        SettlementDecision::Abort(AdapterAbortReason::MalformedResponse),
        SettlementDecision::Abort(AdapterAbortReason::TraceRejected),
        SettlementDecision::Abort(AdapterAbortReason::HostUnwind),
    ];
    for (index, decision) in decisions.into_iter().enumerate() {
        let invocation = NonZeroU64::new((index + 1) as u64).unwrap();
        let mut phased = owned_certificate
            .prepare_start_transaction(invocation)
            .unwrap();
        let phased_receipt = complete_phase_transaction(&owned_certificate, &mut phased, decision);

        let mut legacy = owned_certificate.prepare_start_frame(invocation).unwrap();
        let legacy_receipt = owned_certificate
            .settle(&mut legacy, decision)
            .unwrap()
            .receipt;
        assert_eq!(
            phased_receipt.canonical_json(),
            legacy_receipt.canonical_json()
        );
        assert_eq!(phased_receipt.fingerprint(), legacy_receipt.fingerprint());
    }

    for outcome in [
        SettlementOutcome::ScalarSuccess,
        SettlementOutcome::SemanticFailure,
    ] {
        let states = vec![SettlementResourceState::Live, SettlementResourceState::Dead];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(outcome),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        let decision = SettlementDecision::Accept(outcome);
        let mut transaction = certificate
            .prepare_start_transaction(NonZeroU64::new(99).unwrap())
            .unwrap();
        complete_phase_transaction(&certificate, &mut transaction, decision);
    }
}

#[test]
fn phase_machine_covers_every_certified_checkpoint_and_abort_reason() {
    let checkpoints = vec![
        SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Live; 3],
            None,
            vec![2, 1, 0],
            Vec::new(),
        ),
        SettlementCheckpointSpec::new(
            2,
            vec![
                SettlementResourceState::Live,
                SettlementResourceState::Live,
                SettlementResourceState::Dead,
            ],
            None,
            vec![1, 0],
            Vec::new(),
        ),
        SettlementCheckpointSpec::new(
            3,
            vec![
                SettlementResourceState::Live,
                SettlementResourceState::ProvisionalResult,
                SettlementResourceState::Dead,
            ],
            None,
            vec![1, 0],
            Vec::new(),
        ),
        SettlementCheckpointSpec::new(
            4,
            vec![
                SettlementResourceState::Live,
                SettlementResourceState::ProvisionalResult,
                SettlementResourceState::Dead,
            ],
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            vec![1, 0],
            vec![0],
        ),
    ];
    let progress = [
        SettlementProgressAction::Finalize { owner_ordinal: 2 },
        SettlementProgressAction::StageOwnedResult { owner_ordinal: 1 },
        SettlementProgressAction::CertifyOutcome {
            trace_evidence: [0x39; 32],
        },
    ];
    let certificate = NativeSettlementCertificate::try_new_with_progress(
        DeclarationId::new("token.phase-corpus"),
        CONTRACT,
        3,
        checkpoints,
        vec![1],
        vec![
            SettlementProgressEdge::new(1, 2, progress[0]),
            SettlementProgressEdge::new(2, 3, progress[1]),
            SettlementProgressEdge::new(3, 4, progress[2]),
        ],
    )
    .unwrap();
    let aborts = [
        AdapterAbortReason::PhysicalResult(1),
        AdapterAbortReason::PhysicalResult(u32::MAX),
        AdapterAbortReason::MalformedResponse,
        AdapterAbortReason::TraceRejected,
        AdapterAbortReason::HostUnwind,
    ];
    let mut invocation = 1_u64;
    for checkpoint in 1..=4_u32 {
        let accepts = (checkpoint == 4).then_some(SettlementDecision::Accept(
            SettlementOutcome::OwnedSuccess { owner_ordinal: 1 },
        ));
        for decision in aborts
            .into_iter()
            .map(SettlementDecision::Abort)
            .chain(accepts)
        {
            let mut transaction = certificate
                .prepare_start_transaction(NonZeroU64::new(invocation).unwrap())
                .unwrap();
            invocation += 1;
            for action in progress.iter().take((checkpoint - 1) as usize) {
                certificate
                    .advance_transaction(&mut transaction, *action)
                    .unwrap();
            }
            assert_eq!(transaction.checkpoint(), checkpoint);
            let receipt = complete_phase_transaction(&certificate, &mut transaction, decision);
            assert_eq!(receipt.checkpoint(), checkpoint);
            assert_eq!(receipt.decision(), decision);
        }
    }
}

#[test]
fn unwind_is_phase_aware_and_finalizer_uncertainty_is_absorbing() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        Some(SettlementOutcome::ScalarSuccess),
        reverse_non_dead(&states),
        reverse_live(&states),
    )]);
    let mut before_lock = certificate
        .prepare_start_transaction(NonZeroU64::new(1).unwrap())
        .unwrap();
    let host_unwind = SettlementDecision::Abort(AdapterAbortReason::HostUnwind);
    assert_eq!(
        certificate.observe_transaction_unwind(&mut before_lock),
        Ok(host_unwind)
    );
    assert_eq!(
        before_lock.phase(),
        SettlementTransactionPhase::DecisionLocked
    );

    let decision = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
    let mut after_lock = certificate
        .prepare_start_transaction(NonZeroU64::new(2).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut after_lock, decision)
        .unwrap();
    assert_eq!(
        certificate.observe_transaction_unwind(&mut after_lock),
        Ok(decision)
    );

    let ticket = certificate.begin_next_finalizer(&mut after_lock).unwrap();
    assert_eq!(
        certificate.observe_transaction_unwind(&mut after_lock),
        Err(SettlementError::FinalizerCompletionUncertain)
    );
    assert_eq!(after_lock.phase(), SettlementTransactionPhase::Quarantined);
    assert_eq!(
        certificate.complete_finalizer(&mut after_lock, ticket),
        Err(SettlementError::TransactionQuarantined)
    );
    assert_eq!(
        certificate.observe_transaction_unwind(&mut after_lock),
        Err(SettlementError::TransactionQuarantined)
    );

    let mut settled = certificate
        .prepare_start_transaction(NonZeroU64::new(3).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut settled, decision)
        .unwrap();
    let ticket = certificate.begin_next_finalizer(&mut settled).unwrap();
    certificate
        .complete_finalizer(&mut settled, ticket)
        .unwrap();
    let candidate = certificate
        .finish_provider_settlement(&mut settled)
        .unwrap();
    assert_eq!(
        certificate.observe_transaction_unwind(&mut settled),
        Ok(decision)
    );
    let receipt = candidate.receipt().clone();
    certificate
        .commit_provider_receipt(&mut settled, &receipt)
        .unwrap();
    assert_eq!(
        certificate.observe_transaction_unwind(&mut settled),
        Ok(decision)
    );
}

#[test]
fn conflicts_and_skips_monotonically_quarantine_without_publication() {
    let states = vec![
        SettlementResourceState::Live,
        SettlementResourceState::ProvisionalResult,
    ];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
        reverse_non_dead(&states),
        reverse_live(&states),
    )]);
    let accept = SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 });
    let abort = SettlementDecision::Abort(AdapterAbortReason::MalformedResponse);

    let mut conflict = certificate
        .prepare_start_transaction(NonZeroU64::new(1).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut conflict, accept)
        .unwrap();
    assert_eq!(
        certificate.lock_transaction_decision(&mut conflict, abort),
        Err(SettlementError::ConflictingLockedDecision)
    );
    assert_eq!(conflict.phase(), SettlementTransactionPhase::Quarantined);

    let mut skipped = certificate
        .prepare_start_transaction(NonZeroU64::new(2).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut skipped, accept)
        .unwrap();
    assert_eq!(
        certificate.publish_owned_candidate(&mut skipped),
        Err(SettlementError::PublishActionNotPending)
    );
    assert_eq!(skipped.phase(), SettlementTransactionPhase::Quarantined);
    assert!(!skipped
        .resources()
        .contains(&SettlementResourceState::Published));

    let mut unfinished = certificate
        .prepare_start_transaction(NonZeroU64::new(3).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut unfinished, abort)
        .unwrap();
    assert_eq!(
        certificate.finish_provider_settlement(&mut unfinished),
        Err(SettlementError::SettlementActionsIncomplete)
    );
    assert_eq!(unfinished.phase(), SettlementTransactionPhase::Quarantined);

    let mut in_progress = certificate
        .prepare_start_transaction(NonZeroU64::new(4).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut in_progress, abort)
        .unwrap();
    let _ticket = certificate.begin_next_finalizer(&mut in_progress).unwrap();
    assert_eq!(
        certificate.lock_transaction_decision(&mut in_progress, abort),
        Err(SettlementError::InvalidSettlementPhase)
    );
    assert_eq!(in_progress.phase(), SettlementTransactionPhase::Quarantined);

    let terminal_states = vec![SettlementResourceState::Dead];
    let terminal_certificate = self::certificate(vec![SettlementCheckpointSpec::new(
        1,
        terminal_states,
        Some(SettlementOutcome::ScalarSuccess),
        Vec::new(),
        Vec::new(),
    )]);
    let terminal_decision = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
    let mut provider_settled = terminal_certificate
        .prepare_start_transaction(NonZeroU64::new(5).unwrap())
        .unwrap();
    terminal_certificate
        .lock_transaction_decision(&mut provider_settled, terminal_decision)
        .unwrap();
    terminal_certificate
        .finish_provider_settlement(&mut provider_settled)
        .unwrap();
    assert_eq!(
        terminal_certificate.lock_transaction_decision(&mut provider_settled, abort),
        Err(SettlementError::ConflictingLockedDecision)
    );
    assert_eq!(
        provider_settled.phase(),
        SettlementTransactionPhase::Quarantined
    );
}

#[test]
fn forged_progress_and_cross_certificate_calls_quarantine_exact_transaction() {
    let start = SettlementCheckpointSpec::new(
        1,
        vec![SettlementResourceState::Live],
        None,
        vec![0],
        Vec::new(),
    );
    let end = SettlementCheckpointSpec::new(
        2,
        vec![SettlementResourceState::Dead],
        None,
        Vec::new(),
        Vec::new(),
    );
    let action = SettlementProgressAction::Finalize { owner_ordinal: 0 };
    let make_certificate = |function| {
        NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new(function),
            CONTRACT,
            1,
            vec![start.clone(), end.clone()],
            vec![1],
            vec![SettlementProgressEdge::new(1, 2, action)],
        )
        .unwrap()
    };
    let certificate = make_certificate("token.progress-a");
    let other = make_certificate("token.progress-b");

    let mut forged_action = certificate
        .prepare_start_transaction(NonZeroU64::new(1).unwrap())
        .unwrap();
    assert_eq!(
        certificate.advance_transaction(
            &mut forged_action,
            SettlementProgressAction::StageOwnedResult { owner_ordinal: 0 },
        ),
        Err(SettlementError::ProgressActionNotAdmitted)
    );
    assert_eq!(
        forged_action.phase(),
        SettlementTransactionPhase::Quarantined
    );

    let mut forged_state = certificate
        .prepare_start_transaction(NonZeroU64::new(2).unwrap())
        .unwrap();
    forged_state.resources[0] = SettlementResourceState::Dead;
    assert_eq!(
        certificate.advance_transaction(&mut forged_state, action),
        Err(SettlementError::FrameStateMismatch)
    );
    assert_eq!(
        forged_state.phase(),
        SettlementTransactionPhase::Quarantined
    );

    let mut cross_bound = certificate
        .prepare_start_transaction(NonZeroU64::new(3).unwrap())
        .unwrap();
    assert_eq!(
        other.lock_transaction_decision(
            &mut cross_bound,
            SettlementDecision::Abort(AdapterAbortReason::HostUnwind),
        ),
        Err(SettlementError::FrameBindingMismatch)
    );
    assert_eq!(cross_bound.phase(), SettlementTransactionPhase::Quarantined);
    assert_eq!(
        certificate.observe_transaction_unwind(&mut cross_bound),
        Err(SettlementError::TransactionQuarantined)
    );
}

#[test]
fn stale_cross_binding_and_duplicate_finalizer_completion_never_retry() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        None,
        reverse_non_dead(&states),
        Vec::new(),
    )]);
    let decision = SettlementDecision::Abort(AdapterAbortReason::TraceRejected);
    let mut first = certificate
        .prepare_start_transaction(NonZeroU64::new(1).unwrap())
        .unwrap();
    let mut second = certificate
        .prepare_start_transaction(NonZeroU64::new(2).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut first, decision)
        .unwrap();
    certificate
        .lock_transaction_decision(&mut second, decision)
        .unwrap();
    let first_ticket = certificate.begin_next_finalizer(&mut first).unwrap();
    let second_ticket = certificate.begin_next_finalizer(&mut second).unwrap();
    assert_eq!(
        certificate.complete_finalizer(&mut first, second_ticket),
        Err(SettlementError::FinalizerTicketMismatch)
    );
    assert_eq!(first.phase(), SettlementTransactionPhase::Quarantined);
    certificate.mark_finalizer_uncertain(&mut second).unwrap();
    assert_eq!(second.phase(), SettlementTransactionPhase::Quarantined);
    assert_eq!(
        certificate.complete_finalizer(&mut first, first_ticket),
        Err(SettlementError::TransactionQuarantined)
    );

    let mut duplicate = certificate
        .prepare_start_transaction(NonZeroU64::new(3).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut duplicate, decision)
        .unwrap();
    let ticket = certificate.begin_next_finalizer(&mut duplicate).unwrap();
    let forged_duplicate = SettlementFinalizerTicket {
        certificate_fingerprint: ticket.certificate_fingerprint,
        invocation: ticket.invocation,
        checkpoint: ticket.checkpoint,
        decision: ticket.decision,
        action_index: ticket.action_index,
        owner_ordinal: ticket.owner_ordinal,
    };
    certificate
        .complete_finalizer(&mut duplicate, ticket)
        .unwrap();
    assert_eq!(
        certificate.complete_finalizer(&mut duplicate, forged_duplicate),
        Err(SettlementError::InvalidSettlementPhase)
    );
    assert_eq!(duplicate.phase(), SettlementTransactionPhase::Quarantined);
}

#[test]
fn unwind_at_every_finalizer_index_preserves_prefix_current_and_suffix() {
    let states = vec![SettlementResourceState::Live; 4];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        None,
        reverse_non_dead(&states),
        Vec::new(),
    )]);
    let decision = SettlementDecision::Abort(AdapterAbortReason::MalformedResponse);
    for interruption in 0..4_usize {
        let invocation = NonZeroU64::new((interruption + 1) as u64).unwrap();
        let mut transaction = certificate.prepare_start_transaction(invocation).unwrap();
        certificate
            .lock_transaction_decision(&mut transaction, decision)
            .unwrap();
        for _ in 0..interruption {
            let ticket = certificate.begin_next_finalizer(&mut transaction).unwrap();
            certificate
                .complete_finalizer(&mut transaction, ticket)
                .unwrap();
        }
        let ticket = certificate.begin_next_finalizer(&mut transaction).unwrap();
        let current_owner = 3 - interruption;
        for owner in 0..4_usize {
            let expected = match owner.cmp(&current_owner) {
                std::cmp::Ordering::Greater => SettlementResourceState::Dead,
                std::cmp::Ordering::Equal => SettlementResourceState::Finalizing,
                std::cmp::Ordering::Less => SettlementResourceState::Live,
            };
            assert_eq!(transaction.resources[owner], expected);
        }
        let resources_at_uncertainty = transaction.resources.clone();
        assert_eq!(
            certificate.observe_transaction_unwind(&mut transaction),
            Err(SettlementError::FinalizerCompletionUncertain)
        );
        assert_eq!(transaction.resources, resources_at_uncertainty);
        assert_eq!(transaction.phase(), SettlementTransactionPhase::Quarantined);

        let mut legacy = certificate.prepare_start_frame(invocation).unwrap();
        let receipt = certificate.settle(&mut legacy, decision).unwrap().receipt;
        assert_eq!(
            certificate.advance_transaction(
                &mut transaction,
                SettlementProgressAction::Finalize { owner_ordinal: 0 },
            ),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.lock_transaction_decision(&mut transaction, decision),
            Err(SettlementError::TransactionQuarantined)
        );
        assert!(matches!(
            certificate.begin_next_finalizer(&mut transaction),
            Err(SettlementError::TransactionQuarantined)
        ));
        assert_eq!(
            certificate.complete_finalizer(&mut transaction, ticket),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.mark_finalizer_uncertain(&mut transaction),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.publish_owned_candidate(&mut transaction),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.finish_provider_settlement(&mut transaction),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.commit_provider_receipt(&mut transaction, &receipt),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.replay_provider_candidate(&mut transaction, decision),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.replay_committed_receipt(&mut transaction, decision),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(transaction.resources, resources_at_uncertainty);
    }
}

#[test]
fn hostile_internal_mutations_are_detected_in_every_irreversible_phase() {
    let states = vec![SettlementResourceState::Live];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states.clone(),
        None,
        reverse_non_dead(&states),
        Vec::new(),
    )]);
    let decision = SettlementDecision::Abort(AdapterAbortReason::TraceRejected);

    let mut locked = certificate
        .prepare_start_transaction(NonZeroU64::new(1).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut locked, decision)
        .unwrap();
    locked.next_action = 1;
    assert_eq!(
        certificate.lock_transaction_decision(&mut locked, decision),
        Err(SettlementError::FrameStateMismatch)
    );
    assert_eq!(locked.phase(), SettlementTransactionPhase::Quarantined);

    let mut in_progress = certificate
        .prepare_start_transaction(NonZeroU64::new(2).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut in_progress, decision)
        .unwrap();
    let _ticket = certificate.begin_next_finalizer(&mut in_progress).unwrap();
    in_progress.next_action = 1;
    assert_eq!(
        certificate.observe_transaction_unwind(&mut in_progress),
        Err(SettlementError::FrameStateMismatch)
    );
    assert_eq!(in_progress.phase(), SettlementTransactionPhase::Quarantined);

    let terminal_certificate = self::certificate(vec![SettlementCheckpointSpec::new(
        1,
        vec![SettlementResourceState::Dead],
        Some(SettlementOutcome::ScalarSuccess),
        Vec::new(),
        Vec::new(),
    )]);
    let accepted = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
    let mut provider = terminal_certificate
        .prepare_start_transaction(NonZeroU64::new(3).unwrap())
        .unwrap();
    terminal_certificate
        .lock_transaction_decision(&mut provider, accepted)
        .unwrap();
    terminal_certificate
        .finish_provider_settlement(&mut provider)
        .unwrap();
    provider
        .candidate_receipt
        .as_mut()
        .unwrap()
        .active_finalizers = 1;
    assert_eq!(
        terminal_certificate.replay_provider_candidate(&mut provider, accepted),
        Err(SettlementError::FrameStateMismatch)
    );
    assert_eq!(provider.phase(), SettlementTransactionPhase::Quarantined);

    let mut committed = terminal_certificate
        .prepare_start_transaction(NonZeroU64::new(4).unwrap())
        .unwrap();
    complete_phase_transaction(&terminal_certificate, &mut committed, accepted);
    committed
        .committed_receipt
        .as_mut()
        .unwrap()
        .recovery_contract[0] ^= 1;
    assert_eq!(
        terminal_certificate.replay_committed_receipt(&mut committed, accepted),
        Err(SettlementError::FrameStateMismatch)
    );
    assert_eq!(committed.phase(), SettlementTransactionPhase::Quarantined);
}

#[test]
fn receipt_commit_is_exact_and_quarantine_preserves_terminal_evidence() {
    let states = vec![SettlementResourceState::Dead];
    let certificate = certificate(vec![SettlementCheckpointSpec::new(
        1,
        states,
        Some(SettlementOutcome::ScalarSuccess),
        Vec::new(),
        Vec::new(),
    )]);
    let decision = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
    let mut transaction = certificate
        .prepare_start_transaction(NonZeroU64::new(7).unwrap())
        .unwrap();
    certificate
        .lock_transaction_decision(&mut transaction, decision)
        .unwrap();
    let candidate = certificate
        .finish_provider_settlement(&mut transaction)
        .unwrap()
        .receipt;
    let mut forged = candidate.clone();
    forged.recovery_contract[0] ^= 1;
    assert_eq!(
        certificate.commit_provider_receipt(&mut transaction, &forged),
        Err(SettlementError::ReceiptCommitMismatch)
    );
    assert_eq!(transaction.phase(), SettlementTransactionPhase::Quarantined);
    assert_eq!(transaction.candidate_receipt(), Some(&candidate));

    let mut committed = certificate
        .prepare_start_transaction(NonZeroU64::new(8).unwrap())
        .unwrap();
    complete_phase_transaction(&certificate, &mut committed, decision);
    let preserved = committed.committed_receipt().unwrap().canonical_json();
    assert_eq!(
        certificate.lock_transaction_decision(
            &mut committed,
            SettlementDecision::Abort(AdapterAbortReason::MalformedResponse),
        ),
        Err(SettlementError::ConflictingLockedDecision)
    );
    assert_eq!(committed.phase(), SettlementTransactionPhase::Quarantined);
    assert_eq!(
        committed.committed_receipt().unwrap().canonical_json(),
        preserved
    );
}

#[test]
fn phase_transactions_are_start_only_and_linear() {
    let checkpoints = vec![
        SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Live],
            None,
            vec![0],
            Vec::new(),
        ),
        SettlementCheckpointSpec::new(
            2,
            vec![SettlementResourceState::Dead],
            None,
            Vec::new(),
            Vec::new(),
        ),
    ];
    let snapshots = certificate(checkpoints);
    assert!(matches!(
        snapshots.prepare_start_transaction(NonZeroU64::new(1).unwrap()),
        Err(SettlementError::InvalidProgressStart)
    ));
    assert_not_impl!(NativeSettlementTransaction, Clone);
    assert_not_impl!(NativeSettlementTransaction, fmt::Debug);
    assert_not_impl!(NativeSettlementTransaction, fmt::Display);
    assert_not_impl!(SettlementFinalizerTicket, Clone);
    assert_not_impl!(SettlementFinalizerTicket, fmt::Debug);
    assert_not_impl!(SettlementFinalizerTicket, fmt::Display);
}

#[test]
fn frame_traits_are_deliberately_linear_and_nonformatting() {
    assert_not_impl!(NativeSettlementFrame, Clone);
    assert_not_impl!(NativeSettlementFrame, fmt::Debug);
    assert_not_impl!(NativeSettlementFrame, fmt::Display);
}

#[test]
fn certificate_bounds_accept_exact_limits_and_reject_zero_over_and_excess_work() {
    fn specs(resources: usize, checkpoints: usize) -> Vec<SettlementCheckpointSpec> {
        (1..=checkpoints)
            .map(|checkpoint| {
                SettlementCheckpointSpec::new(
                    u32::try_from(checkpoint).unwrap(),
                    vec![SettlementResourceState::Dead; resources],
                    None,
                    Vec::new(),
                    Vec::new(),
                )
            })
            .collect()
    }
    let build = |resources, checkpoints| {
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.bounds"),
            CONTRACT,
            resources,
            specs(resources.max(1), checkpoints),
        )
    };
    assert!(build(1, 1).is_ok());
    assert!(build(MAX_SETTLEMENT_RESOURCES, 1).is_ok());
    assert_eq!(build(0, 1), Err(SettlementError::ResourceCountOutOfBounds));
    assert_eq!(
        build(MAX_SETTLEMENT_RESOURCES + 1, 1),
        Err(SettlementError::ResourceCountOutOfBounds)
    );
    assert!(build(1, MAX_SETTLEMENT_CHECKPOINTS).is_ok());
    assert_eq!(
        build(1, 0),
        Err(SettlementError::CheckpointCountOutOfBounds)
    );
    assert_eq!(
        build(1, MAX_SETTLEMENT_CHECKPOINTS + 1),
        Err(SettlementError::CheckpointCountOutOfBounds)
    );
    assert!(build(1_000, 1_000).is_ok());
    assert_eq!(
        build(1_001, 1_000),
        Err(SettlementError::WorkBudgetExceeded)
    );
}
