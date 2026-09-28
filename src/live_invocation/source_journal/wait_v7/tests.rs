use super::*;

#[derive(Default)]
struct Store {
    document: String,
    fail: Option<(u64, bool)>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        if self.fail == Some((generation, false)) {
            return Err(CheckpointStoreError);
        }
        self.document = document.to_owned();
        if self.fail == Some((generation, true)) {
            return Err(CheckpointStoreError);
        }
        Ok(())
    }
}
fn hash(label: &str) -> String {
    digest(b"semaprax.source-wait.test\0", label.as_bytes())
}
fn seed() -> SourceInvocationSeed {
    SourceInvocationSeed {
        lifecycle_digest: hash("lifecycle"),
        source_revision: hash("source"),
        deployment_binding: hash("deployment"),
        task: b"wait fixture".to_vec(),
        task_budget: 4,
        proposal_schema_digest: hash("proposal"),
        response_limit: 65_536,
        max_iterations: 2,
        max_stages: 7,
        max_attempts: 2,
        max_steps_per_stage: 10,
        max_total_steps: 1_000_000,
        ceiling: 6,
        reservation_units: 3,
        unit: "operator_unit_v1".into(),
        clock_domain: "restart_stable_ms_v1".into(),
        initial_millis: 0,
        deadline_millis: 100,
        program_root: None,
    }
}
fn ordinary_binding() -> SourceInvocationBinding {
    SourceInvocationBinding::bind_execution(seed(), &hash("evaluator")).unwrap()
}
fn binding() -> SourceInvocationBinding {
    ordinary_binding()
        .with_model_wait_v7(
            SourceModelWaitProfileV7::new(hash("wrapper"), hash("source"), 5).unwrap(),
        )
        .unwrap()
}
fn stage(role: SourceStageRole) -> SourceJournalEntry {
    SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: None,
        role,
        fuel: 10,
    }
}
fn observed() -> SourceJournalEntry {
    SourceJournalEntry::TurnObserved {
        turn: 0,
        state: hash("state"),
        observation: hash("observation"),
        feedback: hash("feedback"),
    }
}
fn begin(sink: &mut SourceCheckpointSink<'_>) {
    for row in [
        SourceJournalEntry::RunOpened,
        stage(SourceStageRole::Initialize),
        stage(SourceStageRole::Observe),
        observed(),
    ] {
        sink.append_at(row, 0).unwrap();
    }
}
fn reserved(
    b: &SourceInvocationBinding,
    phase: SourceModelWaitPhaseV7,
    replay_of: Option<u32>,
) -> SourceModelWaitEntryV7 {
    SourceModelWaitEntryV7::EvaluationReserved {
        turn: 0,
        attempt: 0,
        wait: b.model_wait_id(0, 0).unwrap(),
        phase,
        replay_of,
        fuel: 5,
    }
}
fn prepared(b: &SourceInvocationBinding) -> SourceModelWaitEntryV7 {
    let checkpoint = vec![42; SOURCE_MODEL_WAIT_CHECKPOINT_LIMIT];
    SourceModelWaitEntryV7::Prepared {
        turn: 0,
        attempt: 0,
        wait: b.model_wait_id(0, 0).unwrap(),
        reservation: 4,
        observation_digest: hash("observation-carrier"),
        checkpoint_digest: source_model_wait_checkpoint_digest(&checkpoint),
        checkpoint,
    }
}
fn checked(b: &SourceInvocationBinding, reservation: u32) -> SourceModelWaitEntryV7 {
    let SourceModelWaitEntryV7::Prepared {
        checkpoint_digest, ..
    } = prepared(b)
    else {
        unreachable!()
    };
    SourceModelWaitEntryV7::ReplayChecked {
        turn: 0,
        attempt: 0,
        wait: b.model_wait_id(0, 0).unwrap(),
        reservation,
        original: 5,
        result_digest: checkpoint_digest,
    }
}
fn intent(b: &SourceInvocationBinding) -> SourceJournalEntry {
    let request_digest = hash("request");
    let prompt_digest = hash("prompt");
    SourceJournalEntry::AttemptIntent {
        turn: 0,
        attempt: 0,
        attempt_digest: b.attempt_digest(0, 0, &request_digest, &prompt_digest, 12),
        request_digest,
        prompt_digest,
        request_bytes: 12,
        reserved_units: 3,
        response_limit: b.response_limit(),
    }
}
fn raw(response: Vec<u8>) -> SourceJournalEntry {
    SourceJournalEntry::AttemptSettled {
        turn: 0,
        attempt: 0,
        response_digest: source_response_digest(&response),
        response,
    }
}
fn push_wait(journal: &mut SourceJournal, row: SourceModelWaitEntryV7) {
    journal
        .wait_entries
        .push((journal.combined_len() as u32, row));
}

#[test]
fn combined_inventory_preserves_ordinary_projection_and_waits_do_not_count_as_stages() {
    let b = binding();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, b.clone());
    begin(&mut sink);
    sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, None), 0)
        .unwrap();
    sink.append_wait_at(prepared(&b), 0).unwrap();
    assert_eq!(sink.journal().entries().len(), 4);
    assert_eq!(sink.generation(), 6);
    let inventory = sink.journal().execution_entries_v7();
    assert_eq!(
        inventory.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
        (0..6).collect::<Vec<_>>()
    );
    let fold = sink.journal().execution_fold().unwrap();
    assert_eq!(fold.stages, 2);
    assert_eq!(fold.stage_fuel, 25);
    let ordinary = SourceJournal::new(ordinary_binding());
    assert!(ordinary.execution_entries_v7().is_empty());
}

#[test]
fn interrupted_replay_is_charged_and_only_latest_checked_replay_permits_intent() {
    let b = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, b.clone());
        begin(&mut sink);
        sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, None), 0)
            .unwrap();
        sink.append_wait_at(prepared(&b), 0).unwrap();
        sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, Some(4)), 0)
            .unwrap();
    }
    let recovered = recover_source_checkpoint(&store.document, &b).unwrap();
    assert_eq!(recovered.wait_fuel().unwrap(), 10);
    let mut sink = SourceCheckpointSink::resume(&mut store, recovered).unwrap();
    sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, Some(4)), 0)
        .unwrap();
    assert_eq!(
        sink.preflight_at(&intent(&b), 0),
        Err(SourceJournalError::Order)
    );
    sink.append_wait_at(checked(&b, 7), 0).unwrap();
    assert_eq!(sink.journal().wait_fuel().unwrap(), 15);
    assert_eq!(
        sink.append_wait_at(checked(&b, 7), 0),
        Err(SourceJournalError::Order)
    );
    sink.append_at(intent(&b), 0).unwrap();
    assert_eq!(
        sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, Some(4)), 0),
        Err(SourceJournalError::Order)
    );
    drop(sink);
    let recovered = recover_source_checkpoint(&store.document, &b).unwrap();
    assert!(recovered.is_uncertain());
    assert!(matches!(
        SourceCheckpointSink::resume(&mut store, recovered),
        Err(SourceJournalError::Uncertain)
    ));
}

#[test]
fn reservation_ack_loss_recovery_charges_only_durably_published_work() {
    for after in [false, true] {
        let b = binding();
        let mut store = Store::default();
        {
            let mut sink = SourceCheckpointSink::new(&mut store, b.clone());
            begin(&mut sink);
        }
        store.fail = Some((5, after));
        {
            let recovered = recover_source_checkpoint(&store.document, &b).unwrap();
            let mut sink = SourceCheckpointSink::resume(&mut store, recovered).unwrap();
            assert!(matches!(
                sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, None), 0),
                Err(SourceJournalError::Store(_))
            ));
            assert_eq!(
                sink.append_wait_at(prepared(&b), 0),
                Err(SourceJournalError::Poisoned)
            );
        }
        let recovered = recover_source_checkpoint(&store.document, &b).unwrap();
        assert_eq!(recovered.wait_fuel().unwrap(), if after { 5 } else { 0 });
        assert_eq!(recovered.wait_state(0, 0).unwrap().is_some(), after);
    }
}

#[test]
fn versions_binding_and_checkpoint_metadata_fail_closed() {
    assert_eq!(
        ordinary_binding()
            .with_model_wait_v7(
                SourceModelWaitProfileV7::new(hash("wrapper"), hash("other-source"), 5).unwrap()
            )
            .unwrap_err(),
        SourceJournalError::Binding
    );
    assert_eq!(
        SourceInvocationBinding::bind(seed())
            .unwrap()
            .with_model_wait_v7(
                SourceModelWaitProfileV7::new(hash("wrapper"), hash("source"), 5).unwrap()
            )
            .unwrap_err(),
        SourceJournalError::Binding
    );
    let b = binding();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, b.clone());
        begin(&mut sink);
        sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Start, None), 0)
            .unwrap();
        let mut bad = prepared(&b);
        if let SourceModelWaitEntryV7::Prepared {
            checkpoint_digest, ..
        } = &mut bad
        {
            *checkpoint_digest = hash("wrong");
        }
        assert_eq!(sink.append_wait_at(bad, 0), Err(SourceJournalError::Order));
        sink.append_wait_at(prepared(&b), 0).unwrap();
        assert_eq!(
            sink.append_wait_at(prepared(&b), 0),
            Err(SourceJournalError::Order)
        );
    }
    assert_eq!(
        recover_source_checkpoint(&store.document, &ordinary_binding()).unwrap_err(),
        SourceJournalError::Malformed
    );
    let changed = ordinary_binding()
        .with_model_wait_v7(
            SourceModelWaitProfileV7::new(hash("different-wrapper"), hash("source"), 5).unwrap(),
        )
        .unwrap();
    assert_eq!(
        recover_source_checkpoint(&store.document, &changed).unwrap_err(),
        SourceJournalError::Binding
    );
    let document = store.document.clone();
    let recovered = recover_source_checkpoint(&document, &b).unwrap();
    assert_eq!(
        super::super::wire::encode_envelope(&recovered.journal, recovered.generation).unwrap(),
        document
    );
    // Ordinary v2 encodings remain exactly independent of the opt-in inventory.
    let mut ordinary_store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut ordinary_store, ordinary_binding());
        begin(&mut sink);
    }
    assert!(!ordinary_store.document.contains("wait_"));
    assert!(ordinary_store
        .document
        .contains(SOURCE_EXECUTION_JOURNAL_SCHEMA));
    assert_eq!(
        recover_source_checkpoint(&ordinary_store.document, &b).unwrap_err(),
        SourceJournalError::Malformed
    );
}

#[test]
fn near_capacity_intent_ack_keeps_room_for_maximum_settlement_and_completion() {
    let b = binding();
    let mut journal = SourceJournal::new(b.clone());
    journal.entries.extend([
        SourceJournalEntry::RunOpened,
        stage(SourceStageRole::Initialize),
        stage(SourceStageRole::Observe),
        observed(),
    ]);
    push_wait(
        &mut journal,
        reserved(&b, SourceModelWaitPhaseV7::Start, None),
    );
    push_wait(&mut journal, prepared(&b));
    // Populate only valid, fully checked historical reconstructions. Calculate
    // wire growth incrementally so this boundary regression is linear in bytes.
    let mut bytes = super::super::wire::encode_envelope(&journal, journal.combined_len() as u64)
        .unwrap()
        .len();
    let (allowance, entries) = capacity::outstanding(&journal).unwrap();
    loop {
        let seq = journal.combined_len() as u32;
        let reservation = reserved(&b, SourceModelWaitPhaseV7::Start, Some(4));
        let check = checked(&b, seq);
        let growth =
            wire::encode(&reservation, seq).len() + wire::encode(&check, seq + 1).len() + 2;
        if bytes + growth + allowance + 64 > MAX_SOURCE_DOCUMENT_BYTES
            || journal.combined_len() + 2 + entries > MAX_SOURCE_ENTRIES
        {
            break;
        }
        push_wait(&mut journal, reservation);
        push_wait(&mut journal, check);
        bytes += growth;
    }
    let document =
        super::super::wire::encode_envelope(&journal, journal.combined_len() as u64).unwrap();
    assert!(
        MAX_SOURCE_DOCUMENT_BYTES - document.len() - allowance < 2048
            || MAX_SOURCE_ENTRIES - journal.combined_len() - entries < 2,
        "must actually exercise a capacity boundary"
    );
    capacity::check(&journal, document.len()).unwrap();
    let recovered = recover_source_checkpoint(&document, &b).unwrap();
    let mut store = Store {
        document,
        ..Store::default()
    };
    let mut sink = SourceCheckpointSink::resume(&mut store, recovered).unwrap();
    sink.append_at(intent(&b), 0).unwrap();
    sink.append_at(raw(vec![255; b.response_limit()]), 0)
        .unwrap();
    let resume = sink.generation() as u32;
    sink.append_wait_at(reserved(&b, SourceModelWaitPhaseV7::Resume, None), 0)
        .unwrap();
    sink.append_wait_at(
        SourceModelWaitEntryV7::Completed {
            turn: 0,
            attempt: 0,
            wait: b.model_wait_id(0, 0).unwrap(),
            reservation: resume,
            proposal_digest: hash("proposal-carrier"),
        },
        0,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: hash("proposal-carrier"),
        },
        0,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: SourceStopStatus::Cancelled,
            reason: SourceStopReason::Cancelled,
        },
        0,
    )
    .unwrap();
    assert_eq!(
        capacity::outstanding(sink.journal()).unwrap(),
        (execution::TERMINAL_ROOM_BYTES, 1)
    );
    let terminal = sink
        .terminal_snapshot_entry(
            Some(0),
            SourceTerminalStatus::Cancelled,
            None,
            SourceTerminalEvidenceInput {
                completed_stages: 0,
                omitted_stage_rows: 0,
                stage_rows: Vec::new(),
                checked_run_evidence: None,
            },
        )
        .unwrap();
    let SourceJournalEntry::TerminalSnapshot {
        evidence_digest, ..
    } = &terminal
    else {
        unreachable!()
    };
    let evidence_digest = evidence_digest.clone();
    sink.append_at(terminal, 0).unwrap();
    assert!(sink
        .journal()
        .wait_evidence(&evidence_digest)
        .unwrap()
        .starts_with(b"{\"schema\":\"semaprax.source-model-wait.evidence.v1\""));
    drop(sink);
    assert!(store.document.len() <= MAX_SOURCE_DOCUMENT_BYTES);
    recover_source_checkpoint(&store.document, &b).unwrap();
}

#[test]
fn byte_allowance_shrinks_after_intent_and_settlement_instead_of_reserving_old_rows() {
    let b = binding();
    let mut journal = SourceJournal::new(b.clone());
    journal.entries.extend([
        SourceJournalEntry::RunOpened,
        stage(SourceStageRole::Initialize),
        stage(SourceStageRole::Observe),
        observed(),
    ]);
    push_wait(
        &mut journal,
        reserved(&b, SourceModelWaitPhaseV7::Start, None),
    );
    push_wait(&mut journal, prepared(&b));
    let (before, _) = capacity::outstanding(&journal).unwrap();
    journal.entries.push(intent(&b));
    let (after_intent, _) = capacity::outstanding(&journal).unwrap();
    let intent_size = super::super::wire::encode_entry(&intent(&b), 6).len() + 1;
    assert!(before - after_intent >= intent_size);
    let at_limit = MAX_SOURCE_DOCUMENT_BYTES - after_intent;
    assert_eq!(capacity::check(&journal, at_limit), Ok(()));
    assert_eq!(
        capacity::check(&journal, at_limit + 1),
        Err(SourceJournalError::Capacity)
    );
    journal.entries.push(raw(vec![255; b.response_limit()]));
    let (after_settlement, _) = capacity::outstanding(&journal).unwrap();
    assert!(after_intent - after_settlement >= 2 * b.response_limit());
    let resume = journal.combined_len() as u32;
    push_wait(
        &mut journal,
        reserved(&b, SourceModelWaitPhaseV7::Resume, None),
    );
    push_wait(
        &mut journal,
        SourceModelWaitEntryV7::Completed {
            turn: 0,
            attempt: 0,
            wait: b.model_wait_id(0, 0).unwrap(),
            reservation: resume,
            proposal_digest: hash("proposal"),
        },
    );
    let (after_completed, _) = capacity::outstanding(&journal).unwrap();
    assert!(after_completed < after_settlement);
}

#[test]
fn wait_fuel_limit_refuses_reservation_before_checkpoint_ack() {
    let mut limits = seed();
    limits.max_total_steps = 24;
    let b = SourceInvocationBinding::bind_execution(limits, &hash("evaluator"))
        .unwrap()
        .with_model_wait_v7(
            SourceModelWaitProfileV7::new(hash("wrapper"), hash("source"), 5).unwrap(),
        )
        .unwrap();
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, b.clone());
    begin(&mut sink);
    assert_eq!(
        sink.preflight_wait_at(&reserved(&b, SourceModelWaitPhaseV7::Start, None), 0),
        Err(SourceJournalError::Capacity)
    );
    assert_eq!(sink.generation(), 4);
    assert_eq!(sink.journal().wait_fuel().unwrap(), 0);
}
