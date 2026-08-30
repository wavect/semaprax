//! Complete candidate recovery evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
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
            "spx-candidate-recovery-{}-{}",
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
fn apply(candidate: &ProjectCandidate, name: &str) -> ProjectCandidate {
    let change = SemanticChange::new(
        candidate.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":name}),
    )
    .unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    format!("{value}\n")
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

#[test]
fn complete_history_recovers_exact_identity_and_can_continue() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let one = apply(&root, "sum");
    let two = apply(&one, "total");
    let capsule = two.recovery_capsule().unwrap();
    let value: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["changes"].as_array().unwrap().len(), 2);
    assert!(value.get("source").is_none());
    assert!(value.get("hir").is_none());
    let recovered = ProjectCandidate::restore(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(recovered.to_json(), two.to_json());
    assert_eq!(recovered.recovery_capsule().unwrap(), capsule);
    assert_eq!(
        apply(&recovered, "addition").to_json(),
        apply(&two, "addition").to_json()
    );
}

#[test]
fn malformed_stale_and_self_rehashed_wrong_final_identity_are_rejected() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let candidate = apply(&root, "sum");
    let capsule = candidate.recovery_capsule().unwrap();
    let restore = |bytes: &[u8]| {
        ProjectCandidate::restore(
            Arc::clone(root.base_revision()),
            root.base_revision().project_revision(),
            bytes,
        )
    };
    code(restore(format!("{capsule}\n").as_bytes()), "SPX-G236");
    let mut value: Value = serde_json::from_str(&capsule).unwrap();
    value["compiler"]["compatibility"] = json!("unknown");
    code(restore(canonical(value).as_bytes()), "SPX-G236");
    code(
        ProjectCandidate::restore(
            Arc::clone(candidate.revision()),
            candidate.revision().project_revision(),
            capsule.as_bytes(),
        ),
        "SPX-G238",
    );
    let mut value: Value = serde_json::from_str(&capsule).unwrap();
    value["candidate_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    value.as_object_mut().unwrap().remove("capsule_digest");
    let body = canonical(value.clone());
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-candidate-recovery.payload.v1\0");
    hash.update((body.len() as u64).to_le_bytes());
    hash.update(body.as_bytes());
    let hex = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    value["capsule_digest"] = json!(format!("sha256:{hex}"));
    code(restore(canonical(value).as_bytes()), "SPX-G238");
    let duplicate = capsule.replacen("{", "{\"schema\":\"ignored\",", 1);
    code(restore(duplicate.as_bytes()), "SPX-G236");
}

#[test]
fn cli_export_restore_are_read_only_and_require_regular_explicit_inputs() {
    use std::process::Command;
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let manifest = fixture.0.join("semaprax.toml");
    let input = fixture.0.join("change.json");
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":"sum"}),
    )
    .unwrap();
    std::fs::write(&input, change.to_json()).unwrap();
    let source = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let exported = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("project-candidate-export")
        .arg(&manifest)
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let path = fixture.0.join("capsule.json");
    std::fs::write(&path, &exported.stdout).unwrap();
    let restored = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("project-candidate-restore")
        .arg(&manifest)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        String::from_utf8(restored.stdout).unwrap(),
        apply(&root, "sum").to_json()
    );
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        source
    );
    assert!(!fixture.0.join(".semaprax-candidates").exists());
    let rejected = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("project-candidate-restore")
        .arg(&manifest)
        .arg(&fixture.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let arity = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("project-candidate-export")
        .arg(&manifest)
        .output()
        .unwrap();
    assert_eq!(arity.status.code(), Some(2));
    assert!(arity.stdout.is_empty());
}
