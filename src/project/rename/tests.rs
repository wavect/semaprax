use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-rename-unit-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(example.join(path), root.join(path)).unwrap();
    }
    Fixture(root.canonicalize().unwrap())
}

fn inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_string_lossy();
            if entry.file_type().unwrap().is_dir() {
                result.insert(format!("directory:{relative}"), Vec::new());
                visit(root, &path, result);
            } else {
                result.insert(format!("file:{relative}"), std::fs::read(path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn assert_rename_error(result: Result<PreparedProjectRename, Vec<Diagnostic>>, text: &str) {
    let diagnostics = result.err().expect("rename unexpectedly succeeded");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "SPX-J109" && diagnostic.message.contains(text) }));
}

#[test]
fn stable_export_plan_is_deterministic_digest_bound_and_read_only() {
    let fixture = fixture();
    let before = inventory(&fixture.0);
    let snapshot = super::super::load_snapshot(&fixture.0.join("semaprax.toml")).unwrap();
    let first = snapshot
        .prepare_rename("calculator.add", "add", "sum")
        .unwrap();
    let second = snapshot
        .prepare_rename("calculator.add", "add", "sum")
        .unwrap();

    assert_eq!(first.candidate_source().path(), "src/core.spx");
    assert_eq!(first.preview(), second.preview());
    assert_eq!(first.preview_digest(), second.preview_digest());
    assert_eq!(first.patch_bytes(), second.patch_bytes());
    assert!(first
        .patch_bytes()
        .starts_with(&format!("base {}\n", first.base_source().source_revision())));
    assert!(first
        .patch_bytes()
        .ends_with("rename calculator.add to sum\n"));
    assert!(first
        .candidate_source()
        .source()
        .contains("@id(\"calculator.add\")\nfn sum("));
    assert_ne!(
        first.base_workspace_revision(),
        first.candidate_workspace_revision()
    );
    assert_ne!(
        first.base_project_revision(),
        first.candidate_project_revision()
    );
    let graph: serde_json::Value = serde_json::from_str(first.candidate_project_graph()).unwrap();
    assert_eq!(
        graph["graph_digest"],
        first.candidate_project_graph_digest()
    );
    assert!(first.candidate_project_graph().contains("calculator.add"));

    let change: serde_json::Value = serde_json::from_str(first.change_preview()).unwrap();
    assert_eq!(change["schema"], PROJECT_CHANGE_PREVIEW_SCHEMA);
    assert_eq!(change["derivation_digest"], first.derivation_digest());
    assert_eq!(change["rename_preview_digest"], first.preview_digest());
    assert_eq!(change["impact_digest"], first.impact_digest());
    assert_eq!(change["review_digest"], first.review_digest());
    let marker = format!(
        ",\"artifact_digest\":\"{}\"}}",
        first.change_preview_digest()
    );
    let change_payload = first
        .change_preview()
        .strip_suffix(&marker)
        .unwrap()
        .to_owned()
        + "}";
    assert_eq!(
        domain_digest(CHANGE_PREVIEW_DIGEST_DOMAIN, change_payload.as_bytes()),
        first.change_preview_digest()
    );

    let marker = format!(",\"preview_digest\":\"{}\"}}", first.preview_digest());
    let payload = first.preview().strip_suffix(&marker).unwrap().to_owned() + "}";
    assert_eq!(
        domain_digest(PREVIEW_DIGEST_DOMAIN, payload.as_bytes()),
        first.preview_digest()
    );
    assert_eq!(before, inventory(&fixture.0));
}

#[test]
fn wrong_from_non_export_collision_and_invalid_complete_candidate_fail_closed() {
    let fixture = fixture();
    let before = inventory(&fixture.0);
    let snapshot = super::super::load_snapshot(&fixture.0.join("semaprax.toml")).unwrap();

    assert_rename_error(
        snapshot.prepare_rename("calculator.add", "wrong", "sum"),
        "does not match",
    );
    assert_rename_error(
        snapshot.prepare_rename("calculator.tests.main", "main", "renamed"),
        "web_exports",
    );
    assert!(snapshot
        .prepare_rename("calculator.add", "add", "divide")
        .is_err());
    assert!(snapshot
        .prepare_rename("calculator.add", "add", "main")
        .is_err());
    assert_eq!(before, inventory(&fixture.0));
}

#[test]
fn newer_project_profiles_fail_before_v1_rename_evidence_is_constructed() {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/binary-frame-project/semaprax.toml");
    let snapshot = super::super::load_snapshot(&manifest).unwrap();
    assert_rename_error(
        snapshot.prepare_rename("binary-frame.length", "frame_length", "measured_length"),
        "only semaprax.project.v1",
    );
}

#[test]
fn automatic_function_identity_is_rejected_before_planning() {
    let fixture = fixture();
    let core = fixture.0.join("src/core.spx");
    let mut source = std::fs::read_to_string(&core).unwrap();
    source.push_str("\nfn helper(value: i64) -> i64\n{\n    value\n}\n");
    std::fs::write(&core, source).unwrap();
    let diagnostics = super::super::load_snapshot(&fixture.0.join("semaprax.toml"))
        .err()
        .expect("automatic function entered a plannable Project snapshot");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "SPX-W115" && diagnostic.message.contains("explicit stable identity")
    }));
}

#[test]
fn sealed_plan_acquires_a0_for_validated_imported_module_without_main() {
    let fixture = fixture();
    let manifest_path = fixture.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(
        "\"src/core.spx\", \"src/tests.spx\"",
        "\"src/core.spx\", \"src/helpers.spx\", \"src/tests.spx\"",
    );
    std::fs::write(&manifest_path, manifest).unwrap();
    std::fs::write(
        fixture.0.join("src/helpers.spx"),
        "module calculator.helpers;\n\n@id(\"calculator.identity\")\nfn identity(value: i64) -> i64\n{\n    value\n}\n",
    )
    .unwrap();
    let core_path = fixture.0.join("src/core.spx");
    let core = std::fs::read_to_string(&core_path)
        .unwrap()
        .replace(
            "module calculator.core;\n",
            "module calculator.core;\nuse function @id(\"calculator.identity\") from calculator.helpers as identity;\n",
        )
        .replacen("left + right", "identity(left) + right", 1);
    std::fs::write(&core_path, core).unwrap();
    let before = inventory(&fixture.0);

    let snapshot = super::super::load_snapshot(&manifest_path).unwrap();
    let prepared = snapshot
        .prepare_rename("calculator.add", "add", "sum")
        .unwrap();
    let authority = prepared.acquire_a0().unwrap();
    drop(authority);

    assert_eq!(before, inventory(&fixture.0));
}
