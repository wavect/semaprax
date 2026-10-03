//! Explicit real-tool gates. No recorded callback can satisfy these tests.
use super::*;
use semaprax::{
    agent_runtime::AgentCancellation,
    assurance_manifest::law_set::strict::{self, RequiredLawEvidence, StrictLawPolicy},
    proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits, ToolKind},
        installed_project::prove_postcondition,
    },
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

struct Project {
    root: PathBuf,
}
impl Project {
    fn new(label: &str, false_law: bool) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-installed-law-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = format!("module app.fresh;\n@id(\"fresh.seventeen\")\nfn seventeen(a: i64) -> i64\n requires a >= 0\n requires a <= 100\n ensures result == a + {}\n{{ a + 17 }}\n@id(\"fresh.main\")\nfn main() -> i64 {{ seventeen(0) }}\n", if false_law {18} else {17});
        let source = semaprax::format::canonical(&semaprax::parse(&source, "src/app.spx").unwrap());
        std::fs::write(root.join("src/app.spx"), source).unwrap();
        let tests = semaprax::format::canonical(
            &semaprax::parse(
                "module app.tests;\n@id(\"fresh.tests\")\nfn main() -> i64 { 0 }\n",
                "src/tests.spx",
            )
            .unwrap(),
        );
        std::fs::write(root.join("src/tests.spx"), tests).unwrap();
        std::fs::write(root.join("semaprax.toml"), "schema = \"semaprax.project.v8\"\nname = \"fresh-law\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"app.fresh\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = []\ntests = [\"app.tests\"]\n").unwrap();
        Self { root }
    }
    fn revision(&self) -> Arc<semaprax::project::ProjectRevision> {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn provisioned(project: &Project, kind: ToolKind) -> InstalledProofTool {
    let prefix = if kind == ToolKind::Lean {
        "SEMAPRAX_LAW_LEAN"
    } else {
        "SEMAPRAX_LAW_Z3"
    };
    let path = PathBuf::from(std::env::var(prefix).expect("explicit installed tool path required"));
    let version =
        std::env::var(format!("{prefix}_VERSION")).expect("explicit exact version pin required");
    InstalledProofTool::open(
        &path,
        &project.root,
        kind,
        &version,
        HostProfile::TrustedLocal,
        Limits::default(),
        AgentCancellation::new(),
    )
    .unwrap()
}

fn laws(revision: &semaprax::project::ProjectRevision, kind: ToolKind) -> LawSet {
    let mut declared = module();
    declared.source_path = "src/app.spx".into();
    declared.laws[0].law_id = "fresh.law.seventeen".into();
    declared.laws[0].selector = LawSelector::Contract {
        declaration_id: "fresh.seventeen".into(),
        clause: ContractKind::Postcondition,
        proposition: "result == a + 17".into(),
    };
    declared.laws[0].evidence = if kind == ToolKind::Lean {
        EvidenceRequirement::TheoremProved
    } else {
        EvidenceRequirement::SmtProved
    };
    LawSet::derive(revision, "checked-v1", vec![declared]).unwrap()
}
fn requirement(tool: &InstalledProofTool) -> RequiredLawEvidence {
    if tool.kind() == ToolKind::Z3 {
        RequiredLawEvidence::PinnedSmtSource {
            toolchain: tool.expected_version().into(),
            accepted_translation: semaprax::assurance_manifest::smt_discharge::BOUNDS_V1.into(),
        }
    } else {
        RequiredLawEvidence::PinnedLeanSource {
            toolchain: semaprax::proof_export::PINNED_TOOLCHAIN.into(),
            accepted_assumptions: semaprax::proof_export::ASSUMPTIONS
                .iter()
                .map(|(id, _)| (*id).into())
                .collect(),
            accepted_axioms: semaprax::proof_export::kernel_report::STANDARD_AXIOMS
                .iter()
                .map(|id| (*id).into())
                .collect(),
        }
    }
}

#[test]
#[ignore = "requires explicitly provisioned installed Lean and Z3"]
fn installed_law_real_kernels_prove_new_exact_law_and_refuse_false_stale_or_changed_trust() {
    let project = Project::new("real", false);
    let revision = project.revision();
    let before = std::fs::read(project.root.join("src/app.spx")).unwrap();
    for kind in [ToolKind::Lean, ToolKind::Z3] {
        let tool = provisioned(&project, kind);
        let proof =
            prove_postcondition(&revision, "src/app.spx", "fresh.seventeen", 0, &tool).unwrap();
        let laws = laws(&revision, kind);
        let req = requirement(&tool);
        let policy = StrictLawPolicy::new(
            laws.clone(),
            BTreeMap::from([("fresh.law.seventeen".into(), req.clone())]),
        )
        .unwrap();
        let report = strict::derive(&revision, &laws, &policy, &[proof.clone()]).unwrap();
        strict::require(&report, &revision, &laws, &policy, &[proof.clone()]).unwrap();
        let candidate = semaprax::project::ProjectCandidate::open(
            revision.clone(),
            revision.project_revision(),
        )
        .unwrap();
        let candidate_report = candidate
            .strict_law_assurance(
                candidate.candidate_digest(),
                &laws,
                &policy,
                &[proof.clone()],
            )
            .unwrap();
        candidate
            .require_strict_law_assurance(&candidate_report, &laws, &policy, &[proof.clone()])
            .unwrap();
        let changed = match req {
            RequiredLawEvidence::PinnedLeanSource {
                toolchain,
                accepted_axioms,
                ..
            } => RequiredLawEvidence::PinnedLeanSource {
                toolchain,
                accepted_axioms,
                accepted_assumptions: vec![],
            },
            RequiredLawEvidence::PinnedSmtSource {
                accepted_translation,
                ..
            } => RequiredLawEvidence::PinnedSmtSource {
                toolchain: "wrong-version".into(),
                accepted_translation,
            },
            _ => unreachable!(),
        };
        let refused = StrictLawPolicy::new(
            laws.clone(),
            BTreeMap::from([("fresh.law.seventeen".into(), changed)]),
        )
        .unwrap();
        let report = strict::derive(&revision, &laws, &refused, &[proof.clone()]).unwrap();
        assert!(strict::require(&report, &revision, &laws, &refused, &[proof.clone()]).is_err());
        assert!(
            prove_postcondition(&revision, "src/app.spx", "fresh.seventeen", 1, &tool).is_err()
        );
        let false_project = Project::new("false", true);
        let false_revision = false_project.revision();
        assert!(prove_postcondition(
            &false_revision,
            "src/app.spx",
            "fresh.seventeen",
            0,
            &provisioned(&false_project, kind)
        )
        .is_err());
        assert!(
            semaprax::assurance_manifest::project::derive_with_verified_proofs(
                &false_revision,
                &Default::default(),
                &[proof]
            )
            .is_err()
        );
    }
    assert_eq!(
        std::fs::read(project.root.join("src/app.spx")).unwrap(),
        before
    );
}

#[test]
#[ignore = "requires explicitly provisioned installed Lean and Z3"]
fn installed_law_cli_runs_both_real_tools_and_refuses_unavailable_confinement() {
    let project = Project::new("cli", false);
    for (kind, env) in [("lean", "SEMAPRAX_LAW_LEAN"), ("z3", "SEMAPRAX_LAW_Z3")] {
        let common = vec![
            "project-proof-check".to_owned(),
            project.root.join("semaprax.toml").display().to_string(),
            "--tool".into(),
            kind.into(),
            "--executable".into(),
            std::env::var(env).unwrap(),
            "--version-line".into(),
            std::env::var(format!("{env}_VERSION")).unwrap(),
            "--source".into(),
            "src/app.spx".into(),
            "--declaration".into(),
            "fresh.seventeen".into(),
            "--ensures".into(),
            "0".into(),
            "--host-profile".into(),
        ];
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(&common)
            .arg("trusted-local")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["application_executed"], false);
        assert_eq!(report["publication_authority"], false);
        let denied = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(&common)
            .arg("confined")
            .output()
            .unwrap();
        assert!(!denied.status.success());
        assert!(String::from_utf8_lossy(&denied.stderr).contains("confinement is unavailable"));
    }
}

#[test]
#[ignore = "requires explicitly provisioned C compiler for hostile process controls"]
fn installed_law_hostile_process_bounds_cancellation_and_descendant_settlement() {
    let project = Project::new("process", false);
    let source = project.root.join("hostile.c");
    std::fs::write(
        &source,
        r#"#include <unistd.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
#if MODE == 1
for (;;) pause();
#elif MODE == 2
for (;;) write(1,"0123456789",10);
#elif MODE == 3
if (fork()==0) { close(0); close(1); close(2); for (;;) pause(); }
puts("fixture");
#elif MODE == 4
if (argc>1 && !strcmp(argv[1],"--version")) puts("fixture"); else puts("unsat\nunknown");
#else
puts("fixture");
#endif
return 0;
}
"#,
    )
    .unwrap();
    for mode in [1, 2, 3, 4] {
        let executable = project.root.join(format!("hostile-{mode}"));
        let status =
            std::process::Command::new(std::env::var("CLANG").expect("explicit compiler required"))
                .arg(&source)
                .arg(format!("-DMODE={mode}"))
                .arg("-o")
                .arg(&executable)
                .status()
                .unwrap();
        assert!(status.success());
        let cancel = AgentCancellation::new();
        let tool = InstalledProofTool::open(
            &executable,
            &project.root,
            ToolKind::Z3,
            "fixture",
            HostProfile::TrustedLocal,
            Limits {
                version_timeout_ms: if mode == 1 { 100 } else { 3_000 },
                proof_timeout_ms: 3_000,
                stream_max: 128,
            },
            cancel.clone(),
        )
        .unwrap();
        let started = std::time::Instant::now();
        let outcome = tool.version();
        match mode {
            1 => assert!(outcome.unwrap_err().message.contains("TimedOut")),
            2 => {
                let error = outcome.unwrap_err();
                assert!(error.message.contains("CapacityExceeded"), "{error:?}");
            }
            3 | 4 => assert_eq!(outcome.unwrap(), "fixture"),
            _ => unreachable!(),
        }
        if mode == 4 {
            assert!(tool
                .confirm_smt("(check-sat)\n")
                .unwrap_err()
                .message
                .contains("partial evidence"));
        }
        // Descendant-held or already-closed pipes cannot keep a successful
        // version probe alive; the provider settles its owned group first.
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        cancel.cancel();
        assert!(tool.version().unwrap_err().message.contains("cancelled"));
        if mode == 1 {
            let cancel = AgentCancellation::new();
            let running = InstalledProofTool::open(
                &executable,
                &project.root,
                ToolKind::Z3,
                "fixture",
                HostProfile::TrustedLocal,
                Limits {
                    version_timeout_ms: 5_000,
                    proof_timeout_ms: 5_000,
                    stream_max: 128,
                },
                cancel.clone(),
            )
            .unwrap();
            let worker = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                cancel.cancel();
            });
            let started = std::time::Instant::now();
            assert!(running.version().unwrap_err().message.contains("Cancelled"));
            worker.join().unwrap();
            assert!(started.elapsed() < std::time::Duration::from_secs(4));
        }
    }
}

#[path = "installed_native_law.rs"]
mod installed_native_law;
