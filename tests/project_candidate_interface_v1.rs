//! Source-backed interface intention evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-interface-change-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "interface-change"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "iface.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["iface.evaluate"]
tests = ["iface.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module iface.core;
@id("iface.counter") record Counter { @id("iface.counter.value") value: i64, }
@id("iface.readable") protocol Readable {
    @id("iface.readable.read") fn read(receiver: Self) -> i64;
    @id("iface.readable.positive") fn positive(receiver: Self) -> bool;
}
@id("iface.read") fn counter_read(receiver: Counter) -> i64 { receiver.value }
@id("iface.positive") fn counter_positive(receiver: Counter) -> bool { receiver.value > 0 }
@id("iface.wrong") fn wrong(receiver: Counter) -> usize { 0usize }
@id("iface.restricted") fn restricted(receiver: Counter) -> i64 requires receiver.value > 0 { receiver.value }
@id("iface.evaluate") fn evaluate(value: i64) -> i64 { counter_read(Counter { value: value }) }
"#,
            ),
            (
                "src/app.spx",
                r#"module iface.app;
use function @id("iface.evaluate") from iface.core as evaluate;
@id("iface.main") fn main() -> i64 { evaluate(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module iface.tests;
use function @id("iface.evaluate") from iface.core as evaluate;
@id("iface.test") fn evaluates() -> bool { evaluate(42) == 42 }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn request() -> Value {
    json!({"kind":"implement_interface","target":"iface.counter","protocol":"iface.readable","id":"iface.counter.readable","members":[
        {"method":"iface.readable.read","implementation":"iface.read"},
        {"method":"iface.readable.positive","implementation":"iface.positive"}
    ]})
}
fn apply(base: &ProjectCandidate, intent: &Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), intent)?,
    )
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
fn discovers_real_signatures_and_records_replayable_source_conformance_without_writes() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let untouched = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let catalog: Value = serde_json::from_str(
        &base
            .interface_catalog(base.candidate_digest(), "iface.counter")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(catalog["protocols"][0]["complete_mapping_available"], true);
    let methods = catalog["protocols"][0]["members"].as_array().unwrap();
    let read = methods
        .iter()
        .find(|method| method["method"] == "iface.readable.read")
        .unwrap();
    assert_eq!(read["eligible_implementations"], json!(["iface.read"]));
    let change = SemanticChange::new(base.revision().project_revision(), &request()).unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    let source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(source.contains("impl \"iface.readable\" for \"iface.counter\""));
    assert!(source.contains("\"iface.readable.read\" = \"iface.read\";"));
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(
        report["operations"][0]["new_declaration"]["id"],
        "iface.counter.readable"
    );
    assert_eq!(
        report["operations"][0]["new_declaration"]["runtime_graph_declaration"],
        false
    );
    let delta = candidate
        .semantic_delta(candidate.candidate_digest(), "iface.counter.readable")
        .unwrap();
    let delta_value: Value = serde_json::from_str(&delta).unwrap();
    assert_eq!(delta_value["presence"], "added");
    candidate
        .verify_semantic_delta(
            candidate.candidate_digest(),
            "iface.counter.readable",
            delta.as_bytes(),
        )
        .unwrap();
    let replay = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), candidate.to_json());
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        untouched
    );
}

#[test]
fn rejects_partial_wrong_duplicate_and_existing_identity_mappings() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let mut partial = request();
    partial["members"].as_array_mut().unwrap().pop();
    let mut wrong = request();
    wrong["members"][0]["implementation"] = json!("iface.wrong");
    let mut restricted = request();
    restricted["members"][0]["implementation"] = json!("iface.restricted");
    let mut duplicate = request();
    duplicate["members"][1] = duplicate["members"][0].clone();
    let mut collision = request();
    collision["id"] = json!("iface.readable.read");
    for intent in [partial, wrong, restricted, duplicate, collision] {
        code(apply(&base, &intent), "SPX-G272");
    }
    let candidate = apply(&base, &request()).unwrap();
    let mut repeated = request();
    repeated["id"] = json!("iface.second");
    code(apply(&candidate, &repeated), "SPX-G272");
}

#[test]
fn later_changes_preserve_binding_ids_and_revalidate_member_requirements() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let candidate = apply(&base, &request()).unwrap();
    let renamed = apply(
        &candidate,
        &json!({"kind":"rename_declaration","target":"iface.read","name":"renamed_read"}),
    )
    .unwrap();
    let source = renamed
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(source.contains("fn renamed_read("));
    assert!(source.contains("\"iface.readable.read\" = \"iface.read\";"));
    code(
        apply(
            &candidate,
            &json!({"kind":"add_contract","target":"iface.read","phase":"requires","predicate":{"kind":"bool","value":true}}),
        ),
        "SPX-Q107",
    );
    let unrelated = apply(
        &base,
        &json!({"kind":"rename_declaration","target":"iface.evaluate","name":"evaluate_again"}),
    )
    .unwrap();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(unrelated.revision()),
            unrelated.revision().project_revision(),
        ),
        "SPX-G275",
    );
}
