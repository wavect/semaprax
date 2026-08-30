//! Conservative semantic rebase evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
};
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
            "spx-candidate-rebase-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = self.revision();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
    fn helper(&self) {
        let path = self.0.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap()
            + "\n@id(\"calculator.helper\") fn helper(value: i64) -> i64 { add(value, 1) }\n";
        std::fs::write(
            path,
            semaprax::format::canonical(&semaprax::parse(&source, "src/core.spx").unwrap()),
        )
        .unwrap();
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
fn body(target: &str, value: i64) -> Value {
    json!({"kind":"replace_function_body","target":target,"body":{"kind":"i64","value":value}})
}
fn rename(target: &str, name: &str) -> Value {
    json!({"kind":"rename_declaration","target":target,"name":name})
}
fn signature(name: &str) -> Value {
    json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":name,"type":"i64","argument":{"kind":"i64","value":0}}]})
}
fn source(candidate: &ProjectCandidate) -> String {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source()
        .to_owned()
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
fn unchanged_type_spelling_cannot_hide_a_concurrent_nominal_identity_change() {
    let fixture = Fixture::new();
    let manifest_path = fixture.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("semaprax.project.v1", "semaprax.project.v8")
        .replace(
            "name = \"calculator\"",
            "name = \"calculator\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"",
        )
        .replace("\"calculator.divide\", ", "");
    std::fs::write(manifest_path, manifest).unwrap();
    let path = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&path).unwrap()
        + r#"
@id("calculator.money.old") record Money { @id("calculator.money.amount.old") amount: i64, }
@id("calculator.nominal") fn nominal(value: Money) -> i64 { value.amount }
@id("calculator.nominal-call") fn nominal_call() -> i64 { nominal(Money { amount: 7 }) }
"#;
    let canonical = semaprax::format::canonical(&semaprax::parse(&source, "src/core.spx").unwrap());
    std::fs::write(&path, &canonical).unwrap();
    let root = fixture.candidate();
    let candidate = apply(
        &root,
        json!({"kind":"change_function_signature","target":"calculator.nominal","parameters":[{"from":"value","name":"payment"}]}),
    );
    let candidate_bytes = candidate.to_json().to_owned();
    let changed = canonical
        .replace("calculator.money.old", "calculator.money.new")
        .replace("calculator.money.amount.old", "calculator.money.amount.new");
    std::fs::write(&path, &changed).unwrap();
    let new_base = fixture.revision();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(&new_base),
            new_base.project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(candidate.to_json(), candidate_bytes);
    assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
}

#[test]
fn merge_keeps_both_same_file_changes_and_the_original_diff_base() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let left = apply(&root, body("calculator.add", 7));
    let right = apply(&root, rename("calculator.subtract", "difference"));
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert_eq!(
        merged.candidate().base_revision().project_revision(),
        root.base_revision().project_revision()
    );
    assert!(source(merged.candidate()).contains("fn difference("));
    assert!(source(merged.candidate()).contains("    7\n"));
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(report["left_parent_candidate"], left.candidate_digest());
    assert_eq!(report["right_parent_candidate"], right.candidate_digest());
    let evidence: Value = serde_json::from_str(merged.candidate().to_json()).unwrap();
    assert_eq!(evidence["changes"].as_array().unwrap().len(), 2);
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
}

#[test]
fn aggregate_intentions_reject_concurrent_member_identity_or_type_changes() {
    for edit in ["identity", "type"] {
        let fixture = Fixture::new();
        let manifest_path = fixture.0.join("semaprax.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("semaprax.project.v1", "semaprax.project.v8")
            .replace(
                "name = \"calculator\"",
                "name = \"calculator\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"",
            )
            .replace("\"calculator.divide\", ", "");
        std::fs::write(manifest_path, manifest).unwrap();
        let path = fixture.0.join("src/core.spx");
        let text = std::fs::read_to_string(&path).unwrap()
            + r#"
@id("calculator.money") record Money { @id("calculator.money.amount") amount: i64, }
@id("calculator.consume") fn consume(value: Money) -> i64 { 7 }
@id("calculator.aggregate-user") fn aggregate_user() -> i64 { 7 }
"#;
        let canonical =
            semaprax::format::canonical(&semaprax::parse(&text, "src/core.spx").unwrap());
        std::fs::write(&path, &canonical).unwrap();
        let root = fixture.candidate();
        let candidate = apply(
            &root,
            json!({"kind":"replace_function_body","target":"calculator.aggregate-user","body":{
                "kind":"call","target":"calculator.consume","arguments":[{
                    "kind":"record","target":"calculator.money","fields":[{
                        "target":"calculator.money.amount","value":{"kind":"i64","value":7}
                    }]
                }]
            }}),
        );
        let unchanged = candidate.to_json().to_owned();
        let independent = apply(&root, rename("calculator.subtract", "difference"));
        let merged = candidate
            .merge(
                candidate.candidate_digest(),
                &independent,
                independent.candidate_digest(),
            )
            .unwrap();
        assert!(source(merged.candidate()).contains("fn difference("));
        assert!(source(merged.candidate()).contains("consume(Money { amount: 7 })"));
        let changed = if edit == "identity" {
            canonical.replace("calculator.money.amount", "calculator.money.new-amount")
        } else {
            canonical.replace("amount: i64", "amount: bool")
        };
        assert_ne!(changed, canonical);
        std::fs::write(&path, &changed).unwrap();
        let new_base = fixture.revision();
        code(
            candidate.rebase(
                candidate.candidate_digest(),
                Arc::clone(&new_base),
                new_base.project_revision(),
            ),
            "SPX-G235",
        );
        assert_eq!(candidate.to_json(), unchanged);
        assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
    }
}

#[test]
fn same_target_body_and_display_rename_rebase_and_merge() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = apply(&root, body("calculator.add", 9));
    let right = apply(&root, rename("calculator.add", "sum"));
    let rebased = left
        .rebase(
            left.candidate_digest(),
            Arc::clone(right.revision()),
            right.revision().project_revision(),
        )
        .unwrap();
    assert_eq!(
        rebased.candidate().base_revision().project_revision(),
        right.revision().project_revision()
    );
    assert!(source(rebased.candidate()).contains("fn sum("));
    assert!(source(rebased.candidate()).contains("    9\n"));
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert_eq!(
        merged.candidate().revision().project_revision(),
        rebased.candidate().revision().project_revision()
    );
    assert_ne!(
        merged.candidate().candidate_digest(),
        rebased.candidate().candidate_digest()
    );
}

#[test]
fn generic_aggregate_rebase_binds_checked_template_field_types() {
    let fixture = Fixture::new();
    let manifest_path = fixture.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("semaprax.project.v1", "semaprax.project.v8")
        .replace(
            "name = \"calculator\"",
            "name = \"calculator\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"",
        )
        .replace("\"calculator.divide\", ", "");
    std::fs::write(manifest_path, manifest).unwrap();
    let path = fixture.0.join("src/core.spx");
    let text = std::fs::read_to_string(&path).unwrap()
        + r#"
@id("calculator.box") record Box<T> { @id("calculator.box.value") value: T, }
@id("calculator.consume-box") fn consume_box(value: Box<i64>) -> i64 { 7 }
@id("calculator.generic-user") fn generic_user() -> i64 { 7 }
"#;
    let canonical = semaprax::format::canonical(&semaprax::parse(&text, "src/core.spx").unwrap());
    std::fs::write(&path, &canonical).unwrap();
    let root = fixture.candidate();
    let candidate = apply(
        &root,
        json!({"kind":"replace_function_body","target":"calculator.generic-user","body":{
            "kind":"call","target":"calculator.consume-box","arguments":[{
                "kind":"record","target":"calculator.box","type_arguments":["i64"],"fields":[{
                    "target":"calculator.box.value","value":{"kind":"i64","value":7}
                }]
            }]
        }}),
    );
    let independent = apply(&root, rename("calculator.subtract", "difference"));
    let merged = candidate
        .merge(
            candidate.candidate_digest(),
            &independent,
            independent.candidate_digest(),
        )
        .unwrap();
    assert!(source(merged.candidate()).contains("consume_box(Box<i64> { value: 7 })"));
    assert!(source(merged.candidate()).contains("fn difference("));
    let before = candidate.to_json().to_owned();
    // The function and nominal instance identities stay the same. Only the
    // checked template field changes, so the aggregate dependency must catch it.
    let changed = canonical.replace("value: T", "value: bool");
    assert_ne!(changed, canonical);
    std::fs::write(&path, &changed).unwrap();
    let new_base = fixture.revision();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(&new_base),
            new_base.project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(candidate.to_json(), before);
    assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
}

#[test]
fn callee_display_rename_does_not_create_a_false_body_conflict() {
    let fixture = Fixture::new();
    fixture.helper();
    let root = fixture.candidate();
    let left = apply(
        &root,
        json!({"kind":"replace_function_body","target":"calculator.helper","body":{"kind":"call","target":"calculator.add","arguments":[{"kind":"place","name":"value"},{"kind":"i64","value":2}]}}),
    );
    let right = apply(&root, rename("calculator.add", "sum"));
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert!(source(merged.candidate()).contains("sum(value, 2)"));
}

#[test]
fn body_and_new_contract_revalidate_instead_of_implicitly_claiming_equivalence() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let body_branch = apply(&root, body("calculator.add", 42));
    let contract_branch = apply(
        &root,
        json!({"kind":"add_contract","target":"calculator.add","phase":"ensures","predicate":{"kind":"bool","value":true}}),
    );
    let merged = body_branch
        .merge(
            body_branch.candidate_digest(),
            &contract_branch,
            contract_branch.candidate_digest(),
        )
        .unwrap();
    assert!(source(merged.candidate()).contains("ensures true"));
    assert!(source(merged.candidate()).contains("    42\n"));
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(
        report["classifications"][0]["concurrent_contract_change"],
        true
    );
    assert_eq!(report["source_authority"], false);
}

#[test]
fn conflicting_signatures_bodies_deleted_targets_and_stale_handles_reject() {
    let fixture = Fixture::new();
    fixture.helper();
    let root = fixture.candidate();
    let left = apply(&root, signature("offset"));
    let right = apply(&root, signature("delta"));
    code(
        left.merge(left.candidate_digest(), &right, right.candidate_digest()),
        "SPX-G235",
    );
    let first = apply(&root, body("calculator.helper", 1));
    let second = apply(&root, body("calculator.helper", 2));
    let added_caller = apply(
        &root,
        json!({
            "kind":"replace_function_body", "target":"calculator.subtract",
            "body":{"kind":"call","target":"calculator.helper","arguments":[{"kind":"place","name":"left"}]},
        }),
    );
    let unchanged = first.to_json().to_owned();
    code(
        first.merge(first.candidate_digest(), &second, second.candidate_digest()),
        "SPX-G235",
    );
    code(
        first.rebase(
            "stale",
            Arc::clone(root.revision()),
            root.revision().project_revision(),
        ),
        "SPX-G235",
    );
    let path = fixture.0.join("src/core.spx");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut ast = semaprax::parse(&text, "src/core.spx").unwrap();
    ast.functions
        .retain(|function| function.stable_id != "calculator.helper");
    std::fs::write(path, semaprax::format::canonical(&ast)).unwrap();
    let deleted = fixture.revision();
    code(
        first.rebase(
            first.candidate_digest(),
            Arc::clone(&deleted),
            deleted.project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(first.to_json(), unchanged);
    code(
        added_caller.rebase(
            added_caller.candidate_digest(),
            Arc::clone(&deleted),
            deleted.project_revision(),
        ),
        "SPX-G235",
    );
}

#[test]
fn common_prefix_is_not_duplicated_and_manifest_changes_are_closed() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let prefix = apply(&root, rename("calculator.subtract", "difference"));
    let left = apply(&prefix, body("calculator.add", 1));
    let right = apply(&prefix, body("calculator.multiply", 2));
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(report["shared_history_prefix"], 1);
    let evidence: Value = serde_json::from_str(merged.candidate().to_json()).unwrap();
    assert_eq!(evidence["changes"].as_array().unwrap().len(), 3);
    let manifest = fixture.0.join("semaprax.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("name = \"calculator\"", "name = \"another-calculator\"");
    std::fs::write(manifest, text).unwrap();
    let changed = fixture.revision();
    code(
        left.rebase(
            left.candidate_digest(),
            Arc::clone(&changed),
            changed.project_revision(),
        ),
        "SPX-G233",
    );
}
