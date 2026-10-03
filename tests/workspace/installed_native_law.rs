//! Real native-law proofs are generated from exact retained typed declarations.
use super::*;
use semaprax::assurance_manifest::law_set::native_proof::prove_scalar_law;
use semaprax::assurance_manifest::law_set::native_proof::prove_scalar_law_lean_cached;
use semaprax::assurance_manifest::law_set::native_proof::prove_scalar_law_z3_cached;
use semaprax::assurance_manifest::modular_law::cache::ProofTaskCache;

fn native_project(label: &str, proposition: &str) -> Project {
    let project = Project::new(label, false);
    let text = format!("module fresh.laws;\n@id(\"fresh.law.identity\")\nlaw relational (n: i64)\n {proposition}\n evidence smt_proved;\n");
    let parsed = semaprax::native_law_source::parse(&text, "src/contracts.spx").unwrap();
    std::fs::write(
        project.root.join("src/contracts.spx"),
        semaprax::native_law_source::canonical(&parsed),
    )
    .unwrap();
    std::fs::write(project.root.join("semaprax.toml"), "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"fresh-law\"\nversion = \"1.0.0\"\n\n[modules]\nentry = \"app.fresh\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"app.tests\"]\n\n[exports]\nweb = [\"fresh.seventeen\"]\n").unwrap();
    project
}

#[test]
#[ignore = "requires explicitly provisioned installed Lean"]
fn installed_native_relational_lean_cache_reuses_checked_report_and_rebinds_law() {
    let project = native_project("native-relational-lean-cache", "n + 0 == n");
    let revision = project.revision();
    let laws = LawSet::derive(
        &revision,
        "native-proof-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let tool = provisioned(&project, ToolKind::Lean);
    let mut cache = ProofTaskCache::for_project(&project.root).unwrap();
    let (cold, cold_work) = prove_scalar_law_lean_cached(
        &project.root,
        &revision,
        &laws,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .unwrap();
    assert_eq!((cold_work.fresh, cold_work.reused), (1, 0));
    let (warm, warm_work) = prove_scalar_law_lean_cached(
        &project.root,
        &revision,
        &laws,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .unwrap();
    assert_eq!((warm_work.fresh, warm_work.reused), (0, 1));
    let policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
    )
    .unwrap();
    let cold_report =
        strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[cold]).unwrap();
    let warm_report =
        strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[warm.clone()]).unwrap();
    assert_eq!(cold_report, warm_report);
    strict::require_with_native_proofs(
        &warm_report,
        &revision,
        &laws,
        &policy,
        &[],
        &[warm.clone()],
    )
    .unwrap();

    let profiled = LawSet::derive(
        &revision,
        "native-proof-v2",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let (_, profile_work) = prove_scalar_law_lean_cached(
        &project.root,
        &revision,
        &profiled,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .unwrap();
    assert_eq!(
        (profile_work.fresh, profile_work.reused, profile_work.stale),
        (1, 0, 1)
    );

    let changed = "module fresh.laws;\n@id(\"fresh.law.identity\")\nlaw relational (n: i64)\n n + 0 != n\n evidence smt_proved;\n";
    let parsed = semaprax::native_law_source::parse(changed, "src/contracts.spx").unwrap();
    std::fs::write(
        project.root.join("src/contracts.spx"),
        semaprax::native_law_source::canonical(&parsed),
    )
    .unwrap();
    let current = project.revision();
    let changed_laws =
        LawSet::derive(&current, "native-proof-v1", current.law_modules().to_vec()).unwrap();
    assert!(prove_scalar_law_lean_cached(
        &project.root,
        &current,
        &changed_laws,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .is_err());
    assert_eq!(
        strict::derive_with_native_proofs(&current, &changed_laws, &policy, &[], &[warm])
            .unwrap_err()[0]
            .code,
        "SPX-LW104"
    );
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn installed_native_relational_cache_reuses_checked_query_and_rebinds_current_law() {
    let project = native_project("native-relational-cache", "n + 0 == n");
    let revision = project.revision();
    let laws = LawSet::derive(
        &revision,
        "native-proof-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let tool = provisioned(&project, ToolKind::Z3);
    let mut cache = ProofTaskCache::for_project(&project.root).unwrap();
    let (cold, cold_work) = prove_scalar_law_z3_cached(
        &project.root,
        &revision,
        &laws,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .unwrap();
    assert_eq!((cold_work.fresh, cold_work.reused), (1, 0));
    let (warm, warm_work) = prove_scalar_law_z3_cached(
        &project.root,
        &revision,
        &laws,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .unwrap();
    assert_eq!((warm_work.fresh, warm_work.reused), (0, 1));
    let policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
    )
    .unwrap();
    let cold_report =
        strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[cold]).unwrap();
    let warm_report =
        strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[warm.clone()]).unwrap();
    assert_eq!(cold_report, warm_report);
    strict::require_with_native_proofs(
        &warm_report,
        &revision,
        &laws,
        &policy,
        &[],
        &[warm.clone()],
    )
    .unwrap();

    // A changed native law statement cannot borrow the old checked success.
    let changed = "module fresh.laws;\n@id(\"fresh.law.identity\")\nlaw relational (n: i64)\n n + 0 != n\n evidence smt_proved;\n";
    let parsed = semaprax::native_law_source::parse(changed, "src/contracts.spx").unwrap();
    std::fs::write(
        project.root.join("src/contracts.spx"),
        semaprax::native_law_source::canonical(&parsed),
    )
    .unwrap();
    let current = project.revision();
    let changed_laws =
        LawSet::derive(&current, "native-proof-v1", current.law_modules().to_vec()).unwrap();
    assert!(prove_scalar_law_z3_cached(
        &project.root,
        &current,
        &changed_laws,
        "fresh.law.identity",
        &tool,
        &mut cache,
    )
    .is_err());
    assert_eq!(
        strict::derive_with_native_proofs(&current, &changed_laws, &policy, &[], &[warm])
            .unwrap_err()[0]
            .code,
        "SPX-LW104"
    );
}

#[test]
#[ignore = "requires explicitly provisioned installed Lean and Z3"]
fn installed_native_law_checks_exact_typed_proposition_and_rejects_false_missing_stale_forged() {
    let project = native_project("native", "n + 0 == n");
    let revision = project.revision();
    let laws = LawSet::derive(
        &revision,
        "native-proof-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    for kind in [ToolKind::Lean, ToolKind::Z3] {
        let tool = provisioned(&project, kind);
        let proof = prove_scalar_law(&revision, &laws, "fresh.law.identity", &tool).unwrap();
        let policy = StrictLawPolicy::new(
            laws.clone(),
            BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
        )
        .unwrap();
        let proofs = [proof.clone()];
        let report =
            strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &proofs).unwrap();
        strict::require_with_native_proofs(&report, &revision, &laws, &policy, &[], &proofs)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(value["counts"]["satisfied"], 1);
        assert_eq!(
            value["laws"][0]["evidence"]["scope"],
            "universal_typed_scalar_law"
        );
        assert_eq!(value["laws"][0]["evidence"]["proved_lowering"], false);
        // Report bytes, including a valid report, cannot replace host-held proofs.
        assert_eq!(
            strict::require(&report, &revision, &laws, &policy, &[]).unwrap_err()[0].code,
            "SPX-LW104"
        );
        let mut forged = value;
        forged["laws"][0]["evidence"]["proof_digest"] = "forged".into();
        assert_eq!(
            strict::require_with_native_proofs(
                &serde_json::to_string(&forged).unwrap(),
                &revision,
                &laws,
                &policy,
                &[],
                &proofs
            )
            .unwrap_err()[0]
                .code,
            "SPX-LW104"
        );
        assert_eq!(
            strict::derive_with_native_proofs(
                &revision,
                &laws,
                &policy,
                &[],
                &[proof.clone(), proof.clone()]
            )
            .unwrap_err()[0]
                .code,
            "SPX-LW104"
        );
        let candidate = semaprax::project::ProjectCandidate::open(
            revision.clone(),
            revision.project_revision(),
        )
        .unwrap();
        let candidate_report = candidate
            .strict_law_assurance_with_native_proofs(
                candidate.candidate_digest(),
                &laws,
                &policy,
                &[],
                &proofs,
            )
            .unwrap();
        candidate
            .require_strict_law_assurance_with_native_proofs(
                &candidate_report,
                &laws,
                &policy,
                &[],
                &proofs,
            )
            .unwrap();
        assert!(candidate
            .require_strict_law_assurance(&candidate_report, &laws, &policy, &[])
            .is_err());
        let false_project = native_project("native-false", "n + 0 != n");
        let false_revision = false_project.revision();
        let false_laws = LawSet::derive(
            &false_revision,
            "native-proof-v1",
            false_revision.law_modules().to_vec(),
        )
        .unwrap();
        assert!(prove_scalar_law(
            &false_revision,
            &false_laws,
            "fresh.law.identity",
            &provisioned(&false_project, kind)
        )
        .is_err());
        // Integer identities that overflow at a boundary are not total laws.
        let overflowing = native_project("native-overflow", "n + 1 > n");
        let overflowing_revision = overflowing.revision();
        let overflowing_laws = LawSet::derive(
            &overflowing_revision,
            "native-proof-v1",
            overflowing_revision.law_modules().to_vec(),
        )
        .unwrap();
        assert!(prove_scalar_law(
            &overflowing_revision,
            &overflowing_laws,
            "fresh.law.identity",
            &provisioned(&overflowing, kind)
        )
        .is_err());
        let false_policy = StrictLawPolicy::new(
            false_laws.clone(),
            BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
        )
        .unwrap();
        assert_eq!(
            strict::derive_with_native_proofs(
                &false_revision,
                &false_laws,
                &false_policy,
                &[],
                &proofs
            )
            .unwrap_err()[0]
                .code,
            "SPX-LW104"
        );
        // Source evidence cannot be promoted to a proof of backend lowering.
        let lowering = StrictLawPolicy::new(
            laws.clone(),
            BTreeMap::from([(
                "fresh.law.identity".into(),
                RequiredLawEvidence::VerifiedLowering,
            )]),
        )
        .unwrap();
        let refused =
            strict::derive_with_native_proofs(&revision, &laws, &lowering, &[], &proofs).unwrap();
        assert_eq!(
            strict::require_with_native_proofs(&refused, &revision, &laws, &lowering, &[], &proofs)
                .unwrap_err()[0]
                .code,
            "SPX-LW130"
        );
    }
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn installed_native_law_proofs_preserve_open_assumptions_and_prerequisites() {
    let project = native_project("native-open", "n + 0 == n");
    let revision = project.revision();
    let tool = provisioned(&project, ToolKind::Z3);
    for with_assumption in [true, false] {
        let mut modules = revision.law_modules().to_vec();
        if with_assumption {
            modules[0].assumptions.push("external.assumption".into());
            modules[0].laws[0]
                .assumption_ids
                .push("external.assumption".into());
        } else {
            let mut dependency = modules[0].laws[0].clone();
            dependency.law_id = "fresh.law.prerequisite".into();
            modules[0].laws[0]
                .requires_laws
                .push(dependency.law_id.clone());
            modules[0].laws.push(dependency);
        }
        let laws = LawSet::derive(&revision, "native-proof-v1", modules.clone()).unwrap();
        let proof = prove_scalar_law(&revision, &laws, "fresh.law.identity", &tool).unwrap();
        let requirements = modules[0]
            .laws
            .iter()
            .map(|law| (law.law_id.clone(), requirement(&tool)))
            .collect();
        let policy = StrictLawPolicy::new(laws.clone(), requirements).unwrap();
        let report =
            strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[proof.clone()])
                .unwrap();
        assert_eq!(
            strict::require_with_native_proofs(&report, &revision, &laws, &policy, &[], &[proof])
                .unwrap_err()[0]
                .code,
            "SPX-LW130"
        );
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(value["accepted"], false);
        assert_eq!(
            value["laws"][0]["failure"],
            "law_missing_unsupported_or_open"
        );
    }
}
#[test]
#[ignore = "requires explicitly provisioned installed Lean and Z3"]
fn installed_native_law_cli_checks_new_law_and_refuses_false_or_mixed_selection() {
    let project = native_project("native-cli", "n + 0 == n");
    let before = [
        "semaprax.toml",
        "src/app.spx",
        "src/contracts.spx",
        "src/tests.spx",
    ]
    .map(|path| (path, std::fs::read(project.root.join(path)).unwrap()));
    let false_project = native_project("native-cli-false", "n + 0 != n");
    for (kind, prefix) in [("lean", "SEMAPRAX_LAW_LEAN"), ("z3", "SEMAPRAX_LAW_Z3")] {
        let invoke = |root: &std::path::Path, mixed: bool| {
            let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"));
            command
                .arg("project-proof-check")
                .arg(root.join("semaprax.toml"))
                .args(["--tool", kind, "--executable"])
                .arg(std::env::var(prefix).unwrap())
                .arg("--version-line")
                .arg(std::env::var(format!("{prefix}_VERSION")).unwrap())
                .args([
                    "--host-profile",
                    "trusted-local",
                    "--law",
                    "fresh.law.identity",
                ]);
            if mixed {
                command.args(["--source", "src/app.spx"]);
            }
            command.output().unwrap()
        };
        let output = invoke(&project.root, false);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["schema"],
            "semaprax.installed-native-law-proof-check.v1"
        );
        assert_eq!(report["law_id"], "fresh.law.identity");
        assert_eq!(report["proof_checked"], true);
        assert_eq!(report["application_executed"], false);
        assert_eq!(report["publication_authority"], false);
        let refused = invoke(&false_project.root, false);
        assert!(!refused.status.success());
        assert!(refused.stdout.is_empty());
        assert_eq!(invoke(&project.root, true).status.code(), Some(2));
    }
    for (path, contents) in before {
        assert_eq!(std::fs::read(project.root.join(path)).unwrap(), contents);
    }
    assert!(!project.root.join("ACTIVE").exists());
    assert!(!project.root.join(".git").exists());
}
#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn installed_native_law_selected_host_publication_keeps_law_inventory_and_refuses_drift() {
    use semaprax::assurance_manifest::law_set::protected::{
        ProtectedLawBaseline, ProtectedLawReview, SpecificationChangeApproval,
        SpecificationChangeAuthority,
    };
    use semaprax::project::{
        apply_strict_law_publication, install_host_strict_law_policy,
        prepare_strict_law_publication, with_strict_authenticated_project, ProjectCandidate,
        ProjectExecutionOptions, SemanticChange, StrictCandidateLawInputs,
    };
    struct Host;
    impl SpecificationChangeAuthority for Host {
        fn approve_specification_change(&mut self, _: &ProtectedLawReview) -> bool {
            true
        }
    }
    let project = native_project("selected-publication", "n + 0 == n");
    // Match the checked native-law example's provider/entry/tests topology:
    // the changed provider signature rewrites two independent callers.
    for (path, source) in [
        ("src/app.spx", "module app.fresh; use function @id(\"fresh.seventeen\") from core.fresh as seventeen; @id(\"fresh.main\") fn main() -> i64 { seventeen(0) }"),
        ("src/core.spx", "module core.fresh; @id(\"fresh.seventeen\") fn seventeen(a: i64) -> i64 requires a >= 0 requires a <= 100 ensures result == a + 17 { a + 17 }"),
        ("src/tests.spx", "module app.tests; use function @id(\"fresh.seventeen\") from core.fresh as seventeen; @id(\"fresh.tests\") fn main() -> i64 { seventeen(0) }"),
    ] {
        let canonical = semaprax::format::canonical(&semaprax::parse(source, path).unwrap());
        std::fs::write(project.root.join(path), canonical).unwrap();
    }
    let manifest = project.root.join("semaprax.toml");
    std::fs::write(&manifest, "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"fresh-law\"\nversion = \"1.0.0\"\n\n[modules]\nentry = \"app.fresh\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/core.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"app.tests\"]\n\n[exports]\nweb = [\"fresh.seventeen\"]\n").unwrap();
    let paths = project.root.join("paths.json");
    std::fs::write(&paths, concat!(
        r#"{"schema":"semaprax.workspace-semantic-path-set.v1","files":[{"path":"src/app.spx"},{"path":"src/contracts.spx"},{"path":"src/core.spx"},{"path":"src/tests.spx"}]}"#, "\n",
    )).unwrap();
    let initial_workspace =
        semaprax::semantic_workspace::initialize(&project.root, &paths).unwrap();
    let graph = semaprax::workspace_graph::snapshot(&project.root, "app.fresh").unwrap();
    assert_eq!(graph.workspace_revision(), initial_workspace);
    let graph_json: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
    let active: serde_json::Value = serde_json::from_slice(
        &std::fs::read(project.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
    )
    .unwrap();
    let generation = active["workspace_revision"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let manifest_bytes = std::fs::read(
        project
            .root
            .join(".semaprax-workspace/generations")
            .join(generation)
            .join("manifest.json"),
    )
    .unwrap();
    let managed_manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let law = managed_manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "src/contracts.spx")
        .unwrap();
    assert_eq!(law["source_graph_schema"], "semaprax.native-law.v1");
    assert!(graph_json["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|declaration| declaration["path"] != "src/contracts.spx"));

    let base = project.revision();
    assert_eq!(base.workspace_revision(), initial_workspace);
    let authenticated_law = base
        .sources()
        .iter()
        .find(|source| source.path() == "src/contracts.spx")
        .unwrap();
    assert_eq!(law["source_revision"], authenticated_law.source_revision());
    assert_eq!(law["source_digest"], authenticated_law.source_digest());
    let laws = LawSet::derive(&base, "native-proof-v1", base.law_modules().to_vec()).unwrap();
    let tool = provisioned(&project, ToolKind::Z3);
    let policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
    )
    .unwrap();
    let protection = ProtectedLawBaseline::new(&base, laws.clone(), vec![]).unwrap();
    install_host_strict_law_policy(&manifest, &policy, vec![]).unwrap();
    let base_proof = prove_scalar_law(&base, &laws, "fresh.law.identity", &tool).unwrap();
    with_strict_authenticated_project(&manifest, &[], &[base_proof], |session| {
        session.execute_entry(&ProjectExecutionOptions::default())?;
        session.build_web_inline(8 * 1024 * 1024)?;
        Ok(())
    })
    .unwrap();

    let candidate = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
    let change = SemanticChange::new(
        base.project_revision(),
        &serde_json::json!({
            "kind":"change_function_signature", "target":"fresh.seventeen",
            "append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]
        }),
    )
    .unwrap();
    let candidate = candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap();
    let candidate_laws = LawSet::derive(
        candidate.revision(),
        "native-proof-v1",
        candidate.revision().law_modules().to_vec(),
    )
    .unwrap();
    let proof = prove_scalar_law(
        candidate.revision(),
        &candidate_laws,
        "fresh.law.identity",
        &tool,
    )
    .unwrap();
    let proofs = [proof];
    let intent = candidate
        .protected_law_review(&protection, &candidate_laws)
        .unwrap();
    let approval = SpecificationChangeApproval::request(&intent, &mut Host).unwrap();
    let inputs = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &candidate_laws,
        proofs: &[],
        native_proofs: &proofs,
        specification_approval: Some(&approval),
    };
    let active = project.root.join(".semaprax-workspace/ACTIVE");
    let before = std::fs::read(&active).unwrap();
    let proposal = prepare_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &initial_workspace,
    )
    .unwrap();
    assert_eq!(std::fs::read(&active).unwrap(), before);
    // The same exact workspace proposal/evidence cannot use the generic
    // semantic Change publisher to skip the host-selected strict gate.
    let outer: serde_json::Value = serde_json::from_str(proposal.to_json()).unwrap();
    let publication: serde_json::Value =
        serde_json::from_str(outer["publication"].as_str().unwrap()).unwrap();
    let raw_proposal = project.root.join("selected-change.json");
    let raw_evidence = project.root.join("selected-evidence.json");
    std::fs::write(
        &raw_proposal,
        publication["workspace_change_proposal"].as_str().unwrap(),
    )
    .unwrap();
    std::fs::write(
        &raw_evidence,
        publication["workspace_change_evidence"].as_str().unwrap(),
    )
    .unwrap();
    let generic_error =
        semaprax::semantic_workspace_change::apply(&project.root, &raw_proposal, &raw_evidence)
            .err()
            .unwrap();
    assert_eq!(generic_error[0].code, "SPX-LW150");
    assert_eq!(std::fs::read(&active).unwrap(), before);

    let missing = LawSet::derive(candidate.revision(), "native-proof-v1", vec![]).unwrap();
    let missing_inputs = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &missing,
        proofs: &[],
        native_proofs: &proofs,
        specification_approval: Some(&approval),
    };
    let missing_error = prepare_strict_law_publication(
        &candidate,
        &missing_inputs,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &initial_workspace,
    )
    .err()
    .unwrap();
    assert_eq!(missing_error[0].code, "SPX-LW150");
    assert_eq!(std::fs::read(&active).unwrap(), before);

    let law_path = project.root.join("src/contracts.spx");
    let authored = std::fs::read_to_string(&law_path).unwrap();
    std::fs::write(&law_path, authored.replace("n + 0 == n", "n + 0 != n")).unwrap();
    assert!(apply_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &initial_workspace,
        proposal.to_json().as_bytes()
    )
    .is_err());
    assert_eq!(std::fs::read(&active).unwrap(), before);
    std::fs::write(&law_path, &authored).unwrap();

    apply_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &initial_workspace,
        proposal.to_json().as_bytes(),
    )
    .unwrap();
    assert_ne!(std::fs::read(&active).unwrap(), before);
    assert_eq!(std::fs::read_to_string(law_path).unwrap(), authored);
}

#[test]
#[ignore = "LAW-14 strict selector: requires explicitly provisioned installed Lean and Z3"]
fn installed_native_law_law14_adversarial_gate() {
    // Calling `provisioned` is intentional: the explicit selector fails its
    // setup when either exact tool pin is absent.  No fixture transcript can
    // satisfy this test.
    let project = native_project("law14-real", "n + 0 == n");
    let source_paths = [
        "semaprax.toml",
        "src/app.spx",
        "src/contracts.spx",
        "src/tests.spx",
    ];
    let before = source_paths.map(|path| (path, std::fs::read(project.root.join(path)).unwrap()));
    let revision = project.revision();
    let laws = LawSet::derive(&revision, "law14-real-v1", revision.law_modules().to_vec()).unwrap();
    for kind in [ToolKind::Lean, ToolKind::Z3] {
        let tool = provisioned(&project, kind);
        let proof = prove_scalar_law(&revision, &laws, "fresh.law.identity", &tool).unwrap();
        let policy = StrictLawPolicy::new(
            laws.clone(),
            BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
        )
        .unwrap();
        let report =
            strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[proof.clone()])
                .unwrap();
        strict::require_with_native_proofs(&report, &revision, &laws, &policy, &[], &[])
            .unwrap_err();

        // A real kernel must reject a false implementation; a successful
        // outer report cannot hide the failed proof acquisition.
        let false_project = native_project("law14-false", "n + 0 != n");
        let false_revision = false_project.revision();
        let false_laws = LawSet::derive(
            &false_revision,
            "law14-real-v1",
            false_revision.law_modules().to_vec(),
        )
        .unwrap();
        assert!(prove_scalar_law(
            &false_revision,
            &false_laws,
            "fresh.law.identity",
            &provisioned(&false_project, kind),
        )
        .is_err());
        assert!(!false_project
            .root
            .join(".semaprax-workspace/ACTIVE")
            .exists());

        // A proof token is bound to the exact retained body.  It must not
        // satisfy the otherwise identical law in the false Project.
        let false_policy = StrictLawPolicy::new(
            false_laws.clone(),
            BTreeMap::from([("fresh.law.identity".into(), requirement(&tool))]),
        )
        .unwrap();
        assert_eq!(
            strict::derive_with_native_proofs(
                &false_revision,
                &false_laws,
                &false_policy,
                &[],
                &[proof],
            )
            .unwrap_err()[0]
                .code,
            "SPX-LW104"
        );
    }

    // These are actual Lean runs, not recorded text fixtures.  The kernel
    // report must reject both an admitted hole and a non-policy axiom even
    // when Lean itself elaborates the declaration successfully.
    let lean = provisioned(&project, ToolKind::Lean);
    for (source, expected) in [
        (
            "theorem law14_sorry : True := by sorry\n#print axioms law14_sorry\n",
            "admitted_hole",
        ),
        (
            "axiom law14_untrusted : True\n#print axioms law14_untrusted\n",
            "forbidden_axiom",
        ),
    ] {
        let run = semaprax::proof_export::LeanKernel::check(&lean, source).unwrap();
        let verdict = semaprax::proof_export::kernel_report::parse(
            &[if expected == "admitted_hole" {
                "law14_sorry".into()
            } else {
                "law14_untrusted".into()
            }],
            &run.toolchain,
            &run.output,
        );
        match verdict {
            semaprax::proof_export::kernel_report::KernelVerdict::Rejected(reason) => {
                assert_eq!(reason.code(), expected);
            }
            accepted => panic!("LAW-14 accepted forbidden Lean evidence: {accepted:?}"),
        }
    }

    // Missing tools refuse on explicit acquisition.  The adapter never falls
    // back to PATH or installs a solver.
    let missing = project.root.join("missing-z3");
    let missing = match InstalledProofTool::open(
        &missing,
        &project.root,
        ToolKind::Z3,
        "missing",
        HostProfile::TrustedLocal,
        Limits::default(),
        AgentCancellation::new(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("LAW-14 opened an unavailable proof tool"),
    };
    assert_eq!(missing.code, "SPX-LW140");

    // The bounded runner rejects a timeout from an actual provisioned Z3.
    let z3 = std::path::PathBuf::from(std::env::var("SEMAPRAX_LAW_Z3").unwrap());
    let tight = InstalledProofTool::open(
        &z3,
        &project.root,
        ToolKind::Z3,
        &std::env::var("SEMAPRAX_LAW_Z3_VERSION").unwrap(),
        HostProfile::TrustedLocal,
        Limits {
            version_timeout_ms: 1_000,
            proof_timeout_ms: 1,
            stream_max: 32_752,
        },
        AgentCancellation::new(),
    )
    .unwrap();
    assert!(prove_scalar_law(&revision, &laws, "fresh.law.identity", &tight).is_err());

    for (path, contents) in before {
        assert_eq!(
            std::fs::read(project.root.join(path)).unwrap(),
            contents,
            "{path}"
        );
    }
    assert!(!project.root.join(".semaprax-workspace/ACTIVE").exists());
    assert!(!project.root.join(".git").exists());
}
