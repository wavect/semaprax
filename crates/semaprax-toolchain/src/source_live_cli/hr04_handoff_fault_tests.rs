use super::*;

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

#[test]
fn physical_migration_reservation_and_handoff_claim_faults_block_successor_dispatch() {
    use super::super::checkpoint::{
        inject_commit_fault, inject_handoff_claim_fault, CommitFault, HandoffClaimFault,
    };
    use semaprax::project::{
        with_authenticated_project, HotReloadDecision, HotReloadSession,
        HotReloadSourceAgentHandoffStatus, PreparedProjectInterpreterOptions,
    };

    enum Fault {
        MigrationReservation,
        ClaimWrite,
        ClaimSync,
    }

    for (name, fault) in [
        ("migration-reservation", Fault::MigrationReservation),
        ("claim-write", Fault::ClaimWrite),
        ("claim-sync", Fault::ClaimSync),
    ] {
        let fixture = Fixture::new();
        let a_source = source_fixture::SOURCE.replace(
            "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
            "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
        );
        let project = fixture.0.join("project");
        let manifest = source_project(&project, &a_source, "fixture.agent.type.state");
        let a_config = source_config(&fixture, &manifest);
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
            runner(recorded_answer(&manifest), &a_calls),
        )
        .unwrap();
        assert_eq!(a_calls.get(), 1);
        let a_project =
            with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
                .unwrap();

        let manifest = source_project(&project, &successor_source(), "fixture.agent.type.state_b");
        let b_config = source_config(&fixture, &manifest);
        let saved_b_config = fixture.0.join("config-b.json");
        fs::rename(b_config, &saved_b_config).unwrap();
        let b_project =
            with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
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
        let arguments = migration_arguments(
            &saved_a_config,
            &a_checkpoint,
            &saved_b_config,
            &b_checkpoint,
            "fixture.agent.fn.migrate_b",
            &fixture.0.join(format!("scratch-b-{name}")),
        );
        let b_calls = Rc::new(Cell::new(0));
        match fault {
            Fault::MigrationReservation => {
                inject_commit_fault(CommitFault::BeforeWrite("\"migration_evaluation_intent\""));
            }
            Fault::ClaimWrite => inject_handoff_claim_fault(HandoffClaimFault::BeforeWrite),
            Fault::ClaimSync => inject_handoff_claim_fault(HandoffClaimFault::BeforeSync),
        }
        assert!(super::super::run::execute_hot_reload_migration_with_runner(
            &mut supervisor,
            plan.clone(),
            &arguments,
            runner(recorded_answer(&manifest), &b_calls),
        )
        .is_err());
        assert_eq!(
            b_calls.get(),
            0,
            "{name} cannot dispatch B after a lost boundary"
        );
        assert_eq!(supervisor.generation(), 0);
        assert_eq!(
            supervisor.active_project_revision(),
            a_project.project_revision()
        );

        match fault {
            Fault::MigrationReservation => {
                assert_eq!(
                    supervisor.source_agent_handoff_status(),
                    HotReloadSourceAgentHandoffStatus::TerminalUncertainty
                );
                let journal = fs::read_to_string(b_checkpoint.join("checkpoint.json")).unwrap();
                assert!(journal.contains("\"migration_opened\""));
                assert!(!journal.contains("\"migration_evaluation_intent\""));
            }
            Fault::ClaimWrite => {
                assert_eq!(
                    fs::read(a_checkpoint.join("handoff.claim")).unwrap(),
                    Vec::<u8>::new(),
                    "{name} creates no acknowledged claim bytes"
                );
            }
            Fault::ClaimSync => {
                assert!(!fs::read(a_checkpoint.join("handoff.claim"))
                    .unwrap()
                    .is_empty());
                super::super::run::execute_hot_reload_migration_with_runner(
                    &mut supervisor,
                    plan,
                    &arguments,
                    runner(recorded_answer(&manifest), &b_calls),
                )
                .unwrap();
                assert_eq!(
                    b_calls.get(),
                    1,
                    "exact retained claim permits one B dispatch"
                );
                assert_eq!(supervisor.generation(), 1);
            }
        }
    }
}
