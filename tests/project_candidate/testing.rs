//! Candidate test execution regressions authored but deliberately not run.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    verify_execution_envelope, with_authenticated_project, CandidateTestPolicy, ProjectCandidate,
    ProjectCandidateTestTaskOutcome, ProjectExecutionCancellation, ProjectExecutionOutcome,
    SemanticChange, MAX_CANDIDATE_TEST_STEPS,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-tests-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), "schema = \"semaprax.project.v1\"\nname = \"candidate-tests\"\nentry = \"test_plan.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\nweb_exports = [\"test.add\"]\ntests = [\"test_plan.tests\"]\n").unwrap();
        for (file, source) in [
            ("app", "module test_plan.app; use function @id(\"test.bridge\") from test_plan.core as bridge; @id(\"test.app.main\") fn main()->i64 {bridge(1)}"),
            ("core", "module test_plan.core; @id(\"test.add\") fn add(left:i64,right:i64)->i64 {left+right} @id(\"test.bridge\") fn bridge(value:i64)->i64 {add(value,1)} @id(\"test.unused\") fn unused(value:i64)->i64 {value}"),
            ("tests", "module test_plan.tests; use function @id(\"test.bridge\") from test_plan.core as increment; @id(\"test.local-check\") fn local_check()->i64 {if increment(1)==2 {0} else {1}} @id(\"test.tests.main\") fn main()->i64 {local_check()}"),
        ] {
            let path = format!("src/{file}.spx");
            std::fs::write(root.join(&path), semaprax::format::canonical(&semaprax::parse(source, &path).unwrap())).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
}
fn rename(target: &str, name: &str) -> Value {
    json!({"kind":"rename_declaration","target":target,"name":name})
}
fn plan(candidate: &ProjectCandidate) -> Value {
    serde_json::from_str(&candidate.test_plan(candidate.candidate_digest()).unwrap()).unwrap()
}
fn policy(steps: usize) -> CandidateTestPolicy {
    CandidateTestPolicy::new(steps, 65_536, 262_144).unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(errors.iter().any(|d| d.code == expected), "{errors:?}"),
    }
}

#[test]
fn static_selection_traverses_local_and_imported_calls_but_excludes_unused_same_module_function() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    assert_eq!(plan(&root)["selected"], false);
    let affected = apply(&root, rename("test.add", "sum"));
    let selected = plan(&affected);
    assert_eq!(selected["selected"], true);
    assert_eq!(
        selected["candidate_reachable_changed_targets"],
        json!(["test.add"])
    );
    assert_eq!(selected["test_origin"]["stable_id"], "test.tests.main");
    assert_eq!(selected["execution"], "not_run");
    let unused = apply(&root, rename("test.unused", "idle"));
    assert_eq!(plan(&unused)["selected"], false);
    let moved = apply(
        &root,
        json!({"kind":"move_declaration","target":"test.unused","destination":"test.app.main"}),
    );
    assert_eq!(plan(&moved)["selected"], true);
    assert_eq!(
        plan(&moved)["conservative_reasons"],
        json!(["module_binding_and_origin_change"])
    );
}

#[test]
fn explicit_execution_replays_candidate_and_binds_sources_diffs_policy_and_exact_envelope() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let candidate = apply(&root, rename("test.add", "sum"));
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let result = candidate
        .execute_tests(candidate.candidate_digest(), &policy(10_000))
        .unwrap();
    assert!(result.passed());
    assert_eq!(
        result.execution().outcome(),
        &ProjectExecutionOutcome::Returned(0)
    );
    verify_execution_envelope(result.execution().envelope()).unwrap();
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert_eq!(report["candidate_digest"], candidate.candidate_digest());
    assert_eq!(
        report["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        report["candidate_replay"],
        "exact_source_and_evidence_replay_before_execution"
    );
    assert_eq!(report["options"]["max_steps"], 10_000);
    assert_eq!(report["options"]["trace"]["max_events"], 0);
    assert_eq!(
        report["execution"]["envelope"],
        result.execution().envelope()
    );
    assert_eq!(report["source_diffs"].as_array().unwrap().len(), 1);
    let mut diff_inventory = report["source_diffs"].clone();
    diff_inventory.sort_all_objects();
    let diff_bytes = format!("{}\n", serde_json::to_string(&diff_inventory).unwrap());
    let mut diff_hash = Sha256::new();
    diff_hash.update(b"semaprax.candidate-test.diffs.v1\0");
    diff_hash.update((diff_bytes.len() as u64).to_le_bytes());
    diff_hash.update(diff_bytes.as_bytes());
    assert_eq!(
        report["source_diff_inventory_digest"],
        format!(
            "sha256:{}",
            diff_hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    );
    assert_eq!(report["test_origin"]["path"], "src/tests.spx");
    let mut hash = Sha256::new();
    hash.update(b"semaprax.candidate-test.report.v1\0");
    hash.update((result.to_json().len() as u64).to_le_bytes());
    hash.update(result.to_json().as_bytes());
    assert_eq!(
        result.report_digest(),
        format!(
            "sha256:{}",
            hash.finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    );
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
    let repeated = candidate
        .execute_tests(candidate.candidate_digest(), &policy(10_000))
        .unwrap();
    assert_eq!(repeated.to_json(), result.to_json());
}

#[test]
fn cancellation_aware_completion_preserves_exact_legacy_report_bytes() {
    let fixture = Fixture::new();
    let candidate = apply(&fixture.candidate(), rename("test.add", "sum"));
    let policy = policy(10_000);
    let legacy = candidate
        .execute_tests(candidate.candidate_digest(), &policy)
        .unwrap();
    let completed = candidate
        .execute_tests_cancellable(
            candidate.candidate_digest(),
            &policy,
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    let ProjectCandidateTestTaskOutcome::Completed(completed) = completed else {
        panic!("uncancelled candidate test unexpectedly cancelled");
    };
    assert_eq!(completed.to_json(), legacy.to_json());
    assert_eq!(completed.report_digest(), legacy.report_digest());
    assert_eq!(completed.execution(), legacy.execution());
}

#[test]
fn pre_cancelled_candidate_test_releases_no_report_and_uses_zero_fuel() {
    let fixture = Fixture::new();
    let candidate = apply(&fixture.candidate(), rename("test.add", "sum"));
    let policy = policy(10_000);
    let cancellation = ProjectExecutionCancellation::new();
    cancellation.cancel();
    let outcome = candidate
        .execute_tests_cancellable(candidate.candidate_digest(), &policy, &cancellation)
        .unwrap();
    match outcome {
        ProjectCandidateTestTaskOutcome::Cancelled {
            before_step,
            steps_used,
            max_steps,
        } => {
            assert_eq!(before_step, 1);
            assert_eq!(steps_used, 0);
            assert_eq!(max_steps, 10_000);
        }
        ProjectCandidateTestTaskOutcome::Completed(_) => {
            panic!("pre-cancelled candidate test released a report")
        }
    }
    assert_eq!(
        candidate
            .execute_tests_cancellable(
                fixture.candidate().candidate_digest(),
                &policy,
                &cancellation,
            )
            .expect_err("stale selector must reject before cancellation")[0]
            .code,
        "SPX-G224",
        "cancellation must not bypass exact candidate authentication"
    );
}

#[test]
fn explicit_test_executes_full_declared_closure_even_when_static_plan_is_empty() {
    let fixture = Fixture::new();
    let candidate = apply(&fixture.candidate(), rename("test.unused", "idle"));
    assert_eq!(plan(&candidate)["selected"], false);
    let result = candidate
        .execute_tests(candidate.candidate_digest(), &policy(10_000))
        .unwrap();
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert_eq!(report["statically_selected"], false);
    assert_eq!(
        report["execution_scope"],
        "complete_manifest_declared_test_closure"
    );
    assert!(result.passed());
    assert!(result.execution().steps_used() > 1);
}

#[test]
fn failing_tests_and_fuel_exhaustion_never_become_success_reports() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let failing = apply(
        &root,
        json!({"kind":"replace_function_body","target":"test.add","body":{"kind":"i64","value":0}}),
    );
    let failure = failing
        .execute_tests(failing.candidate_digest(), &policy(10_000))
        .unwrap();
    assert!(!failure.passed());
    assert_eq!(
        failure.execution().outcome(),
        &ProjectExecutionOutcome::Returned(1)
    );
    let exhausted = root
        .execute_tests(root.candidate_digest(), &policy(1))
        .unwrap();
    assert!(!exhausted.passed());
    assert_eq!(
        exhausted.execution().outcome(),
        &ProjectExecutionOutcome::FuelExhausted
    );
    assert!(exhausted.execution().steps_used() <= 1);
}

#[test]
fn host_policy_and_stale_candidate_checks_are_closed_and_do_not_mutate_candidates() {
    code(CandidateTestPolicy::new(0, 65_536, 262_144), "SPX-G239");
    code(
        CandidateTestPolicy::new(MAX_CANDIDATE_TEST_STEPS + 1, 65_536, 262_144),
        "SPX-G239",
    );
    code(CandidateTestPolicy::new(100, 65_537, 262_144), "SPX-G239");
    code(CandidateTestPolicy::new(100, 65_536, 1), "SPX-G239");
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let candidate = apply(&root, rename("test.add", "sum"));
    let before = candidate.to_json().to_owned();
    code(candidate.test_plan(root.candidate_digest()), "SPX-G224");
    code(
        candidate.execute_tests(root.candidate_digest(), &policy(100)),
        "SPX-G224",
    );
    assert_eq!(candidate.to_json(), before);
}
