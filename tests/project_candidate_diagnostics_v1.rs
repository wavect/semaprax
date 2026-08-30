//! Rejected candidate attempt and typed repair evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateAttempt,
    ProjectCandidateAttemptOutcome, ProjectRevision, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-attempt-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = self.revision();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn rejected(base: &Arc<ProjectCandidate>, intent: &Value) -> Arc<ProjectCandidateAttempt> {
    match ProjectCandidateAttempt::apply(Arc::clone(base), base.candidate_digest(), intent).unwrap()
    {
        ProjectCandidateAttemptOutcome::Rejected(attempt) => attempt,
        ProjectCandidateAttemptOutcome::Accepted(_) => panic!("expected rejected intention"),
    }
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}

fn wire_repair(base: &Arc<ProjectCandidate>) -> Value {
    let attempt = rejected(
        base,
        &json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":42}}),
    );
    let catalog: Value =
        serde_json::from_str(&attempt.repair_catalog(attempt.attempt_digest()).unwrap()).unwrap();
    catalog["repairs"][0]["semantic_change_intent"].clone()
}

#[test]
fn repair_wire_preserves_actual_intention_history_and_rederives_on_recovery() {
    let fixture = Fixture::new();
    let base = Arc::new(fixture.candidate());
    let source_before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let intent = wire_repair(&base);
    let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
    let repaired = base.apply(base.candidate_digest(), &change).unwrap();
    let report: Value = serde_json::from_str(repaired.to_json()).unwrap();
    assert_eq!(report["operations"][0]["kind"], "repair_diagnostic");
    let capsule = repaired.recovery_capsule().unwrap();
    let recovery: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(recovery["changes"][0]["intent"], intent);
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), repaired.to_json());
    let replay = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        repaired.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), repaired.to_json());
    let ordinary = base.apply(base.candidate_digest(), &SemanticChange::new(base.revision().project_revision(), &json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":42}})).unwrap()).unwrap();
    assert_eq!(
        ordinary.revision().project_revision(),
        repaired.revision().project_revision()
    );
    assert_ne!(ordinary.candidate_digest(), repaired.candidate_digest());
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        source_before
    );
}

#[test]
fn repair_wire_rejects_tampering_recursion_and_successful_attempts() {
    let fixture = Fixture::new();
    let base = Arc::new(fixture.candidate());
    let intent = wire_repair(&base);
    let mut changed_literal = intent.clone();
    changed_literal["rejected_intent"]["body"]["value"] = json!(43);
    let mut successful = intent.clone();
    successful["rejected_intent"]["body"]["kind"] = json!("i64");
    let mut mismatched_target = intent.clone();
    mismatched_target["rejected_intent"]["target"] = json!("calculator.subtract");
    let mut recursive = intent.clone();
    recursive["rejected_intent"] = intent.clone();
    let mut offered = intent.clone();
    offered["replacement"] = json!({"kind":"i64","value":99});
    let before = base.to_json().to_owned();
    for (intent, expected) in [
        (changed_literal, "SPX-G270"),
        (successful, "SPX-G270"),
        (mismatched_target, "SPX-G268"),
        (recursive, "SPX-G268"),
        (offered, "SPX-G268"),
    ] {
        let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
        code(base.apply(base.candidate_digest(), &change), expected);
        assert_eq!(base.to_json(), before);
    }
}

#[test]
fn repair_selector_is_exact_predecessor_bound_and_rebase_requires_rediscovery() {
    let fixture = Fixture::new();
    let base = Arc::new(fixture.candidate());
    let intent = wire_repair(&base);
    let repaired = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap();
    let renamed = base.apply(base.candidate_digest(), &SemanticChange::new(base.revision().project_revision(), &json!({"kind":"rename_declaration","target":"calculator.subtract","name":"difference"})).unwrap()).unwrap();
    let stale_change = SemanticChange::new(renamed.revision().project_revision(), &intent).unwrap();
    code(
        renamed.apply(renamed.candidate_digest(), &stale_change),
        "SPX-G270",
    );
    code(
        repaired.rebase(
            repaired.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        ),
        "SPX-G271",
    );
}

#[test]
fn rejected_attempt_retains_exact_diagnostics_and_one_fully_admitted_numeric_repair() {
    let fixture = Fixture::new();
    let base = Arc::new(fixture.candidate());
    let original = base.to_json().to_owned();
    let source = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let intent = json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":42}});
    let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
    let expected = base
        .apply(base.candidate_digest(), &change)
        .err()
        .expect("wrong return type fails");
    let attempt = rejected(&base, &intent);
    let report: Value = serde_json::from_str(attempt.to_json()).unwrap();
    assert_eq!(report["change"]["intent"], intent);
    assert_eq!(report["base_candidate_revision"], base.candidate_digest());
    assert_eq!(report["materializable"], false);
    assert_eq!(report["checked_image"], false);
    assert!(report.get("candidate_revision").is_none());
    assert!(report.get("sources").is_none());
    assert_eq!(report["target_provenance"]["id"], "calculator.add");
    assert_eq!(report["target_provenance"]["path"], "src/core.spx");
    assert_eq!(
        report["diagnostics"].as_array().unwrap().len(),
        expected.len()
    );
    for (actual, expected) in report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected)
    {
        assert_eq!(actual["code"], expected.code);
        assert_eq!(actual["message"], expected.message);
        assert_eq!(
            actual["location_basis"],
            "uncommitted_attempt_or_constructor_input_not_authenticated_base_span"
        );
    }
    let catalogue: Value =
        serde_json::from_str(&attempt.repair_catalog(attempt.attempt_digest()).unwrap()).unwrap();
    let repairs = catalogue["repairs"].as_array().unwrap();
    assert_eq!(repairs.len(), 1);
    assert_eq!(repairs[0]["expected_type"], "i64");
    assert_eq!(repairs[0]["preserved_integer_value"], 42);
    assert_eq!(repairs[0]["validation"], "normal_full_candidate_apply");
    let repaired = attempt
        .repair_diagnostic(
            attempt.attempt_digest(),
            repairs[0]["repair_id"].as_str().unwrap(),
        )
        .unwrap();
    let ordinary = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(
                base.revision().project_revision(),
                &repairs[0]["change"]["intent"],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(repaired.to_json(), ordinary.to_json());
    assert_eq!(
        repairs[0]["validated_candidate_revision"],
        repaired.candidate_digest()
    );
    assert_eq!(
        attempt.to_json(),
        serde_json::from_str::<Value>(attempt.to_json())
            .map(|mut value| {
                value.sort_all_objects();
                format!("{value}\n")
            })
            .unwrap()
    );
    assert_eq!(base.to_json(), original);
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        source
    );
}

#[test]
fn unsupported_or_out_of_range_repairs_are_explicit_and_bound_to_the_attempt() {
    let fixture = Fixture::new();
    let base = Arc::new(fixture.candidate());
    for body in [
        json!({"kind":"bool","value":true}),
        json!({"kind":"place","name":"missing"}),
        json!({"kind":"usize","value":u64::MAX}),
    ] {
        let attempt = rejected(
            &base,
            &json!({"kind":"replace_function_body","target":"calculator.add","body":body}),
        );
        let catalogue: Value =
            serde_json::from_str(&attempt.repair_catalog(attempt.attempt_digest()).unwrap())
                .unwrap();
        assert!(catalogue["repairs"].as_array().unwrap().is_empty());
        assert!(catalogue["availability_reason"].as_str().is_some());
        code(
            attempt.repair_diagnostic(
                attempt.attempt_digest(),
                &format!("sha256:{}", "0".repeat(64)),
            ),
            "SPX-G243",
        );
        code(attempt.summary(base.candidate_digest()), "SPX-G243");
    }
}

#[test]
fn successful_attempts_return_normal_candidates_and_capacity_stays_an_outer_error() {
    let fixture = Fixture::new();
    let base = Arc::new(fixture.candidate());
    let intent = json!({"kind":"rename_declaration","target":"calculator.add","name":"sum"});
    let accepted =
        ProjectCandidateAttempt::apply(Arc::clone(&base), base.candidate_digest(), &intent)
            .unwrap();
    let ProjectCandidateAttemptOutcome::Accepted(accepted) = accepted else {
        panic!("valid intent must be accepted")
    };
    let normal = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap();
    assert_eq!(accepted.to_json(), normal.to_json());
    assert!(ProjectCandidateAttempt::apply(
        Arc::clone(&base),
        accepted.candidate_digest(),
        &intent
    )
    .is_err());
    assert!(ProjectCandidateAttempt::apply(
        Arc::clone(&base),
        base.candidate_digest(),
        &json!({"unknown":"x".repeat(1024*1024+1)})
    )
    .is_err());
}
