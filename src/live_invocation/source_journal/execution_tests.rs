use super::*;

#[derive(Default)]
struct Store(String);
impl CheckpointStore for Store {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.0 = document.to_owned();
        Ok(())
    }
}
fn hash(label: &str) -> String {
    digest(b"semaprax.source-execution.test\0", label.as_bytes())
}
fn seed() -> SourceInvocationSeed {
    SourceInvocationSeed {
        lifecycle_digest: hash("lifecycle"),
        source_revision: hash("source"),
        deployment_binding: hash("deployment"),
        task: b"execution fixture".to_vec(),
        task_budget: 4,
        proposal_schema_digest: hash("proposal-schema"),
        response_limit: 32,
        max_iterations: 2,
        max_stages: 7,
        max_attempts: 2,
        max_steps_per_stage: 10,
        max_total_steps: 100,
        ceiling: 6,
        reservation_units: 3,
        unit: "operator_unit_v1".into(),
        clock_domain: "restart_stable_ms_v1".into(),
        initial_millis: 0,
        deadline_millis: 100,
        program_root: None,
    }
}
fn binding() -> SourceInvocationBinding {
    SourceInvocationBinding::bind_execution(seed(), &hash("evaluator-profile")).unwrap()
}
fn stage(role: SourceStageRole, attempt: Option<u32>) -> SourceJournalEntry {
    SourceJournalEntry::StageReservation {
        turn: 0,
        attempt,
        role,
        fuel: 10,
    }
}
fn begin(sink: &mut SourceCheckpointSink<'_>) {
    sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    sink.append_at(stage(SourceStageRole::Initialize, None), 1)
        .unwrap();
    sink.append_at(stage(SourceStageRole::Observe, None), 2)
        .unwrap();
    sink.append_at(
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash("state"),
            observation: hash("observation"),
            feedback: hash("feedback"),
        },
        3,
    )
    .unwrap();
}
fn intent(binding: &SourceInvocationBinding) -> SourceJournalEntry {
    let request_digest = hash("request");
    let prompt_digest = hash("prompt");
    SourceJournalEntry::AttemptIntent {
        turn: 0,
        attempt: 0,
        attempt_digest: binding.attempt_digest(0, 0, &request_digest, &prompt_digest, 12),
        request_digest,
        prompt_digest,
        request_bytes: 12,
        reserved_units: 3,
        response_limit: 32,
    }
}
fn admitted(sink: &mut SourceCheckpointSink<'_>, binding: &SourceInvocationBinding) {
    sink.append_at(intent(binding), 4).unwrap();
    sink.append_at(
        SourceJournalEntry::AttemptSettled {
            turn: 0,
            attempt: 0,
            response: b"proposal".to_vec(),
            response_digest: source_response_digest(b"proposal"),
        },
        5,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: hash("admitted"),
        },
        6,
    )
    .unwrap();
    sink.append_at(stage(SourceStageRole::Authorize, Some(0)), 7)
        .unwrap();
    sink.append_at(
        SourceJournalEntry::AuthorizationConsumed {
            turn: 0,
            attempt: 0,
            grant_digest: hash("grant"),
        },
        8,
    )
    .unwrap();
}
fn summary(role: SourceStageRole) -> SourceStageSummary {
    SourceStageSummary {
        role,
        function_id: hash(role.as_str()),
        outcome: SourceStageOutcome::Returned,
        steps_used: 2,
    }
}

#[test]
fn execution_profile_is_distinct_and_cross_version_decode_refuses() {
    let v1 = SourceInvocationBinding::bind(seed()).unwrap();
    let v2 = binding();
    assert_ne!(v1.invocation(), v2.invocation());
    assert_eq!(
        v2.evaluator_profile(),
        Some(hash("evaluator-profile").as_str())
    );
    assert_eq!(v2.max_steps_per_stage(), Some(10));
    assert_eq!(v2.max_total_steps(), Some(100));
    assert_eq!(
        SourceInvocationBinding::bind_execution(seed(), "ambient-profile"),
        Err(SourceJournalError::Binding)
    );
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, v2.clone());
        sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    }
    assert!(store.0.contains(SOURCE_EXECUTION_JOURNAL_SCHEMA));
    assert_eq!(
        recover_source_checkpoint(&store.0, &v1).unwrap_err(),
        SourceJournalError::Malformed
    );
    let mut old = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut old, v1);
        sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    }
    assert_eq!(
        recover_source_checkpoint(&old.0, &v2).unwrap_err(),
        SourceJournalError::Malformed
    );
}

#[test]
fn original_stages_are_mandatory_and_replay_fuel_is_charged_in_order() {
    let binding = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding);
    sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::TurnObserved {
                turn: 0,
                state: hash("state"),
                observation: hash("observation"),
                feedback: hash("feedback"),
            },
            1
        ),
        Err(SourceJournalError::Order)
    );
    assert_eq!(
        sink.append_at(stage(SourceStageRole::Observe, None), 1),
        Err(SourceJournalError::Order)
    );
    sink.append_at(stage(SourceStageRole::Initialize, None), 1)
        .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::ReplayStageReservation {
                replay: 0,
                causal_seq: 2,
                role: SourceStageRole::Initialize,
                fuel: 10,
            },
            1
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::ReplayStageReservation {
            replay: 0,
            causal_seq: 1,
            role: SourceStageRole::Initialize,
            fuel: 10,
        },
        1,
    )
    .unwrap();
    sink.append_at(stage(SourceStageRole::Observe, None), 2)
        .unwrap();
    sink.append_at(
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash("state"),
            observation: hash("observation"),
            feedback: hash("feedback"),
        },
        3,
    )
    .unwrap();
    assert_eq!(sink.committed_stage_fuel().unwrap(), 30);
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::ReplayStageReservation {
                replay: 1,
                causal_seq: 2,
                role: SourceStageRole::Observe,
                fuel: 10,
            },
            4
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::ReplayStageReservation {
            replay: 1,
            causal_seq: 1,
            role: SourceStageRole::Initialize,
            fuel: 10,
        },
        4,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::ReplayStageReservation {
            replay: 1,
            causal_seq: 3,
            role: SourceStageRole::Observe,
            fuel: 10,
        },
        4,
    )
    .unwrap();
    assert_eq!(sink.committed_stage_fuel().unwrap(), 50);
}

#[test]
fn unresolved_intent_and_terminal_refuse_replay() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        sink.append_at(intent(&binding), 4).unwrap();
        assert_eq!(
            sink.append_at(
                SourceJournalEntry::ReplayStageReservation {
                    replay: 0,
                    causal_seq: 1,
                    role: SourceStageRole::Initialize,
                    fuel: 10,
                },
                5
            ),
            Err(SourceJournalError::Order)
        );
    }
    assert!(recover_source_checkpoint(&store.0, &binding)
        .unwrap()
        .is_uncertain());
}

#[test]
fn observational_usage_is_optional_exactly_once_and_never_changes_charge() {
    let binding = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    begin(&mut sink);
    sink.append_at(intent(&binding), 4).unwrap();
    sink.append_at(
        SourceJournalEntry::AttemptSettled {
            turn: 0,
            attempt: 0,
            response: b"proposal".to_vec(),
            response_digest: source_response_digest(b"proposal"),
        },
        5,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::AttemptUsage {
            turn: 0,
            attempt: 0,
            reported: Some(SourceReportedUsage {
                total: Some(11),
                input: Some(7),
                output: None,
                reasoning: None,
                cache_read: Some(0),
                cache_write: None,
            }),
        },
        5,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::AttemptUsage {
                turn: 0,
                attempt: 0,
                reported: None,
            },
            5
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: hash("admitted"),
        },
        6,
    )
    .unwrap();
    let recovered = sink.checkpoint().unwrap();
    assert_eq!(recovered.committed_reserved_units(), 3);
    assert_eq!(recovered.committed_stage_fuel(), 20);
}

#[test]
fn final_snapshot_binds_partial_progress_and_replays_as_opaque_receipt() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        admitted(&mut sink, &binding);
        sink.append_at(
            SourceJournalEntry::EffectIntent {
                turn: 0,
                attempt: 0,
                operation: "read.fixture".into(),
                request_digest: hash("effect"),
            },
            9,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::EffectObserved {
                turn: 0,
                attempt: 0,
                operation: "read.fixture".into(),
                observation: b"result".to_vec(),
                observation_digest: source_effect_digest(b"result"),
            },
            10,
        )
        .unwrap();
        sink.append_at(stage(SourceStageRole::Reduce, Some(0)), 11)
            .unwrap();
        let carrier = b"true".to_vec();
        sink.append_at(
            SourceJournalEntry::Transition {
                turn: 0,
                attempt: 0,
                case: SourceTransitionCase::Complete,
                carrier_digest: digest(b"semaprax.agent-step.value.v2\0", &carrier),
            },
            12,
        )
        .unwrap();
        let entry = sink
            .terminal_snapshot_entry(
                Some(0),
                SourceTerminalStatus::Complete,
                Some(carrier),
                SourceTerminalEvidenceInput {
                    completed_stages: 4,
                    omitted_stage_rows: 1,
                    stage_rows: vec![
                        summary(SourceStageRole::Initialize),
                        summary(SourceStageRole::Observe),
                        summary(SourceStageRole::Authorize),
                    ],
                    checked_run_evidence: None,
                },
            )
            .unwrap();
        sink.append_at(entry, 12).unwrap();
    }
    let recovered = recover_source_checkpoint(&store.0, &binding).unwrap();
    assert_eq!(recovered.committed_reserved_units(), 3);
    assert_eq!(recovered.committed_stage_fuel(), 40);
    let terminal = recovered.terminal_snapshot().unwrap();
    assert_eq!(terminal.status(), SourceTerminalStatus::Complete);
    assert_eq!(terminal.carrier(), Some(b"true".as_slice()));
    assert!(std::str::from_utf8(terminal.evidence())
        .unwrap()
        .contains("\"omitted_stage_rows\":1"));
}

#[test]
fn replay_and_reducer_preflight_charge_the_same_total_fuel() {
    let mut limited = seed();
    limited.max_total_steps = 30;
    let binding =
        SourceInvocationBinding::bind_execution(limited, &hash("evaluator-profile")).unwrap();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    begin(&mut sink);
    sink.append_at(
        SourceJournalEntry::ReplayStageReservation {
            replay: 0,
            causal_seq: 1,
            role: SourceStageRole::Initialize,
            fuel: 10,
        },
        4,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::ReplayStageReservation {
                replay: 0,
                causal_seq: 2,
                role: SourceStageRole::Observe,
                fuel: 10,
            },
            4
        ),
        Err(SourceJournalError::Capacity)
    );

    let mut effect_store = Store::default();
    let mut effect_sink = SourceCheckpointSink::new(&mut effect_store, binding.clone());
    begin(&mut effect_sink);
    admitted(&mut effect_sink, &binding);
    assert_eq!(effect_sink.committed_stage_fuel().unwrap(), 30);
    assert_eq!(
        effect_sink.append_at(
            SourceJournalEntry::EffectIntent {
                turn: 0,
                attempt: 0,
                operation: "read.fixture".into(),
                request_digest: hash("effect"),
            },
            9
        ),
        Err(SourceJournalError::Capacity)
    );
}

#[test]
fn terminal_snapshot_rejects_noncanonical_evidence_and_bounded_overflow() {
    let binding = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding);
    begin(&mut sink);
    sink.append_at(
        SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: None,
            status: SourceStopStatus::DeadlineExceeded,
            reason: SourceStopReason::DeadlineExceeded,
        },
        4,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(stage(SourceStageRole::Observe, None), 5),
        Err(SourceJournalError::Order)
    );
    let input = SourceTerminalEvidenceInput {
        completed_stages: 2,
        omitted_stage_rows: 0,
        stage_rows: vec![
            summary(SourceStageRole::Initialize),
            summary(SourceStageRole::Observe),
        ],
        checked_run_evidence: None,
    };
    let mut altered = sink
        .terminal_snapshot_entry(
            Some(0),
            SourceTerminalStatus::DeadlineExceeded,
            None,
            input.clone(),
        )
        .unwrap();
    if let SourceJournalEntry::TerminalSnapshot {
        evidence,
        evidence_digest,
        ..
    } = &mut altered
    {
        evidence.push(b' ');
        *evidence_digest = digest(b"semaprax.agent-source-terminal-evidence.v2\0", evidence);
    }
    assert_eq!(
        sink.append_at(altered, 5),
        Err(SourceJournalError::Malformed)
    );
    let oversized = SourceTerminalEvidenceInput {
        checked_run_evidence: Some(vec![b'x'; MAX_SOURCE_TERMINAL_EVIDENCE_BYTES]),
        ..input.clone()
    };
    assert_eq!(
        sink.terminal_snapshot_entry(
            Some(0),
            SourceTerminalStatus::DeadlineExceeded,
            None,
            oversized
        ),
        Err(SourceJournalError::Capacity)
    );
    assert_eq!(
        sink.terminal_snapshot_entry(
            Some(0),
            SourceTerminalStatus::DeadlineExceeded,
            Some(vec![b'x'; MAX_SOURCE_CARRIER_BYTES + 1]),
            input.clone()
        ),
        Err(SourceJournalError::Capacity)
    );
    let terminal = sink
        .terminal_snapshot_entry(Some(0), SourceTerminalStatus::DeadlineExceeded, None, input)
        .unwrap();
    sink.append_at(terminal, 5).unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::ReplayStageReservation {
                replay: 0,
                causal_seq: 1,
                role: SourceStageRole::Initialize,
                fuel: 10,
            },
            6
        ),
        Err(SourceJournalError::Order)
    );
}

#[test]
fn original_stage_cannot_be_reserved_after_stop() {
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding());
    sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    sink.append_at(
        SourceJournalEntry::Stop {
            turn: None,
            attempt: None,
            status: SourceStopStatus::DeadlineExceeded,
            reason: SourceStopReason::DeadlineExceeded,
        },
        1,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(stage(SourceStageRole::Initialize, None), 2),
        Err(SourceJournalError::Order)
    );
}

#[test]
fn decode_phase_can_stop_after_a_settled_response() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        sink.append_at(intent(&binding), 4).unwrap();
        sink.append_at(
            SourceJournalEntry::AttemptSettled {
                turn: 0,
                attempt: 0,
                response: b"invalid proposal".to_vec(),
                response_digest: source_response_digest(b"invalid proposal"),
            },
            5,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: Some(0),
                status: SourceStopStatus::ModelFailed,
                reason: SourceStopReason::ModelFailed,
            },
            6,
        )
        .unwrap();
        let terminal = sink
            .terminal_snapshot_entry(
                Some(0),
                SourceTerminalStatus::ModelFailed,
                None,
                SourceTerminalEvidenceInput {
                    completed_stages: 2,
                    omitted_stage_rows: 0,
                    stage_rows: vec![
                        summary(SourceStageRole::Initialize),
                        summary(SourceStageRole::Observe),
                    ],
                    checked_run_evidence: None,
                },
            )
            .unwrap();
        sink.append_at(terminal, 7).unwrap();
    }
    let checkpoint = recover_source_checkpoint(&store.0, &binding).unwrap();
    assert_eq!(checkpoint.committed_reserved_units(), 3);
    assert_eq!(
        checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::ModelFailed
    );
}

#[test]
fn interrupted_replay_may_only_stop_and_publish_partial_fuel() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        sink.append_at(
            SourceJournalEntry::ReplayStageReservation {
                replay: 0,
                causal_seq: 1,
                role: SourceStageRole::Initialize,
                fuel: 10,
            },
            4,
        )
        .unwrap();
        assert_eq!(
            sink.append_at(intent(&binding), 5),
            Err(SourceJournalError::Order)
        );
        sink.append_at(
            SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: None,
                status: SourceStopStatus::DeadlineExceeded,
                reason: SourceStopReason::DeadlineExceeded,
            },
            5,
        )
        .unwrap();
        assert_eq!(
            sink.append_at(
                SourceJournalEntry::ReplayStageReservation {
                    replay: 0,
                    causal_seq: 2,
                    role: SourceStageRole::Observe,
                    fuel: 10,
                },
                6,
            ),
            Err(SourceJournalError::Order)
        );
        let terminal = sink
            .terminal_snapshot_entry(
                Some(0),
                SourceTerminalStatus::DeadlineExceeded,
                None,
                SourceTerminalEvidenceInput {
                    completed_stages: 2,
                    omitted_stage_rows: 0,
                    stage_rows: vec![
                        summary(SourceStageRole::Initialize),
                        summary(SourceStageRole::Observe),
                    ],
                    checked_run_evidence: None,
                },
            )
            .unwrap();
        sink.append_at(terminal, 6).unwrap();
    }
    let checkpoint = recover_source_checkpoint(&store.0, &binding).unwrap();
    assert_eq!(checkpoint.committed_stage_fuel(), 30);
    assert_eq!(
        checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::DeadlineExceeded
    );
}
