use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::assurance_manifest::{
    obligation_id, AssuranceManifestOptions, ExternalRecords, MethodRecord, Obligation,
    ObligationKind,
};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// One function with a `requires` clause, a parameter, and the standalone
/// `main` a single-file `assurance_manifest::generate` call requires
/// (`src/source_verify/declaration.rs`: "executable module must define `fn
/// main() -> i64`"), matching the fixture pattern
/// `src/project/candidate/candidate_assurance.rs` and
/// `tests/projections/assurance_manifest.rs` already use.
const APP_SOURCE: &str = "module app.req;\n\
@id(\"app.req.divide\") fn divide(left: i64, right: i64) -> i64\n\
    requires right != 0\n\
{\n    left / right\n}\n\
@id(\"app.req.main\") fn main() -> i64 { divide(4, 2) }\n";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-requirement-traceability-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn generate(path: &Path) -> String {
    assurance_manifest::generate(path, &AssuranceManifestOptions::default())
        .expect("fixture source compiles and verifies")
}

fn precondition_id() -> String {
    obligation_id(ObligationKind::Precondition, "app.req.divide", "require:0")
}

fn ownership_param_id() -> String {
    obligation_id(
        ObligationKind::OwnershipParameter,
        "app.req.divide",
        "param:0",
    )
}

// --- construction bounds -----------------------------------------------

#[test]
fn requirement_rejects_empty_id() {
    let error = Requirement::new("", "title").unwrap_err();
    assert_eq!(error.code, "SPX-Z401");
}

#[test]
fn requirement_rejects_oversized_id() {
    let huge = "x".repeat(MAX_REQUIREMENT_ID_BYTES + 1);
    let error = Requirement::new(huge, "title").unwrap_err();
    assert_eq!(error.code, "SPX-Z401");
}

#[test]
fn criterion_rejects_empty_source_path_and_obligation_id() {
    let empty_path = RequirementCriterion::new("", "obligation", AssuranceClass::Open).unwrap_err();
    assert_eq!(empty_path.code, "SPX-Z401");
    let empty_obligation =
        RequirementCriterion::new("path.spx", "", AssuranceClass::Open).unwrap_err();
    assert_eq!(empty_obligation.code, "SPX-Z401");
}

#[test]
fn duplicate_criterion_naming_the_same_exact_subject_is_rejected() {
    let criterion_a =
        RequirementCriterion::new("app.spx", "obligation-1", AssuranceClass::RuntimeGuarded)
            .unwrap();
    let criterion_b =
        RequirementCriterion::new("app.spx", "obligation-1", AssuranceClass::CompilerProved)
            .unwrap();
    let requirement = Requirement::new("req.dup", "title")
        .unwrap()
        .with_criterion(criterion_a)
        .unwrap();
    let error = requirement.with_criterion(criterion_b).unwrap_err();
    assert_eq!(error.code, "SPX-Z403");
}

#[test]
fn evaluate_requirement_rejects_zero_criteria() {
    let requirement = Requirement::new("req.empty", "title").unwrap();
    let error = evaluate_requirement(&requirement, &[]).unwrap_err();
    assert_eq!(error.code, "SPX-Z401");
}

#[test]
fn evaluate_requirement_rejects_ambiguous_duplicate_evidence_paths() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.dup-evidence", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [
        EvidenceInput {
            source_path: &path_text,
            envelope: &envelope,
        },
        EvidenceInput {
            source_path: &path_text,
            envelope: &envelope,
        },
    ];
    let error = evaluate_requirement(&requirement, &evidence).unwrap_err();
    assert_eq!(error.code, "SPX-Z403");
    cleanup(&path);
}

// --- positive path -------------------------------------------------------

#[test]
fn a_criterion_met_by_current_evidence_is_satisfied() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.satisfied", "divide never panics on a zero divisor")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];
    let report_text = evaluate_requirement(&requirement, &evidence).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["schema"], SCHEMA);
    assert_eq!(report["satisfaction"], "satisfied");
    assert_eq!(report["criteria"][0]["status"], "satisfied");
    assert_eq!(report["criteria"][0]["achieved_class"], "runtime_guarded");
    cleanup(&path);
}

// --- unmet ---------------------------------------------------------------

#[test]
fn a_criterion_below_its_required_minimum_is_unmet_and_the_requirement_fails() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    // runtime_guarded does not dominate smt_proved (they are incomparable),
    // so requiring smt_proved here must be reported unmet, never satisfied.
    let requirement = Requirement::new("req.unmet", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::SmtProved,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];
    let report_text = evaluate_requirement(&requirement, &evidence).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["satisfaction"], "failed");
    assert_eq!(report["criteria"][0]["status"], "unmet");
    assert_eq!(report["criteria"][0]["achieved_class"], "runtime_guarded");
    cleanup(&path);
}

// --- partial ---------------------------------------------------------------

#[test]
fn one_satisfied_and_one_unmet_criterion_is_partial() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.partial", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap()
        .with_criterion(
            // compiler_proved does not dominate smt_proved: incomparable.
            RequirementCriterion::new(
                path_text.clone(),
                ownership_param_id(),
                AssuranceClass::SmtProved,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];
    let report_text = evaluate_requirement(&requirement, &evidence).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["satisfaction"], "partial");
    let statuses: Vec<&str> = report["criteria"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["status"].as_str().unwrap())
        .collect();
    assert!(statuses.contains(&"satisfied"));
    assert!(statuses.contains(&"unmet"));
    cleanup(&path);
}

// --- dangling --------------------------------------------------------------

#[test]
fn a_reference_to_an_obligation_absent_from_current_evidence_is_dangling_and_fails() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    let fabricated_id = obligation_id(ObligationKind::Precondition, "app.req.divide", "require:99");
    let requirement = Requirement::new("req.dangling", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                fabricated_id,
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];
    let report_text = evaluate_requirement(&requirement, &evidence).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["satisfaction"], "failed");
    assert_eq!(report["criteria"][0]["status"], "dangling");
    assert!(report["criteria"][0]["achieved_class"].is_null());
    cleanup(&path);
}

// --- unevaluable -------------------------------------------------------------

#[test]
fn a_criterion_with_no_supplied_evidence_is_unevaluable() {
    let requirement = Requirement::new("req.unevaluable", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new("nowhere.spx", "some-obligation", AssuranceClass::Open)
                .unwrap(),
        )
        .unwrap();
    let report_text = evaluate_requirement(&requirement, &[]).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["satisfaction"], "unevaluable");
    assert_eq!(report["criteria"][0]["status"], "unevaluable");
}

// --- assumed -----------------------------------------------------------------

#[test]
fn a_criterion_met_only_through_an_explicit_assumption_is_reported_assumed_not_satisfied() {
    let path = write_temp(APP_SOURCE);
    let assumed_obligation =
        Obligation::new(ObligationKind::Effect, "app.req.divide", "assume:0").with_method(
            MethodRecord::new(AssuranceClass::Assumed, "human-reviewer", "n/a"),
        );
    let assumed_id = assumed_obligation.id.clone();
    let options = AssuranceManifestOptions::default().with_external_records(ExternalRecords {
        obligations: vec![assumed_obligation],
        assumptions: vec![],
    });
    let envelope = assurance_manifest::generate(&path, &options)
        .expect("fixture source compiles and verifies");
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.assumed", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(path_text.clone(), assumed_id, AssuranceClass::Assumed)
                .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];
    let report_text = evaluate_requirement(&requirement, &evidence).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["satisfaction"], "assumed");
    assert_eq!(report["criteria"][0]["status"], "assumed");
    assert_eq!(report["criteria"][0]["achieved_class"], "assumed");
    cleanup(&path);
}

// --- stale / drift: the exact-subject binding this module exists for -------

/// The crux of "exact assurance subjects": a criterion evaluated against
/// *unchanged* current source is satisfied; the identical requirement and
/// the identical (unregenerated) envelope evaluated after the file's bytes
/// change on disk must flip to `stale`, never silently keep reporting the
/// old, now-approximate verdict. This is the negative drift control: it
/// proves the transition is caused by the byte change, not by some earlier,
/// unrelated check, by first establishing the positive baseline with the
/// exact same requirement/envelope pair.
#[test]
fn source_drift_after_evidence_was_generated_is_reported_stale_not_satisfied() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.drift", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];

    // Baseline: unchanged source, same envelope -> satisfied.
    let baseline = evaluate_requirement(&requirement, &evidence).unwrap();
    let baseline: serde_json::Value = serde_json::from_str(&baseline).unwrap();
    assert_eq!(baseline["satisfaction"], "satisfied");

    // Mutate the file on disk without regenerating the envelope.
    std::fs::write(
        &path,
        "module app.req;\n\
@id(\"app.req.divide\") fn divide(left: i64, right: i64) -> i64\n\
    requires right != 0\n\
    requires left >= 0\n\
{\n    left / right\n}\n\
@id(\"app.req.main\") fn main() -> i64 { divide(4, 2) }\n",
    )
    .unwrap();

    let drifted = evaluate_requirement(&requirement, &evidence).unwrap();
    let drifted: serde_json::Value = serde_json::from_str(&drifted).unwrap();
    assert_eq!(drifted["satisfaction"], "stale");
    assert_eq!(drifted["criteria"][0]["status"], "stale");

    // Round trip: a freshly regenerated envelope against the new bytes is
    // satisfied again, proving the pipeline itself was never broken -- only
    // the stale, unregenerated evidence was correctly refused.
    let regenerated = generate(&path);
    let fresh_evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &regenerated,
    }];
    let fresh = evaluate_requirement(&requirement, &fresh_evidence).unwrap();
    let fresh: serde_json::Value = serde_json::from_str(&fresh).unwrap();
    assert_eq!(fresh["satisfaction"], "satisfied");

    cleanup(&path);
}

/// A negative control proving the drift test fails at the digest/drift
/// check specifically (`SPX-Z104`), not at some earlier structural check:
/// directly calling the underlying replay on the mutated file reproduces
/// the same code this module downgrades to `stale`.
#[test]
fn drift_is_detected_by_the_documented_assurance_manifest_code() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    std::fs::write(&path, format!("{APP_SOURCE}// a trailing comment\n")).unwrap();
    let error = assurance_manifest::verify_envelope_against_source(&envelope, &path).unwrap_err();
    assert_eq!(error.code, "SPX-Z104");
    cleanup(&path);
}

// --- report determinism -----------------------------------------------------

#[test]
fn evaluate_requirement_is_deterministic_across_repeated_calls() {
    let path = write_temp(APP_SOURCE);
    let envelope = generate(&path);
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.deterministic", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                ownership_param_id(),
                AssuranceClass::CompilerProved,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: &envelope,
    }];
    let first = evaluate_requirement(&requirement, &evidence).unwrap();
    let second = evaluate_requirement(&requirement, &evidence).unwrap();
    assert_eq!(first, second);
    cleanup(&path);
}

#[test]
fn criterion_status_tokens_round_trip_through_from_token() {
    for status in CriterionStatus::ALL {
        assert_eq!(CriterionStatus::from_token(status.token()), Some(status));
    }
    assert_eq!(CriterionStatus::from_token("bogus"), None);
}

#[test]
fn a_non_drift_envelope_malformation_is_propagated_not_downgraded() {
    // A structurally invalid envelope (not even valid JSON) must surface its
    // own `crate::assurance_manifest` failure through `evaluate_requirement`
    // rather than being silently classified `stale`/`unevaluable`.
    let path = write_temp(APP_SOURCE);
    let path_text = path.display().to_string();
    let requirement = Requirement::new("req.malformed", "title")
        .unwrap()
        .with_criterion(
            RequirementCriterion::new(
                path_text.clone(),
                precondition_id(),
                AssuranceClass::RuntimeGuarded,
            )
            .unwrap(),
        )
        .unwrap();
    let evidence = [EvidenceInput {
        source_path: &path_text,
        envelope: "not json at all",
    }];
    let error = evaluate_requirement(&requirement, &evidence).unwrap_err();
    assert_ne!(error.code, "SPX-Z104");
    assert!(error.code.starts_with("SPX-Z10"));
    cleanup(&path);
}
