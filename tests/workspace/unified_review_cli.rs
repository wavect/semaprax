use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    with_authenticated_project, ProjectRevision, SemanticTransaction,
    SemanticTransactionRenameDisplayName, SemanticWorkspaceService,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-unified-review-cli-v1-{}-{}",
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

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
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
    fn visit(root: &Path, path: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
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

fn transaction(fixture: &Fixture, new_name: &str) -> (SemanticTransaction, PathBuf) {
    let revision = fixture.revision();
    let workspace_revision = revision
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();
    let transaction = SemanticTransaction::rename_display_name(
        &workspace_revision,
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", new_name),
    )
    .unwrap();
    let path = fixture.0.join("transaction.json");
    std::fs::write(&path, transaction.to_json()).unwrap();
    (transaction, path)
}

#[test]
fn project_review_prints_exact_review_or_evidence_and_writes_nothing() {
    let fixture = Fixture::new();
    let (transaction, path) = transaction(&fixture, "sum");
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let artifacts = service
        .validate_transaction(transaction.to_json().as_bytes())
        .unwrap();

    for (extra, expected) in [
        (&[][..], artifacts.review()),
        (&["--evidence"][..], artifacts.evidence()),
    ] {
        let mut arguments = vec![
            "review",
            fixture.0.to_str().unwrap(),
            path.to_str().unwrap(),
        ];
        arguments.extend_from_slice(extra);
        let output = fixture.invoke(&arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn project_review_rejects_stale_noncanonical_and_open_grammar_without_writes() {
    let fixture = Fixture::new();
    let (_, path) = transaction(&fixture, "sum");

    let mut noncanonical = std::fs::read_to_string(&path).unwrap();
    noncanonical.pop();
    std::fs::write(&path, noncanonical).unwrap();
    let noncanonical_before = inventory(&fixture.0);
    let output = fixture.invoke(&[
        "review",
        fixture.0.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G525"));
    assert_eq!(inventory(&fixture.0), noncanonical_before);

    let stale = SemanticTransaction::rename_display_name(
        &format!("sha256:{}", "0".repeat(64)),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    std::fs::write(&path, stale.to_json()).unwrap();
    let stale_before = inventory(&fixture.0);
    let output = fixture.invoke(&[
        "review",
        fixture.0.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G530"));
    assert_eq!(inventory(&fixture.0), stale_before);

    for arguments in [
        vec!["review", fixture.0.to_str().unwrap()],
        vec![
            "review",
            fixture.0.to_str().unwrap(),
            path.to_str().unwrap(),
            "--unknown",
        ],
        vec![
            "review",
            fixture.0.to_str().unwrap(),
            path.to_str().unwrap(),
            "--evidence",
            "extra",
        ],
        vec![
            "review",
            fixture.0.to_str().unwrap(),
            path.to_str().unwrap(),
            "--evidence",
            "--evidence",
        ],
    ] {
        let before = inventory(&fixture.0);
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(inventory(&fixture.0), before);
    }
}

#[test]
fn scoped_help_advertises_both_closed_review_forms() {
    let fixture = Fixture::new();
    let output = fixture.invoke(&["review", "--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        b"Usage:\n  semaprax review <file> <patch.spatch>\n  semaprax review <project> <transaction.json> [--evidence]\n"
    );
}
