//! Nominal computed signature rebase evidence, authored and intentionally unrun.
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const CONFIG: &str = "nominal-rebase.config";
const SELECT: &str = "nominal-rebase.select";
const IDLE: &str = "nominal-rebase.idle";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-nominal-signature-rebase-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "nominal-signature-rebase"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "nominal_rebase.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["nominal-rebase.public"]
tests = ["nominal_rebase.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module nominal_rebase.core;
@id("nominal-rebase.config") record Config { @id("nominal-rebase.config.value") value: i64, }
@id("nominal-rebase.select") fn select(left: i64) -> i64 { left }
@id("nominal-rebase.idle") fn idle(left: i64) -> i64 { left }
@id("nominal-rebase.spare") fn spare(value: i64) -> i64 { value }
@id("nominal-rebase.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module nominal_rebase.app;
use type @id("nominal-rebase.config") from nominal_rebase.core as Settings;
use function @id("nominal-rebase.select") from nominal_rebase.core as choose;
@id("nominal-rebase.main") fn main() -> i64 { choose(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module nominal_rebase.tests;
use type @id("nominal-rebase.config") from nominal_rebase.core as TestConfig;
use function @id("nominal-rebase.select") from nominal_rebase.core as choose;
@id("nominal-rebase.test") fn main() -> i64 { if choose(42) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
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
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
}
fn place() -> Value {
    json!({"kind":"place","name":"left"})
}
fn mapping(target: &str, expression: Value) -> Value {
    json!({"kind":"change_function_signature","target":target,"parameters":[
        {"from":"left"},
        {"name":"settings","type":{"kind":"nominal","target":CONFIG,"type_arguments":[]},"argument_expression":expression},
    ]})
}
fn constructor(with_flag: bool) -> Value {
    let mut fields = vec![json!({"target":"nominal-rebase.config.value","value":place()})];
    if with_flag {
        fields.push(
            json!({"target":"nominal-rebase.config.flag","value":{"kind":"bool","value":false}}),
        );
    }
    json!({"kind":"record","target":CONFIG,"fields":fields})
}
fn add_field() -> Value {
    json!({"kind":"add_record_field","target":CONFIG,"field":{"id":"nominal-rebase.config.flag","name":"flag","type":"bool","default":{"kind":"bool","value":false}}})
}
fn unrelated_rename() -> Value {
    json!({"kind":"rename_declaration","target":"nominal-rebase.spare","name":"renamed_spare"})
}
fn source<'a>(candidate: &'a ProjectCandidate, path: &str) -> &'a str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
}
fn assert_signature(candidate: &ProjectCandidate, target: &str) {
    let program = semaprax::parse(source(candidate, "src/core.spx"), "src/core.spx").unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == target)
        .unwrap();
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[1].name, "settings");
    assert_eq!(function.params[1].ty.to_string(), "Config");
}
fn assert_recovery(candidate: &ProjectCandidate) {
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.candidate_digest(), candidate.candidate_digest());
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(restored.recovery_capsule().unwrap(), capsule);
}

#[test]
fn nominal_signature_merges_unrelated_rename_and_recovers_exact_cross_file_sources() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let signature = apply(&root, mapping(SELECT, constructor(false)));
    let renamed = apply(&root, unrelated_rename());
    let merged = signature
        .merge(
            signature.candidate_digest(),
            &renamed,
            renamed.candidate_digest(),
        )
        .unwrap();
    let candidate = merged.candidate();
    assert_signature(candidate, SELECT);
    assert_eq!(
        candidate.base_revision().project_revision(),
        root.base_revision().project_revision()
    );
    assert!(source(candidate, "src/core.spx").contains("fn renamed_spare("));
    assert!(source(candidate, "src/app.spx").contains("Settings {"));
    assert!(source(candidate, "src/tests.spx").contains("TestConfig {"));
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(
        report["left_parent_candidate"],
        signature.candidate_digest()
    );
    assert_eq!(report["right_parent_candidate"], renamed.candidate_digest());
    assert_recovery(candidate);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn type_descriptor_alone_binds_owner_shape_even_without_an_aggregate_expression() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    // The target has no callers. Its scalar template passes constructor scope
    // preflight but is not an executed or caller-typed Config expression.
    // Only the new parameter's nominal descriptor refers to the record owner.
    let signature = apply(&root, mapping(IDLE, place()));
    assert_signature(&signature, IDLE);
    let changed = apply(&root, add_field());
    let signature_bytes = signature.to_json().to_owned();
    let changed_bytes = changed.to_json().to_owned();
    match signature.merge(
        signature.candidate_digest(),
        &changed,
        changed.candidate_digest(),
    ) {
        Ok(_) => panic!("concurrent nominal owner shape change must conflict"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == "SPX-G235"),
            "{errors:?}"
        ),
    }
    match signature.rebase(
        signature.candidate_digest(),
        Arc::clone(changed.revision()),
        changed.revision().project_revision(),
    ) {
        Ok(_) => panic!("rebasing onto a changed nominal descriptor owner must conflict"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == "SPX-G235"),
            "{errors:?}"
        ),
    }
    assert_eq!(signature.to_json(), signature_bytes);
    assert_eq!(changed.to_json(), changed_bytes);
    assert_recovery(&signature);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn earlier_shape_change_is_the_computed_signature_dependency_base_when_merging() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let root = fixture.candidate();
    let extended = apply(&root, add_field());
    let signature = apply(&extended, mapping(SELECT, constructor(true)));
    let renamed = apply(&root, unrelated_rename());
    let merged = signature
        .merge(
            signature.candidate_digest(),
            &renamed,
            renamed.candidate_digest(),
        )
        .unwrap();
    let candidate = merged.candidate();
    assert_signature(candidate, SELECT);
    assert!(source(candidate, "src/core.spx").contains("flag: bool"));
    assert!(source(candidate, "src/core.spx").contains("fn renamed_spare("));
    assert!(source(candidate, "src/app.spx").contains("flag: false"));
    assert!(source(candidate, "src/tests.spx").contains("flag: false"));
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(report["changes"].as_array().unwrap().len(), 3);
    assert_recovery(candidate);
    assert_eq!(fixture.bytes(), before);
}
