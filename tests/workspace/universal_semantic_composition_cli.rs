use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    with_authenticated_project, ProjectRevision, SemanticTransaction,
    SemanticTransactionMergeOrder, SemanticTransactionRenameDisplayName,
    SemanticWorkspaceStructuralDiff,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-universal-semantic-composition-cli-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.manifest(), |snapshot| Ok(snapshot.retain_revision()))
            .unwrap()
    }

    fn rename_subtract_projection(&self) {
        for relative in ["src/core.spx", "src/app.spx", "src/tests.spx"] {
            let path = self.0.join(relative);
            let source = std::fs::read_to_string(&path).unwrap();
            let source = source
                .replace("fn subtract(", "fn difference(")
                .replace(" as subtract;", " as difference;")
                .replace("subtract(", "difference(");
            std::fs::write(path, source).unwrap();
        }
        self.revision();
    }

    fn invoke(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
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

fn rename(
    revision: &ProjectRevision,
    target: &str,
    old_name: &str,
    new_name: &str,
) -> SemanticTransaction {
    let workspace = revision.canonical_workspace_revision().unwrap();
    SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new(target, old_name, new_name),
    )
    .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
}

#[test]
fn preview_structural_diff_is_exact_core_output_and_legacy_artifacts_stay_exact() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let base = fixture.revision();
    let transaction = rename(&base, "calculator.add", "add", "sum");
    let artifacts = transaction.validate(Arc::clone(&base)).unwrap();
    let expected_diff = SemanticWorkspaceStructuralDiff::derive(
        artifacts.candidate(),
        artifacts.candidate().candidate_digest(),
    )
    .unwrap();
    let workspace_revision = transaction.expected_workspace_revision();
    let common = [
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "rename-display-name",
        "calculator.add",
        "sum",
        "--revision",
        workspace_revision,
    ];

    let mut diff = common.to_vec();
    diff.push("--structural-diff");
    let output = fixture.invoke(&diff);
    assert_success(&output);
    assert_eq!(output.stdout, expected_diff.to_json().as_bytes());

    let output = fixture.invoke(&common);
    assert_success(&output);
    assert_eq!(output.stdout, artifacts.result().as_bytes());
    let mut evidence = common.to_vec();
    evidence.push("--evidence");
    let output = fixture.invoke(&evidence);
    assert_success(&output);
    assert_eq!(output.stdout, artifacts.evidence().as_bytes());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn rebase_is_exact_core_output_and_rechecks_both_unchanged_roots() {
    let base_fixture = Fixture::new();
    let onto_fixture = Fixture::new();
    onto_fixture.rename_subtract_projection();
    let base_before = inventory(&base_fixture.0);
    let onto_before = inventory(&onto_fixture.0);
    let base = base_fixture.revision();
    let onto = onto_fixture.revision();
    let transaction = rename(&base, "calculator.add", "add", "sum");
    let onto_workspace = onto.canonical_workspace_revision().unwrap();
    let expected = transaction
        .rebase(
            Arc::clone(&base),
            Arc::clone(&onto),
            onto_workspace.workspace_revision(),
        )
        .unwrap();
    let output = base_fixture.invoke(&[
        "change",
        "rebase",
        base_fixture.0.to_str().unwrap(),
        "rename-display-name",
        "calculator.add",
        "sum",
        "--onto",
        onto_fixture.0.to_str().unwrap(),
        "--revision",
        transaction.expected_workspace_revision(),
        "--onto-revision",
        onto_workspace.workspace_revision(),
    ]);
    assert_success(&output);
    assert_eq!(output.stdout, expected.to_json().as_bytes());
    assert_eq!(inventory(&base_fixture.0), base_before);
    assert_eq!(inventory(&onto_fixture.0), onto_before);
}

#[test]
fn both_explicit_merge_orders_equal_the_exact_core_artifacts() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let base = fixture.revision();
    let left = rename(&base, "calculator.add", "add", "sum");
    let right = rename(&base, "calculator.subtract", "subtract", "difference");
    for (cli_order, core_order) in [
        (
            "left-then-right",
            SemanticTransactionMergeOrder::LeftThenRight,
        ),
        (
            "right-then-left",
            SemanticTransactionMergeOrder::RightThenLeft,
        ),
    ] {
        let expected = left.merge(&right, Arc::clone(&base), core_order).unwrap();
        let output = fixture.invoke(&[
            "change",
            "merge",
            fixture.0.to_str().unwrap(),
            "rename-display-name",
            "calculator.add",
            "sum",
            "--with",
            "rename-display-name",
            "calculator.subtract",
            "difference",
            "--revision",
            left.expected_workspace_revision(),
            "--order",
            cli_order,
        ]);
        assert_success(&output);
        assert_eq!(output.stdout, expected.to_json().as_bytes(), "{cli_order}");
    }
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn hostile_options_orders_and_stale_revisions_fail_without_writes() {
    let fixture = Fixture::new();
    let onto = Fixture::new();
    let fixture_before = inventory(&fixture.0);
    let onto_before = inventory(&onto.0);
    let stale = format!("sha256:{}", "0".repeat(64));
    let root = fixture.0.to_str().unwrap();
    let onto_root = onto.0.to_str().unwrap();
    let cases = [
        vec![
            "change",
            "preview",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--structural-diff",
            "--structural-diff",
        ],
        vec![
            "change",
            "preview",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--evidence",
            "--structural-diff",
        ],
        vec![
            "change",
            "rebase",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--onto",
            onto_root,
            "--unknown",
            "value",
        ],
        vec![
            "change",
            "merge",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--with",
            "rename-display-name",
            "calculator.subtract",
            "difference",
        ],
        vec![
            "change",
            "merge",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--with",
            "rename-display-name",
            "calculator.subtract",
            "difference",
            "--order",
            "automatic",
        ],
    ];
    for arguments in cases {
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }

    for arguments in [
        vec![
            "change",
            "preview",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--revision",
            &stale,
            "--structural-diff",
        ],
        vec![
            "change",
            "rebase",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--onto",
            onto_root,
            "--onto-revision",
            &stale,
        ],
        vec![
            "change",
            "merge",
            root,
            "rename-display-name",
            "calculator.add",
            "sum",
            "--with",
            "rename-display-name",
            "calculator.subtract",
            "difference",
            "--revision",
            &stale,
            "--order",
            "left-then-right",
        ],
    ] {
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }
    assert_eq!(inventory(&fixture.0), fixture_before);
    assert_eq!(inventory(&onto.0), onto_before);
}
