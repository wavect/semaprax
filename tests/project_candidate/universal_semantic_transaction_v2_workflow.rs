use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SemanticTransactionV2Workflow,
    MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS, SEMANTIC_TRANSACTION_V2_WORKFLOW_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-universal-semantic-transaction-v2-workflow-{}-{}",
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

/// Exact snippet match, mirroring `universal_semantic_transaction_v2.rs`'s
/// helper: returns the replaceable body expression's identity from the
/// revision's own catalog, never a caller-transcribed guess.
fn selection(revision: &Arc<ProjectRevision>, target: &str, snippet: &str) -> (String, String) {
    let root = ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap();
    let catalog: Value = serde_json::from_str(&root.expression_catalog(target).unwrap()).unwrap();
    let path = catalog["source"]["path"].as_str().unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == path)
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
        .unwrap_or_else(|| panic!("missing expression {snippet:?} in {target}"));
    (
        row["expression_id"].as_str().unwrap().to_owned(),
        snippet.to_owned(),
    )
}

fn transaction(
    revision: &Arc<ProjectRevision>,
    target: &str,
    snippet: &str,
    replacement: Value,
) -> SemanticTransactionV2 {
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(revision, target, snippet);
    SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(target, expression_id, old, replacement),
    )
    .unwrap()
}

fn swap_multiply() -> Value {
    json!({
        "kind": "binary", "op": "*",
        "left": {"kind": "place", "name": "right"},
        "right": {"kind": "place", "name": "left"},
    })
}

fn literal(value: i64) -> Value {
    json!({"kind": "i64", "value": value})
}

#[test]
fn three_file_feature_workflow_produces_one_deterministic_reviewable_candidate() {
    let fixture = Fixture::new();
    let disk_before = inventory(&fixture.0);
    let base = fixture.revision();

    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap_multiply(),
    );
    let after0 = step0.validate(Arc::clone(&base)).unwrap();
    let revision1 = Arc::clone(after0.candidate().revision());

    let step1 = transaction(
        &revision1,
        "calculator.app.main",
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
        literal(100),
    );
    let after1 = step1.validate(Arc::clone(&revision1)).unwrap();
    let revision2 = Arc::clone(after1.candidate().revision());

    let step2 = transaction(
        &revision2,
        "calculator.tests.main",
        "if add(19, 23) == 42 && subtract(23, 19) == 4 && multiply(6, 7) == 42 && divide(84, 2) == 42 && is_negative(-1) && not(false) { 0 } else { 1 }",
        literal(0),
    );

    let transactions = [step0.clone(), step1.clone(), step2.clone()];
    let workflow = SemanticTransactionV2Workflow::derive(Arc::clone(&base), &transactions).unwrap();

    assert_eq!(
        serde_json::from_str::<Value>(workflow.to_json()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_WORKFLOW_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(workflow.to_json()).unwrap()["step_count"],
        json!(3)
    );

    let candidate = workflow.candidate();
    let source = |path: &str| {
        candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap()
            .source()
            .to_owned()
    };
    assert!(source("src/core.spx").contains("right * left"));
    assert!(source("src/app.spx").contains("100"));
    assert!(!source("src/app.spx").contains("add(multiply(6, 7)"));
    assert!(source("src/tests.spx").contains("fn main() -> i64\n{\n    0\n}"));

    // All three touched files are captured with their own precise steps.
    let value: Value = serde_json::from_str(workflow.to_json()).unwrap();
    let steps = value["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0]["source_path"], "src/core.spx");
    assert_eq!(steps[1]["source_path"], "src/app.spx");
    assert_eq!(steps[2]["source_path"], "src/tests.spx");
    assert_eq!(steps[0]["target"], "calculator.multiply");
    assert_eq!(steps[1]["target"], "calculator.app.main");
    assert_eq!(steps[2]["target"], "calculator.tests.main");

    // Determinism: rederiving from the same inputs is byte-identical.
    let repeated = SemanticTransactionV2Workflow::derive(Arc::clone(&base), &transactions).unwrap();
    assert_eq!(workflow.to_json(), repeated.to_json());
    assert_eq!(workflow.digest(), repeated.digest());

    // Exact canonical replay round-trips.
    let transaction_bytes = transactions
        .iter()
        .map(|transaction| transaction.to_json().as_bytes().to_vec())
        .collect::<Vec<_>>();
    let replayed = SemanticTransactionV2Workflow::replay(
        Arc::clone(&base),
        &transaction_bytes,
        workflow.digest(),
        workflow.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), workflow.to_json());

    // Composition is authority-free and never touched the filesystem.
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn invalid_middle_operation_cannot_publish_a_prefix_or_execute_a_draft() {
    let fixture = Fixture::new();
    let disk_before = inventory(&fixture.0);
    let base = fixture.revision();

    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap_multiply(),
    );
    let after0 = step0.validate(Arc::clone(&base)).unwrap();
    let revision1 = Arc::clone(after0.candidate().revision());

    // Step 1 is well-formed except its expected-old-expression precondition
    // no longer holds: it names real workspace/expression identities but the
    // wrong old text, so the reused v2 core must fail it closed.
    let workspace1 = revision1.canonical_workspace_revision().unwrap();
    let (expression_id, _old) = selection(
        &revision1,
        "calculator.app.main",
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
    );
    let invalid_step1 = SemanticTransactionV2::replace_expression(
        workspace1.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.app.main",
            expression_id,
            "this is not the actual old source",
            literal(100),
        ),
    )
    .unwrap();

    // Step 2 would succeed in isolation, but must never execute: it is a
    // draft sitting after a rejected middle step.
    let step2_source = transaction(
        &revision1,
        "calculator.tests.main",
        "if add(19, 23) == 42 && subtract(23, 19) == 4 && multiply(6, 7) == 42 && divide(84, 2) == 42 && is_negative(-1) && not(false) { 0 } else { 1 }",
        literal(0),
    );

    let transactions = [step0, invalid_step1, step2_source];
    assert_code(
        SemanticTransactionV2Workflow::derive(Arc::clone(&base), &transactions),
        "SPX-G527",
    );
    match SemanticTransactionV2Workflow::derive(Arc::clone(&base), &transactions) {
        Ok(_) => panic!("expected the invalid middle step to fail the whole workflow"),
        Err(diagnostics) => assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("workflow step 1")
                    && diagnostic.message.contains("calculator.app.main")),
            "{diagnostics:?}"
        ),
    }

    // The workflow published no prefix: every original source, including
    // `calculator.multiply`, which step 0 would have changed on its own, is
    // byte-identical to what was on disk before the workflow ran.
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn stale_expression_id_after_an_earlier_step_is_rejected_not_misapplied() {
    let fixture = Fixture::new();
    let base = fixture.revision();

    // Captured against the shared original base, before any step ran.
    let (stale_expression_id, stale_old) = selection(&base, "calculator.multiply", "left * right");
    let base_workspace = base.canonical_workspace_revision().unwrap();

    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap_multiply(),
    );
    let after0 = step0.validate(Arc::clone(&base)).unwrap();
    let revision1 = Arc::clone(after0.candidate().revision());
    let workspace1 = revision1.canonical_workspace_revision().unwrap();

    // Step 1 reuses the pre-step-0 identity and old-source text for the exact
    // same function step 0 already rewrote. This must be rejected as stale
    // by the reused v2 core rather than silently reselected onto whatever
    // expression that identity happens to name in the new revision.
    let stale_step1 = SemanticTransactionV2::replace_expression(
        workspace1.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.multiply",
            &stale_expression_id,
            &stale_old,
            literal(0),
        ),
    )
    .unwrap();

    // Sanity: the stale transaction is well-formed against the *original*
    // base workspace revision (it is a legitimate, valid v2 transaction on
    // its own); it is stale only once step 0 has already moved the revision.
    assert_ne!(
        base_workspace.workspace_revision(),
        workspace1.workspace_revision()
    );

    let transactions = [step0, stale_step1];
    assert_code(
        SemanticTransactionV2Workflow::derive(Arc::clone(&base), &transactions),
        "SPX-G527",
    );
}

#[test]
fn empty_and_oversized_step_sequences_are_rejected() {
    let fixture = Fixture::new();
    let base = fixture.revision();

    assert_code(
        SemanticTransactionV2Workflow::derive(Arc::clone(&base), &[]),
        "SPX-G600",
    );

    let step = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap_multiply(),
    );
    let too_many = vec![step; MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS + 1];
    assert_code(
        SemanticTransactionV2Workflow::derive(Arc::clone(&base), &too_many),
        "SPX-G601",
    );
}

#[test]
fn replay_rejects_tampering() {
    let fixture = Fixture::new();
    let base = fixture.revision();

    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap_multiply(),
    );
    let after0 = step0.validate(Arc::clone(&base)).unwrap();
    let revision1 = Arc::clone(after0.candidate().revision());
    let step1 = transaction(
        &revision1,
        "calculator.app.main",
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
        literal(100),
    );
    let transactions = [step0, step1];
    let workflow = SemanticTransactionV2Workflow::derive(Arc::clone(&base), &transactions).unwrap();
    let transaction_bytes = transactions
        .iter()
        .map(|transaction| transaction.to_json().as_bytes().to_vec())
        .collect::<Vec<_>>();

    // A well-formed digest string that does not match the submitted bytes.
    let mut tampered_digest = workflow.digest().to_owned();
    let last = tampered_digest.pop().unwrap();
    tampered_digest.push(if last == '0' { '1' } else { '0' });
    assert_code(
        SemanticTransactionV2Workflow::replay(
            Arc::clone(&base),
            &transaction_bytes,
            &tampered_digest,
            workflow.to_json().as_bytes(),
        ),
        "SPX-G602",
    );

    // Noncanonical submitted bytes (trailing byte outside the rendered form)
    // are refused before the digest is even compared.
    let mut tampered_bytes = workflow.to_json().as_bytes().to_vec();
    tampered_bytes.push(b' ');
    assert_code(
        SemanticTransactionV2Workflow::replay(
            Arc::clone(&base),
            &transaction_bytes,
            workflow.digest(),
            &tampered_bytes,
        ),
        "SPX-G600",
    );

    // A different, independently derived workflow's canonical bytes fail
    // exact replay against these steps and this base: content and digest
    // must correspond, not merely each be independently well-formed.
    let other = SemanticTransactionV2Workflow::derive(
        Arc::clone(&base),
        std::slice::from_ref(&transactions[0]),
    )
    .unwrap();
    assert_code(
        SemanticTransactionV2Workflow::replay(
            Arc::clone(&base),
            &transaction_bytes,
            other.digest(),
            workflow.to_json().as_bytes(),
        ),
        "SPX-G602",
    );
}
