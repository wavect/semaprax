//! Owning variant-case addition evidence; authored without running local gates.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(with_pattern: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-variant-case-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "variant-case"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "variant.app"
sources = ["src/app.spx"]
web_exports = []
tests = []
"#,
        )
        .unwrap();
        let body = if with_pattern {
            "let value = Choice::Empty {}; match value { Choice::Empty {} => 7, Choice::Number { value: _ } => 8, }"
        } else {
            "let value = Choice::Empty {}; 7"
        };
        let source = format!(
            r#"module variant.app;
@id("variant.choice") variant Choice {{
    @id("variant.choice.empty") Empty,
    @id("variant.choice.number") Number {{
        @id("variant.choice.number.value") value: i64,
    }},
}}
@id("variant.unrelated") fn unrelated() -> i64 {{ 1 }}
@id("variant.app") fn main() -> i64 {{ {body} }}
"#
        );
        let program = semaprax::parse(&source, Path::new("src/app.spx")).unwrap();
        std::fs::write(
            root.join("src/app.spx"),
            semaprax::format::canonical(&program),
        )
        .unwrap();
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
        ["semaprax.toml", "src/app.spx"]
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

fn request(case_id: &str, case_name: &str) -> Value {
    json!({
        "kind":"add_variant_case",
        "target":"variant.choice",
        "case":{
            "id":case_id,
            "name":case_name,
            "field":{"id":format!("{case_id}.bytes"),"name":"payload","type":"Bytes"}
        }
    })
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

fn diagnostic<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

#[test]
fn appends_one_owned_bytes_case_without_rewriting_existing_constructors() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let root = fixture.candidate();
    let change = SemanticChange::new(
        root.revision().project_revision(),
        &request("variant.choice.data", "Data"),
    )
    .unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let source = candidate.revision().sources()[0].source();
    assert!(source.contains("@id(\"variant.choice.data\") Data"));
    assert!(source.contains("@id(\"variant.choice.data.bytes\") payload: Bytes"));
    assert_eq!(source.matches("Choice::Empty {}").count(), 1);
    assert!(!source.contains("Choice::Data"));
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    assert!(graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["id"] == "variant.choice.data"
                && entry["kind"] == "variant_case"
                && entry["owner"] == "variant.choice"
        }));
    assert!(graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["id"] == "variant.choice.data.bytes"
            && entry["kind"] == "case_field"
                && entry["owner"] == "variant.choice.data"
        }));
    let replay = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), candidate.to_json());
    candidate
        .revision()
        .execute_entry(&Default::default())
        .unwrap();
    semaprax::codegen::emit_hir_c(candidate.revision().entry_program()).unwrap();
    semaprax::wasm::emit_resolved_module(candidate.revision().entry_program()).unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn rejects_patterns_string_payloads_and_identity_collisions_atomically() {
    let patterned = Fixture::new(true);
    let patterned_root = patterned.candidate();
    diagnostic(
        apply(&patterned_root, &request("variant.choice.data", "Data")),
        "SPX-G516",
    );

    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let root = fixture.candidate();
    let mut string = request("variant.choice.data", "Data");
    string["case"]["field"]["type"] = json!("string");
    diagnostic(apply(&root, &string), "SPX-G516");
    diagnostic(
        apply(&root, &request("variant.choice.empty", "Different")),
        "SPX-G516",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn merge_replays_after_unrelated_change_and_rejects_competing_case_additions() {
    let fixture = Fixture::new(false);
    let root = fixture.candidate();
    let left = apply(&root, &request("variant.choice.data", "Data")).unwrap();
    let right = apply(
        &root,
        &json!({"kind":"rename_declaration","target":"variant.unrelated","name":"renamed"}),
    )
    .unwrap();
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert!(merged.candidate().revision().sources()[0]
        .source()
        .contains("fn renamed("));

    let competing = apply(&root, &request("variant.choice.other", "Other")).unwrap();
    diagnostic(
        left.merge(
            left.candidate_digest(),
            &competing,
            competing.candidate_digest(),
        ),
        "SPX-G235",
    );
}
