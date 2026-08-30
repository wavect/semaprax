//! Authored expression-selection evidence. No local test execution was used
//! when introducing this surface.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);

impl Fixture {
    fn new(add_body: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-expression-v1-{}-{}",
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
        let core_path = root.join("src/core.spx");
        let core = std::fs::read_to_string(&core_path).unwrap();
        let changed = core.replacen("{\n    left + right\n}", &format!("{{\n{add_body}\n}}"), 1);
        assert_ne!(core, changed);
        let parsed = semaprax::parse(&changed, &core_path).unwrap();
        std::fs::write(core_path, semaprax::format::canonical(&parsed)).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                out.insert(relative, Vec::new());
                visit(root, &path, out);
            } else {
                out.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn root(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
    ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
}
fn catalog(candidate: &ProjectCandidate, target: &str) -> Value {
    serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap()
}
fn selected<'a>(catalog: &'a Value, revision: &ProjectRevision, snippet: &str) -> &'a Value {
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == catalog["source"]["path"].as_str().unwrap())
        .unwrap()
        .source();
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let span = &entry["source_span"];
            source.get(
                span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize,
            ) == Some(snippet)
        })
        .unwrap_or_else(|| panic!("missing source expression {snippet:?}"))
}
fn change(
    revision: &ProjectRevision,
    target: &str,
    entry: &Value,
    replacement: Value,
) -> SemanticChange {
    SemanticChange::new(revision.project_revision(),&json!({"kind":"replace_expression","target":target,"expression_id":entry["expression_id"],"replacement":replacement})).unwrap()
}
fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

#[test]
fn actual_hir_expression_ids_expose_local_scope_and_replace_without_writing_sources() {
    let fixture = Fixture::new(
        "    let subtotal = left + right;\n    let bonus = 1;\n    subtotal + bonus - 1",
    );
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let candidate = root(&revision);
    let catalogue = catalog(&candidate, "calculator.add");
    assert_eq!(
        catalogue["schema"],
        "semaprax.project-expression-catalog.v1"
    );
    assert_eq!(catalogue["candidate_digest"], candidate.candidate_digest());
    assert_eq!(catalogue["project_revision"], revision.project_revision());
    let entry = selected(&catalogue, &revision, "subtotal + bonus");
    assert_eq!(entry["expected_type"], "i64");
    assert_eq!(entry["ownership"], "value");
    assert_eq!(entry["replaceable"], true);
    for name in ["left", "right", "subtotal", "bonus"] {
        assert!(entry["scope"]
            .as_array()
            .unwrap()
            .iter()
            .any(|binding| binding["name"] == name));
    }
    let request = change(
        &revision,
        "calculator.add",
        entry,
        json!({"kind":"binary","op":"+","left":{"kind":"place","name":"subtotal"},"right":{"kind":"i64","value":2}}),
    );
    let applied = candidate
        .apply(candidate.candidate_digest(), &request)
        .unwrap();
    assert!(applied
        .revision()
        .sources()
        .iter()
        .any(|source| source.path() == "src/core.spx"
            && source.source().contains("subtotal + 2 - 1")));
    let replay = ProjectCandidate::replay(
        Arc::clone(&revision),
        revision.project_revision(),
        &[request.clone()],
        applied.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.candidate_digest(), applied.candidate_digest());
    assert_code(
        applied.apply(applied.candidate_digest(), &request),
        "SPX-G224",
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn initializer_does_not_see_later_bindings_and_inferred_type_drift_is_rejected() {
    let fixture =
        Fixture::new("    let unused = 1;\n    let subtotal = left + right;\n    subtotal");
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let candidate = root(&revision);
    let catalogue = catalog(&candidate, "calculator.add");
    let entry = selected(&catalogue, &revision, "1");
    assert!(!entry["scope"]
        .as_array()
        .unwrap()
        .iter()
        .any(|binding| binding["name"] == "unused" || binding["name"] == "subtotal"));
    let invisible = change(
        &revision,
        "calculator.add",
        entry,
        json!({"kind":"place","name":"subtotal"}),
    );
    assert_code(
        candidate.apply(candidate.candidate_digest(), &invisible),
        "SPX-G225",
    );
    // A bool-valued unused let could pass whole-function return-type checking.
    // Post-rebuild expression admission must nevertheless preserve the selected
    // initializer's independently reported i64 expected type.
    let wrong_type = change(
        &revision,
        "calculator.add",
        entry,
        json!({"kind":"bool","value":true}),
    );
    assert_code(
        candidate.apply(candidate.candidate_digest(), &wrong_type),
        "SPX-G225",
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn contract_and_forged_selectors_are_rejected_while_main_expressions_are_available() {
    let fixture = Fixture::new("    let subtotal = left + right;\n    subtotal");
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let candidate = root(&revision);
    let catalogue = catalog(&candidate, "calculator.divide");
    let contract = selected(&catalogue, &revision, "right != 0");
    assert_eq!(contract["phase"], "requires");
    assert_eq!(contract["replaceable"], false);
    let request = change(
        &revision,
        "calculator.divide",
        contract,
        json!({"kind":"bool","value":true}),
    );
    assert_code(
        candidate.apply(candidate.candidate_digest(), &request),
        "SPX-G225",
    );
    let forged=SemanticChange::new(revision.project_revision(),&json!({"kind":"replace_expression","target":"calculator.add","expression_id":"caller-invented-id","replacement":{"kind":"i64","value":0}})).unwrap();
    assert_code(
        candidate.apply(candidate.candidate_digest(), &forged),
        "SPX-G225",
    );
    let main = catalog(&candidate, "calculator.app.main");
    let entry = main["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["phase"] == "body" && entry["kind"] == "call" && entry["replaceable"] == true
        })
        .unwrap();
    let request = change(
        &revision,
        "calculator.app.main",
        entry,
        json!({"kind":"i64","value":42}),
    );
    candidate
        .apply(candidate.candidate_digest(), &request)
        .unwrap();
    let block = main["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["phase"] == "body" && entry["kind"] == "block" && entry["replaceable"] == true
        })
        .unwrap();
    let request = change(
        &revision,
        "calculator.app.main",
        block,
        json!({"kind":"i64","value":43}),
    );
    // Function/branch block categories remain blocks after scalar construction.
    candidate
        .apply(candidate.candidate_digest(), &request)
        .unwrap();
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn match_binding_is_visible_only_in_its_guard_and_arm() {
    let fixture = Fixture::new(
        "    let subtotal = left + right;\n    match subtotal { n if n >= 0 => n, _ => subtotal }",
    );
    let revision = fixture.revision();
    let candidate = root(&revision);
    let catalogue = catalog(&candidate, "calculator.add");
    let guard = selected(&catalogue, &revision, "n >= 0");
    assert!(guard["scope"]
        .as_array()
        .unwrap()
        .iter()
        .any(|binding| binding["name"] == "n" && binding["type"] == "i64"));
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    let fallback = catalogue["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["kind"] == "place")
        .filter(|entry| {
            let span = &entry["source_span"];
            source.get(
                span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize,
            ) == Some("subtotal")
        })
        .last()
        .unwrap();
    assert!(!fallback["scope"]
        .as_array()
        .unwrap()
        .iter()
        .any(|binding| binding["name"] == "n"));
    let request = change(
        &revision,
        "calculator.add",
        guard,
        json!({"kind":"binary","op":">","left":{"kind":"place","name":"n"},"right":{"kind":"i64","value":-1}}),
    );
    candidate
        .apply(candidate.candidate_digest(), &request)
        .unwrap();
}

#[test]
fn sequential_expression_rebase_uses_each_original_intermediate_revision() {
    let fixture = Fixture::new("    let subtotal = left + right;\n    subtotal");
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let candidate = root(&revision);
    let first_catalog = catalog(&candidate, "calculator.multiply");
    let first = selected(&first_catalog, &revision, "left * right");
    let request = change(
        &revision,
        "calculator.multiply",
        first,
        json!({
            "kind":"binary","op":"*","left":{"kind":"place","name":"left"},
            "right":{"kind":"binary","op":"+","left":{"kind":"place","name":"right"},"right":{"kind":"i64","value":1}}
        }),
    );
    let first = candidate
        .apply(candidate.candidate_digest(), &request)
        .unwrap();
    let second_catalog = catalog(&first, "calculator.multiply");
    let second = selected(&second_catalog, first.revision(), "(right + 1)");
    let request = change(
        first.revision(),
        "calculator.multiply",
        second,
        json!({
            "kind":"binary","op":"+","left":{"kind":"place","name":"right"},"right":{"kind":"i64","value":2}
        }),
    );
    let second = first.apply(first.candidate_digest(), &request).unwrap();
    let rename = SemanticChange::new(
        revision.project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
    )
    .unwrap();
    let sibling = candidate
        .apply(candidate.candidate_digest(), &rename)
        .unwrap();
    let rebased = second
        .rebase(
            second.candidate_digest(),
            Arc::clone(sibling.revision()),
            sibling.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    let core = rebased
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(core.contains("fn addition("));
    assert!(core.contains("left * (right + 2)"));
    assert_eq!(inventory(&fixture.0), before);
}
