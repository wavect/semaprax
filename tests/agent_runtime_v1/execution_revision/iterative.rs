use super::*;
use semaprax::agent_lifecycle::iterative::{
    compile_source_agent_lifecycle_v2, IterativeBudget, IterativeStatus,
};
use semaprax::execution_revision::iterative::bind_iterative_execution_revision;

pub(super) fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    // Explicit source ceilings admit the three-turn success fixture.
    let source = source
        .replace(r#"\"max_turns\":2"#, r#"\"max_turns\":3"#)
        .replace(r#"\"max_tool_calls\":1"#, r#"\"max_tool_calls\":3"#);

    let reducer = r#"
@id("fixture.agent.type.step")
variant Step {
    @id("fixture.agent.step.continue") Continue {
        @id("fixture.agent.step.continue.objective") objective: Bytes,
        @id("fixture.agent.step.continue.budget") budget: i64,
        @id("fixture.agent.step.continue.epoch") epoch: i64,
    },
    @id("fixture.agent.step.complete") Complete {
        @id("fixture.agent.step.complete.summary") summary: Bytes,
        @id("fixture.agent.step.complete.budget") budget: i64,
        @id("fixture.agent.step.complete.status") status: i64,
    },
    @id("fixture.agent.step.suspend") Suspend {
        @id("fixture.agent.step.suspend.objective") objective: Bytes,
        @id("fixture.agent.step.suspend.budget") budget: i64,
        @id("fixture.agent.step.suspend.epoch") epoch: i64,
    },
    @id("fixture.agent.step.fail") Fail { @id("fixture.agent.step.fail.code") code: i64, },
}
@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Step {
    if state.epoch < 3 {
        Step::Continue { objective: state.objective, budget: state.budget, epoch: state.epoch + 1 }
    } else {
        Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }
    }
}
"#;
    let mut program = semaprax::parse(&source, "src/app.spx").unwrap();
    let mut addition = semaprax::parse(
        &format!("module fixture.agent.lifecycle;\n{reducer}"),
        "src/app.spx",
    )
    .unwrap();
    program.types.extend(addition.types.clone());
    let position = program
        .functions
        .iter()
        .position(|function| function.stable_id == "fixture.agent.fn.reduce")
        .unwrap();
    program.functions[position] = addition.functions.remove(0);
    let source = semaprax::format::canonical(&program);
    std::fs::write(path, source).unwrap();
    fixture
}

#[test]
fn iterative_roots_bind_every_input_and_actual_multiturn_producer() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let source = &project.sources()[0];
        let lifecycle = compile_source_agent_lifecycle_v2(
            source.source(),
            source.path(),
            "fixture.agent",
            "fixture.agent.type.step",
        )?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let proposed = vec![
            proposal(
                lifecycle.proposal_schema().schema().digest(),
                "5",
                false,
                "1"
            );
            3
        ];
        let bind = |task_budget, proposals: &[String], budget| {
            bind_iterative_execution_revision(
                project.clone(),
                ProgramRootRef::V1(&root),
                root.program_root_digest(),
                source.path(),
                "fixture.agent",
                "fixture.agent.type.step",
                &deployment,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget: task_budget,
                },
                proposals,
                budget,
            )
        };
        let first = bind(12, &proposed, IterativeBudget::default())?;
        assert_eq!(
            first.execution_revision(),
            bind(12, &proposed, IterativeBudget::default())?.execution_revision()
        );
        assert_ne!(
            first.instance_root(),
            bind(13, &proposed, IterativeBudget::default())?.instance_root()
        );
        let mut changed = proposed.clone();
        changed.push(proposed[0].clone());
        assert_ne!(
            first.instance_root(),
            bind(12, &changed, IterativeBudget::default())?.instance_root()
        );
        for budget in [
            IterativeBudget {
                max_iterations: 4,
                ..IterativeBudget::default()
            },
            IterativeBudget {
                max_stages: 20,
                ..IterativeBudget::default()
            },
            IterativeBudget {
                max_steps_per_stage: 50_000,
                ..IterativeBudget::default()
            },
        ] {
            assert_ne!(
                first.instance_root(),
                bind(12, &proposed, budget)?.instance_root()
            );
        }
        let mut operation = FixtureRead::new(b"observed".to_vec());
        let revision = first.execution_revision().clone();
        let evidence = first.run(&mut operation, &AgentCancellation::new())?;
        assert_eq!(evidence.run().status(), IterativeStatus::Complete);
        assert_eq!((evidence.run().iterations(), operation.calls()), (3, 3));
        assert_eq!(evidence.execution_revision(), &revision);
        assert!(evidence
            .evidence_root()
            .canonical_json()
            .contains(evidence.run().evidence_digest()));
        assert!(!evidence.evidence_root().canonical_json().contains("alpha"));
        let cancel = AgentCancellation::new();
        cancel.cancel();
        let first =
            bind(12, &proposed, IterativeBudget::default())?.run(&mut operation, &cancel)?;
        let second =
            bind(13, &proposed, IterativeBudget::default())?.run(&mut operation, &cancel)?;
        assert_eq!(first.run().status(), IterativeStatus::Cancelled);
        assert_ne!(first.evidence_root(), second.evidence_root());
        assert_eq!(operation.calls(), 3);
        for key in ["max_turns", "max_tool_calls"] {
            let limited = deployment.replace(&format!("\"{key}\":3"), &format!("\"{key}\":1"));
            assert_ne!(limited, deployment);
            let bound = bind_iterative_execution_revision(
                project.clone(),
                ProgramRootRef::V1(&root),
                root.program_root_digest(),
                source.path(),
                "fixture.agent",
                "fixture.agent.type.step",
                &limited,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget: 12,
                },
                &proposed,
                IterativeBudget::default(),
            )?;
            let facts: serde_json::Value =
                serde_json::from_str(bound.instance_root().canonical_json()).unwrap();
            assert_eq!(facts["facts"]["max_iterations"], 32);
            assert_eq!(facts["facts"]["effective_max_iterations"], 1);
            let mut read = FixtureRead::new(b"observed".to_vec());
            let run = bound.run(&mut read, &AgentCancellation::new())?;
            assert_eq!(run.run().status(), IterativeStatus::BudgetExhausted);
            assert_eq!((run.run().iterations(), read.calls()), (1, 1));
        }
        let error = bind(12, &vec![String::new(); 4097], IterativeBudget::default())
            .err()
            .unwrap();
        assert_eq!(error[0].code, "SPX-G583");
        Ok(())
    })
    .unwrap();
}
