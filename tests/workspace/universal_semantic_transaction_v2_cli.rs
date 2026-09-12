use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{
    self, render_project_lock, with_authenticated_project, ExactProgramContext,
    ExactProgramContextV2, ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2,
    ProjectCandidate, ProjectRevision, SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SemanticTransactionV2Workflow,
    SemanticWorkspaceRevision, SemanticWorkspaceService, SemanticWorkspaceServiceHistoryQuery,
    MAX_IMAGE_ARTIFACT_BUILD_BYTES,
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

// ---------------------------------------------------------------------
// `change workflow`: ordered multi-step Universal Semantic Transaction v2
// workflows (issue #219, follow-up from #127).
// ---------------------------------------------------------------------

fn swap_op(op: &str, left: &str, right: &str) -> Value {
    json!({
        "kind": "binary", "op": op,
        "left": {"kind": "place", "name": right},
        "right": {"kind": "place", "name": left},
    })
}

/// A calculator-project fixture with a distinct, human-authored leading
/// comment on `src/core.spx` only; `src/app.spx` and `src/tests.spx` stay
/// canonical. Known, load-bearing fact this fixture depends on (confirmed by
/// reading `src/project/semantic_transaction_v2.rs`'s `validate`): the
/// candidate a v2 step admits is ALWAYS fully canonical, comment-free
/// source -- `require_canonical_comment_free_sources` runs unconditionally
/// on the freshly-applied candidate, with no exemption for the just-edited
/// file. The comment-preserving splice (#126) is a SEPARATE, Rust-only
/// convenience (`SemanticTransactionArtifactsV2::preserved_target_source`),
/// never part of `candidate().revision()`'s own stored text, and not
/// currently exposed anywhere on `SemanticTransactionV2Workflow`'s own
/// public surface. So a comment only needs to survive as far as the ONE
/// step that admits it: by the time a LATER step runs, that file is already
/// canonical again, which is exactly what lets a later step target a
/// genuinely DIFFERENT file without tripping
/// `require_canonical_comment_free_sources_except`'s "outside the edited
/// file must already be canonical" requirement.
struct CommentedFixture(PathBuf);

impl CommentedFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-universal-semantic-transaction-v2-cli-workflow-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        std::fs::copy(sample.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
        std::fs::copy(sample.join("src/app.spx"), root.join("src/app.spx")).unwrap();
        std::fs::copy(sample.join("src/tests.spx"), root.join("src/tests.spx")).unwrap();
        let core_source = std::fs::read_to_string(sample.join("src/core.spx")).unwrap();
        std::fs::write(
            root.join("src/core.spx"),
            format!("// core-owner note: keep me above add\n{core_source}"),
        )
        .unwrap();
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
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }
}

impl Drop for CommentedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn workflow_over_two_files_matches_the_direct_core_when_the_first_files_comment_is_admitted() {
    let fixture = CommentedFixture::new();
    let before = inventory(&fixture.0);
    let base = fixture.revision();
    let workspace_revision = base
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();

    // Step 0 replaces `calculator.multiply`'s body in the one comment-bearing
    // file, src/core.spx. Its own artifacts still expose the comment via the
    // Rust-only comment-preserving-splice convenience, proving the workflow
    // core's first step admits and correctly handles comment-bearing source
    // rather than merely tolerating an already-canonical one.
    let (expression_id_0, old_0) = selection(&base, "calculator.multiply", "left * right");
    let replacement_0 = swap_op("*", "left", "right");
    let step_0 = SemanticTransactionV2::replace_expression(
        &workspace_revision,
        SemanticTransactionReplaceExpression::new(
            "calculator.multiply",
            &expression_id_0,
            &old_0,
            replacement_0.clone(),
        ),
    )
    .unwrap();
    let after_0 = step_0.validate(Arc::clone(&base)).unwrap();
    assert_eq!(
        after_0.preserved_target_source().unwrap(),
        format!(
            "// core-owner note: keep me above add\n{}",
            std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/calculator-project/src/core.spx")
            )
            .unwrap()
        )
        .replace("left * right", "right * left")
    );
    let intermediate = Arc::clone(after_0.candidate().revision());

    // Step 1 reselects fresh against the revision step 0 actually produced,
    // and edits a genuinely DIFFERENT file, src/app.spx -- possible only
    // because step 0's candidate is fully canonical again (see
    // `CommentedFixture`'s doc comment), so src/core.spx no longer trips
    // "outside the edited file must be canonical" for this second step.
    let target_1 = "calculator.app.main";
    let snippet_1 = "add(multiply(6, 7), subtract(divide(4, 2), 2))";
    let (expression_id_1, old_1) = selection(&intermediate, target_1, snippet_1);
    let replacement_1 = json!({"kind": "i64", "value": 100});
    let workspace_revision_1 = intermediate
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();
    let step_1 = SemanticTransactionV2::replace_expression(
        &workspace_revision_1,
        SemanticTransactionReplaceExpression::new(
            target_1,
            &expression_id_1,
            &old_1,
            replacement_1.clone(),
        ),
    )
    .unwrap();

    let direct = SemanticTransactionV2Workflow::derive(Arc::clone(&base), &[step_0, step_1])
        .expect("direct core workflow composition must succeed");

    let replacement_0_text = serde_json::to_string(&replacement_0).unwrap();
    let replacement_1_text = serde_json::to_string(&replacement_1).unwrap();
    let common = [
        "change",
        "workflow",
        fixture.0.to_str().unwrap(),
        "replace-expression",
        "calculator.multiply",
        &expression_id_0,
        &replacement_0_text,
        "--then",
        "replace-expression",
        target_1,
        &expression_id_1,
        &replacement_1_text,
        "--revision",
        &workspace_revision,
    ];

    // CLI/service results refer to the same final candidate as the direct
    // core composition (issue #127's own criterion), not merely a
    // structurally similar one: the exact result envelope bytes agree.
    let output = fixture.invoke(&common);
    assert_success(&output);
    assert_eq!(output.stdout, direct.to_json().as_bytes());

    let mut structural = common.to_vec();
    structural.push("--structural-diff");
    let structural_output = fixture.invoke(&structural);
    assert_success(&structural_output);
    assert_eq!(
        structural_output.stdout,
        direct.structural_diff().to_json().as_bytes()
    );

    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn workflow_grammar_rejects_a_malformed_second_step_without_writes() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let base = fixture.revision();
    let workspace_revision = base
        .canonical_workspace_revision()
        .unwrap()
        .workspace_revision()
        .to_owned();
    let (expression_id_0, _old_0) = selection(&base, "calculator.multiply", "left * right");
    let replacement_0 = serde_json::to_string(&swap_op("*", "left", "right")).unwrap();

    // The second step names an expression identity that never existed
    // (stale/unavailable), which must surface as a diagnostic on stderr
    // and a nonzero exit, never a written file.
    let output = fixture.invoke(&[
        "change",
        "workflow",
        fixture.0.to_str().unwrap(),
        "replace-expression",
        "calculator.multiply",
        &expression_id_0,
        &replacement_0,
        "--then",
        "replace-expression",
        "calculator.app.main",
        "no-such-expression-identity",
        &replacement_0,
        "--revision",
        &workspace_revision,
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G527"));
    assert_eq!(inventory(&fixture.0), before);
}
