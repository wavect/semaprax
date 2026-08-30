//! Authenticated expression-hole regressions; authored, not locally executed.
use semaprax::project::{with_authenticated_project, ProjectCandidate, ProjectCandidateDraft};
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
            "spx-expression-holes-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let path = root.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap()
            + r#"
@id("holes.locals") fn locals(left: i64, right: i64) -> i64 {
    let subtotal = left + right;
    let bonus = 1;
    subtotal + bonus
}
"#;
        let program = semaprax::parse(&source, Path::new("src/core.spx")).unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn selection(candidate: &ProjectCandidate, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog("holes.locals").unwrap()).unwrap();
    let source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            let span = &item["source_span"];
            item["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn has_name(context: &Value, name: &str) -> bool {
    context["scope"]
        .as_array()
        .unwrap()
        .iter()
        .any(|binding| binding["name"] == name)
}

#[test]
fn lexical_context_and_disjoint_fill_remap_preserve_unresolved_state() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let candidate = fixture.candidate();
    let first = selection(&candidate, "left + right");
    let second = selection(&candidate, "1");
    let draft = ProjectCandidateDraft::open(Arc::clone(&candidate)).unwrap();
    let draft = draft
        .with_expression_hole(draft.draft_digest(), "holes.locals", &first, "a")
        .unwrap();
    let draft = draft
        .with_expression_hole(draft.draft_digest(), "holes.locals", &second, "b")
        .unwrap();
    let context: Value =
        serde_json::from_str(&draft.hole_context(draft.draft_digest(), "a").unwrap()).unwrap();
    assert_eq!(context["expected_type_id"], "i64");
    assert!(has_name(&context, "left"));
    assert!(!has_name(&context, "subtotal"));
    assert!(!has_name(&context, "bonus"));
    assert!(draft.complete(draft.draft_digest()).is_err());
    let digest = draft.draft_digest().to_owned();
    assert!(draft
        .fill_hole(&digest, "a", &json!({"kind":"bool","value":false}))
        .is_err());
    assert_eq!(draft.draft_digest(), digest);
    let filled=draft.fill_hole(&digest,"a",&json!({"kind":"binary","op":"+","left":{"kind":"place","name":"left"},"right":{"kind":"if","condition":{"kind":"bool","value":true},"then":{"kind":"i64","value":2},"else":{"kind":"i64","value":3}}})).unwrap();
    assert!(filled.complete(filled.draft_digest()).is_err());
    let context: Value =
        serde_json::from_str(&filled.hole_context(filled.draft_digest(), "b").unwrap()).unwrap();
    assert!(has_name(&context, "subtotal"));
    assert!(!has_name(&context, "bonus"));
    assert_eq!(
        context["last_valid_revision"],
        serde_json::from_str::<Value>(filled.to_json()).unwrap()["last_valid_revision"]
    );
    let complete = filled
        .fill_hole(
            filled.draft_digest(),
            "b",
            &json!({"kind":"place","name":"subtotal"}),
        )
        .unwrap();
    let result = complete.complete(complete.draft_digest()).unwrap();
    assert!(result
        .revision()
        .sources()
        .iter()
        .any(|source| source.source().contains("let bonus = subtotal;")));
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
    assert!(draft
        .fill_hole(filled.draft_digest(), "b", &json!({"kind":"i64","value":0}))
        .is_err());
}

#[test]
fn ancestor_body_and_duplicate_holes_reject_without_overwriting_siblings() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let binary = selection(&candidate, "left + right");
    let child = selection(&candidate, "left");
    let draft = ProjectCandidateDraft::open(Arc::clone(&candidate)).unwrap();
    let draft = draft
        .with_expression_hole(draft.draft_digest(), "holes.locals", &binary, "selected")
        .unwrap();
    for result in [
        draft.with_expression_hole(draft.draft_digest(), "holes.locals", &child, "nested"),
        draft.with_body_hole(draft.draft_digest(), "holes.locals", "body"),
        draft.with_expression_hole(draft.draft_digest(), "holes.locals", &binary, "selected"),
    ] {
        match result {
            Ok(_) => panic!("overlap admitted"),
            Err(errors) => assert!(errors.iter().any(|error| error.code == "SPX-G230")),
        }
    }
    let mixed = draft
        .with_body_hole(draft.draft_digest(), "calculator.multiply", "body")
        .unwrap();
    let mixed=mixed.fill_hole(mixed.draft_digest(),"body",&json!({"kind":"binary","op":"*","left":{"kind":"place","name":"left"},"right":{"kind":"place","name":"right"}})).unwrap();
    assert!(mixed.complete(mixed.draft_digest()).is_err());
    assert!(mixed.hole_context(mixed.draft_digest(), "selected").is_ok());
}
