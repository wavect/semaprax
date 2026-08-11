use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{repair, workspace, workspace_patch_evidence};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Family {
    V1,
    V2,
    V3,
}

struct Fixture {
    root: PathBuf,
    patch: PathBuf,
    evidence: PathBuf,
    raw_sources: Vec<(PathBuf, String)>,
}

impl Fixture {
    fn mixed(label: &str) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-patch-evidence-apply-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();

        let mut paths = Vec::new();
        let mut child_patches = Vec::new();
        let mut raw_sources = Vec::new();
        for (index, family) in [Family::V1, Family::V2, Family::V3].into_iter().enumerate() {
            let stem = ["alpha", "beta", "gamma"][index];
            let path = format!("{stem}.spx");
            let module = format!("workspace_evidence.{stem}");
            let source = match family {
                Family::V1 | Family::V2 => canonical(
                    &format!(
                        "module {module}; @id(\"{module}.helper\") fn helper()->i64{{{index}}} @id(\"{module}.main\") fn main()->i64{{helper()}}"
                    ),
                    &path,
                ),
                Family::V3 => canonical(
                    &format!(
                        "module {module}; fn helper(value:i64)->i64{{value+1}} @id(\"{module}.caller\") fn caller(value:i64)->i64{{helper(value)}} @id(\"{module}.main\") fn main()->i64{{caller({index})}}"
                    ),
                    &path,
                ),
            };
            let source_path = root.join(&path);
            std::fs::write(&source_path, &source).unwrap();
            let child_patch = match family {
                Family::V1 => format!(
                    "base {}\nrename {module}.helper to {stem}_answer\n",
                    revision(&source, &path)
                ),
                Family::V2 => format!(
                    "schema semaprax.semantic-patch.v2\nbase {}\nrename {module}.helper to {stem}_answer\nrequire no-new-effects\n",
                    revision(&source, &path)
                ),
                Family::V3 => {
                    let query = repair::DiagnosticRepairQuery::assign_function_id(format!(
                        "auto:{module}.helper"
                    ))
                    .unwrap();
                    let repairs: serde_json::Value =
                        serde_json::from_str(&repair::query(&source_path, &query).unwrap()).unwrap();
                    let preview: serde_json::Value = serde_json::from_str(
                        &repair::instantiate(
                            &source_path,
                            repairs["repair"]["id"].as_str().unwrap(),
                            &repair::PersistentDeclarationId::new(format!("{module}.helper"))
                                .unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    preview["patch"]["source"].as_str().unwrap().to_owned()
                }
            };
            paths.push(path);
            child_patches.push(child_patch);
            raw_sources.push((source_path, source));
        }

        let path_set = root.join("paths.json");
        let files = paths
            .iter()
            .map(|path| format!("{{\"path\":{}}}", serde_json::to_string(path).unwrap()))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &path_set,
            format!("{{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{files}]}}\n"),
        )
        .unwrap();
        let base_revision = workspace::initialize(&root, &path_set).unwrap();

        let patch = root.join("change.wspatch");
        let files = paths
            .iter()
            .zip(&child_patches)
            .map(|(path, patch)| {
                format!(
                    "{{\"path\":{},\"patch\":{}}}",
                    serde_json::to_string(path).unwrap(),
                    serde_json::to_string(patch).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &patch,
            format!(
                "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{base_revision}\",\"files\":[{files}]}}\n"
            ),
        )
        .unwrap();

        Self {
            evidence: root.join("evidence.json"),
            root,
            patch,
            raw_sources,
        }
    }

    fn write_capsule(&self) -> String {
        let capsule = workspace_patch_evidence::generate(&self.root, &self.patch).unwrap();
        std::fs::write(&self.evidence, &capsule).unwrap();
        capsule
    }

    fn assert_raw_sources_unchanged(&self) {
        for (path, expected) in &self.raw_sources {
            assert_eq!(std::fs::read_to_string(path).unwrap(), *expected);
        }
    }

    fn control_inventory(&self) -> Vec<(String, bool, Vec<u8>)> {
        let control = self.root.join(".semaprax-workspace");
        let mut output = Vec::new();
        let mut stack = vec![control.clone()];
        while let Some(directory) = stack.pop() {
            for entry in std::fs::read_dir(&directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let metadata = entry.metadata().unwrap();
                output.push((
                    path.strip_prefix(&control)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    metadata.is_dir(),
                    if metadata.is_file() {
                        std::fs::read(&path).unwrap()
                    } else {
                        Vec::new()
                    },
                ));
                if metadata.is_dir() {
                    stack.push(path);
                }
            }
        }
        output.sort();
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn canonical(source: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, path).unwrap())
}

fn revision(source: &str, path: &str) -> String {
    semaprax::graph::revision(&semaprax::parse(source, path).unwrap())
}

fn candidate_revision(capsule: &str) -> String {
    serde_json::from_str::<serde_json::Value>(capsule).unwrap()["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn mixed_v1_v2_v3_apply_publishes_exact_candidate_and_stales() {
    let fixture = Fixture::mixed("success");
    let capsule = fixture.write_capsule();
    let candidate = candidate_revision(&capsule);
    let preview = workspace::preview(&fixture.root, &fixture.patch).unwrap();
    let preview_candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(preview_candidate, candidate);

    assert_eq!(
        workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence).unwrap(),
        candidate
    );
    let snapshot = workspace::snapshot(&fixture.root).unwrap();
    assert_eq!(snapshot.workspace_revision(), candidate);
    assert!(snapshot.files()[0].source().contains("fn alpha_answer()"));
    assert!(snapshot.files()[1].source().contains("fn beta_answer()"));
    assert!(snapshot.files()[2]
        .source()
        .contains("@id(\"workspace_evidence.gamma.helper\")"));
    fixture.assert_raw_sources_unchanged();

    let stale = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("the same capsule cannot authorize a second stale apply");
    assert_eq!(stale[0].code, "SPX-G152");
    assert_eq!(
        workspace::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision(),
        candidate
    );
    fixture.assert_raw_sources_unchanged();
}

#[test]
fn apply_cli_success_bytes_and_arity_are_exact() {
    let fixture = Fixture::mixed("cli");
    let capsule = fixture.write_capsule();
    let candidate = candidate_revision(&capsule);
    let binary = env!("CARGO_BIN_EXE_semaprax");

    let applied = Command::new(binary)
        .arg("workspace-apply-with-evidence")
        .arg(&fixture.root)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert!(applied.status.success());
    assert!(applied.stderr.is_empty());
    assert_eq!(
        applied.stdout,
        format!(
            "applied semantic workspace transaction with exact evidence replay; workspace is now {candidate}\n"
        )
        .as_bytes()
    );
    assert_eq!(
        workspace::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision(),
        candidate
    );
    fixture.assert_raw_sources_unchanged();

    for arguments in [
        vec!["workspace-apply-with-evidence"],
        vec![
            "workspace-apply-with-evidence",
            "root",
            "patch",
            "evidence",
            "extra",
        ],
    ] {
        let rejected = Command::new(binary).args(arguments).output().unwrap();
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
        assert_eq!(
            String::from_utf8(rejected.stderr).unwrap(),
            "workspace-apply-with-evidence requires exactly <root> <patch.wspatch> <evidence.json>\n"
        );
    }
}

#[test]
fn replay_mismatch_receipt_and_malformed_capsules_create_no_workspace_state() {
    let fixture = Fixture::mixed("rejections");
    let capsule = fixture.write_capsule();
    let inventory = fixture.control_inventory();

    let current = candidate_revision(&capsule);
    let replacement = format!("sha256:{}", "0".repeat(64));
    assert_ne!(current, replacement);
    let mismatch = capsule.replacen(&current, &replacement, 1);
    std::fs::write(&fixture.evidence, mismatch).unwrap();
    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("a canonical but foreign binding must fail exact replay");
    assert_eq!(error[0].code, "SPX-G162");
    assert_eq!(fixture.control_inventory(), inventory);

    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence).unwrap();
    std::fs::write(&fixture.evidence, receipt).unwrap();
    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("a verification receipt is not an apply capsule");
    assert_eq!(error[0].code, "SPX-G160");
    assert_eq!(fixture.control_inventory(), inventory);

    std::fs::write(&fixture.evidence, "not-json\n").unwrap();
    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("malformed evidence must fail before candidate construction");
    assert_eq!(error[0].code, "SPX-G160");
    assert_eq!(fixture.control_inventory(), inventory);
    fixture.assert_raw_sources_unchanged();
}

#[test]
fn apply_input_diagnostic_precedence_is_exact_and_write_free() {
    let fixture = Fixture::mixed("input-precedence");
    let capsule = fixture.write_capsule();
    let inventory = fixture.control_inventory();
    let missing_patch = fixture.root.join("missing.wspatch");
    let missing_evidence = fixture.root.join("missing-evidence.json");

    let error = workspace_patch_evidence::apply(&fixture.root, &missing_patch, &missing_evidence)
        .expect_err("the owned patch read must precede the evidence read");
    assert_eq!(error[0].code, "SPX-I209");
    assert_eq!(fixture.control_inventory(), inventory);

    std::fs::write(&fixture.patch, "not a workspace patch\n").unwrap();
    std::fs::write(&fixture.evidence, "not-json\n").unwrap();
    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("evidence parsing must precede workspace patch parsing");
    assert_eq!(error[0].code, "SPX-G160");
    assert_eq!(fixture.control_inventory(), inventory);

    std::fs::write(&fixture.evidence, capsule).unwrap();
    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("valid evidence permits the readable malformed patch to reach its parser");
    assert_eq!(error[0].code, "SPX-G150");
    assert_eq!(fixture.control_inventory(), inventory);
    fixture.assert_raw_sources_unchanged();
}

#[test]
fn apply_acquires_exclusive_workspace_lock_before_reading_evidence() {
    let fixture = Fixture::mixed("lock-first");
    let missing_patch = fixture.root.join("missing.wspatch");
    let missing_evidence = fixture.root.join("missing-evidence.json");
    let inventory = fixture.control_inventory();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.root.join(".semaprax-workspace/LOCK"))
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();

    let error = workspace_patch_evidence::apply(&fixture.root, &missing_patch, &missing_evidence)
        .expect_err("lock contention must precede missing input reads");
    assert_eq!(error[0].code, "SPX-I210");
    assert_eq!(fixture.control_inventory(), inventory);

    std::fs::write(&fixture.evidence, "not-json\n").unwrap();
    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("lock contention must precede malformed evidence parsing");
    assert_eq!(error[0].code, "SPX-I210");
    assert_eq!(fixture.control_inventory(), inventory);
    fs2::FileExt::unlock(&lock).unwrap();

    let error = workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &missing_evidence)
        .expect_err("after lock release the missing evidence read becomes visible");
    assert_eq!(error[0].code, "SPX-I213");
    assert_eq!(fixture.control_inventory(), inventory);
    fixture.assert_raw_sources_unchanged();
}
