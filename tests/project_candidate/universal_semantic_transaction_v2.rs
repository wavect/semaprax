use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
    SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SEMANTIC_TRANSACTION_SCHEMA,
    SEMANTIC_TRANSACTION_V2_EVIDENCE_SCHEMA, SEMANTIC_TRANSACTION_V2_IMPACT_SCHEMA,
    SEMANTIC_TRANSACTION_V2_RESULT_SCHEMA, SEMANTIC_TRANSACTION_V2_REVIEW_SCHEMA,
    SEMANTIC_TRANSACTION_V2_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-universal-semantic-transaction-v2-{}-{}",
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
        let source = std::fs::read_to_string(&core_path).unwrap();
        let source = source.replacen(
            "{\n    left + right\n}",
            "{\n    let subtotal = left + right;\n    let bonus = 1;\n    subtotal + bonus - 1\n}",
            1,
        );
        let parsed = semaprax::parse(&source, &core_path).unwrap();
        std::fs::write(&core_path, semaprax::format::canonical(&parsed)).unwrap();
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
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                entries.insert(relative, Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

fn selection(revision: &Arc<ProjectRevision>, target: &str, snippet: &str) -> (String, String) {
    let root = ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap();
    let catalog: Value = serde_json::from_str(&root.expression_catalog(target).unwrap()).unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == catalog["source"]["path"].as_str().unwrap())
        .unwrap()
        .source();
    let row = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let start = row["source_span"]["start"].as_u64().unwrap() as usize;
            let end = row["source_span"]["end"].as_u64().unwrap() as usize;
            source.get(start..end) == Some(snippet)
        })
        .unwrap_or_else(|| panic!("missing expression {snippet:?}"));
    (
        row["expression_id"].as_str().unwrap().to_owned(),
        snippet.to_owned(),
    )
}

fn replacement() -> Value {
    json!({
        "kind":"binary", "op":"+",
        "left":{"kind":"place","name":"subtotal"},
        "right":{"kind":"i64","value":2}
    })
}

#[test]
fn nested_expression_is_deterministic_candidate_equivalent_read_only_and_replayable() {
    let fixture = Fixture::new();
    let disk_before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.add", "subtotal + bonus");
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            &expression_id,
            &old,
            replacement(),
        ),
    )
    .unwrap();
    assert_eq!(
        SemanticTransactionV2::from_json(transaction.to_json().as_bytes())
            .unwrap()
            .to_json(),
        transaction.to_json()
    );
    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let repeated = transaction.validate(Arc::clone(&revision)).unwrap();
    assert_eq!(artifacts.evidence(), repeated.evidence());
    assert_eq!(artifacts.result(), repeated.result());
    assert_eq!(
        serde_json::from_str::<Value>(transaction.to_json()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.impact()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_IMPACT_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.review()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_REVIEW_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.result()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_RESULT_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.evidence()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_EVIDENCE_SCHEMA
    );
    assert!(artifacts
        .candidate()
        .revision()
        .sources()
        .iter()
        .any(|source| source.path() == "src/core.spx"
            && source.source().contains("subtotal + 2 - 1")));

    let direct_root =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let direct_change = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "expression_id":expression_id, "kind":"replace_expression",
            "replacement":replacement(), "target":"calculator.add"
        }),
    )
    .unwrap();
    let direct = direct_root
        .apply(direct_root.candidate_digest(), &direct_change)
        .unwrap();
    assert_eq!(artifacts.candidate().to_json(), direct.to_json());
    assert_eq!(
        SemanticTransactionV2::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            artifacts.evidence().as_bytes(),
        )
        .unwrap()
        .result(),
        artifacts.result()
    );
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn stale_contract_type_and_wire_mutations_fail_closed() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.add", "subtotal + bonus");
    let make = |id: &str, expected: &str, replacement: Value| {
        SemanticTransactionV2::replace_expression(
            workspace.workspace_revision(),
            SemanticTransactionReplaceExpression::new("calculator.add", id, expected, replacement),
        )
        .unwrap()
    };
    assert_code(
        make("stale-expression-id", &old, replacement()).validate(Arc::clone(&revision)),
        "SPX-G527",
    );
    assert_code(
        make(&expression_id, "subtotal - bonus", replacement()).validate(Arc::clone(&revision)),
        "SPX-G527",
    );
    assert!(
        make(&expression_id, &old, json!({"kind":"bool","value":true}))
            .validate(Arc::clone(&revision))
            .is_err()
    );

    let (contract_id, contract) = selection(&revision, "calculator.divide", "right != 0");
    let contract = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.divide",
            contract_id,
            contract,
            json!({"kind":"bool","value":true}),
        ),
    )
    .unwrap();
    assert_code(contract.validate(Arc::clone(&revision)), "SPX-G525");

    let transaction = make(&expression_id, &old, replacement());
    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let other = make(
        &expression_id,
        &old,
        json!({
            "kind":"binary", "op":"+",
            "left":{"kind":"place","name":"subtotal"},
            "right":{"kind":"i64","value":3}
        }),
    );
    let other_artifacts = other.validate(Arc::clone(&revision)).unwrap();
    assert_code(
        SemanticTransactionV2::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            other_artifacts.evidence().as_bytes(),
        ),
        "SPX-G527",
    );
    let mut tampered: Value = serde_json::from_str(artifacts.evidence()).unwrap();
    tampered["artifacts"]["result"]["value"]["authority"]["granted"] = json!(true);
    let mut tampered = serde_json::to_string(&tampered).unwrap();
    tampered.push('\n');
    assert_code(
        SemanticTransactionV2::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            tampered.as_bytes(),
        ),
        "SPX-G527",
    );
    assert_code(
        SemanticTransactionV2::from_json(transaction.to_json().trim_end().as_bytes()),
        "SPX-G525",
    );
}

#[test]
fn main_is_admitted_and_v1_wire_remains_closed_and_byte_stable() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.app.main", "6");
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.app.main",
            expression_id,
            old,
            json!({"kind":"i64","value":7}),
        ),
    )
    .unwrap();
    transaction.validate(Arc::clone(&revision)).unwrap();

    let v1 = SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let expected_v1: Value = json!({
        "expected_workspace_revision": workspace.workspace_revision(),
        "invariants": [
            "preserve_stable_identity", "preserve_public_exports", "update_all_callers",
            "no_new_effects", "no_new_capabilities", "preserve_contracts",
            "revalidate_ownership_and_cleanup", "preserve_project_profile_admission",
            "preserve_admitted_core_targets"
        ],
        "operations": [{
            "expected_old_value":"add", "kind":"rename_display_name",
            "new_value":"sum", "target":"calculator.add"
        }],
        "requested_authority":"none",
        "requested_validation":[
            "canonical_source_round_trip", "complete_project_admission",
            "ownership_and_cleanup", "native_and_wasm_emission",
            "canonical_workspace_revision"
        ],
        "schema":SEMANTIC_TRANSACTION_SCHEMA,
    });
    let mut expected_v1 = serde_json::to_string(&expected_v1).unwrap();
    expected_v1.push('\n');
    assert_eq!(v1.to_json(), expected_v1);
    assert_code(
        SemanticTransaction::from_json(transaction.to_json().as_bytes()),
        "SPX-G525",
    );
    assert_code(
        SemanticTransactionV2::from_json(v1.to_json().as_bytes()),
        "SPX-G525",
    );
}

#[test]
fn replacement_values_are_bounded_iteratively_before_cloning_or_rendering() {
    let digest = format!("sha256:{}", "0".repeat(64));
    let mut deep = json!({"kind":"i64","value":0});
    for _ in 0..65 {
        deep = json!({"kind":"nested","value":deep});
    }
    assert_code(
        SemanticTransactionV2::replace_expression(
            &digest,
            SemanticTransactionReplaceExpression::new("target", "expression", "0", deep),
        ),
        "SPX-G526",
    );
    assert_code(
        SemanticTransactionV2::replace_expression(
            &digest,
            SemanticTransactionReplaceExpression::new(
                "target",
                "expression",
                "0",
                Value::Array(vec![Value::Null; 8_193]),
            ),
        ),
        "SPX-G526",
    );
    assert_code(
        SemanticTransactionV2::replace_expression(
            &digest,
            SemanticTransactionReplaceExpression::new(
                "target",
                "expression",
                "0",
                json!({"kind":"text","value":"x".repeat(1024 * 1024 + 1)}),
            ),
        ),
        "SPX-G526",
    );
}
