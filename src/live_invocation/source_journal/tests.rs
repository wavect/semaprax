use super::*;

#[derive(Default)]
struct Store {
    document: String,
    fail_at: Option<(u64, bool)>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        if self.fail_at == Some((generation, false)) {
            return Err(CheckpointStoreError);
        }
        self.document = document.to_owned();
        if self.fail_at == Some((generation, true)) {
            return Err(CheckpointStoreError);
        }
        Ok(())
    }
}

fn hash(label: &str) -> String {
    digest(b"semaprax.source-journal.test\0", label.as_bytes())
}
fn seed() -> SourceInvocationSeed {
    SourceInvocationSeed {
        lifecycle_digest: hash("lifecycle"),
        source_revision: hash("revision"),
        deployment_binding: hash("deployment"),
        task: b"bounded source test".to_vec(),
        task_budget: 3,
        proposal_schema_digest: hash("schema"),
        response_limit: 4,
        max_iterations: 2,
        max_stages: 8,
        max_attempts: 2,
        max_steps_per_stage: 100,
        max_total_steps: 800,
        ceiling: 6,
        reservation_units: 3,
        unit: "operator_unit_v1".to_owned(),
        clock_domain: "restart_stable_ms_v1".to_owned(),
        initial_millis: 0,
        deadline_millis: 10,
        program_root: None,
    }
}
fn binding() -> SourceInvocationBinding {
    SourceInvocationBinding::bind(seed()).unwrap()
}
fn intent(binding: &SourceInvocationBinding, attempt: u32) -> SourceJournalEntry {
    let request_digest = hash("request");
    let prompt_digest = hash("prompt");
    SourceJournalEntry::AttemptIntent {
        turn: 0,
        attempt,
        attempt_digest: binding.attempt_digest(0, attempt, &request_digest, &prompt_digest, 12),
        request_digest,
        prompt_digest,
        request_bytes: 12,
        reserved_units: binding.reservation_units(),
        response_limit: binding.response_limit(),
    }
}
fn begin(sink: &mut SourceCheckpointSink<'_>) {
    sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    sink.append_at(
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash("state"),
            observation: hash("observation"),
            feedback: hash("feedback"),
        },
        1,
    )
    .unwrap();
}
fn settled(response: &[u8]) -> SourceJournalEntry {
    SourceJournalEntry::AttemptSettled {
        turn: 0,
        attempt: 0,
        response: response.to_vec(),
        response_digest: source_response_digest(response),
    }
}

#[test]
fn canonical_full_roundtrip_replays_exact_response_and_effect() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        sink.append_at(intent(&binding, 0), 2).unwrap();
        sink.append_at(settled(b"abcd"), 3).unwrap();
        sink.append_at(
            SourceJournalEntry::ProposalAdmitted {
                turn: 0,
                attempt: 0,
                proposal_digest: hash("proposal"),
            },
            3,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::AuthorizationConsumed {
                turn: 0,
                attempt: 0,
                grant_digest: hash("grant"),
            },
            3,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::EffectIntent {
                turn: 0,
                attempt: 0,
                operation: "read.fixture".into(),
                request_digest: hash("effect-request"),
            },
            4,
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
            5,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::Transition {
                turn: 0,
                attempt: 0,
                case: SourceTransitionCase::Complete,
                carrier_digest: hash("carrier"),
            },
            6,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::TerminalOutcome {
                turn: Some(0),
                status: SourceTerminalStatus::Complete,
                carrier_digest: Some(hash("carrier")),
            },
            6,
        )
        .unwrap();
        assert_eq!(sink.generation(), 10);
    }
    let recovered = recover_source_checkpoint(&store.document, &binding).unwrap();
    assert_eq!(recovered.generation(), 10);
    assert_eq!(recovered.committed_reserved_units(), 3);
    assert_eq!(recovered.last_checked_millis(), 6);
    assert!(!recovered.is_uncertain());
    assert!(
        matches!(&recovered.entries()[3], SourceJournalEntry::AttemptSettled { response, .. }
        if response == b"abcd")
    );
}

#[test]
fn request_limit_response_limit_and_digest_are_checked_against_binding() {
    let binding = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    begin(&mut sink);
    for request_bytes in [0, MAX_SOURCE_REQUEST_BYTES + 1] {
        let mut entry = intent(&binding, 0);
        if let SourceJournalEntry::AttemptIntent {
            request_bytes: bytes,
            ..
        } = &mut entry
        {
            *bytes = request_bytes;
        }
        assert_eq!(sink.append_at(entry, 2), Err(SourceJournalError::Order));
    }
    let mut wrong_cap = intent(&binding, 0);
    if let SourceJournalEntry::AttemptIntent { response_limit, .. } = &mut wrong_cap {
        *response_limit += 1;
    }
    assert_eq!(sink.append_at(wrong_cap, 2), Err(SourceJournalError::Order));
    let mut wrong_digest = intent(&binding, 0);
    if let SourceJournalEntry::AttemptIntent { attempt_digest, .. } = &mut wrong_digest {
        *attempt_digest = hash("wrong");
    }
    assert_eq!(
        sink.append_at(wrong_digest, 2),
        Err(SourceJournalError::Order)
    );
    let mut max_request = intent(&binding, 0);
    if let SourceJournalEntry::AttemptIntent {
        attempt_digest,
        request_bytes,
        ..
    } = &mut max_request
    {
        *request_bytes = MAX_SOURCE_REQUEST_BYTES;
        *attempt_digest =
            binding.attempt_digest(0, 0, &hash("request"), &hash("prompt"), *request_bytes);
    }
    sink.append_at(max_request, 2).unwrap();
    sink.append_at(settled(b"abcd"), 3).unwrap();
    assert_eq!(sink.generation(), 4);

    let mut second = Store::default();
    let second_binding = binding.clone();
    let mut sink = SourceCheckpointSink::new(&mut second, binding);
    begin(&mut sink);
    sink.append_at(intent(&second_binding, 0), 2).unwrap();
    assert_eq!(
        sink.append_at(settled(b"abcde"), 3),
        Err(SourceJournalError::Order)
    );
    let mut bad_hash = settled(b"abcd");
    if let SourceJournalEntry::AttemptSettled {
        response_digest, ..
    } = &mut bad_hash
    {
        *response_digest = hash("other");
    }
    assert_eq!(sink.append_at(bad_hash, 3), Err(SourceJournalError::Order));
}

#[test]
fn effect_operation_observation_and_terminal_correlations_are_strict() {
    let binding = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    begin(&mut sink);
    sink.append_at(intent(&binding, 0), 2).unwrap();
    sink.append_at(settled(b"ok"), 3).unwrap();
    sink.append_at(
        SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: hash("proposal"),
        },
        3,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::AuthorizationConsumed {
            turn: 0,
            attempt: 0,
            grant_digest: hash("grant"),
        },
        3,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::EffectIntent {
            turn: 0,
            attempt: 0,
            operation: "read.one".into(),
            request_digest: hash("effect"),
        },
        4,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::EffectObserved {
                turn: 0,
                attempt: 0,
                operation: "read.two".into(),
                observation: b"x".to_vec(),
                observation_digest: source_effect_digest(b"x"),
            },
            5
        ),
        Err(SourceJournalError::Order)
    );
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::EffectObserved {
                turn: 0,
                attempt: 0,
                operation: "read.one".into(),
                observation: b"x".to_vec(),
                observation_digest: hash("wrong"),
            },
            5
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::EffectObserved {
            turn: 0,
            attempt: 0,
            operation: "read.one".into(),
            observation: b"x".to_vec(),
            observation_digest: source_effect_digest(b"x"),
        },
        5,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::Transition {
            turn: 0,
            attempt: 0,
            case: SourceTransitionCase::Fail,
            carrier_digest: hash("failed-carrier"),
        },
        6,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: Some(0),
                status: SourceStopStatus::DeadlineExceeded,
                reason: SourceStopReason::DeadlineExceeded,
            },
            7
        ),
        Err(SourceJournalError::Order)
    );
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::TerminalOutcome {
                turn: Some(0),
                status: SourceTerminalStatus::Fail,
                carrier_digest: Some(hash("other-carrier")),
            },
            7
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::TerminalOutcome {
            turn: Some(0),
            status: SourceTerminalStatus::Fail,
            carrier_digest: Some(hash("failed-carrier")),
        },
        7,
    )
    .unwrap();
}

#[test]
fn failed_attempt_requires_matching_stop_and_closed_terminal() {
    let binding = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    begin(&mut sink);
    sink.append_at(intent(&binding, 0), 2).unwrap();
    sink.append_at(
        SourceJournalEntry::AttemptFailed {
            turn: 0,
            attempt: 0,
            reason: SourceAttemptFailure::ProviderError,
            attempted_bytes: 4,
        },
        3,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::Stop {
                turn: Some(0),
                attempt: Some(0),
                status: SourceStopStatus::DeadlineExceeded,
                reason: SourceStopReason::DeadlineExceeded,
            },
            4
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: SourceStopStatus::ModelFailed,
            reason: SourceStopReason::ModelFailed,
        },
        4,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::TerminalOutcome {
                turn: Some(0),
                status: SourceTerminalStatus::ModelFailed,
                carrier_digest: Some(hash("fabricated")),
            },
            4
        ),
        Err(SourceJournalError::Order)
    );
    sink.append_at(
        SourceJournalEntry::TerminalOutcome {
            turn: Some(0),
            status: SourceTerminalStatus::ModelFailed,
            carrier_digest: None,
        },
        4,
    )
    .unwrap();
}

#[test]
fn canonical_unknown_keys_generation_chain_and_binding_are_checked() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        sink.append_at(intent(&binding, 0), 2).unwrap();
    }
    assert!(recover_source_checkpoint(&store.document, &binding)
        .unwrap()
        .is_uncertain());
    let extra = store
        .document
        .replacen("\"entries\":[", "\"extra\":0,\"entries\":[", 1);
    assert_eq!(
        recover_source_checkpoint(&extra, &binding).unwrap_err(),
        SourceJournalError::Malformed
    );
    let bad_generation = store
        .document
        .replacen("\"generation\":3", "\"generation\":4", 1);
    assert_eq!(
        recover_source_checkpoint(&bad_generation, &binding).unwrap_err(),
        SourceJournalError::Generation
    );
    let bad_chain =
        store
            .document
            .replacen("\"last_checked_millis\":2", "\"last_checked_millis\":3", 1);
    assert_eq!(
        recover_source_checkpoint(&bad_chain, &binding).unwrap_err(),
        SourceJournalError::Chain
    );
    let spaces = store.document.replacen("\"schema\":", "\"schema\" :", 1);
    assert_eq!(
        recover_source_checkpoint(&spaces, &binding).unwrap_err(),
        SourceJournalError::Malformed
    );
    let mut changed = seed();
    changed.max_steps_per_stage += 1;
    let other = SourceInvocationBinding::bind(changed).unwrap();
    assert_eq!(
        recover_source_checkpoint(&store.document, &other).unwrap_err(),
        SourceJournalError::Binding
    );
    assert_eq!(
        recover_source_checkpoint(&"x".repeat(MAX_SOURCE_DOCUMENT_BYTES + 1), &binding)
            .unwrap_err(),
        SourceJournalError::Capacity
    );
}

#[test]
fn store_ack_loss_poisons_sink_and_recovery_keeps_uncertain_charge() {
    let binding = binding();
    let mut store = Store {
        fail_at: Some((3, true)),
        ..Store::default()
    };
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        assert_eq!(
            sink.append_at(intent(&binding, 0), 2),
            Err(SourceJournalError::Store(CheckpointStoreError))
        );
        assert!(sink.poisoned());
        assert_eq!(sink.generation(), 2);
        assert_eq!(
            sink.append_at(intent(&binding, 0), 3),
            Err(SourceJournalError::Poisoned)
        );
    }
    let recovered = recover_source_checkpoint(&store.document, &binding).unwrap();
    assert_eq!(recovered.generation(), 3);
    assert_eq!(recovered.committed_reserved_units(), 3);
    assert!(recovered.is_uncertain());
    assert!(matches!(
        SourceCheckpointSink::resume(&mut store, recovered),
        Err(SourceJournalError::Uncertain)
    ));

    let mut no_write = Store {
        fail_at: Some((3, false)),
        ..Store::default()
    };
    {
        let mut sink = SourceCheckpointSink::new(&mut no_write, binding.clone());
        begin(&mut sink);
        assert_eq!(
            sink.append_at(intent(&binding, 0), 2),
            Err(SourceJournalError::Store(CheckpointStoreError))
        );
        assert!(sink.poisoned());
    }
    assert_eq!(
        recover_source_checkpoint(&no_write.document, &binding)
            .unwrap()
            .committed_reserved_units(),
        0
    );
}

#[test]
fn policy_capacity_and_stage_budget_refuse_before_effect_intent() {
    let mut limited = seed();
    limited.max_stages = 3; // initialize, observe, authorize; no reducer capacity
    let binding = SourceInvocationBinding::bind(limited).unwrap();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    begin(&mut sink);
    sink.append_at(intent(&binding, 0), 2).unwrap();
    sink.append_at(settled(b"ok"), 3).unwrap();
    sink.append_at(
        SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: hash("proposal"),
        },
        3,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::AuthorizationConsumed {
            turn: 0,
            attempt: 0,
            grant_digest: hash("grant"),
        },
        3,
    )
    .unwrap();
    assert_eq!(
        sink.append_at(
            SourceJournalEntry::EffectIntent {
                turn: 0,
                attempt: 0,
                operation: "read.fixture".into(),
                request_digest: hash("effect"),
            },
            4
        ),
        Err(SourceJournalError::Order)
    );
}

#[test]
fn malformed_retry_charges_each_intent_and_cannot_exceed_bound_attempts() {
    let binding = binding();
    let mut entries = vec![
        SourceJournalEntry::RunOpened,
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash("state"),
            observation: hash("observation"),
            feedback: hash("feedback"),
        },
        intent(&binding, 0),
        settled(b"bad"),
        SourceJournalEntry::ProposalRefused {
            turn: 0,
            attempt: 0,
            reason: SourceProposalRefusal::MalformedDecode,
        },
        intent(&binding, 1),
    ];
    assert_eq!(validate::validate(&binding, &entries), Ok(6));
    entries.push(SourceJournalEntry::AttemptSettled {
        turn: 0,
        attempt: 1,
        response: b"bad2".to_vec(),
        response_digest: source_response_digest(b"bad2"),
    });
    entries.push(SourceJournalEntry::ProposalRefused {
        turn: 0,
        attempt: 1,
        reason: SourceProposalRefusal::MalformedDecode,
    });
    assert_eq!(validate::validate(&binding, &entries), Ok(6));
    entries.push(intent(&binding, 2));
    assert_eq!(
        validate::validate(&binding, &entries),
        Err(SourceJournalError::Order)
    );
    entries.pop();
    entries.push(SourceJournalEntry::Stop {
        turn: Some(0),
        attempt: Some(1),
        status: SourceStopStatus::ModelFailed,
        reason: SourceStopReason::ModelFailed,
    });
    entries.push(SourceJournalEntry::TerminalOutcome {
        turn: Some(0),
        status: SourceTerminalStatus::ModelFailed,
        carrier_digest: None,
    });
    assert_eq!(validate::validate(&binding, &entries), Ok(6));
}

#[test]
fn recovered_effect_intent_is_uncertain_and_cannot_resume_dispatch() {
    let binding = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        begin(&mut sink);
        sink.append_at(intent(&binding, 0), 2).unwrap();
        sink.append_at(settled(b"ok"), 3).unwrap();
        sink.append_at(
            SourceJournalEntry::ProposalAdmitted {
                turn: 0,
                attempt: 0,
                proposal_digest: hash("proposal"),
            },
            3,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::AuthorizationConsumed {
                turn: 0,
                attempt: 0,
                grant_digest: hash("grant"),
            },
            3,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::EffectIntent {
                turn: 0,
                attempt: 0,
                operation: "read.fixture".to_owned(),
                request_digest: hash("effect"),
            },
            4,
        )
        .unwrap();
    }
    let recovered = recover_source_checkpoint(&store.document, &binding).unwrap();
    assert!(recovered.is_uncertain());
    assert_eq!(recovered.committed_reserved_units(), 3);
    assert!(matches!(
        SourceCheckpointSink::resume(&mut store, recovered),
        Err(SourceJournalError::Uncertain)
    ));
}

#[test]
fn binding_and_prefix_capacity_reject_invalid_configuration_and_overflow() {
    let mut invalid = seed();
    invalid.response_limit = MAX_SOURCE_RESPONSE_BYTES + 1;
    assert_eq!(
        SourceInvocationBinding::bind(invalid),
        Err(SourceJournalError::Binding)
    );
    let mut invalid = seed();
    invalid.max_attempts = MAX_SOURCE_ATTEMPTS + 1;
    assert_eq!(
        SourceInvocationBinding::bind(invalid),
        Err(SourceJournalError::Binding)
    );
    let mut invalid = seed();
    invalid.max_total_steps = 0;
    assert_eq!(
        SourceInvocationBinding::bind(invalid),
        Err(SourceJournalError::Binding)
    );
    let mut invalid = seed();
    invalid.deadline_millis = invalid.initial_millis;
    assert_eq!(
        SourceInvocationBinding::bind(invalid),
        Err(SourceJournalError::Binding)
    );

    let binding = binding();
    let oversized_entries = vec![SourceJournalEntry::RunOpened; MAX_SOURCE_ENTRIES + 1];
    assert_eq!(
        validate::validate(&binding, &oversized_entries),
        Err(SourceJournalError::Capacity)
    );

    let mut high = seed();
    high.ceiling = i64::MAX;
    high.reservation_units = i64::MAX;
    let high_binding = SourceInvocationBinding::bind(high).unwrap();
    let entries = vec![
        SourceJournalEntry::RunOpened,
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash("state"),
            observation: hash("observation"),
            feedback: hash("feedback"),
        },
        intent(&high_binding, 0),
        settled(b"bad"),
        SourceJournalEntry::ProposalRefused {
            turn: 0,
            attempt: 0,
            reason: SourceProposalRefusal::MalformedDecode,
        },
        intent(&high_binding, 1),
    ];
    assert_eq!(
        validate::validate(&high_binding, &entries),
        Err(SourceJournalError::Capacity)
    );
}
