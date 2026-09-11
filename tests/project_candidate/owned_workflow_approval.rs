//! #128 (SPX-AI-029): conservative owned-data staleness and separately
//! approved publication for Universal Semantic Transaction v2 workflows.
//! See `docs/OWNED-WORKFLOW-APPROVAL-V1.md`.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    apply_approved_owned_workflow_publication, apply_candidate_publication,
    prepare_approved_owned_workflow_publication, require_owned_targets_unchanged,
    reselect_owned_workflow, with_authenticated_project, OwnedWorkflowApproval,
    OwnedWorkflowCandidate, ProjectCandidate, ProjectRevision, SemanticChange,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SemanticTransactionV2Workflow,
    OWNED_WORKFLOW_APPROVAL_SCHEMA,
};
use semaprax::{semantic_workspace, workspace_graph};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-owned-workflow-approval-{}-{}",
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
        let paths = root.join("paths.json");
        std::fs::write(&paths,"{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"src/app.spx\"},{\"path\":\"src/core.spx\"},{\"path\":\"src/tests.spx\"}]}\n").unwrap();
        let root = root.canonicalize().unwrap();
        semantic_workspace::initialize(&root, &paths).unwrap();
        Self(root)
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.manifest(), |snapshot| Ok(snapshot.retain_revision()))
            .unwrap()
    }

    fn workspace_revision(&self) -> String {
        workspace_graph::snapshot(&self.0, "calculator.app")
            .unwrap()
            .workspace_revision()
            .to_owned()
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

/// Exact snippet match, mirroring
/// `universal_semantic_transaction_v2_workflow.rs`'s helper.
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

fn swap(left: &str, right: &str) -> Value {
    swap_op("*", left, right)
}

fn swap_op(op: &str, left: &str, right: &str) -> Value {
    json!({
        "kind": "binary", "op": op,
        "left": {"kind": "place", "name": right},
        "right": {"kind": "place", "name": left},
    })
}

fn literal(value: i64) -> Value {
    json!({"kind": "i64", "value": value})
}

/// A concurrent sibling edit unrelated to `calculator.multiply`: a v2 body
/// replacement on a different declaration, from the same original base.
fn sibling_edit(base: &Arc<ProjectRevision>) -> Arc<ProjectRevision> {
    let edit = transaction(
        base,
        "calculator.is-negative",
        "value < 0",
        json!({
            "kind": "binary", "op": "<",
            "left": {"kind": "place", "name": "value"},
            "right": {"kind": "i64", "value": 1},
        }),
    );
    Arc::clone(
        edit.validate(Arc::clone(base))
            .unwrap()
            .candidate()
            .revision(),
    )
}

/// A concurrent edit that changes `calculator.multiply`'s own signature
/// (an appended parameter, and thus its ownership-mode-carrying parameter
/// list) without touching its body text at all.
fn same_target_signature_edit(base: &Arc<ProjectRevision>) -> Arc<ProjectRevision> {
    let root = ProjectCandidate::open(Arc::clone(base), base.project_revision()).unwrap();
    let change = SemanticChange::new(
        base.project_revision(),
        &json!({
            "kind": "change_function_signature",
            "target": "calculator.multiply",
            "append_parameters": [
                {"name": "unused", "type": "i64", "argument": {"kind": "i64", "value": 0}},
            ],
        }),
    )
    .unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    Arc::clone(candidate.revision())
}

/// A concurrent edit that adds a contract to `calculator.divide` without
/// touching its body text.
fn same_target_contract_edit(base: &Arc<ProjectRevision>) -> Arc<ProjectRevision> {
    let root = ProjectCandidate::open(Arc::clone(base), base.project_revision()).unwrap();
    let change = SemanticChange::new(
        base.project_revision(),
        &json!({
            "kind": "add_contract",
            "target": "calculator.divide",
            "phase": "ensures",
            "predicate": {"kind": "bool", "value": true},
        }),
    )
    .unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    Arc::clone(candidate.revision())
}

#[test]
fn disjoint_sibling_edit_survives_the_staleness_gate_and_reselection() {
    let fixture = Fixture::new();
    let base = fixture.revision();

    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap("left", "right"),
    );
    let original_owned =
        OwnedWorkflowCandidate::derive(Arc::clone(&base), std::slice::from_ref(&step0)).unwrap();
    let original_approval = OwnedWorkflowApproval::approve(&original_owned).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(original_approval.to_json()).unwrap()["schema"],
        OWNED_WORKFLOW_APPROVAL_SCHEMA
    );
    assert_eq!(
        original_approval.candidate_digest(),
        original_owned.candidate().candidate_digest()
    );

    // An unrelated declaration moves concurrently, from the same base.
    let current_base = sibling_edit(&base);
    assert_ne!(base.project_revision(), current_base.project_revision());

    // The gate reports the drift as safe: it never touched a declaration
    // this workflow's one step depends on.
    require_owned_targets_unchanged(&base, &current_base, std::slice::from_ref(&step0)).unwrap();

    // Reselecting replays the *same* transaction content against the live
    // base and independently reselects/recompiles.
    let reselected = reselect_owned_workflow(
        &base,
        Arc::clone(&current_base),
        std::slice::from_ref(&step0),
    )
    .unwrap();
    let source = reselected
        .candidate()
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source()
        .to_owned();
    assert!(
        source.contains("right * left"),
        "multiply edit preserved: {source}"
    );
    assert!(
        source.contains("value < 1"),
        "sibling edit preserved: {source}"
    );

    // A different base means a different digest: the original approval
    // never authorizes the reselected candidate.
    assert_ne!(
        reselected.workflow().digest(),
        original_owned.workflow().digest()
    );
    assert_ne!(
        reselected.candidate().candidate_digest(),
        original_approval.candidate_digest()
    );
}

#[test]
fn same_declaration_signature_conflict_is_refused_even_though_a_naive_reselection_would_admit_it() {
    let fixture = Fixture::new();
    let base = fixture.revision();

    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap("left", "right"),
    );
    let current_base = same_target_signature_edit(&base);
    assert_ne!(base.project_revision(), current_base.project_revision());

    // The concurrent edit never touched the body text step 0 selects, so a
    // naive reselection that only refreshes the workspace-revision wrapper
    // (keeping the exact same expression identity and expected old-source
    // text) is NOT conservative enough: `multiply`'s appended parameter is
    // unused, so the whole program still admits cleanly.
    let naive_workspace = current_base.canonical_workspace_revision().unwrap();
    let operation = step0.operation();
    let naive = SemanticTransactionV2::replace_expression(
        naive_workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            operation.target(),
            operation.expression_id(),
            operation.expected_old_expression(),
            operation.replacement().clone(),
        ),
    )
    .unwrap();
    SemanticTransactionV2Workflow::derive(Arc::clone(&current_base), std::slice::from_ref(&naive))
        .expect("a naive reselection silently admits the signature-changed target");

    // The owned-data staleness gate refuses it explicitly instead: the
    // touched declaration's signature (and therefore its parameter list's
    // ownership modes) changed concurrently.
    assert_code(
        require_owned_targets_unchanged(&base, &current_base, std::slice::from_ref(&step0)),
        "SPX-G345",
    );
    assert_code(
        reselect_owned_workflow(
            &base,
            Arc::clone(&current_base),
            std::slice::from_ref(&step0),
        ),
        "SPX-G345",
    );
}

#[test]
fn same_declaration_contract_conflict_is_refused() {
    let fixture = Fixture::new();
    let base = fixture.revision();

    let step0 = transaction(
        &base,
        "calculator.divide",
        "left / right",
        swap("left", "right"),
    );
    let current_base = same_target_contract_edit(&base);
    assert_ne!(base.project_revision(), current_base.project_revision());

    assert_code(
        require_owned_targets_unchanged(&base, &current_base, std::slice::from_ref(&step0)),
        "SPX-G345",
    );
    assert_code(
        reselect_owned_workflow(
            &base,
            Arc::clone(&current_base),
            std::slice::from_ref(&step0),
        ),
        "SPX-G345",
    );
}

#[test]
fn approving_one_candidate_never_authorizes_publishing_a_different_one() {
    let fixture = Fixture::new();
    let base = fixture.revision();
    let workspace_revision = fixture.workspace_revision();
    let before = inventory(&fixture.0);

    // Each candidate's publication must genuinely change at least two files
    // (the existing managed-Workspace Change-v1 envelope's own bound), so
    // each workflow here has two steps across two different source files.
    let step_a0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap("left", "right"),
    );
    let after_a0 = step_a0.validate(Arc::clone(&base)).unwrap();
    let step_a1 = transaction(
        &Arc::clone(after_a0.candidate().revision()),
        "calculator.app.main",
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
        literal(100),
    );
    let owned_a = OwnedWorkflowCandidate::derive(Arc::clone(&base), &[step_a0, step_a1]).unwrap();

    let step_b0 = transaction(
        &base,
        "calculator.subtract",
        "left - right",
        swap_op("-", "left", "right"),
    );
    let after_b0 = step_b0.validate(Arc::clone(&base)).unwrap();
    let step_b1 = transaction(
        &Arc::clone(after_b0.candidate().revision()),
        "calculator.tests.main",
        "if add(19, 23) == 42 && subtract(23, 19) == 4 && multiply(6, 7) == 42 && divide(84, 2) == 42 && is_negative(-1) && not(false) { 0 } else { 1 }",
        literal(0),
    );
    let owned_b = OwnedWorkflowCandidate::derive(Arc::clone(&base), &[step_b0, step_b1]).unwrap();
    assert_ne!(
        owned_a.candidate().candidate_digest(),
        owned_b.candidate().candidate_digest()
    );

    let approval_a = OwnedWorkflowApproval::approve(&owned_a).unwrap();

    // A candidate that was created but never separately approved: no
    // approval exists to present for `owned_b` at all.
    assert_code(
        prepare_approved_owned_workflow_publication(
            &approval_a,
            &owned_b,
            &fixture.0,
            &fixture.manifest(),
            &workspace_revision,
        ),
        "SPX-G605",
    );

    // The exact approved candidate is preparable and read-only.
    let proof = prepare_approved_owned_workflow_publication(
        &approval_a,
        &owned_a,
        &fixture.0,
        &fixture.manifest(),
        &workspace_revision,
    )
    .unwrap();
    assert_eq!(inventory(&fixture.0), before);

    // The same mismatched approval still cannot drive the real `ACTIVE`
    // pivot for the other candidate.
    assert_code(
        apply_approved_owned_workflow_publication(
            &approval_a,
            &owned_b,
            &fixture.0,
            &fixture.manifest(),
            &workspace_revision,
            proof.to_json().as_bytes(),
        ),
        "SPX-G605",
    );
    assert_eq!(inventory(&fixture.0), before);

    // Only the exact independently approved final candidate publishes.
    let receipt = apply_approved_owned_workflow_publication(
        &approval_a,
        &owned_a,
        &fixture.0,
        &fixture.manifest(),
        &workspace_revision,
        proof.to_json().as_bytes(),
    )
    .unwrap();
    let receipt: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(receipt["result"], "managed_generation_published");
    let managed = workspace_graph::snapshot(&fixture.0, "calculator.app").unwrap();
    assert_ne!(managed.workspace_revision(), workspace_revision);

    // Original raw source files are untouched by the managed publication:
    // only the managed `.semaprax-workspace` generation tree may change.
    for file in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        let path = Path::new(file);
        assert_eq!(&std::fs::read(fixture.0.join(path)).unwrap(), &before[path]);
    }

    // Re-applying the same already-consumed approval/proof a second time is
    // refused rather than silently repeated; disk is unchanged again.
    let after_first_publish = inventory(&fixture.0);
    assert!(apply_candidate_publication(
        owned_a.candidate(),
        approval_a.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &workspace_revision,
        proof.to_json().as_bytes(),
    )
    .is_err());
    assert_eq!(inventory(&fixture.0), after_first_publish);
}

#[test]
fn approval_replay_requires_exact_canonical_bytes_and_digest() {
    let fixture = Fixture::new();
    let base = fixture.revision();
    let step0 = transaction(
        &base,
        "calculator.multiply",
        "left * right",
        swap("left", "right"),
    );
    let owned =
        OwnedWorkflowCandidate::derive(Arc::clone(&base), std::slice::from_ref(&step0)).unwrap();
    let approval = OwnedWorkflowApproval::approve(&owned).unwrap();

    let replayed =
        OwnedWorkflowApproval::replay(approval.digest(), approval.to_json().as_bytes()).unwrap();
    assert_eq!(replayed.to_json(), approval.to_json());
    assert_eq!(replayed.candidate_digest(), approval.candidate_digest());

    let mut tampered_digest = approval.digest().to_owned();
    let last = tampered_digest.pop().unwrap();
    tampered_digest.push(if last == '0' { '1' } else { '0' });
    assert_code(
        OwnedWorkflowApproval::replay(&tampered_digest, approval.to_json().as_bytes()),
        "SPX-G605",
    );

    let mut tampered_bytes = approval.to_json().as_bytes().to_vec();
    tampered_bytes.push(b' ');
    assert_code(
        OwnedWorkflowApproval::replay(approval.digest(), &tampered_bytes),
        "SPX-G603",
    );
}
