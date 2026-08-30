//! Ephemeral-hole evidence authored without running local tests or gates.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, ProjectCandidateDraft};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-candidate-holes-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap()
    }
    fn append_source(&self, file: &str, extra: &str) {
        let path = self.0.join(file);
        let source = std::fs::read_to_string(&path).unwrap() + extra;
        let ast = semaprax::parse(&source, file).unwrap();
        std::fs::write(path, semaprax::format::canonical(&ast)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
fn reject_materialization_fields(value: &Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    ![
                        "candidate_revision",
                        "replacement_source",
                        "source_changes",
                        "source_diff"
                    ]
                    .contains(&key.as_str()),
                    "forbidden field {key}"
                );
                reject_materialization_fields(value);
            }
        }
        Value::Array(array) => {
            for value in array {
                reject_materialization_fields(value);
            }
        }
        _ => {}
    }
}

#[test]
fn unresolved_holes_have_typed_context_but_no_materializable_candidate() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let original = candidate.to_json().to_owned();
    let empty = ProjectCandidateDraft::open(Arc::clone(&candidate)).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "calculator.divide", "body.divide")
        .unwrap();
    code(draft.complete(draft.draft_digest()), "SPX-G232");
    let summary: Value = serde_json::from_str(draft.to_json()).unwrap();
    assert_eq!(summary["state"], "incomplete");
    assert_eq!(summary["materializable"], false);
    reject_materialization_fields(&summary);
    let context: Value = serde_json::from_str(
        &draft
            .hole_context(draft.draft_digest(), "body.divide")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(context["scope"].as_array().unwrap().len(), 2);
    assert_eq!(context["scope"][1]["name"], "right");
    assert_eq!(context["scope"][1]["ownership"], "value");
    assert_eq!(context["contracts"][0]["phase"], "requires");
    assert_eq!(context["contracts"][0]["expression"]["op"], "!=");
    assert_eq!(
        context["prior_body_proof"]["basis"],
        "last_valid_body_not_the_unfilled_hole"
    );
    assert_eq!(context["validation"], "pending_fill_full_source_replay");
    reject_materialization_fields(&context);
    assert_eq!(candidate.to_json(), original);
    assert_eq!(
        empty
            .complete(empty.draft_digest())
            .unwrap()
            .candidate_digest(),
        candidate.candidate_digest()
    );
}

#[test]
fn filling_one_of_two_holes_never_releases_an_incomplete_candidate() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let empty = ProjectCandidateDraft::open(candidate).unwrap();
    let one = empty
        .with_body_hole(empty.draft_digest(), "calculator.add", "add")
        .unwrap();
    let two = one
        .with_body_hole(one.draft_digest(), "calculator.subtract", "subtract")
        .unwrap();
    let previous = two.to_json().to_owned();
    assert!(two
        .fill_hole(
            two.draft_digest(),
            "add",
            &json!({"kind":"bool","value":true})
        )
        .is_err());
    assert_eq!(two.to_json(), previous);
    let first = two
        .fill_hole(two.draft_digest(), "add", &json!({"kind":"i64","value":7}))
        .unwrap();
    code(first.complete(first.draft_digest()), "SPX-G232");
    let complete = first
        .fill_hole(
            first.draft_digest(),
            "subtract",
            &json!({"kind":"i64","value":3}),
        )
        .unwrap();
    let candidate = complete.complete(complete.draft_digest()).unwrap();
    assert!(candidate.to_json().contains("replacement_source"));
    code(two.complete(two.draft_digest()), "SPX-G232");
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
}

#[test]
fn context_exposes_only_local_and_authenticated_import_bindings() {
    let fixture = Fixture::new();
    fixture.append_source(
        "src/app.spx",
        "\n@id(\"calculator.helper\") fn helper(value: i64) -> i64 { add(value, 0) }\n",
    );
    let empty = ProjectCandidateDraft::open(fixture.candidate()).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "calculator.helper", "helper")
        .unwrap();
    let context: Value =
        serde_json::from_str(&draft.hole_context(draft.draft_digest(), "helper").unwrap()).unwrap();
    let calls = context["accessible_calls"].as_array().unwrap();
    assert!(calls.iter().any(|call| call["id"] == "calculator.add"));
    assert!(calls.iter().any(|call| call["id"] == "calculator.helper"));
    assert!(!calls.iter().any(|call| call["id"] == "calculator.not"));
    let filled=draft.fill_hole(draft.draft_digest(),"helper",&json!({"kind":"call","target":"calculator.add","arguments":[{"kind":"place","name":"value"},{"kind":"i64","value":1}]})).unwrap();
    assert!(filled.complete(filled.draft_digest()).is_ok());
}

#[test]
fn stale_duplicate_unknown_and_oversized_holes_fail_without_mutation() {
    let fixture = Fixture::new();
    let empty = ProjectCandidateDraft::open(fixture.candidate()).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "calculator.add", "first")
        .unwrap();
    code(
        draft.hole_context(empty.draft_digest(), "first"),
        "SPX-G232",
    );
    code(
        draft.with_body_hole(draft.draft_digest(), "calculator.add", "second"),
        "SPX-G230",
    );
    code(
        draft.with_body_hole(draft.draft_digest(), "calculator.subtract", "first"),
        "SPX-G230",
    );
    code(
        draft.hole_context(draft.draft_digest(), "missing"),
        "SPX-G230",
    );
    code(
        draft.with_body_hole(
            draft.draft_digest(),
            "calculator.subtract",
            &"h".repeat(129),
        ),
        "SPX-G231",
    );
    code(
        draft.with_body_hole(draft.draft_digest(), "calculator.app.main", "main"),
        "SPX-G230",
    );
    code(
        draft.with_body_hole(draft.draft_digest(), "calculator.subtract", "bad\0id"),
        "SPX-G230",
    );
    let sibling = empty
        .with_body_hole(empty.draft_digest(), "calculator.subtract", "first")
        .unwrap();
    code(
        sibling.fill_hole(
            draft.draft_digest(),
            "first",
            &json!({"kind":"i64","value":1}),
        ),
        "SPX-G232",
    );
}

#[test]
fn pending_hole_capacity_is_exact_and_deterministic_across_roots() {
    let fixture = Fixture::new();
    let mut extra = String::new();
    for index in 0..17 {
        extra.push_str(&format!(
            "\n@id(\"hole.function.{index}\") fn hole_{index}() -> i64 {{ {index} }}\n"
        ));
    }
    fixture.append_source("src/core.spx", &extra);
    let mut draft = ProjectCandidateDraft::open(fixture.candidate()).unwrap();
    for index in 0..16 {
        draft = draft
            .with_body_hole(
                draft.draft_digest(),
                &format!("hole.function.{index}"),
                &format!("h{index}"),
            )
            .unwrap();
    }
    code(
        draft.with_body_hole(draft.draft_digest(), "hole.function.16", "h16"),
        "SPX-G231",
    );
    let left = ProjectCandidateDraft::open(Fixture::new().candidate()).unwrap();
    let right = ProjectCandidateDraft::open(Fixture::new().candidate()).unwrap();
    assert_eq!(left.to_json(), right.to_json());
    assert_eq!(left.draft_digest(), right.draft_digest());
}
