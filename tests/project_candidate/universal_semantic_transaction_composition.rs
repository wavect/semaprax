use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectRevision, SemanticTransaction, SemanticTransactionMerge,
    SemanticTransactionMergeOrder, SemanticTransactionRebase, SemanticTransactionRenameDisplayName,
    SemanticWorkspaceStructuralDiff, SEMANTIC_TRANSACTION_MERGE_SCHEMA,
    SEMANTIC_TRANSACTION_REBASE_SCHEMA, SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-universal-transaction-composition-v1-{}-{}",
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

    fn add_unrelated_function(&self) {
        let path = self.0.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap()
            + "\n@id(\"calculator.unrelated\") fn unrelated(value: i64) -> i64 { value }\n";
        let canonical = semaprax::format::canonical(&semaprax::parse(&source, &path).unwrap());
        std::fs::write(path, canonical).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if entry.file_type().unwrap().is_dir() {
                output.insert(relative, Vec::new());
                visit(root, &path, output);
            } else {
                output.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn rename(revision: &ProjectRevision, target: &str, old: &str, new: &str) -> SemanticTransaction {
    let workspace = revision.canonical_workspace_revision().unwrap();
    SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new(target, old, new),
    )
    .unwrap()
}

fn source(revision: &ProjectRevision, path: &str) -> String {
    revision
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
        .to_owned()
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("expected composition rejection");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected),
        "{diagnostics:?}"
    );
}

fn assert_canonical_document(text: &str, schema: &str) -> Value {
    assert!(text.ends_with('\n'));
    let value: Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["schema"], schema);
    let mut sorted = value.clone();
    sorted.sort_all_objects();
    assert_eq!(
        format!("{}\n", serde_json::to_string(&sorted).unwrap()),
        text
    );
    assert_eq!(value["authority"], false);
    value
}

#[test]
fn structural_diff_is_deterministic_exact_and_freshly_replayable() {
    let fixture = Fixture::new();
    let disk = inventory(&fixture.0);
    let base = fixture.revision();
    let transaction = rename(&base, "calculator.add", "add", "sum");
    let artifacts = transaction.validate(Arc::clone(&base)).unwrap();
    let candidate = artifacts.candidate();

    let first =
        SemanticWorkspaceStructuralDiff::derive(candidate, candidate.candidate_digest()).unwrap();
    let second =
        SemanticWorkspaceStructuralDiff::derive(candidate, candidate.candidate_digest()).unwrap();
    assert_eq!(first.base_program_root(), artifacts.base_program_root());
    assert_eq!(
        first.candidate_program_root(),
        artifacts.candidate_program_root()
    );
    assert_eq!(
        first
            .base_program_root()
            .segments()
            .iter()
            .map(|segment| (segment.kind(), segment.node_digest()))
            .collect::<Vec<_>>(),
        artifacts
            .base_program_root()
            .segments()
            .iter()
            .map(|segment| (segment.kind(), segment.node_digest()))
            .collect::<Vec<_>>()
    );
    assert_eq!(first.to_json(), second.to_json());
    assert_eq!(first.digest(), second.digest());
    assert_eq!(
        first.source_review(),
        candidate
            .source_review(candidate.candidate_digest())
            .unwrap()
    );
    let value =
        assert_canonical_document(first.to_json(), SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_SCHEMA);
    assert_eq!(value["candidate_digest"], candidate.candidate_digest());
    for segment in first.base_program_root().segments() {
        assert_eq!(
            value["base"]["nodes"][segment.kind()],
            segment.node_digest()
        );
    }
    for segment in first.candidate_program_root().segments() {
        assert_eq!(
            value["candidate"]["nodes"][segment.kind()],
            segment.node_digest()
        );
    }
    assert_eq!(
        value["base"]["workspace_revision"],
        base.canonical_workspace_revision()
            .unwrap()
            .workspace_revision()
    );
    assert_eq!(
        value["candidate"]["workspace_revision"],
        candidate
            .revision()
            .canonical_workspace_revision()
            .unwrap()
            .workspace_revision()
    );
    assert!(value["changed_components"]
        .as_array()
        .unwrap()
        .contains(&json!("semantic")));
    assert!(value["changed_components"]
        .as_array()
        .unwrap()
        .contains(&json!("source_projection")));
    assert_eq!(value["source_review"]["value"]["source_authority"], false);

    let replay = SemanticWorkspaceStructuralDiff::replay(
        candidate,
        candidate.candidate_digest(),
        first.digest(),
        first.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), first.to_json());
    assert_eq!(inventory(&fixture.0), disk);
}

#[test]
fn unrelated_drift_rebase_matches_candidate_rebase_and_exact_replay() {
    let fixture = Fixture::new();
    let disk = inventory(&fixture.0);
    let base = fixture.revision();
    let original = rename(&base, "calculator.add", "add", "sum");
    let original_artifacts = original.validate(Arc::clone(&base)).unwrap();
    let original_bytes = (
        original.to_json().to_owned(),
        original_artifacts.evidence().to_owned(),
        original_artifacts.candidate().to_json().to_owned(),
    );
    let drift = rename(&base, "calculator.subtract", "subtract", "difference");
    let drift_artifacts = drift.validate(Arc::clone(&base)).unwrap();
    let onto = Arc::clone(drift_artifacts.candidate().revision());
    let onto_workspace = onto.canonical_workspace_revision().unwrap();

    let rebased = original
        .rebase(
            Arc::clone(&base),
            Arc::clone(&onto),
            onto_workspace.workspace_revision(),
        )
        .unwrap();
    let repeated = SemanticTransactionRebase::derive(
        &original,
        Arc::clone(&base),
        Arc::clone(&onto),
        onto_workspace.workspace_revision(),
    )
    .unwrap();
    assert_eq!(rebased.to_json(), repeated.to_json());
    assert_eq!(rebased.digest(), repeated.digest());
    let value = assert_canonical_document(rebased.to_json(), SEMANTIC_TRANSACTION_REBASE_SCHEMA);
    assert_eq!(value["validation"]["fresh_transaction_validation"], true);
    assert_eq!(
        rebased.transaction().expected_workspace_revision(),
        onto_workspace.workspace_revision()
    );
    let merged_source = source(rebased.artifacts().candidate().revision(), "src/core.spx");
    assert!(merged_source.contains("fn sum("));
    assert!(merged_source.contains("fn difference("));

    let direct = original_artifacts
        .candidate()
        .rebase(
            original_artifacts.candidate().candidate_digest(),
            Arc::clone(&onto),
            onto.project_revision(),
        )
        .unwrap();
    assert_eq!(rebased.reconciliation(), direct.to_json());
    assert_eq!(
        rebased.artifacts().candidate().to_json(),
        direct.candidate().to_json()
    );
    let replay = SemanticTransactionRebase::replay(
        Arc::clone(&base),
        Arc::clone(&onto),
        original.to_json().as_bytes(),
        onto_workspace.workspace_revision(),
        rebased.digest(),
        rebased.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), rebased.to_json());
    assert_eq!(original.to_json(), original_bytes.0);
    assert_eq!(original_artifacts.evidence(), original_bytes.1);
    assert_eq!(original_artifacts.candidate().to_json(), original_bytes.2);
    assert_eq!(inventory(&fixture.0), disk);
}

#[test]
fn explicit_both_order_disjoint_merge_matches_direct_candidate_merge() {
    let fixture = Fixture::new();
    let disk = inventory(&fixture.0);
    let base = fixture.revision();
    let left = rename(&base, "calculator.add", "add", "sum");
    let right = rename(&base, "calculator.subtract", "subtract", "difference");
    let left_artifacts = left.validate(Arc::clone(&base)).unwrap();
    let right_artifacts = right.validate(Arc::clone(&base)).unwrap();

    let left_then_right = left
        .merge(
            &right,
            Arc::clone(&base),
            SemanticTransactionMergeOrder::LeftThenRight,
        )
        .unwrap();
    let right_then_left = SemanticTransactionMerge::derive(
        &left,
        &right,
        Arc::clone(&base),
        SemanticTransactionMergeOrder::RightThenLeft,
    )
    .unwrap();
    let left_value =
        assert_canonical_document(left_then_right.to_json(), SEMANTIC_TRANSACTION_MERGE_SCHEMA);
    let right_value =
        assert_canonical_document(right_then_left.to_json(), SEMANTIC_TRANSACTION_MERGE_SCHEMA);
    assert_eq!(left_value["order"], "left_then_right");
    assert_eq!(right_value["order"], "right_then_left");
    assert_ne!(left_then_right.digest(), right_then_left.digest());
    assert_ne!(
        left_then_right.candidate().candidate_digest(),
        right_then_left.candidate().candidate_digest()
    );
    assert_eq!(
        source(left_then_right.candidate().revision(), "src/core.spx"),
        source(right_then_left.candidate().revision(), "src/core.spx")
    );

    let direct_left_then_right = right_artifacts
        .candidate()
        .merge(
            right_artifacts.candidate().candidate_digest(),
            left_artifacts.candidate(),
            left_artifacts.candidate().candidate_digest(),
        )
        .unwrap();
    assert_eq!(
        left_then_right.reconciliation(),
        direct_left_then_right.to_json()
    );
    assert_eq!(
        left_then_right.candidate().to_json(),
        direct_left_then_right.candidate().to_json()
    );

    for (order, result) in [
        (
            SemanticTransactionMergeOrder::LeftThenRight,
            &left_then_right,
        ),
        (
            SemanticTransactionMergeOrder::RightThenLeft,
            &right_then_left,
        ),
    ] {
        let replay = SemanticTransactionMerge::replay(
            Arc::clone(&base),
            left.to_json().as_bytes(),
            right.to_json().as_bytes(),
            order,
            result.digest(),
            result.to_json().as_bytes(),
        )
        .unwrap();
        assert_eq!(replay.to_json(), result.to_json());
    }
    assert_eq!(inventory(&fixture.0), disk);
}

#[test]
fn competing_stale_tampered_and_cross_base_compositions_fail_closed() {
    let fixture = Fixture::new();
    let disk = inventory(&fixture.0);
    let base = fixture.revision();
    let left = rename(&base, "calculator.add", "add", "sum");
    let competing = rename(&base, "calculator.add", "add", "total");
    assert_code(
        left.merge(
            &competing,
            Arc::clone(&base),
            SemanticTransactionMergeOrder::LeftThenRight,
        ),
        "SPX-G539",
    );

    let right = rename(&base, "calculator.subtract", "subtract", "difference");
    let merged = left
        .merge(
            &right,
            Arc::clone(&base),
            SemanticTransactionMergeOrder::LeftThenRight,
        )
        .unwrap();
    let mut tampered: Value = serde_json::from_str(merged.to_json()).unwrap();
    tampered["authority"] = json!(true);
    tampered.sort_all_objects();
    let tampered = format!("{}\n", serde_json::to_string(&tampered).unwrap());
    assert_code(
        SemanticTransactionMerge::replay(
            Arc::clone(&base),
            left.to_json().as_bytes(),
            right.to_json().as_bytes(),
            SemanticTransactionMergeOrder::LeftThenRight,
            merged.digest(),
            tampered.as_bytes(),
        ),
        "SPX-G538",
    );
    assert_code(
        SemanticTransactionMerge::replay(
            Arc::clone(&base),
            left.to_json().as_bytes(),
            right.to_json().as_bytes(),
            SemanticTransactionMergeOrder::LeftThenRight,
            merged.digest(),
            merged.to_json().trim_end().as_bytes(),
        ),
        "SPX-G536",
    );

    fixture.add_unrelated_function();
    let other_base = fixture.revision();
    let other = rename(&other_base, "calculator.subtract", "subtract", "difference");
    assert_code(
        left.merge(
            &other,
            Arc::clone(&base),
            SemanticTransactionMergeOrder::LeftThenRight,
        ),
        "SPX-G538",
    );
    assert_code(
        left.rebase(
            Arc::clone(&base),
            Arc::clone(&other_base),
            &format!("sha256:{}", "0".repeat(64)),
        ),
        "SPX-G538",
    );
    let changed_disk = inventory(&fixture.0);
    assert_ne!(changed_disk, disk);
    assert_eq!(inventory(&fixture.0), changed_disk);
}

#[test]
fn composition_preserves_all_v1_transaction_and_workspace_bytes() {
    let fixture = Fixture::new();
    let disk = inventory(&fixture.0);
    let base = fixture.revision();
    let workspace = base.canonical_workspace_revision().unwrap();
    let left = rename(&base, "calculator.add", "add", "sum");
    let right = rename(&base, "calculator.subtract", "subtract", "difference");
    let left_artifacts = left.validate(Arc::clone(&base)).unwrap();
    let frozen = (
        workspace.to_json().to_owned(),
        left.to_json().to_owned(),
        left_artifacts.impact().to_owned(),
        left_artifacts.review().to_owned(),
        left_artifacts.result().to_owned(),
        left_artifacts.evidence().to_owned(),
        left_artifacts.candidate().to_json().to_owned(),
    );
    let merged = left
        .merge(
            &right,
            Arc::clone(&base),
            SemanticTransactionMergeOrder::LeftThenRight,
        )
        .unwrap();
    let diff = SemanticWorkspaceStructuralDiff::derive(
        merged.candidate(),
        merged.candidate().candidate_digest(),
    )
    .unwrap();
    assert!(!diff.to_json().is_empty());
    assert_eq!(
        base.canonical_workspace_revision().unwrap().to_json(),
        frozen.0
    );
    assert_eq!(left.to_json(), frozen.1);
    assert_eq!(left_artifacts.impact(), frozen.2);
    assert_eq!(left_artifacts.review(), frozen.3);
    assert_eq!(left_artifacts.result(), frozen.4);
    assert_eq!(left_artifacts.evidence(), frozen.5);
    assert_eq!(left_artifacts.candidate().to_json(), frozen.6);
    assert_eq!(inventory(&fixture.0), disk);
}
