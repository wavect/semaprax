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
        } else if extra.contains("fn lazy(") {
            "if lazy(1) { total(1) } else { 0 }"
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

    fn rewrite_app(&self, change: impl FnOnce(String) -> String) {
        let path = self.root.join("src/app.spx");
        let changed = change(std::fs::read_to_string(&path).unwrap());
        let canonical = semaprax::format::canonical(&semaprax::parse(&changed, &path).unwrap());
        std::fs::write(path, canonical).unwrap();
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
fn callee_precondition_and_summary_contract_changes_stale_caller_identity() {
    let baseline = Fixture::new("contract-baseline", CORE, "").plan().unwrap();
    for (label, index, core) in [
        (
            "changed-requires",
            0,
            CORE.replacen("requires value <= 100", "requires value <= 99", 1),
        ),
        (
            "changed-ensures",
            1,
            CORE.replace("ensures result == value + 2", "ensures result >= value + 2"),
        ),
    ] {
        let changed = Fixture::new(label, &core, "").plan().unwrap();
        assert_ne!(
            baseline.summaries[index].digest,
            changed.summaries[index].digest
        );
        assert_ne!(baseline.summaries[2].digest, changed.summaries[2].digest);
    }
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
fn dynamic_and_generic_summary_calls_refuse_before_solver_invocation() {
    let dynamic = Fixture::new("dynamic-refusal", CORE, "");
    dynamic.rewrite_app(|source| {
        source.replace(
            "    tax(base(value))",
            "    let callback = local; tax(callback(value))",
        )
    });
    dynamic.rewrite_app(|source| {
        source + "\n@id(\"accounting.local\") fn local(value: i64) -> i64 { value }\n"
    });
    assert_eq!(dynamic.plan().unwrap_err().code(), "dynamic_call");

    let generic = Fixture::new(
        "generic-refusal",
        CORE,
        "@id(\"accounting.identity\") fn identity<T>(value: T) -> T { value }\n",
    );
    generic.rewrite_app(|source| {
        source.replace(
            "    tax(base(value))",
            "    tax(identity<i64>(base(value)))",
        )
    });
    assert_eq!(generic.plan().unwrap_err().code(), "generic_function");
}

#[test]
fn foreign_summary_call_refuses_before_solver_invocation() {
    let fixture = Fixture::new("foreign-refusal", CORE, "");
    fixture.rewrite_app(|source| {
        source
            .replace(
                "@id(\"accounting.total\")",
                "@id(\"accounting.host\") interface Host permits {} { @id(\"accounting.echo\") import rust fn echo(value: i64) -> i64 effects {} failure infallible; }\n@id(\"accounting.total\")",
            )
            .replace("    tax(base(value))", "    echo(value)")
    });
    assert_eq!(fixture.plan().unwrap_err().code(), "foreign_call");
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

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn real_z3_summary_certificate_replays_and_classifies_dependency_drift() {
    use semaprax::assurance_manifest::modular_law::certificate::{
        classify_drift, export, replay, Drift, ReplayFailure,
    };
    use semaprax::assurance_manifest::smt_discharge::{provision_from_env, RunLimits};
    let solver = provision_from_env().expect("explicit installed Z3");
    let baseline = Fixture::new("certificate-baseline", CORE, "");
    let baseline_revision =
        with_authenticated_project(&baseline.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    let certificate = export(
        &baseline_revision,
        "accounting.total",
        &solver,
        &RunLimits::default(),
    )
    .expect("real solver creates source-bound certificate");
    assert_eq!(
        classify_drift(&certificate, &baseline_revision, "accounting.total").unwrap(),
        Drift::Exact
    );
    assert_eq!(
        replay(
            &certificate,
            &baseline_revision,
            "accounting.total",
            &solver,
            &RunLimits::default()
        )
        .unwrap()
        .caller_postcondition_scripts
        .len(),
        1
    );

    let changed_core = CORE.replace("{ value + 2 }", "{ value + 3 }");
    let changed = Fixture::new("certificate-changed", &changed_core, "");
    let changed_revision =
        with_authenticated_project(&changed.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    assert_eq!(
        classify_drift(&certificate, &changed_revision, "accounting.total").unwrap(),
        Drift::DependencyChanged
    );

    let unrelated = Fixture::new(
        "certificate-unrelated",
        CORE,
        "@id(\"accounting.unrelated\") fn unrelated(value: i64) -> i64 { value }\n",
    );
    let unrelated_revision =
        with_authenticated_project(&unrelated.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    assert_eq!(
        classify_drift(&certificate, &unrelated_revision, "accounting.total").unwrap(),
        Drift::UnrelatedRevision
    );
    assert!(matches!(
        replay(
            &certificate,
            &unrelated_revision,
            "accounting.total",
            &solver,
            &RunLimits::default()
        ),
        Err(ReplayFailure::Stale)
    ));
}

#[test]
fn branching_summary_profile_refuses_before_solver_invocation() {
    use semaprax::assurance_manifest::modular_law::summary::{prove_straight_line, ModularFailure};
    use semaprax::assurance_manifest::smt_discharge::RunLimits;
    let fixture = Fixture::new("branch-refusal", CORE, "");
    let path = fixture.root.join("src/app.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let changed = source.replace(
        "    tax(base(value))",
        "    if value >= 0 { tax(base(value)) } else { tax(base(value)) }",
    );
    assert_ne!(source, changed);
    let canonical = semaprax::format::canonical(&semaprax::parse(&changed, &path).unwrap());
    std::fs::write(path, canonical).unwrap();
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    match prove_straight_line(&revision, "accounting.total", None, &RunLimits::default()) {
        Err(ModularFailure::Refused(Refusal::BranchingSummary { .. })) => {}
        other => panic!("expected named branch refusal: {other:?}"),
    }
}

#[test]
fn lazy_summary_profile_refuses_before_solver_invocation() {
    use semaprax::assurance_manifest::modular_law::summary::{prove_straight_line, ModularFailure};
    use semaprax::assurance_manifest::smt_discharge::RunLimits;
    let extra = "@id(\"accounting.lazy\")\nfn lazy(value: i64) -> bool\n requires value >= 0\n requires value <= 100\n ensures result == true\n{ value >= 0 && base(value) >= 1 }\n";
    let fixture = Fixture::new("lazy-refusal", CORE, extra);
    let revision = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    match prove_straight_line(&revision, "accounting.lazy", None, &RunLimits::default()) {
        Err(ModularFailure::Refused(Refusal::LazySummary { .. })) => {}
        other => panic!("expected named lazy summary refusal: {other:?}"),
    }
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn installed_modular_postcondition_attaches_to_selected_strict_project() {
    use semaprax::agent_runtime::AgentCancellation;
    use semaprax::assurance_manifest::law_set::{
        strict::{RequiredLawEvidence, StrictLawPolicy},
        LawSet,
    };
    use semaprax::project::{
        install_host_strict_law_policy, with_strict_authenticated_project, ProjectExecutionOptions,
    };
    use semaprax::proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits},
        installed_project::prove_modular_postcondition,
    };
    use std::collections::BTreeMap;
    let fixture = Fixture::new("selected-modular", CORE, "");
    let native = "module accounting.laws;\n@id(\"accounting.total.law\")\nlaw contract \"accounting.total\" ensures (value: i64, result: i64)\n result == value + 3\n evidence smt_proved;\n";
    let law_path = fixture.root.join("src/contracts.spx");
    let parsed = semaprax::native_law_source::parse(native, "src/contracts.spx").unwrap();
    std::fs::write(&law_path, semaprax::native_law_source::canonical(&parsed)).unwrap();
    std::fs::write(fixture.root.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"accounting-law\"\nversion = \"1.0.0\"\n\n[modules]\nentry = \"accounting.app\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/core.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"accounting.tests\"]\n\n[exports]\nweb = [\"accounting.total\"]\n").unwrap();
    let manifest = fixture.root.join("semaprax.toml");
    let revision =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    let tool_path =
        std::path::PathBuf::from(std::env::var("SEMAPRAX_LAW_Z3").expect("installed Z3 path"));
    let version = std::env::var("SEMAPRAX_LAW_Z3_VERSION").expect("installed Z3 pin");
    let tool = InstalledProofTool::open_modular_scalar(
        &tool_path,
        &fixture.root,
        &version,
        HostProfile::TrustedLocal,
        Limits::default(),
        AgentCancellation::new(),
    )
    .unwrap();
    let laws = LawSet::derive(
        &revision,
        "modular-proof-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "accounting.total.law".into(),
            RequiredLawEvidence::PinnedModularSmtSource {
                toolchain: version.clone(),
                accepted_translation: semaprax::assurance_manifest::modular_law::BOUNDS_V1.into(),
            },
        )]),
    )
    .unwrap();
    install_host_strict_law_policy(&manifest, &policy, vec![]).unwrap();
    let proof = prove_modular_postcondition(&revision, "src/app.spx", "accounting.total", 0, &tool)
        .expect("registered installed Z3 proves selected modular postcondition");
    with_strict_authenticated_project(&manifest, &[proof.clone()], &[], |session| {
        session.execute_entry(&ProjectExecutionOptions::default())?;
        Ok(())
    })
    .expect("selected strict Project accepts exact modular proof");
    let error =
        with_strict_authenticated_project(&manifest, &[], &[], |_session| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-LW130");
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn installed_modular_postcondition_attaches_to_selected_strict_workspace_publication() {
    use semaprax::agent_runtime::AgentCancellation;
    use semaprax::assurance_manifest::law_set::{
        protected::{
            ProtectedLawBaseline, ProtectedLawReview, SpecificationChangeApproval,
            SpecificationChangeAuthority,
        },
        strict::{RequiredLawEvidence, StrictLawPolicy},
        LawSet,
    };
    use semaprax::project::{
        apply_strict_law_publication, install_host_strict_law_policy,
        prepare_strict_law_publication, ProjectCandidate, SemanticChange, StrictCandidateLawInputs,
    };
    use semaprax::proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits},
        installed_project::prove_modular_postcondition,
    };
    use std::collections::BTreeMap;

    struct Host;
    impl SpecificationChangeAuthority for Host {
        fn approve_specification_change(&mut self, _: &ProtectedLawReview) -> bool {
            true
        }
    }
    let fixture = Fixture::new("selected-modular-workspace", CORE, "");
    let native = "module accounting.laws;\n@id(\"accounting.total.law\")\nlaw contract \"accounting.total\" ensures (value: i64, result: i64)\n result == value + 3\n evidence smt_proved;\n";
    let law_path = fixture.root.join("src/contracts.spx");
    let parsed = semaprax::native_law_source::parse(native, "src/contracts.spx").unwrap();
    std::fs::write(&law_path, semaprax::native_law_source::canonical(&parsed)).unwrap();
    std::fs::write(fixture.root.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"accounting-law\"\nversion = \"1.0.0\"\n\n[modules]\nentry = \"accounting.app\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/core.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"accounting.tests\"]\n\n[exports]\nweb = [\"accounting.total\"]\n").unwrap();
    let paths = fixture.root.join("paths.json");
    std::fs::write(&paths, concat!(r#"{"schema":"semaprax.workspace-semantic-path-set.v1","files":[{"path":"src/app.spx"},{"path":"src/contracts.spx"},{"path":"src/core.spx"},{"path":"src/tests.spx"}]}"#, "\n")).unwrap();
    let workspace = semaprax::semantic_workspace::initialize(&fixture.root, &paths).unwrap();
    let manifest = fixture.root.join("semaprax.toml");
    let base =
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    let tool_path =
        std::path::PathBuf::from(std::env::var("SEMAPRAX_LAW_Z3").expect("installed Z3 path"));
    let version = std::env::var("SEMAPRAX_LAW_Z3_VERSION").expect("installed Z3 pin");
    let tool = InstalledProofTool::open_modular_scalar(
        &tool_path,
        &fixture.root,
        &version,
        HostProfile::TrustedLocal,
        Limits::default(),
        AgentCancellation::new(),
    )
    .unwrap();
    let baseline = LawSet::derive(&base, "modular-proof-v1", base.law_modules().to_vec()).unwrap();
    let policy = StrictLawPolicy::new(
        baseline.clone(),
        BTreeMap::from([(
            "accounting.total.law".into(),
            RequiredLawEvidence::PinnedModularSmtSource {
                toolchain: version,
                accepted_translation: semaprax::assurance_manifest::modular_law::BOUNDS_V1.into(),
            },
        )]),
    )
    .unwrap();
    let protection = ProtectedLawBaseline::new(&base, baseline, vec![]).unwrap();
    install_host_strict_law_policy(&manifest, &policy, vec![]).unwrap();
    let old_proof =
        prove_modular_postcondition(&base, "src/app.spx", "accounting.total", 0, &tool).unwrap();

    let start = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
    let change = SemanticChange::new(
        base.project_revision(),
        &serde_json::json!({
            "kind":"change_function_signature", "target":"accounting.base",
            "append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]
        }),
    )
    .unwrap();
    let candidate = start.apply(start.candidate_digest(), &change).unwrap();
    let laws = LawSet::derive(
        candidate.revision(),
        "modular-proof-v1",
        candidate.revision().law_modules().to_vec(),
    )
    .unwrap();
    let proof = prove_modular_postcondition(
        candidate.revision(),
        "src/app.spx",
        "accounting.total",
        0,
        &tool,
    )
    .unwrap();
    let intent = candidate.protected_law_review(&protection, &laws).unwrap();
    let approval = SpecificationChangeApproval::request(&intent, &mut Host).unwrap();
    let stale_proofs = [old_proof];
    let stale = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &laws,
        proofs: &stale_proofs,
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    let proofs = [proof];
    let inputs = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &laws,
        proofs: &proofs,
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    let active = fixture.root.join(".semaprax-workspace/ACTIVE");
    let before = std::fs::read(&active).unwrap();
    assert!(prepare_strict_law_publication(
        &candidate,
        &stale,
        candidate.candidate_digest(),
        &fixture.root,
        &manifest,
        &workspace,
    )
    .is_err());
    assert_eq!(std::fs::read(&active).unwrap(), before);
    let proposal = prepare_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &fixture.root,
        &manifest,
        &workspace,
    )
    .unwrap();
    assert_eq!(std::fs::read(&active).unwrap(), before);
    let missing = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &laws,
        proofs: &[],
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    assert!(prepare_strict_law_publication(
        &candidate,
        &missing,
        candidate.candidate_digest(),
        &fixture.root,
        &manifest,
        &workspace,
    )
    .is_err());
    assert_eq!(std::fs::read(&active).unwrap(), before);
    apply_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &fixture.root,
        &manifest,
        &workspace,
        proposal.to_json().as_bytes(),
    )
    .unwrap();
    assert_ne!(std::fs::read(&active).unwrap(), before);
}
