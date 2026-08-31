//! Unfinished sibling draft merge evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft,
    ProjectCandidateDraftArchive, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-merge-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> Arc<ProjectCandidate> {
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    Arc::new(
        candidate
            .apply(candidate.candidate_digest(), &change)
            .unwrap(),
    )
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn body(target: &str, value: Value) -> Value {
    json!({"kind":"replace_function_body","target":target,"body":value})
}
fn selected(base: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let text = if contract {
        base.contract_expression_catalog(target)
    } else {
        base.expression_catalog(target)
    }
    .unwrap();
    let catalog: Value = serde_json::from_str(&text).unwrap();
    let source = base
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows: Vec<_> = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
fn open(base: &Arc<ProjectCandidate>) -> ProjectCandidateDraft {
    ProjectCandidateDraft::open(Arc::clone(base)).unwrap()
}
fn body_hole(base: &Arc<ProjectCandidate>, target: &str, hole: &str) -> ProjectCandidateDraft {
    let draft = open(base);
    draft
        .with_body_hole(draft.draft_digest(), target, hole)
        .unwrap()
}
fn archive(draft: &ProjectCandidateDraft) -> ProjectCandidateDraftArchive {
    ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest()).unwrap()
}
fn context(draft: &ProjectCandidateDraft, hole: &str) -> Value {
    serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}

#[test]
fn disjoint_checked_histories_union_all_kinds_coalesce_shared_hole_and_regenerate_contexts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let prefix = apply(
        &base,
        json!({"kind":"rename_declaration","target":"calculator.is-negative","name":"negative"}),
    );
    let left_base = apply(&prefix, body("calculator.add", integer(17)));
    let left_base = apply(
        &left_base,
        json!({"kind":"add_contract","target":"calculator.subtract","phase":"requires","predicate":{"kind":"bool","value":true}}),
    );
    let right_base = apply(
        &prefix,
        body(
            "calculator.is-negative",
            json!({"kind":"bool","value":false}),
        ),
    );
    let right_base = apply(
        &right_base,
        json!({"kind":"add_contract","target":"calculator.multiply","phase":"requires","predicate":{"kind":"bool","value":true}}),
    );
    let left = body_hole(&left_base, "calculator.multiply", "multiply");
    let left = left
        .with_expression_hole(
            left.draft_digest(),
            "calculator.subtract",
            &selected(&left_base, "calculator.subtract", false, "left - right"),
            "subtract",
        )
        .unwrap();
    let right = body_hole(&right_base, "calculator.multiply", "multiply");
    let right = right
        .with_contract_expression_hole(
            right.draft_digest(),
            "calculator.divide",
            &selected(&right_base, "calculator.divide", true, "right != 0"),
            "divide",
        )
        .unwrap();
    let right = right
        .with_body_hole(right.draft_digest(), "calculator.not", "not")
        .unwrap();
    let left_saved = archive(&left);
    let right_saved = archive(&right);
    let old = context(&left, "multiply");
    let result = left
        .merge(left.draft_digest(), &right, right.draft_digest())
        .unwrap();
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert_eq!(
        report["schema"],
        "semaprax.project-candidate-draft-merge.v1"
    );
    assert_eq!(report["left_parent_draft_digest"], left.draft_digest());
    assert_eq!(report["right_parent_draft_digest"], right.draft_digest());
    assert_eq!(
        report["original_base_revision"],
        base.base_revision().project_revision()
    );
    assert_eq!(
        report["result_base_revision"],
        base.base_revision().project_revision()
    );
    assert_eq!(report["result_draft_digest"], result.draft().draft_digest());
    assert_eq!(
        report["last_valid_merge"]["schema"],
        "semaprax.project-candidate-rebase.v1"
    );
    assert_eq!(report["left_holes"].as_array().unwrap().len(), 2);
    assert_eq!(report["right_holes"].as_array().unwrap().len(), 3);
    assert_eq!(report["materializable"], false);
    assert_eq!(report["source_authority"], false);
    let holes = report["holes"].as_array().unwrap();
    assert_eq!(holes.len(), 4);
    assert_eq!(
        holes
            .iter()
            .map(|row| row["hole_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["divide", "multiply", "not", "subtract"]
    );
    assert_eq!(holes[0]["parents"], json!(["right"]));
    assert_eq!(holes[1]["parents"], json!(["left", "right"]));
    assert_eq!(holes[2]["parents"], json!(["right"]));
    assert_eq!(holes[3]["parents"], json!(["left"]));
    for row in holes {
        assert_eq!(row["context_refreshed"], true);
    }
    let merged = result.into_draft();
    assert_eq!(
        context(&merged, "multiply")["contracts"]
            .as_array()
            .unwrap()
            .len(),
        old["contracts"].as_array().unwrap().len() + 1
    );
    assert_eq!(
        holes[0]["expression_id"],
        context(&merged, "divide")["expression_id"]
    );
    assert_eq!(
        holes[3]["expression_id"],
        context(&merged, "subtract")["expression_id"]
    );
    assert!(holes[1]["expression_id"].is_null());
    let saved = archive(&merged);
    let replay = ProjectCandidateDraftArchive::restore(
        saved.to_json().as_bytes(),
        saved.archive_digest(),
        saved.draft_digest(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), merged.to_json());
    for hole in ["divide", "multiply", "not", "subtract"] {
        assert_eq!(context(&replay, hole), context(&merged, hole));
    }
    code(replay.complete(replay.draft_digest()), "SPX-G232");
    let mut ready = replay;
    for (hole, expression) in [
        ("divide", json!({"kind":"bool","value":true})),
        ("multiply", integer(42)),
        ("not", json!({"kind":"bool","value":false})),
        ("subtract", integer(23)),
    ] {
        ready = ready
            .fill_hole(ready.draft_digest(), hole, &expression)
            .unwrap();
    }
    let complete = ready.complete(ready.draft_digest()).unwrap();
    let recovery: Value = serde_json::from_str(&complete.recovery_capsule().unwrap()).unwrap();
    assert_eq!(recovery["changes"].as_array().unwrap().len(), 9);
    assert_eq!(archive(&left).to_json(), left_saved.to_json());
    assert_eq!(archive(&right).to_json(), right_saved.to_json());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn identical_expression_and_contract_holes_coalesce_but_same_id_different_meaning_and_overlap_reject(
) {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    for contract in [false, true] {
        let (target, snippet) = if contract {
            ("calculator.divide", "right != 0")
        } else {
            ("calculator.multiply", "left * right")
        };
        let expression = selected(&base, target, contract, snippet);
        let initial = open(&base);
        let draft = if contract {
            initial.with_contract_expression_hole(
                initial.draft_digest(),
                target,
                &expression,
                "same",
            )
        } else {
            initial.with_expression_hole(initial.draft_digest(), target, &expression, "same")
        }
        .unwrap();
        let result = draft
            .merge(draft.draft_digest(), &draft, draft.draft_digest())
            .unwrap();
        let report: Value = serde_json::from_str(result.to_json()).unwrap();
        assert_eq!(report["holes"].as_array().unwrap().len(), 1);
        assert_eq!(report["holes"][0]["parents"], json!(["left", "right"]));
        assert_eq!(result.draft().to_json(), draft.to_json());
    }
    let left = body_hole(&base, "calculator.multiply", "same");
    let right = body_hole(&base, "calculator.add", "same");
    code(
        left.merge(left.draft_digest(), &right, right.draft_digest()),
        "SPX-G348",
    );
    let initial = open(&base);
    let expression = selected(&base, "calculator.multiply", false, "left * right");
    let overlap = initial
        .with_expression_hole(
            initial.draft_digest(),
            "calculator.multiply",
            &expression,
            "different",
        )
        .unwrap();
    code(
        left.merge(left.draft_digest(), &overlap, overlap.draft_digest()),
        "SPX-G230",
    );
}

#[test]
fn no_op_fill_and_edit_restore_history_cannot_silently_discharge_the_other_pending_intention() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let pending = body_hole(&base, "calculator.multiply", "multiply");
    let saved = archive(&pending);
    let original = json!({"kind":"binary","op":"*","left":{"kind":"place","name":"left"},"right":{"kind":"place","name":"right"}});
    let no_op = pending
        .fill_hole(pending.draft_digest(), "multiply", &original)
        .unwrap();
    let no_op_candidate = no_op.complete(no_op.draft_digest()).unwrap();
    assert_eq!(
        no_op_candidate.revision().project_revision(),
        base.revision().project_revision()
    );
    code(
        pending.merge(pending.draft_digest(), &no_op, no_op.draft_digest()),
        "SPX-G348",
    );
    code(
        no_op.merge(no_op.draft_digest(), &pending, pending.draft_digest()),
        "SPX-G348",
    );
    let edited = apply(&base, body("calculator.multiply", integer(9)));
    let reverted = apply(&edited, body("calculator.multiply", original));
    assert_eq!(
        reverted.revision().project_revision(),
        base.revision().project_revision()
    );
    let reverted = open(&reverted);
    code(
        pending.merge(pending.draft_digest(), &reverted, reverted.draft_digest()),
        "SPX-G348",
    );
    assert_eq!(archive(&pending).to_json(), saved.to_json());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn contract_protected_history_and_changed_pending_regions_are_conflicts_without_mutation() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let initial = open(&base);
    let expression = selected(&base, "calculator.divide", true, "right != 0");
    let pending = initial
        .with_contract_expression_hole(
            initial.draft_digest(),
            "calculator.divide",
            &expression,
            "divide",
        )
        .unwrap();
    let saved = archive(&pending);
    let predicate = json!({"kind":"binary","op":"!=","left":{"kind":"place","name":"right"},"right":integer(0)});
    let no_op = pending
        .fill_hole(pending.draft_digest(), "divide", &predicate)
        .unwrap();
    code(
        pending.merge(pending.draft_digest(), &no_op, no_op.draft_digest()),
        "SPX-G348",
    );
    let changed = apply(
        &base,
        json!({"kind":"add_contract","target":"calculator.divide","phase":"requires","predicate":{"kind":"bool","value":true}}),
    );
    let changed = open(&changed);
    code(
        pending.merge(pending.draft_digest(), &changed, changed.draft_digest()),
        "SPX-G348",
    );
    assert_eq!(archive(&pending).to_json(), saved.to_json());
}

#[test]
fn stale_different_original_base_and_union_above_sixteen_holes_fail_closed() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/core.spx");
    let mut text = std::fs::read_to_string(&path).unwrap();
    for index in 0..17 {
        text.push_str(&format!(
            "\n@id(\"merge.extra.{index}\") fn extra_{index}(value:i64)->i64 {{value}}\n"
        ));
    }
    let parsed = semaprax::parse(&text, "src/core.spx").unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let base = fixture.candidate();
    let mut left = open(&base);
    let mut right = open(&base);
    for index in 0..17 {
        let target = format!("merge.extra.{index}");
        let hole = format!("hole.{index:02}");
        if index < 8 {
            left = left
                .with_body_hole(left.draft_digest(), &target, &hole)
                .unwrap();
        } else {
            right = right
                .with_body_hole(right.draft_digest(), &target, &hole)
                .unwrap();
        }
    }
    let left_saved = archive(&left);
    let right_saved = archive(&right);
    code(
        left.merge(left.draft_digest(), &right, right.draft_digest()),
        "SPX-G347",
    );
    let wrong = format!("sha256:{}", "0".repeat(64));
    code(left.merge(&wrong, &right, right.draft_digest()), "SPX-G232");
    code(left.merge(left.draft_digest(), &right, &wrong), "SPX-G232");
    let changed = apply(&base, body("calculator.add", integer(7)));
    let other = Arc::new(
        ProjectCandidate::open(
            Arc::clone(changed.revision()),
            changed.revision().project_revision(),
        )
        .unwrap(),
    );
    let other = open(&other);
    code(
        left.merge(left.draft_digest(), &other, other.draft_digest()),
        "SPX-G235",
    );
    assert_eq!(archive(&left).to_json(), left_saved.to_json());
    assert_eq!(archive(&right).to_json(), right_saved.to_json());
}

#[test]
fn empty_and_ready_siblings_merge_checked_histories_but_release_candidate_only_on_completion() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let empty = open(&base);
    let left = body_hole(&base, "calculator.add", "add");
    let left = left
        .fill_hole(left.draft_digest(), "add", &integer(17))
        .unwrap();
    let right = body_hole(&base, "calculator.subtract", "subtract");
    let right = right
        .fill_hole(right.draft_digest(), "subtract", &integer(23))
        .unwrap();
    let empty_result = empty
        .merge(empty.draft_digest(), &empty, empty.draft_digest())
        .unwrap();
    assert_eq!(
        empty_result
            .draft()
            .complete(empty_result.draft().draft_digest())
            .unwrap()
            .to_json(),
        base.to_json()
    );
    let result = left
        .merge(left.draft_digest(), &right, right.draft_digest())
        .unwrap();
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert_eq!(report["holes"], json!([]));
    assert_eq!(report["materializable"], false);
    let ready = result.into_draft();
    let summary: Value =
        serde_json::from_str(ready.summary(ready.draft_digest()).unwrap()).unwrap();
    assert_eq!(summary["state"], "ready_to_complete");
    let complete = ready.complete(ready.draft_digest()).unwrap();
    let recovery: Value = serde_json::from_str(&complete.recovery_capsule().unwrap()).unwrap();
    assert_eq!(recovery["changes"].as_array().unwrap().len(), 2);
}
