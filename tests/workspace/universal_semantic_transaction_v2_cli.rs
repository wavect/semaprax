use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    self, render_project_lock, with_authenticated_project, ExactProgramContext,
    ExactProgramContextV2, ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2,
    ProjectCandidate, ProjectRevision, SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SemanticWorkspaceRevision,
    SemanticWorkspaceService, SemanticWorkspaceServiceHistoryQuery, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-universal-semantic-transaction-v2-cli-{}-{}",
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

    fn exact_context(&self) -> Arc<ExactProgramContext> {
        with_authenticated_project(&self.manifest(), |snapshot| {
            let revision = snapshot.retain_revision();
            let default_workspace = snapshot.canonical_workspace_revision()?;
            let base_root = default_workspace.program_root()?;
            let lock = render_project_lock(snapshot)?;
            let association = base_root.associate_dependency_lock(
                snapshot,
                base_root.program_root_digest(),
                &lock,
            )?;
            let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
                &revision,
                revision.project_revision(),
                &[&super::program_root_v2::definition()],
            )?;
            let interface = InterfaceArtifactFacts::derive(
                Arc::clone(&revision),
                revision.project_revision(),
                &[ImageArtifactKind::Web],
                MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            )?;
            let root_v2 = ProgramRootV2::derive(&workspace, &base_root, &interface, &association)?;
            let root_v2_digest = root_v2.program_root_v2_digest().to_owned();
            ExactProgramContext::derive(
                Arc::clone(&revision),
                revision.project_revision(),
                workspace.clone(),
                workspace.workspace_revision(),
                interface,
                association,
                root_v2,
                &root_v2_digest,
            )
            .map(Arc::new)
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

fn selection(revision: &Arc<ProjectRevision>, target: &str, snippet: &str) -> (String, String) {
    let candidate =
        ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap();
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let source_path = catalog["source"]["path"].as_str().unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .unwrap()
        .source();
    let entry = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let start = entry["source_span"]["start"].as_u64().unwrap() as usize;
            let end = entry["source_span"]["end"].as_u64().unwrap() as usize;
            source.get(start..end) == Some(snippet)
        })
        .unwrap();
    (
        entry["expression_id"].as_str().unwrap().to_owned(),
        snippet.to_owned(),
    )
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn assert_code<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn replace_expression_preview_matches_core_and_service_exactly_without_writes() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace_revision = revision
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();
    let (expression_id, old_expression) = selection(&revision, "calculator.add", "left + right");
    let replacement = json!({
        "kind": "binary",
        "left": {"kind": "place", "name": "left"},
        "op": "-",
        "right": {"kind": "place", "name": "right"}
    });
    let transaction = SemanticTransactionV2::replace_expression(
        &workspace_revision,
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            &expression_id,
            &old_expression,
            replacement.clone(),
        ),
    )
    .unwrap();
    let direct = project::validate_semantic_transaction_v2(&revision, &transaction).unwrap();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let retained = service
        .validate_transaction_v2(transaction.to_json().as_bytes())
        .unwrap();
    assert_eq!(
        service.history_snapshot(&workspace_revision).unwrap().len(),
        1
    );
    assert_eq!(retained.result(), direct.result());
    assert_eq!(retained.evidence(), direct.evidence());
    assert_eq!(
        service
            .replay_transaction_v2(
                transaction.to_json().as_bytes(),
                direct.evidence().as_bytes(),
            )
            .unwrap()
            .result(),
        direct.result()
    );
    assert_eq!(
        service.history_snapshot(&workspace_revision).unwrap().len(),
        1
    );

    let replacement = serde_json::to_string(&replacement).unwrap();
    let common = [
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "replace-expression",
        "calculator.add",
        &expression_id,
        &replacement,
        "--revision",
        &workspace_revision,
    ];
    let output = fixture.invoke(&common);
    assert_success(&output);
    assert_eq!(output.stdout, direct.result().as_bytes());
    let mut evidence = common.to_vec();
    evidence.push("--evidence");
    let output = fixture.invoke(&evidence);
    assert_success(&output);
    assert_eq!(output.stdout, direct.evidence().as_bytes());
    let mut structural = common.to_vec();
    structural.push("--structural-diff");
    let output = fixture.invoke(&structural);
    assert_success(&output);
    let expected_diff = project::SemanticWorkspaceStructuralDiff::derive(
        direct.candidate(),
        direct.candidate().candidate_digest(),
    )
    .unwrap();
    assert_eq!(output.stdout, expected_diff.to_json().as_bytes());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn replace_expression_preview_rejects_stale_and_closed_grammar_without_writes() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let (expression_id, _) = selection(&revision, "calculator.add", "left + right");
    let stale = format!("sha256:{}", "0".repeat(64));
    let scalar = r#"{"kind":"i64","value":1}"#;
    let output = fixture.invoke(&[
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "replace-expression",
        "calculator.add",
        &expression_id,
        scalar,
        "--revision",
        &stale,
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G527"));

    let output = fixture.invoke(&[
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "replace-expression",
        "calculator.add",
        "caller-invented-expression-id",
        scalar,
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G527"));

    for arguments in [
        vec![
            "change",
            "preview",
            fixture.0.to_str().unwrap(),
            "replace-expression",
            "calculator.add",
            &expression_id,
            "not-json",
        ],
        vec![
            "change",
            "preview",
            fixture.0.to_str().unwrap(),
            "replace-expression",
            "calculator.add",
            &expression_id,
            scalar,
            "surplus",
        ],
    ] {
        let output = fixture.invoke(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
    }
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn replace_expression_preview_admits_explicit_monomorphic_main_without_writes() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace_revision = revision
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();
    let (expression_id, _) = selection(&revision, "calculator.app.main", "6");
    let replacement = r#"{"kind":"i64","value":7}"#;
    let output = fixture.invoke(&[
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "replace-expression",
        "calculator.app.main",
        &expression_id,
        replacement,
        "--revision",
        &workspace_revision,
    ]);
    assert_success(&output);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["operation_results"][0]["target"],
        "calculator.app.main"
    );
    assert_eq!(result["operation_results"][0]["outcome"], "validated");
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn exact_v2_service_routes_select_v2_and_v3_before_parse_and_replay_without_history() {
    let fixture = Fixture::new();
    let context_v1 = fixture.exact_context();
    let revision = Arc::clone(context_v1.revision());
    let (expression_id, old) = selection(&revision, "calculator.add", "left + right");
    let transaction = SemanticTransactionV2::replace_expression(
        context_v1.base_project_root().workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            expression_id,
            old,
            json!({
                "kind":"binary", "op":"-",
                "left":{"kind":"place","name":"left"},
                "right":{"kind":"place","name":"right"}
            }),
        ),
    )
    .unwrap();
    let workspace = context_v1
        .semantic_workspace()
        .workspace_revision()
        .to_owned();
    let root_v2 = context_v1
        .program_root_v2()
        .program_root_v2_digest()
        .to_owned();
    let service_v1 = SemanticWorkspaceService::open_exact(Arc::clone(&context_v1)).unwrap();
    let artifacts_v1 = service_v1
        .validate_transaction_v2_exact(transaction.to_json().as_bytes(), &workspace, &root_v2)
        .unwrap();
    assert_eq!(
        artifacts_v1.base_program_root_v2(),
        Some(context_v1.program_root_v2())
    );
    assert_eq!(service_v1.history_snapshot(&workspace).unwrap().len(), 1);
    service_v1
        .replay_transaction_v2_exact(
            transaction.to_json().as_bytes(),
            artifacts_v1.evidence().as_bytes(),
            &workspace,
            &root_v2,
        )
        .unwrap();
    assert_eq!(service_v1.history_snapshot(&workspace).unwrap().len(), 1);
    let history_query = SemanticWorkspaceServiceHistoryQuery::new(&workspace, 0, 1).unwrap();
    let history = service_v1
        .history_query_exact(history_query.to_json().as_bytes(), &workspace, &root_v2)
        .unwrap();
    assert_eq!(
        history.items()[0].base_workspace_revision(),
        context_v1.base_project_root().workspace_revision()
    );
    let bad_v2 = format!("sha256:{}", "0".repeat(64));
    assert_code(
        service_v1.validate_transaction_v2_exact(b"{}", &workspace, &bad_v2),
        "SPX-G555",
    );
    assert_eq!(service_v1.history_snapshot(&workspace).unwrap().len(), 1);

    let context_v2 = Arc::new(ExactProgramContextV2::assemble(Arc::clone(&context_v1)).unwrap());
    let root_v3 = context_v2
        .program_root_v3()
        .program_root_v3_digest()
        .to_owned();
    let service_v2 = SemanticWorkspaceService::open_exact_v2(Arc::clone(&context_v2)).unwrap();
    let artifacts_v2 = service_v2
        .validate_transaction_v2_exact_v2(transaction.to_json().as_bytes(), &workspace, &root_v3)
        .unwrap();
    assert_eq!(
        artifacts_v2.base_program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert_eq!(
        artifacts_v2.base_program_root_v3(),
        Some(context_v2.program_root_v3())
    );
    service_v2
        .replay_transaction_v2_exact_v2(
            transaction.to_json().as_bytes(),
            artifacts_v2.evidence().as_bytes(),
            &workspace,
            &root_v3,
        )
        .unwrap();
    assert_eq!(service_v2.history_snapshot(&workspace).unwrap().len(), 1);
    let bad_v3 = format!("sha256:{}", "0".repeat(64));
    assert_code(
        service_v2.validate_transaction_v2_exact_v2(b"{}", &workspace, &bad_v3),
        "SPX-G577",
    );
    assert_eq!(service_v2.history_snapshot(&workspace).unwrap().len(), 1);
}

#[test]
fn additive_route_leaves_legacy_v1_preview_bytes_unchanged() {
    let fixture = Fixture::new();
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
    let expected = transaction.validate(revision).unwrap();
    let output = fixture.invoke(&[
        "change",
        "preview",
        fixture.0.to_str().unwrap(),
        "rename-display-name",
        "calculator.add",
        "sum",
        "--revision",
        &workspace_revision,
    ]);
    assert_success(&output);
    assert_eq!(output.stdout, expected.result().as_bytes());
}
