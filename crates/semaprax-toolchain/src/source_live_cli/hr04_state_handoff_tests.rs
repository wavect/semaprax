use super::*;

fn stage_count(document: &serde_json::Value, role: &str) -> usize {
    document["entries"]
        .as_array()
        .expect("canonical source journal entries")
        .iter()
        .filter(|entry| entry["kind"] == "stage_reservation" && entry["role"] == role)
        .count()
}

#[test]
fn retained_a_to_b_to_c_handoff_carries_state_without_initialize_or_redispatch() {
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
    let a_config = saved_a_config;
    let a_checkpoint = fixture.0.join("checkpoint-a");
    let a_calls = Rc::new(Cell::new(0));
    let a = super::super::run::execute_with_runner(
        run_command(
            "run",
            &a_config,
            &a_checkpoint,
            &fixture.0.join("scratch-a"),
        ),
        runner(recorded_answer(&a_manifest), &a_calls),
    )
    .unwrap();
    let a: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(a["status"], "suspend");
    assert_eq!(a["model_dispatches"], 1);
    assert_eq!(a["effect_dispatches"], 1);

    let b_source = successor_source().replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch, marker: state.marker }",
    );
    let b_manifest = source_project(
        &fixture.0.join("project-b"),
        &b_source,
        "fixture.agent.type.state_b",
    );
    let b_config = source_config(&fixture, &b_manifest);
    let saved_b_config = fixture.0.join("config-b.json");
    fs::rename(b_config, &saved_b_config).unwrap();
    let b_config = saved_b_config;
    let b_checkpoint = fixture.0.join("checkpoint-b");
    let b_calls = Rc::new(Cell::new(0));
    let b = super::super::run::execute_with_runner(
        migrate_command(
            &a_config,
            &a_checkpoint,
            &b_config,
            &b_checkpoint,
            &fixture.0.join("scratch-b"),
        ),
        runner(recorded_answer(&b_manifest), &b_calls),
    )
    .unwrap();
    let b: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(b["status"], "suspend");
    assert_eq!(b["model_dispatches"], 1);
    assert_eq!(b["effect_dispatches"], 1);
    assert_eq!(b["committed_model_units"], 2);
    assert_eq!(b_calls.get(), 1);
    let b_journal = fs::read(b_checkpoint.join("checkpoint.json")).unwrap();
    let b_document: serde_json::Value = serde_json::from_slice(&b_journal).unwrap();
    assert_eq!(stage_count(&b_document, "initialize"), 0);
    assert_eq!(stage_count(&b_document, "observe"), 1);

    let c_manifest = source_project(
        &fixture.0.join("project-c"),
        &successor_c_source(),
        "fixture.agent.type.state_b",
    );
    let c_config = source_config(&fixture, &c_manifest);
    let saved_c_config = fixture.0.join("config-c.json");
    fs::rename(c_config, &saved_c_config).unwrap();
    let c_config = saved_c_config;
    let c_checkpoint = fixture.0.join("checkpoint-c");

    // Corrupting B's retained latest journal cannot start C's provider. The
    // CLI must authenticate the predecessor before it builds a source bridge
    // or claims this handoff.
    fs::write(b_checkpoint.join("checkpoint.json"), b"{\"forged\":true}\n").unwrap();
    let fault_calls = Rc::new(Cell::new(0));
    assert!(super::super::run::execute_with_runner(
        migrate_command_with_function(
            &b_config,
            &b_checkpoint,
            &c_config,
            &c_checkpoint,
            "fixture.agent.fn.migrate_c",
            &fixture.0.join("scratch-c-fault"),
        ),
        runner(recorded_answer(&c_manifest), &fault_calls),
    )
    .is_err());
    assert_eq!(fault_calls.get(), 0);
    fs::write(b_checkpoint.join("checkpoint.json"), &b_journal).unwrap();

    // Both the absolute deadline and every cumulative budget are bound to A's
    // checkpoint. C cannot relax either boundary before a provider exists.
    let c_config_bytes = fs::read(&c_config).unwrap();
    for (name, mutate) in [
        (
            "deadline",
            ("deadline_millis", serde_json::json!(2_000_000_000_001i64)),
        ),
        ("model-budget", ("ceiling", serde_json::json!(1))),
    ] {
        let mut altered: serde_json::Value = serde_json::from_slice(&c_config_bytes).unwrap();
        altered[mutate.0] = mutate.1;
        fs::write(&c_config, serde_json::to_vec(&altered).unwrap()).unwrap();
        let calls = Rc::new(Cell::new(0));
        assert!(super::super::run::execute_with_runner(
            migrate_command_with_function(
                &b_config,
                &b_checkpoint,
                &c_config,
                &c_checkpoint,
                "fixture.agent.fn.migrate_c",
                &fixture.0.join(format!("scratch-c-{name}")),
            ),
            runner(recorded_answer(&c_manifest), &calls),
        )
        .is_err());
        assert_eq!(
            calls.get(),
            0,
            "{name} refusal must precede provider dispatch"
        );
    }
    fs::write(&c_config, &c_config_bytes).unwrap();

    let c_calls = Rc::new(Cell::new(0));
    let c = super::super::run::execute_with_runner(
        migrate_command_with_function(
            &b_config,
            &b_checkpoint,
            &c_config,
            &c_checkpoint,
            "fixture.agent.fn.migrate_c",
            &fixture.0.join("scratch-c"),
        ),
        runner(recorded_answer(&c_manifest), &c_calls),
    )
    .unwrap();
    let c: serde_json::Value = serde_json::from_str(&c).unwrap();
    assert_eq!(c["status"], "complete");
    assert_eq!(c["model_dispatches"], 1);
    assert_eq!(c["effect_dispatches"], 1);
    assert_eq!(c["committed_model_units"], 3);
    assert!(
        c["committed_stage_fuel"].as_u64().unwrap() > b["committed_stage_fuel"].as_u64().unwrap()
    );
    assert_eq!(c_calls.get(), 1);
    let c_journal = fs::read(c_checkpoint.join("checkpoint.json")).unwrap();
    let c_document: serde_json::Value = serde_json::from_slice(&c_journal).unwrap();
    assert_eq!(stage_count(&c_document, "initialize"), 0);
    assert_eq!(stage_count(&c_document, "observe"), 1);

    let replay = super::super::run::execute_with_runner(
        migrate_command_with_function(
            &b_config,
            &b_checkpoint,
            &c_config,
            &c_checkpoint,
            "fixture.agent.fn.migrate_c",
            &fixture.0.join("scratch-c-replay"),
        ),
        runner(recorded_answer(&c_manifest), &c_calls),
    )
    .unwrap();
    let replay: serde_json::Value = serde_json::from_str(&replay).unwrap();
    assert_eq!(replay["status"], "complete");
    assert_eq!(replay["model_dispatches"], 0);
    assert_eq!(replay["effect_dispatches"], 0);
    assert_eq!(c_calls.get(), 1);
    assert_eq!(
        fs::read(c_checkpoint.join("checkpoint.json")).unwrap(),
        c_journal
    );
}

#[cfg(unix)]
#[test]
fn physical_journal_ack_loss_keeps_source_handoff_terminal_and_blocks_c_dispatch() {
    use super::checkpoint::{inject_commit_fault, CommitFault};
    use semaprax::project::{
        with_authenticated_project, HotReloadDecision, HotReloadSession,
        HotReloadSourceAgentHandoffStatus, PreparedProjectInterpreterOptions,
    };

    for (name, fault, retained_entry) in [
        (
            "effect-intent-write",
            CommitFault::BeforeWrite("\"effect_intent\""),
            None,
        ),
        (
            "effect-intent",
            CommitFault::AfterRename("\"effect_intent\""),
            Some("\"effect_intent\""),
        ),
        (
            "stop",
            CommitFault::AfterRename("\"stop\""),
            Some("\"stop\""),
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
        super::run::execute_with_runner(
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
        assert!(super::run::execute_hot_reload_migration_with_runner(
            &mut supervisor,
            plan,
            &arguments,
            runner(recorded_answer(&b_manifest), &b_calls),
        )
        .is_err());
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
                !b_journal_text.contains("\"effect_intent\""),
                "a failed effect-intent write never authorizes the effect"
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
        assert!(super::run::execute_with_runner(
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
