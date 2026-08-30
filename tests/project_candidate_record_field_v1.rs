//! Record field migration evidence authored without executing local gates.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectExecutionOptions, SemanticChange,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-record-field-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "record-field"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "field.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["field.evaluate"]
tests = ["field.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module field.core;
@id("field.pair") record Pair {
    @id("field.pair.left") left: i64,
    @id("field.pair.right") right: i64,
}
@id("field.envelope") record Envelope {
    @id("field.envelope.pair") pair: Pair,
    @id("field.envelope.marker") marker: bool,
}
@id("field.evaluate") fn evaluate(input: i64) -> i64
    requires (Pair { right: 0, left: 0 }).left == 0
{
    let pair = Pair { right: 10, left: input };
    let updated = pair with { left: pair.left + 1 };
    let outer = Envelope { pair: updated, marker: true };
    match outer { Envelope { pair: Pair { left: picked, right: other }, marker: _ } => picked + other }
}
@id("field.unrelated") fn unrelated() -> i64 { 7 }
"#,
            ),
            (
                "src/app.spx",
                r#"module field.app;
use type @id("field.pair") from field.core as Metric;
use function @id("field.evaluate") from field.core as evaluate;
@id("field.app.main") fn main() -> i64 {
    let item = Metric { right: 3, left: 2 };
    let selected = match item { Metric { left: picked, right: _ } => picked };
    evaluate(selected)
}
"#,
            ),
            (
                "src/tests.spx",
                r#"module field.tests;
use function @id("field.evaluate") from field.core as evaluate;
@id("field.tests.main") fn main() -> i64 { if evaluate(2) == 13 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, Path::new(path)).unwrap();
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
    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .into_iter()
        .map(|path| (path.to_owned(), std::fs::read(self.0.join(path)).unwrap()))
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn request() -> Value {
    json!({"kind":"add_record_field","target":"field.pair","field":{"id":"field.pair.tag","name":"tag","type":"i64","default":{"kind":"i64","value":9}}})
}
fn apply(
    candidate: &ProjectCandidate,
    intent: &Value,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), intent)?,
    )
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
fn diagnostic<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}
fn same_outcome(before: &ProjectCandidate, after: &ProjectCandidate) {
    let options = ProjectExecutionOptions::default();
    assert_eq!(
        before.revision().execute_entry(&options).unwrap().outcome(),
        after.revision().execute_entry(&options).unwrap().outcome()
    );
}

#[test]
fn all_alias_constructors_contracts_and_nested_patterns_migrate_with_exact_replay() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let root = fixture.candidate();
    let change = SemanticChange::new(root.revision().project_revision(), &request()).unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let core = source(&candidate, "src/core.spx");
    let app = source(&candidate, "src/app.spx");
    assert!(core.contains("Pair { right: 0, left: 0, tag: 9 }"));
    assert!(core.contains("Pair { right: 10, left: input, tag: 9 }"));
    assert!(core.contains("pair with { left: pair.left + 1 }"));
    assert!(core.contains("Pair { left: picked, right: other, tag: _ }"));
    assert!(app.contains("Metric { right: 3, left: 2, tag: 9 }"));
    assert!(app.contains("Metric { left: picked, right: _, tag: _ }"));
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    let addition = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "field.pair.tag")
        .unwrap();
    assert_eq!(addition["kind"], "field");
    assert_eq!(addition["owner"], "field.pair");
    assert_eq!(addition["path"], "src/core.spx");
    assert_eq!(addition["identity_origin"], "explicit");
    same_outcome(&root, &candidate);
    let replay = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        std::slice::from_ref(&change),
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), candidate.to_json());
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    diagnostic(
        candidate.apply(candidate.candidate_digest(), &change),
        "SPX-G224",
    );
    let mut tampered = candidate.to_json().as_bytes().to_vec();
    tampered.push(b' ');
    diagnostic(
        ProjectCandidate::replay(
            Arc::clone(root.base_revision()),
            root.base_revision().project_revision(),
            &[change],
            &tampered,
        ),
        "SPX-G224",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn existing_binding_order_and_lazy_failure_are_preserved() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let changed=original.replace("Metric { right: 3, left: 2 }","if true { Metric { right: 3, left: 2 } } else { Metric { right: 1 / 0, left: 9223372036854775807 + 1 } }");
    let parsed = semaprax::parse(&changed, Path::new("src/app.spx")).unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    let root = fixture.candidate();
    let candidate = apply(&root, &request()).unwrap();
    assert!(source(&candidate, "src/app.spx")
        .contains("Metric { right: 1 / 0, left: 9223372036854775807 + 1, tag: 9 }"));
    same_outcome(&root, &candidate);
}

#[test]
fn default_grammar_collisions_and_nonrecord_targets_fail_without_writes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let root = fixture.candidate();
    for mutation in 0..6 {
        let mut intent = request();
        match mutation {
            0 => {
                intent["field"]["default"] =
                    json!({"kind":"call","target":"field.unrelated","arguments":[]})
            }
            1 => intent["field"]["default"] = json!({"kind":"bool","value":true}),
            2 => intent["field"]["id"] = json!("field.pair.left"),
            3 => intent["field"]["name"] = json!("left"),
            4 => intent["target"] = json!("field.evaluate"),
            _ => intent["field"]["default"]["unknown"] = json!(0),
        }
        diagnostic(apply(&root, &intent), "SPX-G225");
    }
    let mut boolean = request();
    boolean["field"]["type"] = json!("bool");
    boolean["field"]["default"] = json!({"kind":"bool","value":false});
    let candidate = apply(&root, &boolean).unwrap();
    assert!(source(&candidate, "src/app.spx").contains("tag: false"));
    same_outcome(&root, &candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_and_owned_records_remain_outside_copy_field_evolution() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let changed = format!(
        r#"{original}
@id("field.generic") record Generic<T> {{ @id("field.generic.value") value: T, }}
@id("field.owned") record Owned {{ @id("field.owned.bytes") bytes: Bytes, }}
"#
    );
    let program = semaprax::parse(&changed, Path::new("src/core.spx")).unwrap();
    std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
    let root = fixture.candidate();
    for target in ["field.generic", "field.owned"] {
        let mut intent = request();
        intent["target"] = json!(target);
        diagnostic(apply(&root, &intent), "SPX-G225");
    }
}

#[test]
fn merge_replays_field_migration_after_unrelated_rename_and_rejects_competing_shape() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = apply(&root, &request()).unwrap();
    let right = apply(
        &root,
        &json!({"kind":"rename_declaration","target":"field.unrelated","name":"different"}),
    )
    .unwrap();
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert!(source(merged.candidate(), "src/core.spx").contains("fn different("));
    assert!(source(merged.candidate(), "src/core.spx").contains("tag: i64"));
    same_outcome(&root, merged.candidate());
    let mut conflict = request();
    conflict["field"]["id"] = json!("field.pair.other");
    conflict["field"]["name"] = json!("other");
    let conflict = apply(&root, &conflict).unwrap();
    diagnostic(
        left.merge(
            left.candidate_digest(),
            &conflict,
            conflict.candidate_digest(),
        ),
        "SPX-G235",
    );
}
