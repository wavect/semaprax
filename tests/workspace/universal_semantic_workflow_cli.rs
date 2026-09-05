use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    with_authenticated_project, ProjectRevision, SemanticQuery, SemanticTransaction,
    SemanticTransactionRenameDisplayName, SemanticWorkspaceService,
};
use semaprax::query::QueryFilters;
use semaprax::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
    WorkspaceImpactOptions,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(comment: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-universal-semantic-workflow-cli-v1-{}-{}",
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
        if comment {
            let path = root.join("src/core.spx");
            let source = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, format!("// retained human note\n{source}")).unwrap();
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

    fn invoke(&self, arguments: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
        command.current_dir(&self.0);
        command.args(arguments);
        command.output().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = std::fs::read_dir(current)
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
fn all_five_query_modes_equal_the_exact_direct_core_results_and_write_nothing() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let workspace_revision = service.active_generation().workspace_revision().to_owned();
    let snapshot = service.snapshot(&workspace_revision).unwrap();
    let filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        ..QueryFilters::default()
    };
    let context =
        WorkspaceContextOptions::new(WorkspaceAnalysisDirection::Forward, 1, 4096, 32).unwrap();
    let impact = WorkspaceImpactOptions::new(1, 4096, 32).unwrap();
    let cases = vec![
        (
            vec![
                "query",
                fixture.0.to_str().unwrap(),
                "declarations",
                "--kind",
                "function",
                "--offset",
                "1",
                "--limit",
                "2",
                "--revision",
                &workspace_revision,
            ],
            SemanticQuery::declarations(&workspace_revision, &filters, 1, 2).unwrap(),
        ),
        (
            vec![
                "query",
                fixture.0.to_str().unwrap(),
                "symbol",
                "calculator.add",
                "--revision",
                &workspace_revision,
            ],
            SemanticQuery::symbol(&workspace_revision, "calculator.add").unwrap(),
        ),
        (
            vec![
                "query",
                fixture.0.to_str().unwrap(),
                "context",
                "declaration",
                "calculator.add",
                "--direction",
                "forward",
                "--depth",
                "1",
                "--max-bytes",
                "4096",
                "--max-nodes",
                "32",
                "--revision",
                &workspace_revision,
            ],
            SemanticQuery::context(
                &workspace_revision,
                WorkspaceAnalysisTargetKind::Declaration,
                "calculator.add",
                context,
            )
            .unwrap(),
        ),
        (
            vec![
                "query",
                fixture.0.to_str().unwrap(),
                "impact",
                "declaration",
                "calculator.add",
                "--depth",
                "1",
                "--max-bytes",
                "4096",
                "--max-nodes",
                "32",
                "--revision",
                &workspace_revision,
            ],
            SemanticQuery::impact(
                &workspace_revision,
                WorkspaceAnalysisTargetKind::Declaration,
                "calculator.add",
                impact,
            )
            .unwrap(),
        ),
        (
            vec![
                "query",
                fixture.0.to_str().unwrap(),
                "available-operations",
                "calculator.add",
                "--revision",
                &workspace_revision,
            ],
            SemanticQuery::available_operations(&workspace_revision, "calculator.add").unwrap(),
        ),
    ];

    for (arguments, query) in cases {
        let expected = snapshot.query(&query).unwrap();
        let output = fixture.invoke(&arguments);
        assert_success(&output);
        assert_eq!(
            output.stdout,
            expected.to_json().as_bytes(),
            "{arguments:?}"
        );
    }
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn declaration_paging_and_legacy_query_remain_exact() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let workspace_revision = service.active_generation().workspace_revision();
    let filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        ..QueryFilters::default()
    };
    for (offset, limit) in [(0, 2), (2, 128)] {
        let query =
            SemanticQuery::declarations(workspace_revision, &filters, offset, limit).unwrap();
        let expected = service.query(query.to_json().as_bytes()).unwrap();
        let offset = offset.to_string();
        let limit = limit.to_string();
        let output = fixture.invoke(&[
            "query",
            fixture.0.to_str().unwrap(),
            "declarations",
            "--kind",
            "function",
            "--offset",
            &offset,
            "--limit",
            &limit,
        ]);
        assert_success(&output);
        assert_eq!(output.stdout, expected.to_json().as_bytes());
    }

    let legacy_filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        name: Some("add".to_owned()),
        ..QueryFilters::default()
    };
    let legacy = semaprax::query::run_project(&revision, &legacy_filters).unwrap();
    let expected = semaprax::query::project_json(&legacy);
    let output = fixture.invoke(&[
        "query",
        fixture.manifest().to_str().unwrap(),
        "--kind",
        "function",
        "--name",
        "add",
        "--json",
    ]);
    assert_success(&output);
    assert_eq!(output.stdout, expected.as_bytes());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn explicit_stale_revision_and_malformed_query_grammar_fail_closed() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    let stale = format!("sha256:{}", "0".repeat(64));
    let output = fixture.invoke(&[
        "query",
        fixture.0.to_str().unwrap(),
        "symbol",
        "calculator.add",
        "--revision",
        &stale,
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G533"));

    for arguments in [
        vec!["query", fixture.0.to_str().unwrap(), "symbol"],
        vec![
            "query",
            fixture.0.to_str().unwrap(),
            "context",
            "unknown",
            "calculator.add",
        ],
        vec![
            "query",
            fixture.0.to_str().unwrap(),
            "impact",
            "declaration",
            "calculator.add",
            "--direction",
            "both",
        ],
        vec![
            "query",
            fixture.0.to_str().unwrap(),
            "declarations",
            "--offset",
            "01",
        ],
    ] {
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn change_preview_result_and_evidence_equal_direct_transaction_and_reject_bad_targets() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace_revision = revision
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();
    let transaction = SemanticTransaction::rename_display_name(
        &workspace_revision,
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let common = [
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "rename-display-name",
        "calculator.add",
        "sum",
        "--revision",
        &workspace_revision,
    ];
    let output = fixture.invoke(&common);
    assert_success(&output);
    assert_eq!(output.stdout, artifacts.result().as_bytes());
    let mut evidence = common.to_vec();
    evidence.push("--evidence");
    let output = fixture.invoke(&evidence);
    assert_success(&output);
    assert_eq!(output.stdout, artifacts.evidence().as_bytes());

    let output = fixture.invoke(&[
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "rename-display-name",
        "missing.identity",
        "sum",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert_eq!(inventory(&fixture.0), before);

    let commented = Fixture::new(true);
    let commented_before = inventory(&commented.0);
    let output = commented.invoke(&[
        "change",
        "preview",
        commented.0.to_str().unwrap(),
        "rename-display-name",
        "calculator.add",
        "sum",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G525"));
    assert_eq!(inventory(&commented.0), commented_before);
}

#[test]
fn malformed_change_grammar_is_status_two_and_inert() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    for arguments in [
        vec!["change"],
        vec!["change", "preview", fixture.0.to_str().unwrap()],
        vec![
            "change",
            "preview",
            fixture.0.to_str().unwrap(),
            "rename-display-name",
            "calculator.add",
        ],
        vec![
            "change",
            "preview",
            fixture.0.to_str().unwrap(),
            "rename-display-name",
            "calculator.add",
            "sum",
            "--unknown",
        ],
    ] {
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }
    assert_eq!(inventory(&fixture.0), before);
}
