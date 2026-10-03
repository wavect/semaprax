//! Real native-law proofs are generated from exact retained typed declarations.
use super::*;
use semaprax::assurance_manifest::law_set::native_proof::prove_scalar_law;

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
