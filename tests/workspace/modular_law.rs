//! Retained two-module HIR identity planning for later modular law proofs.
use std::path::PathBuf;

use semaprax::assurance_manifest::modular_law::{plan, Refusal};
use semaprax::project::with_authenticated_project;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str, core: &str, extra: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-modular-law-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let main_body = if extra.contains("fn repeat(") {
            "total(1) + repeat(1)"
        } else {
            "total(1)"
        };
        let app = format!(
            "module accounting.app;\nuse function @id(\"accounting.base\") from accounting.core as base;\nuse function @id(\"accounting.tax\") from accounting.core as tax;\n@id(\"accounting.total\")\nfn total(value: i64) -> i64\n requires value >= 0\n requires value <= 100\n ensures result == value + 3\n{{ tax(base(value)) }}\n@id(\"accounting.main\") fn main() -> i64 {{ {main_body} }}\n{extra}"
        );
        let tests =
            "module accounting.tests;\n@id(\"accounting.tests.main\") fn main() -> i64 { 0 }\n";
        for (name, text) in [("app", app.as_str()), ("core", core), ("tests", tests)] {
            let path = root.join(format!("src/{name}.spx"));
            let canonical = semaprax::format::canonical(&semaprax::parse(text, &path).unwrap());
            std::fs::write(path, canonical).unwrap();
        }
        std::fs::write(root.join("semaprax.toml"),
            "schema = \"semaprax.project.v8\"\nname = \"accounting-law\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"accounting.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\nweb_exports = []\ntests = [\"accounting.tests\"]\n"
        ).unwrap();
        Self { root }
    }

    fn plan(&self) -> Result<semaprax::assurance_manifest::modular_law::Plan, Refusal> {
        let revision = with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        plan(&revision, "accounting.total")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const CORE: &str = "module accounting.core;\n@id(\"accounting.base\")\nfn base(value: i64) -> i64\n requires value >= 0\n requires value <= 100\n ensures result == value + 1\n{ value + 1 }\n@id(\"accounting.tax\")\nfn tax(value: i64) -> i64\n requires value >= 1\n requires value <= 101\n ensures result == value + 2\n{ value + 2 }\n";

#[test]
fn three_function_two_module_summary_plan_is_topological_and_exact() {
    let fixture = Fixture::new("positive", CORE, "");
    let plan = fixture.plan().expect("linked direct pure calls");
    assert_eq!(
        plan.summaries
            .iter()
            .map(|row| row.declaration_id.as_str())
            .collect::<Vec<_>>(),
        ["accounting.base", "accounting.tax", "accounting.total"]
    );
    assert_eq!(plan.summaries[2].dependencies.len(), 2);
    assert_ne!(
        plan.summaries[2].dependencies[0].digest,
        plan.summaries[2].dependencies[1].digest
    );
}

#[test]
fn callee_change_stales_transitive_summary_but_unrelated_function_does_not() {
    let baseline = Fixture::new("baseline", CORE, "").plan().unwrap();
    let changed_core = CORE.replace("value + 2 }", "value + 3 }");
    let changed = Fixture::new("changed", &changed_core, "").plan().unwrap();
    assert_ne!(baseline.summaries[1].digest, changed.summaries[1].digest);
    assert_ne!(baseline.summaries[2].digest, changed.summaries[2].digest);
    let unrelated = Fixture::new(
        "unrelated",
        CORE,
        "@id(\"accounting.unrelated\") fn unrelated(value: i64) -> i64 { value }\n",
    )
    .plan()
    .unwrap();
    assert_eq!(baseline.summaries[2].digest, unrelated.summaries[2].digest);
    assert_ne!(baseline.project_revision, unrelated.project_revision);
}

#[test]
fn repeated_calls_have_distinct_resolved_occurrence_identities() {
    let extra = "@id(\"accounting.repeat\")\nfn repeat(value: i64) -> i64\n requires value >= 0\n requires value <= 100\n ensures result == value + value + 2\n{ base(value) + base(value) }\n";
    let fixture = Fixture::new("repeated", CORE, extra);
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let plan = plan(&revision, "accounting.repeat").unwrap();
    let calls = &plan.summaries.last().unwrap().calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].callee, "accounting.base");
    assert_eq!(calls[1].callee, "accounting.base");
    assert_ne!(calls[0].expression_id, calls[1].expression_id);
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn real_z3_proves_three_function_two_module_chain() {
    use semaprax::assurance_manifest::modular_law::prove::prove_postconditions;
    use semaprax::assurance_manifest::smt_discharge::{provision_from_env, RunLimits};
    let fixture = Fixture::new("z3-positive", CORE, "");
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let solver = provision_from_env().expect("explicit installed Z3");
    let proof = prove_postconditions(
        &revision,
        "accounting.total",
        Some(&solver),
        &RunLimits::default(),
    )
    .expect("every transitive postcondition proved");
    assert_eq!(proof.clauses.len(), 3);
    assert_eq!(proof.clauses[2].declaration_id, "accounting.total");
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn real_z3_exposes_caller_precondition_witness() {
    use semaprax::assurance_manifest::modular_law::prove::{prove_postconditions, ProofFailure};
    use semaprax::assurance_manifest::smt_discharge::{
        provision_from_env, ModelValue, ReplayOutcome, RunLimits,
    };
    let core = CORE.replace("requires value <= 100", "requires value <= 99");
    let fixture = Fixture::new("z3-negative", &core, "");
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let solver = provision_from_env().expect("explicit installed Z3");
    match prove_postconditions(
        &revision,
        "accounting.total",
        Some(&solver),
        &RunLimits::default(),
    ) {
        Err(ProofFailure::Counterexample {
            declaration_id,
            model,
            replay: ReplayOutcome::Trapped { .. },
            ..
        }) => {
            assert_eq!(declaration_id, "accounting.total");
            assert_eq!(model.get("value"), Some(&ModelValue::Int(100)));
        }
        other => panic!("expected checked caller witness: {other:?}"),
    }
}

#[test]
fn self_and_mutual_summary_cycles_refuse() {
    let self_call = CORE.replace("{ value + 1 }", "{ base(value) }");
    let fixture = Fixture::new("self-cycle", &self_call, "");
    assert_eq!(fixture.plan().unwrap_err().code(), "cyclic_summary");

    let mutual = CORE
        .replace("{ value + 1 }", "{ tax(value) }")
        .replace("{ value + 2 }", "{ base(value) }");
    let fixture = Fixture::new("mutual-cycle", &mutual, "");
    assert_eq!(fixture.plan().unwrap_err().code(), "cyclic_summary");
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn real_z3_repeated_calls_and_shadowed_value_are_capture_free() {
    use semaprax::assurance_manifest::modular_law::prove::prove_postconditions;
    use semaprax::assurance_manifest::smt_discharge::{provision_from_env, RunLimits};
    // `base` owns a local named `value`, the same as the caller parameter.
    // The linked value IDs must keep those two bindings separate.
    let core = CORE.replacen(
        "fn base(value: i64) -> i64\n requires value >= 0\n requires value <= 100\n ensures result == value + 1\n{ value + 1 }",
        "fn base(input: i64) -> i64\n requires input >= 0\n requires input <= 100\n ensures result == input + 1\n{ let value = input + 1; value }",
        1,
    );
    let extra = "@id(\"accounting.repeat\")\nfn repeat(value: i64) -> i64\n requires value >= 0\n requires value <= 100\n ensures result == value + value + 2\n{ base(value) + base(value) }\n";
    let fixture = Fixture::new("z3-shadow", &core, extra);
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let solver = provision_from_env().expect("explicit installed Z3");
    let proof = prove_postconditions(
        &revision,
        "accounting.repeat",
        Some(&solver),
        &RunLimits::default(),
    )
    .expect("same authored name in caller and callee remains capture free");
    assert_eq!(proof.clauses.len(), 2);
    assert_eq!(proof.plan.summaries[1].calls.len(), 2);
}
#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn real_z3_checked_summaries_compose_without_caller_body_inlining() {
    use semaprax::assurance_manifest::modular_law::summary::prove_straight_line;
    use semaprax::assurance_manifest::smt_discharge::{provision_from_env, RunLimits};
    let fixture = Fixture::new("z3-summary", CORE, "");
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let solver = provision_from_env().expect("explicit installed Z3");
    let proof = prove_straight_line(
        &revision,
        "accounting.total",
        Some(&solver),
        &RunLimits::default(),
    )
    .expect("checked base/tax summaries compose for total");
    assert_eq!(proof.checked_callee_clauses.len(), 2);
    assert_eq!(proof.caller_precondition_scripts.len(), 6);
    assert_eq!(proof.caller_postcondition_scripts.len(), 1);
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn real_z3_summary_use_refuses_a_weakened_callee_contract() {
    use semaprax::assurance_manifest::modular_law::summary::{prove_straight_line, ModularFailure};
    use semaprax::assurance_manifest::smt_discharge::{provision_from_env, RunLimits};
    let weak_tax = CORE.replace("ensures result == value + 2", "ensures result >= value + 2");
    let fixture = Fixture::new("z3-weak-summary", &weak_tax, "");
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let solver = provision_from_env().expect("explicit installed Z3");
    let outcome = prove_straight_line(
        &revision,
        "accounting.total",
        Some(&solver),
        &RunLimits::default(),
    );
    assert!(
        matches!(outcome, Err(ModularFailure::Postcondition { .. })),
        "{outcome:?}"
    );
}
