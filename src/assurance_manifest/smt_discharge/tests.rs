//! Integration tests for the bounded SMT discharge engine: script
//! rendering, orchestration over [`super::solver::Verdict`], the
//! obligation-id collision this tranche's "Integration status" documents,
//! and (only under `--ignored`, with `SEMAPRAX_SMT_Z3_PATH` provisioned) a
//! real end-to-end run against Z3.
//!
//! Every `#[ignore]` test here follows
//! `tests/public_generic_native_adapter_v1/fixture.rs`'s existing pattern
//! for an externally provisioned tool: skipped by a bare `cargo test`,
//! run only by an operator who explicitly opts in with `--ignored` and has
//! set the provisioning environment variable named in the reason string.

use std::time::Duration;

use super::*;
use crate::assurance_manifest::{
    generate, obligation_id, AssuranceManifestOptions, ExternalRecords, Obligation, ObligationKind,
};
use crate::ast::Function;

fn function(source: &str) -> Function {
    let mut program = crate::parse(source, "smt-discharge-test.spx").expect("parse");
    program.functions.swap_remove(0)
}

/// Every executable module needs `fn main() -> i64`, exactly like
/// `assurance_manifest`'s own `write_temp` test helper appends; `generate()`
/// otherwise rejects the module before `derive_obligations` ever runs.
fn write_temp(source: &str, label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-smt-discharge-{label}-{}-{}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &path,
        format!("{source}\n@id(\"app.t.smt_discharge_test_main\")\nfn main() -> i64 {{ 0 }}\n"),
    )
    .unwrap();
    path
}

fn short_limits() -> RunLimits {
    RunLimits {
        timeout: Duration::from_secs(5),
        max_output_bytes: 65_536,
    }
}

// ---------------------------------------------------------------------
// Script rendering
// ---------------------------------------------------------------------

#[test]
fn postcondition_script_has_the_expected_shape() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
    );
    let encoding = translate_function(&f).expect("supported");
    let script = render_postcondition_script(&encoding, 0, 2000);
    assert!(script.contains("(set-option :timeout 2000)"));
    assert!(script.contains("(set-logic QF_LIA)"));
    assert!(script.contains("(declare-const a Int)"));
    assert!(script.contains("(assert (>= a 0))"));
    assert!(script.contains("(assert (not"));
    assert!(script.contains("(check-sat)"));
    assert!(script.contains("(get-model)"));
}

#[test]
fn a_derived_arithmetic_value_never_gets_an_unconditional_range_axiom() {
    // Regression for a real unsoundness this tranche's own development
    // found against a live Z3: at `a == i64::MAX`, `a + 1` overflows, so
    // if `result`'s declaration carried the same unconditional i64 range
    // axiom a genuine parameter gets, that axiom would directly contradict
    // `result`'s definitional equality to `a + 1` and make the whole query
    // vacuously (and wrongly) `unsat` — reporting a real overflow as
    // proved. `result`'s range must instead only ever appear as a guarded
    // *obligation* on the raw arithmetic term.
    let f = function(&format!(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a == {}\n    ensures result > a\n{{ a + 1 }}\n",
        i64::MAX
    ));
    let encoding = translate_function(&f).expect("supported");
    let script = render_postcondition_script(&encoding, 0, 2000);
    assert!(!script.contains("(and (>= result"));
    assert!(script.contains("(assert (= result (+ a 1)))"));
    assert!(script.contains("(>= (+ a 1)"));
}

#[test]
fn the_same_function_renders_a_byte_identical_script_every_time() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
    );
    let first = render_postcondition_script(&translate_function(&f).unwrap(), 0, 2000);
    let second = render_postcondition_script(&translate_function(&f).unwrap(), 0, 2000);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------
// Orchestration without a real solver process
// ---------------------------------------------------------------------

#[test]
fn discharge_is_inconclusive_with_a_closed_reason_when_no_solver_is_provisioned() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
    );
    let outcome = discharge_postcondition(&f, 0, None, &short_limits());
    match outcome {
        DischargeOutcome::Inconclusive { reason } => assert!(reason.contains(ENV_Z3_PATH)),
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

#[test]
fn discharge_is_inconclusive_for_an_unsupported_function_without_touching_any_process() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64, b: i64) -> i64\n    ensures result >= 0\n{ a / b }\n",
    );
    let outcome = discharge_postcondition(&f, 0, None, &short_limits());
    match outcome {
        DischargeOutcome::Inconclusive { reason } => assert!(reason.contains("unsupported")),
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

#[test]
fn precondition_consistency_is_vacuous_with_no_requires_clauses() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result >= 0\n{ a }\n",
    );
    let outcome = discharge_precondition_consistency(&f, None, &short_limits());
    match outcome {
        DischargeOutcome::Inconclusive { reason } => assert!(reason.contains("vacuously")),
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// `to_method_record`: the closed mapping from outcome to assurance record
// ---------------------------------------------------------------------

#[test]
fn a_proved_outcome_becomes_an_smt_proved_method_record_with_a_proof_ref() {
    let outcome = DischargeOutcome::Proved {
        script_digest: "sha256:abc".to_owned(),
        solver_identity: "z3",
        solver_version: "Z3 version 4.13.0".to_owned(),
    };
    let record = to_method_record(&outcome, Duration::from_secs(2)).expect("a record");
    assert_eq!(
        record.class,
        crate::assurance_manifest::AssuranceClass::SmtProved
    );
    assert_eq!(record.tool, "z3");
    assert_eq!(record.proof_ref.as_deref(), Some("sha256:abc"));
}

#[test]
fn an_inconclusive_outcome_becomes_an_attempt_inconclusive_record_with_no_proof_ref() {
    let outcome = DischargeOutcome::Inconclusive {
        reason: "solver returned unknown".to_owned(),
    };
    let record = to_method_record(&outcome, Duration::from_secs(2)).expect("a record");
    assert_eq!(
        record.class,
        crate::assurance_manifest::AssuranceClass::AttemptInconclusive
    );
    assert_eq!(record.proof_ref, None);
    assert_eq!(record.detail.as_deref(), Some("solver returned unknown"));
}

#[test]
fn a_refuted_outcome_never_becomes_a_method_record() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result > a\n{ a }\n",
    );
    let model: Model = [("a".to_owned(), ModelValue::Int(5))].into_iter().collect();
    let replay = replay_function(&f, &model).unwrap();
    let outcome = DischargeOutcome::Refuted {
        replay,
        script_digest: "sha256:abc".to_owned(),
    };
    assert!(to_method_record(&outcome, Duration::from_secs(2)).is_none());
}

// ---------------------------------------------------------------------
// Obligation-id parity, and today's real integration gap (see the
// module doc's "Integration status"): an external SMT method record
// cannot yet be merged into the SAME obligation `derive_obligations`
// already populated with a `runtime_guarded` record. `generate()` fails
// closed with SPX-Z101 rather than silently dropping or overwriting
// either record.
// ---------------------------------------------------------------------

#[test]
fn postcondition_obligation_id_matches_derive_rs_locator_convention_exactly() {
    let expected = obligation_id(ObligationKind::Postcondition, "app.t.f", "ensure:0");
    assert_eq!(postcondition_obligation_id("app.t.f", 0), expected);
}

#[test]
fn merging_an_smt_method_into_an_already_derived_obligation_fails_closed_today() {
    let source = "module app.t;\n\n@id(\"app.t.check\")\nfn check(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n";
    let path = write_temp(source, "collision");

    // The plain generate() call already derives a `runtime_guarded`
    // postcondition obligation for `ensure:0`.
    let plain = generate(&path, &AssuranceManifestOptions::default());
    assert!(plain.is_ok(), "baseline generate() must succeed");

    // Attempting to attach this module's SmtProved finding to that exact
    // same obligation id, the only way `ExternalRecords` supports today,
    // collides rather than merges.
    let outcome = DischargeOutcome::Proved {
        script_digest: "sha256:deadbeef".to_owned(),
        solver_identity: "z3",
        solver_version: "Z3 version 4.13.0".to_owned(),
    };
    let method = to_method_record(&outcome, Duration::from_secs(2)).expect("a record");
    let external = ExternalRecords {
        obligations: vec![Obligation::new(
            ObligationKind::Postcondition,
            "app.t.check",
            "ensure:0",
        )
        .with_method(method)],
        assumptions: Vec::new(),
    };
    let options = AssuranceManifestOptions::default().with_external_records(external);
    let result = generate(&path, &options);
    std::fs::remove_file(&path).ok();

    let diagnostics = result.expect_err(
        "today, an external record sharing an already-derived obligation id fails closed \
         (SPX-Z101); see docs/SMT-DISCHARGE-V1.md \"Integration status\" for the follow-up \
         this documents rather than silently works around",
    );
    assert_eq!(diagnostics[0].code, "SPX-Z101");
    assert!(diagnostics[0]
        .message
        .contains("collided with an automatically derived obligation"));
}

// ---------------------------------------------------------------------
// Real end-to-end coverage against a provisioned Z3. Skipped by a bare
// `cargo test`; run explicitly with:
//   SEMAPRAX_SMT_Z3_PATH=/opt/homebrew/bin/z3 \
//     cargo test --locked -p semaprax --lib assurance_manifest::smt_discharge \
//     -- --ignored
// ---------------------------------------------------------------------

fn provisioned() -> Provisioning {
    provision_from_env().expect(
        "this test is #[ignore]d and must only be run with SEMAPRAX_SMT_Z3_PATH set to an \
         absolute path to a provisioned z3 binary",
    )
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_proves_a_true_postcondition() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
    );
    let provisioning = provisioned();
    let outcome = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
    match outcome {
        DischargeOutcome::Proved {
            solver_identity,
            solver_version,
            ..
        } => {
            assert_eq!(solver_identity, "z3");
            assert_ne!(solver_version, "unrecorded");
        }
        other => panic!("expected Proved, got {other:?}"),
    }
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_refutes_a_false_postcondition_with_a_validated_counterexample() {
    // `result > a` is false whenever the body just returns `a`; this must
    // never be provable, and any model the solver returns must replay as
    // a genuine `EnsuresViolated`.
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result > a\n{ a }\n",
    );
    let provisioning = provisioned();
    let outcome = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
    match outcome {
        DischargeOutcome::Refuted { replay, .. } => {
            assert!(matches!(
                replay,
                ReplayOutcome::EnsuresViolated { ensures_index: 0 }
            ));
        }
        other => panic!("expected a validated Refuted counterexample, got {other:?}"),
    }
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_refutes_an_overflow_at_the_exact_i64_extremum() {
    // Overflow boundary + exact extremum, as the issue's required test
    // list names explicitly: `a + 1` traps precisely when `a == i64::MAX`.
    let f = function(&format!(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a == {}\n    ensures result > a\n{{ a + 1 }}\n",
        i64::MAX
    ));
    let provisioning = provisioned();
    let outcome = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
    match outcome {
        DischargeOutcome::Refuted { replay, .. } => {
            assert!(matches!(replay, ReplayOutcome::Trapped { .. }));
        }
        other => panic!("expected a validated Trapped counterexample, got {other:?}"),
    }
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_reports_a_contradictory_precondition() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a > 0\n    requires a < 0\n    ensures result >= 0\n{ a }\n",
    );
    let provisioning = provisioned();
    let outcome = discharge_precondition_consistency(&f, Some(&provisioning), &short_limits());
    match outcome {
        DischargeOutcome::Inconclusive { reason } => assert!(reason.contains("contradictory")),
        other => panic!("expected a contradictory-precondition finding, got {other:?}"),
    }
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_covers_every_admitted_numeric_type() {
    let provisioning = provisioned();
    let cases = [
        "module app.t;\n@id(\"app.t.i64\")\nfn i64_f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
        "module app.t;\n@id(\"app.t.i32\")\nfn i32_f(a: i32) -> i32\n    requires a >= 0i32\n    ensures result >= 0i32\n{ a }\n",
        "module app.t;\n@id(\"app.t.u8\")\nfn u8_f(a: u8) -> u8\n    ensures result >= 0u8\n{ a }\n",
        "module app.t;\n@id(\"app.t.usize\")\nfn usize_f(a: usize) -> usize\n    ensures result >= 0usize\n{ a }\n",
        "module app.t;\n@id(\"app.t.bool\")\nfn bool_f(a: bool) -> bool\n    ensures result == a\n{ a }\n",
    ];
    for source in cases {
        let f = function(source);
        let outcome = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
        assert!(
            matches!(outcome, DischargeOutcome::Proved { .. }),
            "{source}: expected Proved, got {outcome:?}"
        );
    }
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_result_is_stable_across_repeated_runs_and_the_cache_key_is_reusable() {
    let f = function(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
    );
    let provisioning = provisioned();
    let encoding = translate_function(&f).unwrap();
    let script = render_postcondition_script(&encoding, 0, 5000);
    let key_input = CacheKeyInput {
        obligation_locator: "app.t.f:ensure:0",
        script: &script,
        solver_identity: provisioning.identity,
        solver_version: &solver_version(&provisioning).unwrap_or_else(|| "unrecorded".to_owned()),
        timeout: Duration::from_secs(5),
        assumption_ids: &[],
        target_policy: "none",
    };
    let key = cache_key(&key_input);

    let mut cache: DischargeCache<String> = DischargeCache::new();
    assert_eq!(cache.get(&key), None);
    let first = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
    assert!(matches!(first, DischargeOutcome::Proved { .. }));
    cache.insert(key.clone(), "proved".to_owned());

    // A second run reuses the identical key; a real caller would skip the
    // solver call entirely on this hit.
    assert_eq!(cache.get(&key), Some("proved".to_owned()));
    let second = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
    assert!(matches!(second, DischargeOutcome::Proved { .. }));
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_handles_branch_sensitive_conditions() {
    let f = function(
        r#"
module app.t;
@id("app.t.f")
fn f(a: i64) -> i64
    ensures result >= 0
{ if a > 0 { a } else { 0 } }
"#,
    );
    let provisioning = provisioned();
    let outcome = discharge_postcondition(&f, 0, Some(&provisioning), &short_limits());
    assert!(matches!(outcome, DischargeOutcome::Proved { .. }));
}
