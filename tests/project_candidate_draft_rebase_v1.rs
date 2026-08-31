//! Typed draft rebase evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft,
    ProjectCandidateDraftArchive, SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-rebase-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "draft-rebase"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "draft_rebase.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["draft.public"]
tests = ["draft_rebase.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module draft_rebase.core;
@id("draft.helper") fn helper(value:i64)->i64 {value + 1}
@id("draft.predicate") fn predicate(value:i64)->bool {value >= 0}
@id("draft.body") fn body(left:i64,right:i64)->i64 requires left >= 0 {left + right}
@id("draft.expression") fn expression(value:i64)->i64 requires value >= 0 {helper(value)}
@id("draft.checked") fn checked(left:i64,right:i64)->i64 requires predicate(right) ensures result >= left {left + right}
@id("draft.filled") fn filled(value:i64)->i64 {value}
@id("draft.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/app.spx",
                r#"module draft_rebase.app;
use function @id("draft.checked") from draft_rebase.core as checked;
@id("draft.main") fn main()->i64 {checked(4,2)}
"#,
            ),
            (
                "src/tests.spx",
                r#"module draft_rebase.tests;
use function @id("draft.checked") from draft_rebase.core as checked;
@id("draft.test") fn main()->i64 {if checked(4,2) == 6 {0}else{1}}
"#,
            ),
        ] {
            let parsed = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
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
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn body(target: &str, value: i64) -> Value {
    json!({"kind":"replace_function_body","target":target,"body":integer(value)})
}
fn contract(target: &str) -> Value {
    json!({"kind":"add_contract","target":target,"phase":"requires","predicate":{"kind":"bool","value":true}})
}
fn selected(candidate: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let text = if contract {
        candidate.contract_expression_catalog(target)
    } else {
        candidate.expression_catalog(target)
    }
    .unwrap();
    let catalog: Value = serde_json::from_str(&text).unwrap();
    let source = candidate
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
fn mixed(base: &Arc<ProjectCandidate>) -> ProjectCandidateDraft {
    let draft = ProjectCandidateDraft::open(Arc::clone(base)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "draft.body", "a.body")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "draft.expression",
            &selected(base, "draft.expression", false, "helper(value)"),
            "b.expression",
        )
        .unwrap();
    let draft = draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            "draft.checked",
            &selected(base, "draft.checked", true, "predicate(right)"),
            "c.contract",
        )
        .unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "draft.filled", "filled")
        .unwrap();
    draft
        .fill_hole(draft.draft_digest(), "filled", &integer(17))
        .unwrap()
}
fn context(draft: &ProjectCandidateDraft, hole: &str) -> Value {
    serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap()
}
fn archive(draft: &ProjectCandidateDraft) -> ProjectCandidateDraftArchive {
    ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest()).unwrap()
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
fn mixed_partial_history_rebases_across_display_renames_and_disjoint_regions_then_completes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let before = archive(&draft);
    let old_body = context(&draft, "a.body");
    let old_expression = context(&draft, "b.expression");
    let next = apply(&base, contract("draft.body"));
    let next = apply(&next, contract("draft.expression"));
    let next = apply(&next, body("draft.checked", 6));
    let next = apply(
        &next,
        json!({"kind":"rename_declaration","target":"draft.helper","name":"increment"}),
    );
    let next = apply(
        &next,
        json!({"kind":"rename_declaration","target":"draft.predicate","name":"nonnegative"}),
    );
    let result = draft
        .rebase(
            draft.draft_digest(),
            Arc::clone(next.revision()),
            next.revision().project_revision(),
        )
        .unwrap();
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert_eq!(
        report["schema"],
        "semaprax.project-candidate-draft-rebase.v1"
    );
    assert_eq!(report["parent_draft_digest"], draft.draft_digest());
    assert_eq!(
        report["original_base_revision"],
        base.base_revision().project_revision()
    );
    assert_eq!(report["onto_revision"], next.revision().project_revision());
    assert_eq!(
        report["result_base_revision"],
        next.revision().project_revision()
    );
    assert_eq!(report["result_draft_digest"], result.draft().draft_digest());
    assert_eq!(
        report["last_valid_rebase"]["schema"],
        "semaprax.project-candidate-rebase.v1"
    );
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["materializable"], false);
    let rows = report["holes"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for (index, hole, kind) in [
        (0, "a.body", "function_body"),
        (1, "b.expression", "expression"),
        (2, "c.contract", "contract_expression"),
    ] {
        assert_eq!(rows[index]["hole_id"], hole);
        assert_eq!(rows[index]["kind"], kind);
        assert_eq!(rows[index]["context_refreshed"], true);
    }
    for row in &rows[..2] {
        assert_eq!(row["concurrent_contract_change"], true);
        assert_eq!(row["concurrent_body_change"], false);
    }
    assert_eq!(rows[2]["concurrent_body_change"], true);
    assert_eq!(rows[2]["concurrent_contract_change"], false);
    assert!(rows[0]["old_expression_id"].is_null());
    assert!(rows[0]["new_expression_id"].is_null());
    let rebased = result.into_draft();
    let new_body = context(&rebased, "a.body");
    let new_expression = context(&rebased, "b.expression");
    assert_eq!(
        new_body["contracts"].as_array().unwrap().len(),
        old_body["contracts"].as_array().unwrap().len() + 1
    );
    assert_eq!(
        new_expression["prior_body_proof"]["contracts"]
            .as_array()
            .unwrap()
            .len(),
        old_expression["prior_body_proof"]["contracts"]
            .as_array()
            .unwrap()
            .len()
            + 1
    );
    assert_eq!(
        rows[1]["old_expression_id"],
        old_expression["expression_id"]
    );
    assert_eq!(
        rows[1]["new_expression_id"],
        new_expression["expression_id"]
    );
    assert_eq!(
        rows[2]["new_expression_id"],
        context(&rebased, "c.contract")["expression_id"]
    );
    let saved = archive(&rebased);
    assert_eq!(saved.base_revision(), next.revision().project_revision());
    let replay = ProjectCandidateDraftArchive::restore(
        saved.to_json().as_bytes(),
        saved.archive_digest(),
        saved.draft_digest(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), rebased.to_json());
    for hole in ["a.body", "b.expression", "c.contract"] {
        assert_eq!(context(&replay, hole), context(&rebased, hole));
    }
    code(replay.complete(replay.draft_digest()), "SPX-G232");
    let ready = replay
        .fill_hole(replay.draft_digest(), "a.body", &integer(6))
        .unwrap();
    let ready = ready
        .fill_hole(ready.draft_digest(), "b.expression", &integer(9))
        .unwrap();
    let ready = ready
        .fill_hole(
            ready.draft_digest(),
            "c.contract",
            &json!({"kind":"bool","value":true}),
        )
        .unwrap();
    let complete = ready.complete(ready.draft_digest()).unwrap();
    assert_eq!(
        complete.base_revision().project_revision(),
        next.revision().project_revision()
    );
    let recovery: Value = serde_json::from_str(&complete.recovery_capsule().unwrap()).unwrap();
    assert_eq!(recovery["changes"].as_array().unwrap().len(), 4);
    assert_eq!(archive(&draft).to_json(), before.to_json());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn pending_owner_region_signature_and_contract_callee_conflicts_preserve_exact_draft() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let saved = archive(&draft);
    for intent in [
        body("draft.body", 5),
        body("draft.expression", 8),
        contract("draft.checked"),
        contract("draft.predicate"),
        json!({"kind":"change_function_signature","target":"draft.body","append_parameters":[{"name":"extra","type":"i64","argument":integer(0)}]}),
        json!({"kind":"change_function_signature","target":"draft.expression","append_parameters":[{"name":"extra","type":"i64","argument":integer(0)}]}),
        json!({"kind":"change_function_signature","target":"draft.checked","append_parameters":[{"name":"extra","type":"i64","argument":integer(0)}]}),
    ] {
        let competing = apply(&base, intent);
        code(
            draft.rebase(
                draft.draft_digest(),
                Arc::clone(competing.revision()),
                competing.revision().project_revision(),
            ),
            "SPX-G345",
        );
        assert_eq!(archive(&draft).to_json(), saved.to_json());
    }
    // Filled intentions retain the pre-existing complete-history conflict gate.
    let competing = apply(&base, body("draft.filled", 99));
    code(
        draft.rebase(
            draft.draft_digest(),
            Arc::clone(competing.revision()),
            competing.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(archive(&draft).to_json(), saved.to_json());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stale_selectors_and_removed_origin_do_not_prevent_exact_selected_new_base_recovery() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let saved = archive(&draft);
    let next = apply(&base, body("draft.checked", 6));
    let wrong = format!("sha256:{}", "0".repeat(64));
    code(
        draft.rebase(
            &wrong,
            Arc::clone(next.revision()),
            next.revision().project_revision(),
        ),
        "SPX-G232",
    );
    assert!(draft
        .rebase(draft.draft_digest(), Arc::clone(next.revision()), &wrong)
        .is_err());
    std::fs::remove_dir_all(&fixture.0).unwrap();
    let restored = ProjectCandidateDraftArchive::restore(
        saved.to_json().as_bytes(),
        saved.archive_digest(),
        saved.draft_digest(),
    )
    .unwrap();
    let rebased = restored
        .rebase(
            restored.draft_digest(),
            Arc::clone(next.revision()),
            next.revision().project_revision(),
        )
        .unwrap();
    assert_eq!(
        archive(rebased.draft()).base_revision(),
        next.revision().project_revision()
    );
    assert_eq!(archive(&restored).to_json(), saved.to_json());
    assert!(!fixture.0.exists());
}

#[test]
fn empty_and_fully_filled_drafts_still_require_explicit_completion_after_rebase() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let next = apply(&base, body("draft.body", 8));
    let result = empty
        .rebase(
            empty.draft_digest(),
            Arc::clone(next.revision()),
            next.revision().project_revision(),
        )
        .unwrap();
    let report: Value = serde_json::from_str(result.to_json()).unwrap();
    assert_eq!(report["holes"], json!([]));
    assert_eq!(report["materializable"], false);
    let ready = result.into_draft();
    let summary: Value =
        serde_json::from_str(ready.summary(ready.draft_digest()).unwrap()).unwrap();
    assert_eq!(summary["state"], "ready_to_complete");
    let complete = ready.complete(ready.draft_digest()).unwrap();
    assert_eq!(
        complete.revision().project_revision(),
        next.revision().project_revision()
    );
    let draft = empty
        .with_body_hole(empty.draft_digest(), "draft.filled", "filled")
        .unwrap();
    let ready = draft
        .fill_hole(draft.draft_digest(), "filled", &integer(17))
        .unwrap();
    let result = ready
        .rebase(
            ready.draft_digest(),
            Arc::clone(next.revision()),
            next.revision().project_revision(),
        )
        .unwrap();
    let complete = result
        .draft()
        .complete(result.draft().draft_digest())
        .unwrap();
    assert_eq!(
        complete.base_revision().project_revision(),
        next.revision().project_revision()
    );
    let recovery: Value = serde_json::from_str(&complete.recovery_capsule().unwrap()).unwrap();
    assert_eq!(recovery["changes"].as_array().unwrap().len(), 1);
}

#[test]
fn copy_record_forwarding_binds_field_identity_even_when_owner_body_and_signature_are_unchanged() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/core.spx");
    let text = std::fs::read_to_string(&path).unwrap()
        + r#"
@id("draft.packet") record Packet { @id("draft.packet.value") value:i64, }
@id("draft.forward") fn forward(packet:Packet)->Packet {packet}
"#;
    let old_program = semaprax::parse(&text, "src/core.spx").unwrap();
    let canonical = semaprax::format::canonical(&old_program);
    std::fs::write(&path, &canonical).unwrap();
    let base = fixture.candidate();
    let draft = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "draft.forward", "forward")
        .unwrap();
    assert_eq!(context(&draft, "forward")["scope"][0]["ownership"], "value");
    let saved = archive(&draft);
    let unchanged = draft
        .rebase(
            draft.draft_digest(),
            Arc::clone(base.revision()),
            base.revision().project_revision(),
        )
        .unwrap();
    assert_eq!(unchanged.draft().to_json(), draft.to_json());

    // Equal-length field-ID replacement leaves every function's source span,
    // body, parameters, return type, effects, and contracts exactly unchanged.
    let changed = canonical.replace("draft.packet.value", "draft.packet.other");
    assert_ne!(changed, canonical);
    assert_eq!(changed.len(), canonical.len());
    let old_program = semaprax::parse(&canonical, "src/core.spx").unwrap();
    let new_program = semaprax::parse(&changed, "src/core.spx").unwrap();
    assert_eq!(old_program.functions, new_program.functions);
    let old_owner = old_program
        .types
        .iter()
        .find(|owner| owner.stable_id == "draft.packet")
        .unwrap();
    let new_owner = new_program
        .types
        .iter()
        .find(|owner| owner.stable_id == "draft.packet")
        .unwrap();
    assert_eq!(old_owner.stable_id, new_owner.stable_id);
    let semaprax::ast::TypeDeclarationKind::Record { fields: old_fields } = &old_owner.kind else {
        panic!("record")
    };
    let semaprax::ast::TypeDeclarationKind::Record { fields: new_fields } = &new_owner.kind else {
        panic!("record")
    };
    assert_eq!(old_fields[0].ty, new_fields[0].ty);
    assert_ne!(old_fields[0].stable_id, new_fields[0].stable_id);
    std::fs::write(&path, semaprax::format::canonical(&new_program)).unwrap();
    let next = fixture.candidate();
    let disk = fixture.bytes();
    let errors = draft
        .rebase(
            draft.draft_digest(),
            Arc::clone(next.revision()),
            next.revision().project_revision(),
        )
        .err()
        .unwrap();
    assert!(
        errors
            .iter()
            .any(|error| error.code == "SPX-G345" && error.message.contains("nominal owner shape")),
        "{errors:?}"
    );
    assert_eq!(archive(&draft).to_json(), saved.to_json());
    assert_eq!(fixture.bytes(), disk);
}
