//! Scoped binding rebase/recovery regressions, authored and intentionally unrun.
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-let-rebase-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}
fn binding(candidate: &ProjectCandidate) -> ProjectCandidate {
    apply(
        candidate,
        json!({"kind":"replace_function_body","target":"calculator.add","body":{
            "kind":"let","name":"cached","value":{"kind":"call","target":"calculator.subtract","arguments":[{"kind":"place","name":"left"},{"kind":"place","name":"right"}]},
            "body":{"kind":"binary","op":"+","left":{"kind":"place","name":"cached"},"right":{"kind":"place","name":"cached"}}
        }}),
    )
}

fn computed_signature(candidate: &ProjectCandidate) -> ProjectCandidate {
    apply(
        candidate,
        json!({"kind":"change_function_signature","target":"calculator.add","parameters":[
            {"from":"left"},{"from":"right"},
            {"name":"derived","type":"i64","argument_expression":{
                "kind":"let","name":"computed","value":{"kind":"call","target":"calculator.subtract","arguments":[{"kind":"place","name":"left"},{"kind":"place","name":"right"}]},
                "body":{"kind":"place","name":"computed"}
            }}
        ]}),
    )
}

#[test]
fn computed_signature_arguments_rebase_callee_display_names_and_recover_exactly() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let left = computed_signature(&root);
    let right = apply(
        &root,
        json!({"kind":"rename_declaration","target":"calculator.subtract","name":"difference"}),
    );
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    let candidate = merged.candidate();
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    let core = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(core.contains("fn difference("));
    assert!(core.contains("derived: i64"));
    let app = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/app.spx")
        .unwrap()
        .source();
    assert!(app.contains("let computed = subtract("));
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn computed_signature_call_dependencies_reject_concurrent_callee_signature_changes() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let left = computed_signature(&root);
    let right = apply(
        &root,
        json!({"kind":"change_function_signature","target":"calculator.subtract","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
    );
    match left.merge(left.candidate_digest(), &right, right.candidate_digest()) {
        Ok(_) => panic!("computed argument hid a changed callee signature"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == "SPX-G235"),
            "{errors:?}"
        ),
    }
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn nested_initializer_call_tracks_callee_rename_through_merge_and_source_recovery() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let left = binding(&root);
    let right = apply(
        &root,
        json!({"kind":"rename_declaration","target":"calculator.subtract","name":"difference"}),
    );
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    let candidate = merged.candidate();
    let source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert_eq!(
        source
            .matches("let cached = difference(left, right);")
            .count(),
        1
    );
    assert!(source.contains("cached + cached"));
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(restored.candidate_digest(), candidate.candidate_digest());
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn changed_signature_inside_a_let_initializer_remains_a_semantic_dependency_conflict() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let left = binding(&root);
    let retained = left.to_json().to_owned();
    let right = apply(
        &root,
        json!({"kind":"change_function_signature","target":"calculator.subtract","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
    );
    let result = left.merge(left.candidate_digest(), &right, right.candidate_digest());
    match result {
        Ok(_) => panic!("expected changed callee signature conflict"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == "SPX-G235"),
            "{errors:?}"
        ),
    }
    assert_eq!(left.to_json(), retained);
    assert_eq!(fixture.bytes(), before);
}
