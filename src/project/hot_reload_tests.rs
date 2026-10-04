use super::super::prepared_interpreter::{
    active_prepared_worker_count_for_test, prepared_worker_test_guard, ExecutionTestHook,
    PreparedReplacementTestHook,
};
use super::*;
use crate::project::{
    verify_project_source_trace_against_revision, ProjectPreparedExecutionOutcome, ProjectProfile,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-hot-reload-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for relative in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(relative), root.join(relative)).unwrap();
        }
        Self(root)
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        crate::project::load_snapshot(&self.0.join("semaprax.toml"))
            .unwrap()
            .retain_revision()
    }

    fn rewrite(&self, relative: &str, old: &str, new: &str) {
        let path = self.0.join(relative);
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains(old));
        let changed = source.replacen(old, new, 1);
        let canonical =
            crate::format::canonical(&crate::parse(&changed, Path::new(relative)).unwrap());
        std::fs::write(path, canonical).unwrap();
    }

    fn indirect_effect() -> Self {
        let fixture = Self::new();
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            concat!(
                "schema = \"semaprax.project.v4\"\n",
                "name = \"hot-reload-effects\"\n",
                "version = \"1.0.0\"\n",
                "profile = \"useful-data-command.v1\"\n",
                "entry = \"reload.effects\"\n",
                "sources = [\"src/app.spx\", \"src/tests.spx\"]\n",
                "web_exports = [\"reload.effects.run\"]\n",
                "command = \"reload.effects.run\"\n",
                "capabilities = [\"process.stdout.write\"]\n",
                "tests = [\"reload.effects.tests\"]\n",
            ),
        )
        .unwrap();
        for (relative, source) in [
            (
                "src/app.spx",
                r#"module reload.effects;
permit { process.stdout.write }
@id("reload.effects.main") fn main()->i64 uses { process.stdout.write } { 42 }
@id("reload.effects.target") fn target(value:i64)->i64 { value }
@id("reload.effects.run") fn run(input: borrow Slice<u8>)->bool { target(42) == 42 }
"#,
            ),
            (
                "src/tests.spx",
                r#"module reload.effects.tests;
@id("reload.effects.tests.main") fn main()->i64 { 0 }
"#,
            ),
        ] {
            let canonical = crate::format::canonical(
                &crate::parse(source, Path::new(relative)).expect("effect fixture parses"),
            );
            std::fs::write(fixture.0.join(relative), canonical).unwrap();
        }
        fixture
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn observed(session: &HotReloadSession) -> ProjectPreparedExecutionOutcome {
    session
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap()
        .outcome()
        .clone()
}

fn resolved_program(source: &str) -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "hot-reload-function-values.spx").unwrap()).unwrap()
}

#[test]
fn indirect_callable_target_universe_participates_in_replacement_compatibility() {
    const SOURCE: &str = r#"
module test.hot_reload_function_values;
permit { clock.read, clock.write }
@id("reload.increment") fn increment(value:i64)->i64{value+1}
@id("reload.decrement") fn decrement(value:i64)->i64{value-1}
@id("reload.effectful") fn effectful(value:i64)->i64 uses { clock.read } { value }
@id("reload.apply") fn apply(callback:fn(i64)->i64,value:i64)->i64{callback(value)}
@id("reload.catalog") fn catalog()->i64{let callback=decrement;callback(41)}
@id("reload.main") fn main()->i64{let callback=if true{increment}else{decrement};apply(callback,41)}
"#;

    let active = resolved_program(SOURCE);
    let body_only = resolved_program(&SOURCE.replace("value+1", "value+2"));
    let indirect_target_contract_changed = resolved_program(&SOURCE.replace(
        "@id(\"reload.decrement\") fn decrement(value:i64)->i64{value-1}",
        "@id(\"reload.decrement\") fn decrement(value:i64)->i64 requires value >= 0 {value-1}",
    ));
    let entry_effect_changed = resolved_program(&SOURCE.replace(
        "@id(\"reload.main\") fn main()->i64{",
        "@id(\"reload.main\") fn main()->i64 uses { clock.read } {",
    ));

    assert_eq!(
        crate::hir::function_value::target_universe(&active)
            .iter()
            .map(|function| function.id.as_str())
            .collect::<Vec<_>>(),
        ["reload.decrement", "reload.increment"],
        "the checked indirect invocation is bound to both eligible references",
    );
    assert!(compatible_program(&active, &body_only));
    assert!(
        !compatible_program(&active, &indirect_target_contract_changed),
        "an indirect invocation must retain every compiler-derived target's contract"
    );
    assert!(
        !compatible_program(&active, &entry_effect_changed),
        "a reachable compiler-derived function cannot widen its declared effect"
    );
}

#[test]
fn source_agent_handoff_digest_binds_definition_and_interaction_schema_facts() {
    let previous = SourceAgentEndpointFacts {
        definition_digest: "sha256:definition-a".to_owned(),
        graph_digest: "sha256:graph-a".to_owned(),
        runtime_profile_digest: "sha256:profile-a".to_owned(),
        state_type_id: "agent.state".to_owned(),
        proposal_type_id: "agent.proposal".to_owned(),
        proposal_type_revision: "sha256:proposal-type-a".to_owned(),
        observation_type_id: "agent.observation".to_owned(),
        observation_type_revision: "sha256:observation-type-a".to_owned(),
        proposal_schema_digest: "sha256:proposal-schema-a".to_owned(),
        observation_schema_digest: "sha256:observation-schema-a".to_owned(),
    };
    let destination = SourceAgentEndpointFacts {
        definition_digest: "sha256:definition-b".to_owned(),
        graph_digest: "sha256:graph-b".to_owned(),
        runtime_profile_digest: "sha256:profile-b".to_owned(),
        state_type_id: "agent.state.next".to_owned(),
        proposal_type_id: "agent.proposal".to_owned(),
        proposal_type_revision: "sha256:proposal-type-b".to_owned(),
        observation_type_id: "agent.observation".to_owned(),
        observation_type_revision: "sha256:observation-type-b".to_owned(),
        proposal_schema_digest: "sha256:proposal-schema-b".to_owned(),
        observation_schema_digest: "sha256:observation-schema-b".to_owned(),
    };
    let digest = source_agent_handoff_row_digest("agent.id", &previous, &destination);

    let mut changed_definition = destination.clone();
    changed_definition.definition_digest = "sha256:definition-c".to_owned();
    let mut changed_proposal_schema = destination.clone();
    changed_proposal_schema.proposal_schema_digest = "sha256:proposal-schema-c".to_owned();
    let mut changed_state = destination.clone();
    changed_state.state_type_id = "agent.state.other".to_owned();
    let mut changed_observation_type = destination;
    changed_observation_type.observation_type_revision = "sha256:observation-type-c".to_owned();

    assert_ne!(
        digest,
        source_agent_handoff_row_digest("agent.id", &previous, &changed_definition)
    );
    assert_ne!(
        digest,
        source_agent_handoff_row_digest("agent.id", &previous, &changed_proposal_schema)
    );
    assert_ne!(
        digest,
        source_agent_handoff_row_digest("agent.id", &previous, &changed_state)
    );
    assert_ne!(
        digest,
        source_agent_handoff_row_digest("agent.id", &previous, &changed_observation_type)
    );
}

#[test]
fn checked_plan_is_separate_from_activation_and_two_plans_cannot_both_commit() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let active = fixture.revision();
    assert_eq!(
        active.manifest().project_profile(),
        ProjectProfile::ScalarV1
    );
    let mut session =
        HotReloadSession::new(active, PreparedProjectInterpreterOptions::default()).unwrap();
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let candidate = fixture.revision();
    session.admit_candidate(candidate.clone()).unwrap();
    let first = session.plan().unwrap();
    let second = session.plan().unwrap();
    assert_eq!(first.decision(), HotReloadDecision::EligibleCodeReplacement);
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
    let view: serde_json::Value = serde_json::from_str(&first.to_json()).unwrap();
    assert_eq!(view["authority"], "none");
    assert_eq!(view["generation"], 0);
    session.activate(first).unwrap();
    assert_eq!(session.generation(), 1);
    assert_eq!(
        session.active_project_revision(),
        candidate.project_revision()
    );
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(48)
    );
    assert_eq!(
        session.activate(second).unwrap_err().reason,
        HotReloadReason::StaleGeneration
    );
    session.admit_candidate(candidate).unwrap();
    let identical = session.plan().unwrap();
    assert_eq!(identical.decision(), HotReloadDecision::Unchanged);
    assert_eq!(identical.reason(), Some(HotReloadReason::IdenticalRevision));
    assert_eq!(
        session.activate(identical).unwrap_err().reason,
        HotReloadReason::IdenticalRevision
    );
    assert_eq!(session.generation(), 1);
}

#[test]
fn changed_contract_and_superseded_pending_plan_leave_active_worker_usable() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let active = fixture.revision();
    let mut session =
        HotReloadSession::new(active, PreparedProjectInterpreterOptions::default()).unwrap();
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    session.admit_candidate(fixture.revision()).unwrap();
    let stale = session.plan().unwrap();
    fixture.rewrite("src/core.spx", "requires right != 0", "requires right > 0");
    session.admit_candidate(fixture.revision()).unwrap();
    assert_eq!(
        session.activate(stale).unwrap_err().reason,
        HotReloadReason::StaleCandidate
    );
    let incompatible = session.plan().unwrap();
    assert_eq!(
        incompatible.decision(),
        HotReloadDecision::UnsupportedRestartRequired
    );
    assert_eq!(
        incompatible.reason(),
        Some(HotReloadReason::IncompatibleClosure)
    );
    assert_eq!(
        session.activate(incompatible).unwrap_err().reason,
        HotReloadReason::IncompatibleClosure
    );
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
    session.generation = u64::MAX;
    fixture.rewrite("src/core.spx", "requires right > 0", "requires right != 0");
    session.admit_candidate(fixture.revision()).unwrap();
    let plan = session.plan().unwrap();
    assert_eq!(plan.decision(), HotReloadDecision::EligibleCodeReplacement);
    assert_eq!(
        session.activate(plan).unwrap_err().reason,
        HotReloadReason::GenerationExhausted
    );
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
}

#[test]
fn checked_identity_cases_and_first_over_bound_submission_preserve_active_code() {
    let _worker_guard = prepared_worker_test_guard();
    for (name, file, old, new, expected) in [
        (
            "display-rename",
            "src/core.spx",
            "fn multiply(",
            "fn productX(",
            Some(HotReloadDecision::EligibleCodeReplacement),
        ),
        (
            "entry-identity",
            "src/app.spx",
            "@id(\"calculator.app.main\")",
            "@id(\"calculator.app.other\")",
            Some(HotReloadDecision::UnsupportedRestartRequired),
        ),
        (
            "missing-import-identity",
            "src/core.spx",
            "@id(\"calculator.multiply\")",
            "@id(\"calculator.productX\")",
            None,
        ),
    ] {
        let fixture = Fixture::new();
        let mut session = HotReloadSession::new(
            fixture.revision(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        fixture.rewrite(file, old, new);
        match expected {
            Some(decision) => {
                session.admit_candidate(fixture.revision()).unwrap();
                assert_eq!(session.plan().unwrap().decision(), decision, "{name}");
            }
            None => {
                let diagnostics = crate::project::load_snapshot(&fixture.0.join("semaprax.toml"))
                    .err()
                    .expect("missing stable import must refuse Project admission");
                assert_eq!(diagnostics[0].code, "SPX-G172", "{name}");
            }
        }
        assert_eq!(
            observed(&session),
            ProjectPreparedExecutionOutcome::Returned(42)
        );
    }

    let fixture = Fixture::new();
    let mut session = HotReloadSession::new(
        fixture.revision(),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    session.submission = u64::MAX;
    assert_eq!(
        session
            .admit_candidate(fixture.revision())
            .unwrap_err()
            .reason,
        HotReloadReason::GenerationExhausted
    );
    assert!(session.pending.is_none());
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
}

#[test]
fn coordinator_transition_table_preserves_the_active_revision() {
    #[derive(Clone, Copy)]
    enum Transition {
        AdmitAndPlan,
        ActivateEligible,
        IncompatibleClosure,
        StaleCandidate,
        IdenticalRevision,
        GenerationOverflow,
        SubmissionFirstOverBound,
        BusyBoundary,
        TerminalUncertainty,
    }

    // The source-Agent checkpoint lifecycle has its own typed owner and its
    // table lives with that migration harness. Keep every prepared-worker
    // coordinator path here so each row starts from the same checked A state
    // and asserts the active worker result after success or refusal.
    for transition in [
        Transition::AdmitAndPlan,
        Transition::ActivateEligible,
        Transition::IncompatibleClosure,
        Transition::StaleCandidate,
        Transition::IdenticalRevision,
        Transition::GenerationOverflow,
        Transition::SubmissionFirstOverBound,
        Transition::BusyBoundary,
        Transition::TerminalUncertainty,
    ] {
        let _worker_guard = prepared_worker_test_guard();
        let fixture = Fixture::new();
        let active = fixture.revision();
        let mut session = HotReloadSession::new(
            Arc::clone(&active),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let (failure, expected_result) = match transition {
            Transition::AdmitAndPlan => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                let candidate = fixture.revision();
                session.admit_candidate(Arc::clone(&candidate)).unwrap();
                assert_eq!(
                    session.plan().unwrap().decision(),
                    HotReloadDecision::EligibleCodeReplacement
                );
                assert_eq!(
                    session.observation().pending_project_revision(),
                    Some(candidate.project_revision())
                );
                (None, Some(42))
            }
            Transition::ActivateEligible => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                let candidate = fixture.revision();
                session.admit_candidate(Arc::clone(&candidate)).unwrap();
                session.activate(session.plan().unwrap()).unwrap();
                assert_eq!(
                    session.active_project_revision(),
                    candidate.project_revision()
                );
                (None, Some(48))
            }
            Transition::IncompatibleClosure => {
                fixture.rewrite("src/core.spx", "requires right != 0", "requires right > 0");
                session.admit_candidate(fixture.revision()).unwrap();
                let plan = session.plan().unwrap();
                assert_eq!(
                    plan.decision(),
                    HotReloadDecision::UnsupportedRestartRequired
                );
                (Some(session.activate(plan).unwrap_err().reason), Some(42))
            }
            Transition::StaleCandidate => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                session.admit_candidate(fixture.revision()).unwrap();
                let stale = session.plan().unwrap();
                fixture.rewrite("src/app.spx", "multiply(6, 8)", "multiply(6, 9)");
                session.admit_candidate(fixture.revision()).unwrap();
                (Some(session.activate(stale).unwrap_err().reason), Some(42))
            }
            Transition::IdenticalRevision => {
                session.admit_candidate(Arc::clone(&active)).unwrap();
                (
                    Some(
                        session
                            .activate(session.plan().unwrap())
                            .unwrap_err()
                            .reason,
                    ),
                    Some(42),
                )
            }
            Transition::GenerationOverflow => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                session.admit_candidate(fixture.revision()).unwrap();
                session.generation = u64::MAX;
                (
                    Some(
                        session
                            .activate(session.plan().unwrap())
                            .unwrap_err()
                            .reason,
                    ),
                    Some(42),
                )
            }
            Transition::SubmissionFirstOverBound => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                session.submission = u64::MAX;
                (
                    Some(
                        session
                            .admit_candidate(fixture.revision())
                            .unwrap_err()
                            .reason,
                    ),
                    Some(42),
                )
            }
            Transition::BusyBoundary => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                session.admit_candidate(fixture.revision()).unwrap();
                let plan = session.plan().unwrap();
                session.worker.set_execution_for_test(true);
                let failure = session.activate(plan).unwrap_err().reason;
                session.worker.set_execution_for_test(false);
                (Some(failure), Some(42))
            }
            Transition::TerminalUncertainty => {
                fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
                session.admit_candidate(fixture.revision()).unwrap();
                session
                    .worker
                    .install_replacement_hook(PreparedReplacementTestHook::PanicBeforePrepare);
                let failure = session
                    .activate(session.plan().unwrap())
                    .unwrap_err()
                    .reason;
                assert!(session.terminal());
                assert_eq!(
                    session.plan().err().unwrap().reason,
                    HotReloadReason::TerminalUncertainty
                );
                (Some(failure), None)
            }
        };
        let expected_failure = match transition {
            Transition::AdmitAndPlan | Transition::ActivateEligible => None,
            Transition::IncompatibleClosure => Some(HotReloadReason::IncompatibleClosure),
            Transition::StaleCandidate => Some(HotReloadReason::StaleCandidate),
            Transition::IdenticalRevision => Some(HotReloadReason::IdenticalRevision),
            Transition::GenerationOverflow | Transition::SubmissionFirstOverBound => {
                Some(HotReloadReason::GenerationExhausted)
            }
            Transition::BusyBoundary => Some(HotReloadReason::BusyBoundary),
            Transition::TerminalUncertainty => Some(HotReloadReason::TerminalUncertainty),
        };
        assert_eq!(failure, expected_failure);
        match expected_result {
            Some(result) => assert_eq!(
                observed(&session),
                ProjectPreparedExecutionOutcome::Returned(result)
            ),
            None => assert_eq!(
                session
                    .execute_entry(
                        &PreparedProjectExecutionOptions::default(),
                        &ProjectExecutionCancellation::new()
                    )
                    .unwrap_err()[0]
                    .code,
                "SPX-HR400"
            ),
        }
    }
}

#[test]
fn changed_effect_is_refused_by_a_session_and_keeps_active_worker_usable() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::indirect_effect();
    let active = fixture.revision();
    let mut session = HotReloadSession::new(
        Arc::clone(&active),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
    fixture.rewrite("src/app.spx", "uses { process.stdout.write }", "");
    let candidate = fixture.revision();
    session.admit_candidate(candidate).unwrap();
    let plan = session.plan().unwrap();
    assert_eq!(
        plan.decision(),
        HotReloadDecision::UnsupportedRestartRequired
    );
    assert_eq!(plan.reason(), Some(HotReloadReason::IncompatibleClosure));
    assert_eq!(
        session.activate(plan).unwrap_err().reason,
        HotReloadReason::IncompatibleClosure
    );
    assert_eq!(session.active_project_revision(), active.project_revision());
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
}

#[test]
fn source_agent_checkpoint_selection_cannot_replace_the_prepared_worker() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let mut session = HotReloadSession::new(
        fixture.revision(),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    session.admit_candidate(fixture.revision()).unwrap();
    let mut selection = session.plan().unwrap();
    selection.decision = HotReloadDecision::EligibleSourceAgentCheckpointHandoff;
    selection.digest = plan_digest(
        selection.generation,
        selection.submission,
        &selection.expected_project_revision,
        &selection.expected_program_root,
        &selection.candidate_project_revision,
        &selection.candidate_program_root,
        &selection.entry_id,
        &selection.test_id,
        selection.decision,
        selection.reason,
        &selection.source_agent_handoff_digest,
    );
    assert_eq!(
        session.activate(selection).unwrap_err().reason,
        HotReloadReason::UnsupportedTarget
    );
    assert_eq!(session.generation(), 0);
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(42)
    );
}

#[test]
fn real_a_to_b_to_c_keeps_one_worker_and_binds_each_trace_to_its_revision() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let active = fixture.revision();
    let mut session = HotReloadSession::new(
        Arc::clone(&active),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    let worker = session.worker_id();
    assert_eq!(
        session.observation().lifecycle(),
        HotReloadLifecycle::Started
    );
    let a = session
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    assert_eq!(a.outcome(), &ProjectPreparedExecutionOutcome::Returned(42));
    verify_project_source_trace_against_revision(&active, a.trace().envelope()).unwrap();

    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let b = fixture.revision();
    session.admit_candidate(Arc::clone(&b)).unwrap();
    assert_eq!(
        session.observation().lifecycle(),
        HotReloadLifecycle::CandidateAdmitted
    );
    assert_eq!(
        session.observation().pending_project_revision(),
        Some(b.project_revision())
    );
    session.activate(session.plan().unwrap()).unwrap();
    assert_eq!(session.worker_id(), worker);
    assert_eq!(
        session.observation().lifecycle(),
        HotReloadLifecycle::Activated
    );
    assert_eq!(session.observation().generation(), 1);
    assert_eq!(
        session.observation().active_project_revision(),
        b.project_revision()
    );
    assert_eq!(session.observation().pending_project_revision(), None);
    let b_run = session
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    assert_eq!(
        b_run.outcome(),
        &ProjectPreparedExecutionOutcome::Returned(48)
    );
    verify_project_source_trace_against_revision(&b, b_run.trace().envelope()).unwrap();
    assert!(verify_project_source_trace_against_revision(&b, a.trace().envelope()).is_err());

    fixture.rewrite("src/app.spx", "multiply(6, 8)", "multiply(6, 9)");
    let c = fixture.revision();
    session.admit_candidate(Arc::clone(&c)).unwrap();
    session.activate(session.plan().unwrap()).unwrap();
    assert_eq!(session.worker_id(), worker);
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(54)
    );
}

#[test]
fn physical_busy_boundary_preserves_pending_plan_until_the_worker_is_idle() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let active = fixture.revision();
    let mut session = HotReloadSession::new(
        Arc::clone(&active),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let candidate = fixture.revision();
    session.admit_candidate(Arc::clone(&candidate)).unwrap();
    let plan = session.plan().unwrap();
    session.worker.set_execution_for_test(true);
    assert_eq!(
        session.activate(plan.clone()).unwrap_err().reason,
        HotReloadReason::BusyBoundary
    );
    assert_eq!(session.active_project_revision(), active.project_revision());
    session.worker.set_execution_for_test(false);
    session.activate(plan).unwrap();
    assert_eq!(
        session.active_project_revision(),
        candidate.project_revision()
    );
}

#[test]
fn held_real_invocation_waits_for_a_safe_boundary_then_activates_the_same_candidate() {
    let _worker_guard = prepared_worker_test_guard();
    let workers_before = active_prepared_worker_count_for_test();
    let fixture = Fixture::new();
    let active = fixture.revision();
    let mut session = HotReloadSession::new(
        Arc::clone(&active),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    assert_eq!(active_prepared_worker_count_for_test(), workers_before + 1);
    let worker = session.worker_id();
    let held_worker = Arc::clone(&session.worker);
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let candidate = fixture.revision();
    session.admit_candidate(Arc::clone(&candidate)).unwrap();
    let plan = session.plan().unwrap();
    let (entered, entry_started) = std::sync::mpsc::sync_channel(0);
    let (resume_entry, resume) = std::sync::mpsc::sync_channel(0);
    session
        .worker
        .install_execution_hook(ExecutionTestHook::Pause { entered, resume });
    std::thread::scope(|scope| {
        let invocation = scope.spawn(move || {
            held_worker.execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new(),
            )
        });
        assert_eq!(
            entry_started
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            worker
        );
        assert_eq!(
            session.activate(plan.clone()).unwrap_err().reason,
            HotReloadReason::BusyBoundary
        );
        assert_eq!(session.worker_id(), worker);
        assert_eq!(session.generation(), 0);
        assert_eq!(session.active_project_revision(), active.project_revision());
        assert_eq!(
            session.observation().lifecycle(),
            HotReloadLifecycle::WaitingForSafePoint
        );
        assert_eq!(
            session.observation().pending_project_revision(),
            Some(candidate.project_revision())
        );
        resume_entry.send(()).unwrap();
        let prior = invocation.join().unwrap().unwrap();
        assert_eq!(
            prior.outcome(),
            &ProjectPreparedExecutionOutcome::Returned(42)
        );
        verify_project_source_trace_against_revision(&active, prior.trace().envelope()).unwrap();
    });
    session.activate(plan).unwrap();
    assert_eq!(session.worker_id(), worker);
    assert_eq!(session.generation(), 1);
    assert_eq!(
        session.active_project_revision(),
        candidate.project_revision()
    );
    assert_eq!(
        observed(&session),
        ProjectPreparedExecutionOutcome::Returned(48)
    );
    drop(session);
    assert_eq!(active_prepared_worker_count_for_test(), workers_before);
}

#[test]
fn post_pivot_acknowledgement_loss_is_terminal_and_never_retries_the_candidate() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let active = fixture.revision();
    let mut session = HotReloadSession::new(
        Arc::clone(&active),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let candidate = fixture.revision();
    session.admit_candidate(Arc::clone(&candidate)).unwrap();
    let plan = session.plan().unwrap();
    session
        .worker
        .install_replacement_hook(PreparedReplacementTestHook::PanicAfterCommit);
    assert_eq!(
        session.activate(plan).unwrap_err().reason,
        HotReloadReason::TerminalUncertainty
    );
    assert!(session.terminal());
    assert_eq!(
        session.observation().lifecycle(),
        HotReloadLifecycle::TerminalUncertainty
    );
    assert_eq!(session.generation(), 0);
    assert_eq!(session.active_project_revision(), active.project_revision());
    assert_eq!(
        session.admit_candidate(candidate).unwrap_err().reason,
        HotReloadReason::TerminalUncertainty
    );
    assert_eq!(
        session
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new()
            )
            .unwrap_err()[0]
            .code,
        "SPX-HR400"
    );
}

#[test]
fn forged_plan_and_terminal_worker_refuse_every_later_transition() {
    let _worker_guard = prepared_worker_test_guard();
    let fixture = Fixture::new();
    let active = fixture.revision();
    let mut session = HotReloadSession::new(
        Arc::clone(&active),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    fixture.rewrite("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let candidate = fixture.revision();
    session.admit_candidate(Arc::clone(&candidate)).unwrap();
    let mut forged = session.plan().unwrap();
    forged.candidate_program_root.push('0');
    assert_eq!(
        session.activate(forged).unwrap_err().reason,
        HotReloadReason::StaleCandidate
    );
    let plan = session.plan().unwrap();
    session
        .worker
        .install_replacement_hook(PreparedReplacementTestHook::PanicBeforePrepare);
    assert_eq!(
        session.activate(plan).unwrap_err().reason,
        HotReloadReason::TerminalUncertainty
    );
    assert!(session.terminal());
    assert_eq!(
        session.admit_candidate(candidate).unwrap_err().reason,
        HotReloadReason::TerminalUncertainty
    );
    assert_eq!(
        session.plan().err().unwrap().reason,
        HotReloadReason::TerminalUncertainty
    );
    assert_eq!(
        session
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new()
            )
            .unwrap_err()[0]
            .code,
        "SPX-HR400"
    );
}
