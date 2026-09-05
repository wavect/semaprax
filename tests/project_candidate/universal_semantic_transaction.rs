use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, ProjectSemanticImage,
    SemanticChange, SemanticTransaction, SemanticTransactionRenameDisplayName,
    MAX_SEMANTIC_TRANSACTION_BYTES, SEMANTIC_TRANSACTION_EVIDENCE_SCHEMA,
    SEMANTIC_TRANSACTION_IMPACT_SCHEMA, SEMANTIC_TRANSACTION_RESULT_SCHEMA,
    SEMANTIC_TRANSACTION_REVIEW_SCHEMA, SEMANTIC_TRANSACTION_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-universal-semantic-transaction-v1-{}-{}",
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

fn transaction(revision: &ProjectRevision) -> SemanticTransaction {
    let workspace = revision.canonical_workspace_revision().unwrap();
    SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap()
}

#[test]
fn rename_display_name_is_deterministic_authority_free_and_candidate_equivalent() {
    let fixture = Fixture::new();
    let disk_before = inventory(&fixture.0);
    let revision = fixture.revision();
    let project_before = revision.project_revision().to_owned();
    let workspace_before = revision.workspace_revision().to_owned();
    let graph_before = revision.semantic_graph().to_owned();
    let image_before =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision())
            .unwrap()
            .to_json()
            .to_owned();
    let source_before = revision
        .sources()
        .iter()
        .map(|source| source.source().to_owned())
        .collect::<Vec<_>>();
    let transaction = transaction(&revision);
    let parsed = SemanticTransaction::from_json(transaction.to_json().as_bytes()).unwrap();
    assert_eq!(parsed.to_json(), transaction.to_json());
    assert_eq!(
        serde_json::from_str::<Value>(transaction.to_json()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_SCHEMA
    );

    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let repeated = transaction.validate(Arc::clone(&revision)).unwrap();
    assert_eq!(artifacts.impact(), repeated.impact());
    assert_eq!(artifacts.review(), repeated.review());
    assert_eq!(artifacts.result(), repeated.result());
    assert_eq!(artifacts.evidence(), repeated.evidence());
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.impact()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_IMPACT_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.review()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_REVIEW_SCHEMA
    );
    let result: Value = serde_json::from_str(artifacts.result()).unwrap();
    assert_eq!(result["schema"], SEMANTIC_TRANSACTION_RESULT_SCHEMA);
    assert_eq!(
        result["authority"],
        json!({"commit_performed":false,"granted":false})
    );
    assert_eq!(result["source_review"]["source_authority"], false);
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.evidence()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_EVIDENCE_SCHEMA
    );

    let direct_open =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let direct_change = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind":"rename_declaration", "target":"calculator.add", "name":"sum"
        }),
    )
    .unwrap();
    let direct = direct_open
        .apply(direct_open.candidate_digest(), &direct_change)
        .unwrap();
    assert_eq!(artifacts.candidate().to_json(), direct.to_json());
    assert_eq!(
        artifacts.candidate().candidate_digest(),
        direct.candidate_digest()
    );
    let replay = SemanticTransaction::replay(
        Arc::clone(&revision),
        transaction.to_json().as_bytes(),
        artifacts.evidence().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.result(), artifacts.result());

    assert_eq!(revision.project_revision(), project_before);
    assert_eq!(revision.workspace_revision(), workspace_before);
    assert_eq!(revision.semantic_graph(), graph_before);
    assert_eq!(
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision())
            .unwrap()
            .to_json(),
        image_before
    );
    assert_eq!(
        revision
            .sources()
            .iter()
            .map(|source| source.source().to_owned())
            .collect::<Vec<_>>(),
        source_before
    );
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn stale_old_value_base_and_reminted_evidence_fail_closed() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let stale_base = SemanticTransaction::rename_display_name(
        &format!("sha256:{}", "0".repeat(64)),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    assert_code(stale_base.validate(Arc::clone(&revision)), "SPX-G527");
    let stale_old = SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "plus", "sum"),
    )
    .unwrap();
    assert_code(stale_old.validate(Arc::clone(&revision)), "SPX-G527");
    let main = SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.app.main", "main", "start"),
    )
    .unwrap();
    assert_code(main.validate(Arc::clone(&revision)), "SPX-G525");

    let transaction = transaction(&revision);
    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let mut evidence: Value = serde_json::from_str(artifacts.evidence()).unwrap();
    evidence["artifacts"]["result"]["value"]["authority"]["granted"] = json!(true);
    let mut reminted = serde_json::to_string(&evidence).unwrap();
    reminted.push('\n');
    assert_code(
        SemanticTransaction::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            reminted.as_bytes(),
        ),
        "SPX-G527",
    );

    let noncanonical = transaction.to_json().trim_end().as_bytes();
    assert_code(SemanticTransaction::from_json(noncanonical), "SPX-G525");
    let mut elevated: Value = serde_json::from_str(transaction.to_json()).unwrap();
    elevated["requested_authority"] = json!("publish");
    let mut elevated = serde_json::to_string(&elevated).unwrap();
    elevated.push('\n');
    assert_code(
        SemanticTransaction::from_json(elevated.as_bytes()),
        "SPX-G525",
    );
    assert_code(
        SemanticTransaction::from_json(&vec![b' '; MAX_SEMANTIC_TRANSACTION_BYTES + 1]),
        "SPX-G526",
    );
}

#[test]
fn comment_bearing_source_is_outside_the_bounded_v1_rewrite_domain() {
    let fixture = Fixture::new();
    let core = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&core).unwrap();
    std::fs::write(&core, format!("// retained human note\n{source}")).unwrap();
    let revision = fixture.revision();
    let transaction = transaction(&revision);
    assert_code(transaction.validate(revision), "SPX-G525");
}
