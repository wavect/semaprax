//! Stable field display changes across pending draft regions; authored, unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, SemanticChange,
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
            "spx-draft-field-display-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "draft-field-display"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "draft_field.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["field.public"]
tests = ["draft_field.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module draft_field.core;
@id("field.pair") record Pair {
    @id("field.pair.left") left: i64,
    @id("field.pair.right") right: i64,
}
@id("field.body") fn body(value: Pair)->i64 {value.left + value.right}
@id("field.expression") fn expression(value: Pair)->i64 {value.left + 1}
@id("field.contract") fn checked(value: Pair)->i64 requires value.left >= 0 {value.left}
@id("field.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/app.spx",
                "module draft_field.app;\n@id(\"field.main\") fn main()->i64 {0}\n",
            ),
            (
                "src/tests.spx",
                "module draft_field.tests;\n@id(\"field.test\") fn main()->i64 {0}\n",
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn base(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap()
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
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
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(base: &ProjectCandidate, intention: Value) -> ProjectCandidate {
    let change = SemanticChange::new(base.revision().project_revision(), &intention).unwrap();
    base.apply(base.candidate_digest(), &change).unwrap()
}
fn rename() -> Value {
    json!({"kind":"rename_declaration","target":"field.pair.left","name":"first"})
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn selected(base: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let text = if contract {
        base.contract_expression_catalog(target)
    } else {
        base.expression_catalog(target)
    }
    .unwrap();
    let catalogue: Value = serde_json::from_str(&text).unwrap();
    let source = base
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows = catalogue["expressions"]
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
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
fn mixed(base: &Arc<ProjectCandidate>) -> ProjectCandidateDraft {
    let draft = ProjectCandidateDraft::open(Arc::clone(base)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "field.body", "a.body")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "field.expression",
            &selected(base, "field.expression", false, "value.left + 1"),
            "b.expression",
        )
        .unwrap();
    draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            "field.contract",
            &selected(base, "field.contract", true, "value.left >= 0"),
            "c.contract",
        )
        .unwrap()
}
fn context(draft: &ProjectCandidateDraft, hole: &str) -> String {
    draft.hole_context(draft.draft_digest(), hole).unwrap()
}
fn reject<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|e| e.code == code), "{errors:?}"),
    }
}

#[test]
fn field_display_rebase_readmits_body_expression_and_contract_selectors_without_release() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let draft = mixed(&base);
    let old_context = context(&draft, "b.expression");
    let renamed = apply(&base, rename());
    let replay = draft
        .rebase(
            draft.draft_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        )
        .unwrap();
    let report: Value = serde_json::from_str(replay.to_json()).unwrap();
    assert_eq!(report["materializable"], false);
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["holes"].as_array().unwrap().len(), 3);
    for row in report["holes"].as_array().unwrap() {
        assert_eq!(row["context_refreshed"], true);
        assert_eq!(row["concurrent_body_change"], false);
        assert_eq!(row["concurrent_contract_change"], false);
    }
    let next = replay.into_draft();
    assert_ne!(context(&next, "b.expression"), old_context);
    assert_eq!(context(&draft, "b.expression"), old_context);
    assert!(next.complete(next.draft_digest()).is_err());
    let catalogue: Value = serde_json::from_str(
        &next
            .expression_catalog(next.draft_digest(), "field.expression")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(catalogue["draft_revision"], next.draft_digest());
    let next = next
        .fill_hole(next.draft_digest(), "a.body", &integer(7))
        .unwrap();
    let next = next
        .fill_hole(next.draft_digest(), "b.expression", &integer(8))
        .unwrap();
    let next = next
        .fill_hole(
            next.draft_digest(),
            "c.contract",
            &json!({"kind":"bool","value":true}),
        )
        .unwrap();
    let completed = next.complete(next.draft_digest()).unwrap();
    assert!(completed
        .revision()
        .sources()
        .iter()
        .any(|s| s.source().contains("first: i64")));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn draft_merge_uses_same_stable_owner_guard_and_remaps_pending_union() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let left = mixed(&base);
    let renamed = Arc::new(apply(&base, rename()));
    let right = ProjectCandidateDraft::open(renamed).unwrap();
    let merged = left
        .merge(left.draft_digest(), &right, right.draft_digest())
        .unwrap();
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(report["holes"].as_array().unwrap().len(), 3);
    for row in report["holes"].as_array().unwrap() {
        assert_eq!(row["parents"], json!(["left"]));
    }
    assert!(merged
        .draft()
        .complete(merged.draft().draft_digest())
        .is_err());
    assert!(context(merged.draft(), "a.body").contains("first"));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stable_field_normalization_does_not_erase_changed_selection_or_stale_draft() {
    let fixture = Fixture::new();
    let base = fixture.base();
    let draft = mixed(&base);
    let before = context(&draft, "b.expression");
    let changed = apply(
        &base,
        json!({"kind":"replace_function_body","target":"field.expression","body":{"kind":"binary","op":"+","left":{"kind":"field_place","target":"field.pair.right","root":"value"},"right":integer(1)}}),
    );
    reject(
        draft.rebase(
            draft.draft_digest(),
            Arc::clone(changed.revision()),
            changed.revision().project_revision(),
        ),
        "SPX-G345",
    );
    let other = ProjectCandidateDraft::open(Arc::new(changed)).unwrap();
    reject(
        draft.merge(draft.draft_digest(), &other, other.draft_digest()),
        "SPX-G348",
    );
    reject(
        draft.rebase(
            &format!("sha256:{}", "0".repeat(64)),
            Arc::clone(base.revision()),
            base.revision().project_revision(),
        ),
        "SPX-G232",
    );
    assert_eq!(context(&draft, "b.expression"), before);
}

fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if params.get("image_revision").is_none() {
        params["image_revision"] = json!(session.image_revision());
    }
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn payload(value: Value) -> Value {
    assert!(value.get("error").is_none(), "{value}");
    value["result"]["payload"].clone()
}

#[test]
fn transport_field_rename_rebase_retains_only_draft_and_preserves_original_on_rejection() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let mut session = fixture.session();
    let candidate =
        payload(call(&mut session, "candidate/open", json!({})))["candidate_revision"].clone();
    let draft = payload(call(&mut session, "hole/open-expression", json!({"candidate_revision":candidate,"target":"field.expression","expression_id":selected(&base,"field.expression",false,"value.left + 1"),"hole_id":"pending"})))["draft_revision"].clone();
    let before = payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":"pending"}),
    ));
    let changed = payload(call(&mut session,"candidate/apply-intent",json!({"candidate_revision":candidate,"intent":{"kind":"replace_function_body","target":"field.expression","body":integer(99)}})))["candidate_revision"].clone();
    let conflict = call(
        &mut session,
        "hole/rebase",
        json!({"draft_revision":draft,"new_base_candidate_revision":changed}),
    );
    assert!(conflict.to_string().contains("SPX-G345"));
    assert_eq!(
        payload(call(
            &mut session,
            "hole/query",
            json!({"draft_revision":draft,"hole_id":"pending"})
        )),
        before
    );
    let renamed = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":candidate,"intent":rename()}),
    ))["candidate_revision"]
        .clone();
    let next = payload(call(
        &mut session,
        "hole/rebase",
        json!({"draft_revision":draft,"new_base_candidate_revision":renamed}),
    ));
    assert_eq!(next["schema"], "semaprax.image-draft-rebase.v1");
    assert!(next.get("candidate_revision").is_none());
    let next_draft = next["draft"]["draft_revision"].clone();
    let summary = payload(call(
        &mut session,
        "hole/summary",
        json!({"draft_revision":next_draft,"hole_id":"pending"}),
    ));
    assert_eq!(summary["draft_revision"], next_draft);
    let wrong = format!("sha256:{}", "0".repeat(64));
    assert!(call(
        &mut session,
        "hole/rebase",
        json!({"draft_revision":wrong,"new_base_candidate_revision":renamed})
    )
    .to_string()
    .contains("SPX-G232"));
    assert!(call(
        &mut session,
        "hole/rebase",
        json!({"image_revision":wrong,"draft_revision":draft,"new_base_candidate_revision":renamed})
    )
    .to_string()
    .contains("SPX-G282"));
    assert_eq!(fixture.bytes(), disk);
}
