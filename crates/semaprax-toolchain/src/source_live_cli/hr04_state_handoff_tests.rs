use super::*;

fn stage_count(document: &serde_json::Value, role: &str) -> usize {
    document["entries"]
        .as_array()
        .expect("canonical source journal entries")
        .iter()
        .filter(|entry| entry["kind"] == "stage_reservation" && entry["role"] == role)
        .count()
}

fn migration_arguments(
    previous_config: &std::path::Path,
    previous_checkpoint: &std::path::Path,
    destination_config: &std::path::Path,
    destination_checkpoint: &std::path::Path,
    function: &str,
    scratch: &std::path::Path,
) -> Vec<String> {
    fs::create_dir(scratch).unwrap();
    vec![
        "migrate".into(),
        previous_config.display().to_string(),
        previous_checkpoint.display().to_string(),
        destination_config.display().to_string(),
        destination_checkpoint.display().to_string(),
        function.into(),
        "1000".into(),
        "--opencode".into(),
        "/usr/bin/true".into(),
        "--scratch".into(),
        scratch.display().to_string(),
    ]
}

fn chain_config(fixture: &Fixture, manifest: &std::path::Path, name: &str) -> std::path::PathBuf {
    let config = source_config(fixture, manifest);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["max_iterations"] = serde_json::json!(3);
    value["max_stages"] = serde_json::json!(64);
    value["max_total_steps"] = serde_json::json!(20_000);
    let saved = fixture.0.join(name);
    fs::write(&saved, serde_json::to_vec(&value).unwrap()).unwrap();
    saved
}

#[test]
fn retained_a_to_b_to_c_handoff_carries_state_without_initialize_or_redispatch() {
    use semaprax::interpreter::retained_call::RetainedValue;
    use semaprax::live_invocation::source_journal::{
        recover_source_checkpoint, SourceTerminalStatus,
    };
    use semaprax::project::{
        with_authenticated_project, HotReloadDecision, HotReloadSession,
        PreparedProjectInterpreterOptions,
    };

    let fixture = Fixture::new();
    super::super::run::reset_read_calls();
    let project = fixture.0.join("project");
    let a_source = source_fixture::SOURCE.replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
    );
    let manifest = source_project(&project, &a_source, "fixture.agent.type.state");
    let saved_a_config = chain_config(&fixture, &manifest, "config-a.json");
    let a_checkpoint = fixture.0.join("checkpoint-a");
    let a_calls = Rc::new(Cell::new(0));
    let a = super::super::run::execute_with_runner(
        run_command(
            "run",
            &saved_a_config,
            &a_checkpoint,
            &fixture.0.join("scratch-a"),
        ),
        runner(recorded_answer(&manifest), &a_calls),
    )
    .unwrap();
    let a: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(a["status"], "suspend");
    assert_eq!(a_calls.get(), 1);
    assert_eq!(super::super::run::read_calls(), 1);
    let a_project =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();

    let b_source = successor_source().replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch, marker: state.marker }",
    );
    let manifest = source_project(&project, &b_source, "fixture.agent.type.state_b");
    let b_answer = recorded_answer(&manifest);
    let saved_b_config = chain_config(&fixture, &manifest, "config-b.json");
    let b_project =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    let mut supervisor =
        HotReloadSession::new(a_project, PreparedProjectInterpreterOptions::default()).unwrap();
    supervisor.admit_candidate(b_project.clone()).unwrap();
    let b_plan = supervisor.plan().unwrap();
    assert_eq!(
        b_plan.decision(),
        HotReloadDecision::EligibleSourceAgentCheckpointHandoff
    );
    let b_checkpoint = fixture.0.join("checkpoint-b");
    let b_calls = Rc::new(Cell::new(0));
    let b_outcome = super::super::run::execute_hot_reload_migration_with_runner(
        &mut supervisor,
        b_plan,
        &migration_arguments(
            &saved_a_config,
            &a_checkpoint,
            &saved_b_config,
            &b_checkpoint,
            "fixture.agent.fn.migrate_b",
            &fixture.0.join("scratch-b"),
        ),
        runner(b_answer, &b_calls),
    )
    .unwrap();
    assert_eq!(supervisor.generation(), 1);
    assert_eq!(b_calls.get(), 1);
    assert_eq!(
        (b_outcome.model_dispatches, b_outcome.effect_dispatches),
        (1, 1)
    );
    assert_eq!(super::super::run::read_calls(), 2);
    let b_journal = fs::read_to_string(b_checkpoint.join("checkpoint.json")).unwrap();
    let b_recovered = recover_source_checkpoint(
        &b_journal,
        supervisor.retained_source_agent_binding().unwrap(),
    )
    .unwrap();
    assert_eq!(b_recovered.committed_reserved_units(), 2);
    assert_eq!(b_recovered.deadline_millis(), 2_000_000_000_000);
    let b_fuel = b_recovered.committed_stage_fuel();
    let b_document: serde_json::Value = serde_json::from_str(&b_journal).unwrap();
    assert_eq!(stage_count(&b_document, "initialize"), 0);
    assert_eq!(stage_count(&b_document, "observe"), 1);

    let c_source = successor_c_source().replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.marker }",
    );
    let manifest = source_project(&project, &c_source, "fixture.agent.type.state_b");
    let c_answer = recorded_answer(&manifest);
    let saved_c_config = chain_config(&fixture, &manifest, "config-c.json");
    let c_project =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    supervisor.admit_candidate(c_project.clone()).unwrap();
    let c_plan = supervisor.plan().unwrap();
    assert_eq!(
        c_plan.decision(),
        HotReloadDecision::EligibleSourceAgentCheckpointHandoff
    );
    let c_checkpoint = fixture.0.join("checkpoint-c");
    let c_calls = Rc::new(Cell::new(0));
    let c_outcome = super::super::run::execute_hot_reload_migration_with_runner(
        &mut supervisor,
        c_plan,
        &migration_arguments(
            &saved_b_config,
            &b_checkpoint,
            &saved_c_config,
            &c_checkpoint,
            "fixture.agent.fn.migrate_c",
            &fixture.0.join("scratch-c"),
        ),
        runner(c_answer, &c_calls),
    )
    .unwrap();
    assert_eq!(supervisor.generation(), 2);
    assert_eq!(
        supervisor.active_project_revision(),
        c_project.project_revision()
    );
    assert_eq!(c_calls.get(), 1);
    assert_eq!(
        (c_outcome.model_dispatches, c_outcome.effect_dispatches),
        (1, 1)
    );
    assert_eq!(super::super::run::read_calls(), 3);
    assert_eq!(
        c_outcome.checked_run.as_ref().unwrap().status(),
        semaprax::agent_lifecycle::iterative::IterativeStatus::Complete
    );
    let RetainedValue::Record(complete) = c_outcome.checked_run.as_ref().unwrap().value().unwrap()
    else {
        panic!("C must publish the checked Complete carrier");
    };
    assert!(complete.fields.iter().any(|field| {
        field.field.as_str() == "fixture.agent.type.result.status"
            && field.value == RetainedValue::I64(7)
    }));
    let c_journal = fs::read_to_string(c_checkpoint.join("checkpoint.json")).unwrap();
    let c_recovered = recover_source_checkpoint(
        &c_journal,
        supervisor.retained_source_agent_binding().unwrap(),
    )
    .unwrap();
    assert_eq!(c_recovered.committed_reserved_units(), 3);
    assert_eq!(c_recovered.deadline_millis(), b_recovered.deadline_millis());
    assert!(c_recovered.committed_stage_fuel() > b_fuel);
    assert_eq!(
        c_recovered.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::Complete
    );
    let c_document: serde_json::Value = serde_json::from_str(&c_journal).unwrap();
    assert_eq!(stage_count(&c_document, "initialize"), 0);
    assert_eq!(stage_count(&c_document, "observe"), 1);
}

#[cfg(unix)]
#[test]
fn physical_journal_ack_loss_keeps_source_handoff_terminal_and_blocks_c_dispatch() {
    use super::super::checkpoint::{commit_fault_pending, inject_commit_fault, CommitFault};
    use semaprax::project::{
        with_authenticated_project, HotReloadDecision, HotReloadSession,
        HotReloadSourceAgentHandoffStatus, PreparedProjectInterpreterOptions,
    };

    for (name, fault, retained_entry) in [
        (
            "model-receipt-write",
            CommitFault::BeforeWrite("\"attempt_settled\""),
            None,
        ),
        (
            "model-receipt-rename",
            CommitFault::AfterRename("\"attempt_settled\""),
            Some("\"attempt_settled\""),
        ),
        (
            "transition-rename",
            CommitFault::AfterRename("\"transition\""),
            Some("\"transition\""),
        ),
    ] {
        let fixture = Fixture::new();
        let a_source = source_fixture::SOURCE.replace(
            "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
            "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
        );
        let a_manifest = source_project(
            &fixture.0.join("project-a"),
            &a_source,
            "fixture.agent.type.state",
        );
        let a_config = source_config(&fixture, &a_manifest);
        let saved_a_config = fixture.0.join("config-a.json");
        fs::rename(a_config, &saved_a_config).unwrap();
        let a_checkpoint = fixture.0.join("checkpoint-a");
        let a_calls = Rc::new(Cell::new(0));
        super::super::run::execute_with_runner(
            run_command(
                "run",
                &saved_a_config,
                &a_checkpoint,
                &fixture.0.join("scratch-a"),
            ),
            runner(recorded_answer(&a_manifest), &a_calls),
        )
        .unwrap();
        assert_eq!(a_calls.get(), 1);
        let a_project =
            with_authenticated_project(&a_manifest, |snapshot| Ok(snapshot.retain_revision()))
                .unwrap();

        let b_manifest = source_project(
            a_manifest.parent().unwrap(),
            &successor_source(),
            "fixture.agent.type.state_b",
        );
        let b_config = source_config(&fixture, &b_manifest);
        let saved_b_config = fixture.0.join("config-b.json");
        fs::rename(b_config, &saved_b_config).unwrap();
        let b_project =
            with_authenticated_project(&b_manifest, |snapshot| Ok(snapshot.retain_revision()))
                .unwrap();
        let mut supervisor = HotReloadSession::new(
            a_project.clone(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        supervisor.admit_candidate(b_project).unwrap();
        let plan = supervisor.plan().unwrap();
        assert_eq!(
            plan.decision(),
            HotReloadDecision::EligibleSourceAgentCheckpointHandoff
        );

        let b_checkpoint = fixture.0.join("checkpoint-b");
        let b_scratch = fixture.0.join("scratch-b");
        fs::create_dir(&b_scratch).unwrap();
        let arguments = vec![
            "migrate".into(),
            saved_a_config.display().to_string(),
            a_checkpoint.display().to_string(),
            saved_b_config.display().to_string(),
            b_checkpoint.display().to_string(),
            "fixture.agent.fn.migrate_b".into(),
            "1000".into(),
            "--opencode".into(),
            "/usr/bin/true".into(),
            "--scratch".into(),
            b_scratch.display().to_string(),
        ];
        let b_calls = Rc::new(Cell::new(0));
        inject_commit_fault(fault);
        let result = super::super::run::execute_hot_reload_migration_with_runner(
            &mut supervisor,
            plan,
            &arguments,
            runner(recorded_answer(&b_manifest), &b_calls),
        );
        assert!(
            result.is_err(),
            "{name} must lose its exact journal acknowledgement"
        );
        assert!(
            !commit_fault_pending(),
            "{name} fault must match a rendered migration checkpoint document"
        );
        assert_eq!(b_calls.get(), 1, "{name} reaches exactly one B model call");
        assert_eq!(
            supervisor.source_agent_handoff_status(),
            HotReloadSourceAgentHandoffStatus::TerminalUncertainty,
            "{name} acknowledgement loss cannot select an in-memory rollback"
        );
        assert_eq!(supervisor.generation(), 0);
        assert_eq!(
            supervisor.active_project_revision(),
            a_project.project_revision()
        );

        let b_journal = fs::read(b_checkpoint.join("checkpoint.json")).unwrap();
        let b_journal_text = std::str::from_utf8(&b_journal).unwrap();
        assert!(
            b_journal_text.contains("\"attempt_intent\""),
            "{name} retains the charged model reservation"
        );
        match retained_entry {
            Some(entry) => assert!(
                b_journal_text.contains(entry),
                "{name} keeps the physically committed journal entry for explicit recovery"
            ),
            None => assert!(
                !b_journal_text.contains("\"attempt_settled\""),
                "a failed model-receipt write never acknowledges the model answer"
            ),
        }

        let c_manifest = source_project(
            &fixture.0.join("project-c"),
            &successor_c_source(),
            "fixture.agent.type.state_b",
        );
        let c_config = source_config(&fixture, &c_manifest);
        let saved_c_config = fixture.0.join("config-c.json");
        fs::rename(c_config, &saved_c_config).unwrap();
        let c_checkpoint = fixture.0.join("checkpoint-c");
        let c_calls = Rc::new(Cell::new(0));
        assert!(super::super::run::execute_with_runner(
            migrate_command_with_function(
                &saved_b_config,
                &b_checkpoint,
                &saved_c_config,
                &c_checkpoint,
                "fixture.agent.fn.migrate_c",
                &fixture.0.join(format!("scratch-c-{name}")),
            ),
            runner(recorded_answer(&c_manifest), &c_calls),
        )
        .is_err());
        assert_eq!(c_calls.get(), 0, "{name} cannot dispatch successor C");
        assert_eq!(
            fs::read(b_checkpoint.join("checkpoint.json")).unwrap(),
            b_journal
        );
        assert!(
            !c_checkpoint.exists(),
            "{name} refuses before a C store exists"
        );
    }
}
