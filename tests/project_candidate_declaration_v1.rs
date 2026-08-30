//! Typed declaration evidence authored without running local compiler/test gates.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(example: &str, module_file: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-declaration-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example);
        for file in ["semaprax.toml", "src/app.spx", module_file, "src/tests.spx"] {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root)
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
    json!({"kind":"add_declaration","target":"calculator.add","declaration":{
        "id":"calculator.increment","name":"increment",
        "parameters":[{"name":"value","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],
        "ensures":[{"kind":"bool","value":true}],
        "body":{"kind":"call","target":"calculator.add","arguments":[{"kind":"place","name":"value"},{"kind":"i64","value":1}]}
    }})
}
fn attempt(
    candidate: &ProjectCandidate,
    intent: Value,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), &intent)?,
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
fn diagnostic<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}

#[test]
fn explicit_typed_addition_is_canonical_replayable_and_does_not_write_sources() {
    let fixture = Fixture::new("calculator-project", "src/core.spx");
    let root = fixture.candidate();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let change = SemanticChange::new(root.revision().project_revision(), &request()).unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let projected = source(&candidate, "src/core.spx");
    assert!(projected.starts_with(source(&root, "src/core.spx")));
    assert!(projected.contains("@id(\"calculator.increment\")"));
    assert!(projected.contains("fn increment(value: i64) -> i64"));
    assert!(projected.contains("add(value, 1)"));
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    assert!(graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["id"] == "calculator.increment" && d["identity_origin"] == "explicit"));
    let replay = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(candidate.to_json(), replay.to_json());
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
}

#[test]
fn new_identity_remains_editable_across_merge_and_exact_replay() {
    let fixture = Fixture::new("calculator-project", "src/core.spx");
    let root = fixture.candidate();
    let added = attempt(&root, request()).unwrap();
    let renamed = attempt(
        &added,
        json!({"kind":"rename_declaration","target":"calculator.increment","name":"step"}),
    )
    .unwrap();
    let changed = attempt(&renamed, json!({"kind":"replace_function_body","target":"calculator.increment","body":{"kind":"call","target":"calculator.multiply","arguments":[{"kind":"place","name":"value"},{"kind":"i64","value":2}]}})).unwrap();
    let right = attempt(
        &root,
        json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
    )
    .unwrap();
    let merged = changed
        .merge(changed.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    let candidate = merged.candidate();
    assert!(source(candidate, "src/core.spx").contains("fn addition("));
    assert!(source(candidate, "src/core.spx").contains("fn step("));
    assert!(source(candidate, "src/core.spx").contains("multiply(value, 2)"));
    assert_eq!(
        candidate.base_revision().project_revision(),
        root.base_revision().project_revision()
    );
    let evidence: Value = serde_json::from_str(candidate.to_json()).unwrap();
    let changes = evidence["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            SemanticChange::new(change["base_revision"].as_str().unwrap(), &change["intent"])
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(changes.len(), 4);
    let replay = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        &changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.candidate_digest(), candidate.candidate_digest());
}

#[test]
fn main_anchor_resolves_imports_by_identity_without_import_or_manifest_edits() {
    let fixture = Fixture::new("calculator-project", "src/core.spx");
    let root = fixture.candidate();
    let mut intent = request();
    intent["target"] = json!("calculator.app.main");
    let candidate = attempt(&root, intent).unwrap();
    assert!(source(&candidate, "src/app.spx").contains("fn increment("));
    assert_eq!(
        source(&candidate, "src/core.spx"),
        source(&root, "src/core.spx")
    );
    assert_eq!(
        candidate.revision().manifest().to_canonical_toml(),
        root.revision().manifest().to_canonical_toml()
    );
}

#[test]
fn collisions_authority_widening_and_invalid_ownership_are_rejected_immutably() {
    let fixture = Fixture::new("calculator-project", "src/core.spx");
    let root = fixture.candidate();
    let before = root.to_json().to_owned();
    let mut invalid = Vec::new();
    for id in ["calculator.add", "calculator.app.main", "unsafe\"id", ""] {
        let mut intent = request();
        intent["declaration"]["id"] = json!(id);
        invalid.push(intent);
    }
    for name in ["add", "main", "fn"] {
        let mut intent = request();
        intent["declaration"]["name"] = json!(name);
        invalid.push(intent);
    }
    let mut intent = request();
    intent["declaration"]["effects"] = json!(["clock.read"]);
    invalid.push(intent);
    let mut intent = request();
    intent["declaration"]["parameters"][0]["mode"] = json!("own");
    invalid.push(intent);
    let mut intent = request();
    intent["declaration"]["return_type"] = json!("str");
    invalid.push(intent);
    let mut intent = request();
    intent["declaration"]["body"] = json!({"kind":"place","name":"result"});
    invalid.push(intent);
    let mut intent = request();
    intent["declaration"]["source"] = json!("fn injected() -> i64 { 0 }");
    invalid.push(intent);
    let mut intent = request();
    intent["target"] = json!("calculator.app.main");
    intent["declaration"]["name"] = json!("add");
    invalid.push(intent);
    for intent in invalid {
        diagnostic(attempt(&root, intent), "SPX-G225");
        assert_eq!(root.to_json(), before);
    }
    let mut mistyped = request();
    mistyped["declaration"]["body"] = json!({"kind":"bool","value":false});
    assert!(attempt(&root, mistyped).is_err());
    assert_eq!(root.to_json(), before);
}

#[test]
fn declaration_list_capacity_fails_before_candidate_creation() {
    let fixture = Fixture::new("calculator-project", "src/core.spx");
    let root = fixture.candidate();
    let mut intent = request();
    intent["declaration"]["parameters"] = json!((0..65)
        .map(|i| json!({"name":format!("v{i}"),"type":"i64","mode":"value"}))
        .collect::<Vec<_>>());
    diagnostic(attempt(&root, intent), "SPX-G226");
}

#[test]
fn borrowed_byte_input_and_owned_byte_result_use_existing_checked_call_bindings() {
    let fixture = Fixture::new("frame-payload-project", "src/frame.spx");
    let root = fixture.candidate();
    let intent = json!({"kind":"add_declaration","target":"frame.payload","declaration":{
        "id":"frame.payload-wrapper","name":"payload_wrapper","parameters":[{"name":"frame","type":"Slice<u8>","mode":"borrow"}],
        "return_type":"Bytes","effects":[],"requires":[],"ensures":[],
        "body":{"kind":"call","target":"frame.payload","arguments":[{"kind":"place","name":"frame"}]}
    }});
    let candidate = attempt(&root, intent).unwrap();
    assert!(source(&candidate, "src/frame.spx")
        .contains("fn payload_wrapper(frame: borrow Slice<u8>) -> Bytes"));
    assert_eq!(
        candidate.revision().manifest().to_canonical_toml(),
        root.revision().manifest().to_canonical_toml()
    );
    let forward = json!({"kind":"add_declaration","target":"frame.payload","declaration":{
        "id":"frame.owned-forward","name":"owned_forward","parameters":[{"name":"bytes","type":"Bytes","mode":"own"}],
        "return_type":"Bytes","effects":[],"requires":[],"ensures":[],
        "body":{"kind":"place","name":"bytes"}
    }});
    let forwarded = attempt(&candidate, forward).unwrap();
    assert!(
        source(&forwarded, "src/frame.spx").contains("fn owned_forward(bytes: own Bytes) -> Bytes")
    );
}
