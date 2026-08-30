//! Authored extraction evidence. This change did not run tests or compilers.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectExecutionOptions, ProjectRevision,
    SemanticChange,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(body: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-extraction-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let path = root.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let changed = original.replacen("{\n    left + right\n}", &format!("{{\n{body}\n}}"), 1);
        assert_ne!(original, changed);
        let program = semaprax::parse(&changed, &path).unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
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
fn open(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
    ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
}
fn core(revision: &ProjectRevision) -> &str {
    revision
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source()
}
fn expression(candidate: &ProjectCandidate, target: &str, snippet: &str) -> Value {
    let report: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let path = report["source"]["path"].as_str().unwrap();
    let source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source();
    report["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let span = &entry["source_span"];
            source.get(
                span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize,
            ) == Some(snippet)
        })
        .unwrap_or_else(|| panic!("missing expression {snippet:?}"))
        .clone()
}
fn request(
    candidate: &ProjectCandidate,
    target: &str,
    expression: &Value,
    id: &str,
    name: &str,
) -> SemanticChange {
    SemanticChange::new(candidate.revision().project_revision(),&json!({"kind":"extract_function","target":target,"expression_id":expression["expression_id"],"new_id":id,"new_name":name})).unwrap()
}
fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}
fn same_outcome(before: &ProjectRevision, after: &ProjectRevision) {
    assert_eq!(
        before
            .execute_entry(&ProjectExecutionOptions::default())
            .unwrap()
            .outcome(),
        after
            .execute_entry(&ProjectExecutionOptions::default())
            .unwrap()
            .outcome()
    );
}

#[test]
fn repeated_capture_is_one_copy_parameter_and_replay_preserves_source_authority() {
    let fixture =
        Fixture::new("    let subtotal = left + right;\n    subtotal + subtotal - subtotal");
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let root = open(&revision);
    let selected = expression(&root, "calculator.add", "subtotal + subtotal");
    let change = request(
        &root,
        "calculator.add",
        &selected,
        "calculator.double-subtotal",
        "double_subtotal",
    );
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let source = core(candidate.revision());
    assert!(source.contains("fn double_subtotal(subtotal: i64) -> i64"));
    assert!(source.contains("double_subtotal(subtotal) - subtotal"));
    assert_eq!(
        source
            .matches("@id(\"calculator.double-subtotal\")")
            .count(),
        1
    );
    same_outcome(&revision, candidate.revision());
    assert_eq!(
        candidate.revision().manifest().to_canonical_toml(),
        revision.manifest().to_canonical_toml()
    );
    let replay = ProjectCandidate::replay(
        Arc::clone(&revision),
        revision.project_revision(),
        std::slice::from_ref(&change),
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.candidate_digest(), candidate.candidate_digest());
    assert_code(
        candidate.apply(candidate.candidate_digest(), &change),
        "SPX-G224",
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn extracted_failing_expression_stays_inside_its_original_lazy_branch() {
    let fixture = Fixture::new("    if left >= 0 { left + right } else { 1 / 0 }");
    let revision = fixture.revision();
    let root = open(&revision);
    let selected = expression(&root, "calculator.add", "1 / 0");
    let change = request(
        &root,
        "calculator.add",
        &selected,
        "calculator.fail-branch",
        "fail_branch",
    );
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    assert!(core(candidate.revision()).contains("else { fail_branch() }"));
    assert!(core(candidate.revision()).contains("fn fail_branch() -> i64"));
    same_outcome(&revision, candidate.revision());
}

#[test]
fn internal_let_and_match_value_id_bindings_are_not_external_captures() {
    for (body,snippet,signature) in [
        ("    let subtotal = left + right;\n    { let doubled = subtotal + subtotal; doubled - subtotal }","{ let doubled = subtotal + subtotal; doubled - subtotal }","fn selected_core(subtotal: i64) -> i64"),
        ("    match left { n if n >= 0 => n + right, _ => left + right }","match left { n if n >= 0 => n + right, _ => left + right }","fn selected_core(left: i64, right: i64) -> i64"),
    ] {
        let fixture=Fixture::new(body);let revision=fixture.revision();let root=open(&revision);
        let selected=expression(&root,"calculator.add",snippet);
        let change=request(&root,"calculator.add",&selected,"calculator.selected-core","selected_core");
        let candidate=root.apply(root.candidate_digest(),&change).unwrap();
        assert!(core(candidate.revision()).contains(signature));
        same_outcome(&revision,candidate.revision());
    }
}

#[test]
fn mutable_captures_contracts_and_identity_collisions_fail_without_mutation() {
    let fixture = Fixture::new("    let mut subtotal = left + right;\n    subtotal");
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let root = open(&revision);
    let original = root.to_json().to_owned();
    let selected = expression(&root, "calculator.add", "subtotal");
    let change = request(
        &root,
        "calculator.add",
        &selected,
        "calculator.mutable-extract",
        "mutable_extract",
    );
    assert_code(root.apply(root.candidate_digest(), &change), "SPX-G225");
    let contract = expression(&root, "calculator.divide", "right != 0");
    let change = request(
        &root,
        "calculator.divide",
        &contract,
        "calculator.contract-extract",
        "contract_extract",
    );
    assert_code(root.apply(root.candidate_digest(), &change), "SPX-G225");
    let selected = expression(&root, "calculator.multiply", "left * right");
    for (id, name) in [
        ("calculator.add", "new_helper"),
        ("calculator.new-helper", "add"),
    ] {
        let change = request(&root, "calculator.multiply", &selected, id, name);
        assert_code(root.apply(root.candidate_digest(), &change), "SPX-G225");
    }
    assert_eq!(root.to_json(), original);
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn extraction_rebase_reauthenticates_expression_after_unrelated_source_shift() {
    let fixture = Fixture::new("    let subtotal = left + right;\n    subtotal");
    let revision = fixture.revision();
    let root = open(&revision);
    let selected = expression(&root, "calculator.multiply", "left * right");
    let change = request(
        &root,
        "calculator.multiply",
        &selected,
        "calculator.multiply-core",
        "multiply_core",
    );
    let extracted = root.apply(root.candidate_digest(), &change).unwrap();
    let rename = SemanticChange::new(
        revision.project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
    )
    .unwrap();
    let shifted = root.apply(root.candidate_digest(), &rename).unwrap();
    let rebased = extracted
        .rebase(
            extracted.candidate_digest(),
            Arc::clone(shifted.revision()),
            shifted.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    assert!(core(rebased.revision()).contains("fn addition("));
    assert!(core(rebased.revision()).contains("fn multiply_core(left: i64, right: i64) -> i64"));
    same_outcome(&revision, rebased.revision());
}
