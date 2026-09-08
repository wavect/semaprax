use super::*;
use crate::agent_lifecycle::tests::{DEFINITION, MODULE, RUNTIME_V1};

fn source(terminal: &str) -> String {
    let start = MODULE.find("@id(\"fixture.agent.fn.reduce\")").unwrap();
    let end = MODULE[start..].find("@id(\"app.main\")").unwrap() + start;
    let reducer = format!(
        r#"
@id("fixture.agent.type.step")
variant Step {{
    @id("fixture.agent.step.continue") Continue {{
        @id("fixture.agent.step.continue.objective") objective: Bytes,
        @id("fixture.agent.step.continue.budget") budget: i64,
        @id("fixture.agent.step.continue.epoch") epoch: i64,
    }},
    @id("fixture.agent.step.complete") Complete {{
        @id("fixture.agent.step.complete.summary") summary: Bytes,
        @id("fixture.agent.step.complete.budget") budget: i64,
        @id("fixture.agent.step.complete.status") status: i64,
    }},
    @id("fixture.agent.step.suspend") Suspend {{
        @id("fixture.agent.step.suspend.objective") objective: Bytes,
        @id("fixture.agent.step.suspend.budget") budget: i64,
        @id("fixture.agent.step.suspend.epoch") epoch: i64,
    }},
    @id("fixture.agent.step.fail") Fail {{
        @id("fixture.agent.step.fail.code") code: i64,
    }},
}}
@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Step {{
    if state.epoch < 3 {{
        Step::Continue {{ objective: state.objective, budget: state.budget, epoch: state.epoch + 1 }}
    }} else {{ {terminal} }}
}}
"#
    );
    format!("{}{}{}", &MODULE[..start], reducer, &MODULE[end..])
}

fn compile(terminal: &str) -> CompiledIterativeLifecycle {
    compile_agent_lifecycle_v2(
        &source(terminal),
        "iterative-unit.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
        "fixture.agent.type.step",
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}
fn proposals(compiled: &CompiledIterativeLifecycle) -> Vec<String> {
    vec![super::super::tests::proposal(&compiled.inner, "1", "1"); 4]
}
fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"task".to_vec(),
        budget: 10,
    }
}

#[test]
fn three_turns_execute_checked_steps_and_distinct_fresh_authorizations() {
    let compiled = compile(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
    );
    let graph: serde_json::Value = serde_json::from_str(compiled.canonical_json()).unwrap();
    assert_eq!(graph["schema"], "semaprax.agent-iterative-lifecycle.v2");
    assert_eq!(graph["execution"]["continue_target"], "observe");
    assert_eq!(graph["step"]["cases"].as_array().unwrap().len(), 4);
    assert!(!compiled.canonical_json().contains("no_iterative"));
    assert!(!compiled.canonical_json().contains("acyclic"));
    let mut read = FixtureRead::new(b"read".to_vec());
    let run = compiled
        .run(
            &task(),
            &proposals(&compiled),
            &mut read,
            IterativeBudget::default(),
            &AgentCancellation::default(),
        )
        .unwrap();
    assert_eq!(run.status, IterativeStatus::Complete);
    assert_eq!(
        (run.iterations, run.effects, run.stages.len(), read.calls()),
        (3, 3, 10, 3)
    );
    assert_eq!(
        run.stages
            .iter()
            .filter(|s| s.role() == "initialize")
            .count(),
        1
    );
    assert_ne!(run.authorization_bindings[0], run.authorization_bindings[1]);
    let mut replay = FixtureRead::new(b"read".to_vec());
    let replay = compiled
        .run(
            &task(),
            &proposals(&compiled),
            &mut replay,
            IterativeBudget::default(),
            &AgentCancellation::default(),
        )
        .unwrap();
    assert_eq!(run.evidence(), replay.evidence());
    assert_eq!(run.evidence_digest(), replay.evidence_digest());
}

#[test]
fn suspend_and_failure_are_reducer_selected_terminal_values() {
    for (expression, expected) in [
        ("Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }", IterativeStatus::Suspend),
        ("Step::Fail { code: 37 }", IterativeStatus::Fail),
    ] {
        let compiled = compile(expression);
        let mut read = FixtureRead::new(Vec::new());
        let run = compiled.run(&task(), &proposals(&compiled), &mut read, IterativeBudget::default(), &AgentCancellation::default()).unwrap();
        assert_eq!(run.status, expected);
        assert_eq!(run.effects, 3);
        assert!(run.value.is_some());
    }
}

#[test]
fn iteration_and_stage_limits_stop_before_unbudgeted_effects() {
    let compiled = compile("Step::Fail { code: 1 }");
    for (iterations, stages, effects) in [(0, 10, 0), (1, 10, 1), (4, 3, 0), (4, 4, 1)] {
        let mut read = FixtureRead::new(Vec::new());
        let run = compiled
            .run(
                &task(),
                &proposals(&compiled),
                &mut read,
                IterativeBudget {
                    max_iterations: iterations,
                    max_stages: stages,
                    max_steps_per_stage: DEFAULT_STAGE_STEPS,
                },
                &AgentCancellation::default(),
            )
            .unwrap();
        assert_eq!(run.status, IterativeStatus::BudgetExhausted);
        assert_eq!(read.calls(), effects);
    }
}

#[test]
fn wrong_step_shape_is_rejected_before_any_runner_exists() {
    let source = source("Step::Fail { code: 1 }")
        .replace(
            "@id(\"fixture.agent.step.fail.code\") code: i64",
            "@id(\"fixture.agent.step.fail.code\") code: usize",
        )
        .replace("Step::Fail { code: 1 }", "Step::Fail { code: 1usize }");
    let errors = match compile_agent_lifecycle_v2(
        &source,
        "iterative-unit.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
        "fixture.agent.type.step",
    ) {
        Err(errors) => errors,
        Ok(_) => panic!("invalid Step admitted"),
    };
    assert!(
        errors.iter().any(|e| format!("{e:?}").contains("SPX-G582")),
        "{errors:?}"
    );
}

#[test]
fn cancellation_during_effect_prevents_reduction_and_later_effects() {
    struct CancelRead {
        cancellation: AgentCancellation,
        calls: usize,
    }
    impl AgentReadOperation for CancelRead {
        fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
            self.calls += 1;
            self.cancellation.cancel();
            Some(Vec::new())
        }
    }
    let compiled = compile("Step::Fail { code: 1 }");
    let cancellation = AgentCancellation::new();
    let mut read = CancelRead {
        cancellation: cancellation.clone(),
        calls: 0,
    };
    let run = compiled
        .run(
            &task(),
            &proposals(&compiled),
            &mut read,
            IterativeBudget::default(),
            &cancellation,
        )
        .unwrap();
    assert_eq!(run.status(), IterativeStatus::Cancelled);
    assert_eq!((run.effects(), run.iterations(), read.calls), (1, 0, 1));
    assert!(!run.stages().iter().any(|stage| stage.role() == "reduce"));
    let too_large = IterativeBudget {
        max_iterations: 4097,
        ..IterativeBudget::default()
    };
    assert!(compiled
        .run(
            &task(),
            &proposals(&compiled),
            &mut read,
            too_large,
            &AgentCancellation::new()
        )
        .is_err());
    assert_eq!(read.calls, 1);
}

#[test]
fn evidence_binds_all_invocation_inputs_even_before_first_stage() {
    let compiled = compile("Step::Fail { code: 1 }");
    let base_proposals = proposals(&compiled);
    let zero = IterativeBudget {
        max_iterations: 0,
        ..IterativeBudget::default()
    };
    let execute = |task: &LifecycleTask, proposals: &[String], budget| {
        let mut read = FixtureRead::new(Vec::new());
        let run = compiled
            .run(
                task,
                proposals,
                &mut read,
                budget,
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!((run.effects(), run.stages().len()), (0, 0));
        run.evidence_digest().to_owned()
    };
    let base = execute(&task(), &base_proposals, zero);
    assert_ne!(
        base,
        execute(
            &LifecycleTask {
                objective: b"other".to_vec(),
                ..task()
            },
            &base_proposals,
            zero
        )
    );
    assert_ne!(
        base,
        execute(
            &LifecycleTask {
                budget: 11,
                ..task()
            },
            &base_proposals,
            zero
        )
    );
    let mut changed = base_proposals.clone();
    changed[3].push(' ');
    assert_ne!(base, execute(&task(), &changed, zero));
    assert_ne!(
        base,
        execute(
            &task(),
            &base_proposals,
            IterativeBudget {
                max_stages: 96,
                ..zero
            }
        )
    );
    assert_ne!(
        base,
        execute(
            &task(),
            &base_proposals,
            IterativeBudget {
                max_steps_per_stage: 999,
                ..zero
            }
        )
    );
}
