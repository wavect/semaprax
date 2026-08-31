//! Explicit typed-draft rebase transport evidence, authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft,
    ProjectCandidateDraftArchive,
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
            "spx-draft-rebase-v5-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap()
    }
    fn session(&self, prepare: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: prepare,
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
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
        .with_body_hole(draft.draft_digest(), "calculator.add", "add")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "calculator.subtract",
            &selected(base, "calculator.subtract", false, "left - right"),
            "subtract",
        )
        .unwrap();
    let draft = draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            "calculator.divide",
            &selected(base, "calculator.divide", true, "right != 0"),
            "divide",
        )
        .unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.multiply", "multiply")
        .unwrap();
    draft
        .fill_hole(
            draft.draft_digest(),
            "multiply",
            &json!({"kind":"i64","value":17}),
        )
        .unwrap()
}
fn archive(draft: &ProjectCandidateDraft) -> ProjectCandidateDraftArchive {
    ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest()).unwrap()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn restore(session: &mut VNextSession, saved: &ProjectCandidateDraftArchive) -> Value {
    serde_json::from_str(
        &session
            .restore_draft_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.draft_digest(),
            )
            .unwrap(),
    )
    .unwrap()
}
fn context(session: &mut VNextSession, draft: &str, hole: &str) -> Value {
    payload(bound(
        session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":hole}),
    ))
}

#[test]
fn historical_archive_from_deleted_origin_rebases_all_pending_kinds_in_new_session_without_candidate_release(
) {
    let original = Fixture::new();
    let original_path = original.0.clone();
    let base = original.candidate();
    let draft = mixed(&base);
    let saved = archive(&draft);
    drop(original);
    assert!(!original_path.exists());
    let sibling = Fixture::new();
    let path = sibling.0.join("src/app.spx");
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.replace("multiply(6, 7)", "multiply(6, 8)");
    assert_ne!(text, changed);
    let parsed = semaprax::parse(&changed, "src/app.spx").unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    let current = sibling.candidate();
    let disk = sibling.bytes();
    let expected = draft
        .rebase(
            draft.draft_digest(),
            Arc::clone(current.revision()),
            current.revision().project_revision(),
        )
        .unwrap();
    let mut session = sibling.session(true);
    let image = session.image_revision().to_owned();
    let old_handle = restore(&mut session, &saved);
    assert!(bound(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":old_handle["source_candidate_revision"]})
    )
    .get("error")
    .is_some());
    let live = payload(bound(&mut session, "candidate/open", json!({})));
    let response = payload(bound(
        &mut session,
        "hole/rebase",
        json!({"draft_revision":saved.draft_digest(),"new_base_candidate_revision":live["candidate_revision"]}),
    ));
    assert_eq!(response["schema"], "semaprax.image-draft-rebase.v1");
    assert_eq!(
        response["selected_candidate_revision"],
        live["candidate_revision"]
    );
    assert_eq!(
        response["report"],
        serde_json::from_str::<Value>(expected.to_json()).unwrap()
    );
    let handle = &response["draft"];
    assert_eq!(handle["draft_revision"], expected.draft().draft_digest());
    assert_eq!(handle["source_authority"], false);
    assert_eq!(handle["buildable"], false);
    assert_ne!(
        handle["source_candidate_revision"],
        live["candidate_revision"]
    );
    assert!(bound(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":handle["source_candidate_revision"]})
    )
    .get("error")
    .is_some());
    for hole in ["add", "subtract", "divide"] {
        let expected_context: Value = serde_json::from_str(
            &expected
                .draft()
                .hole_context(expected.draft().draft_digest(), hole)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            context(&mut session, expected.draft().draft_digest(), hole),
            expected_context
        );
    }
    assert_eq!(
        session
            .export_draft_archive(&image, saved.draft_digest())
            .unwrap()
            .to_json(),
        saved.to_json()
    );
    let unresolved = bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":handle["draft_revision"]}),
    );
    assert!(unresolved.to_string().contains("SPX-G232"));
    let mut next = handle.clone();
    for (hole, expression) in [
        ("add", json!({"kind":"i64","value":42})),
        ("subtract", json!({"kind":"i64","value":23})),
        ("divide", json!({"kind":"bool","value":true})),
    ] {
        next = payload(bound(
            &mut session,
            "hole/fill",
            json!({"draft_revision":next["draft_revision"],"hole_id":hole,"expression":expression}),
        ));
    }
    let complete = payload(bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":next["draft_revision"]}),
    ));
    assert_eq!(
        complete["base_revision"],
        current.revision().project_revision()
    );
    assert_eq!(complete["source_authority"], false);
    assert_eq!(complete["tests"], "not_run");
    assert_eq!(session.image_revision(), image);
    assert_eq!(sibling.bytes(), disk);
    assert!(!original_path.exists());
}

#[test]
fn conflict_and_stale_requests_preserve_original_draft_and_can_be_followed_by_valid_rebase() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let saved = archive(&draft);
    let mut session = fixture.session(true);
    restore(&mut session, &saved);
    let image = session.image_revision().to_owned();
    let before = context(&mut session, draft.draft_digest(), "add");
    let live = payload(bound(&mut session, "candidate/open", json!({})));
    let competing = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":live["candidate_revision"],"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":99}}}),
    ));
    let conflict = bound(
        &mut session,
        "hole/rebase",
        json!({"draft_revision":draft.draft_digest(),"new_base_candidate_revision":competing["candidate_revision"]}),
    );
    assert!(conflict.to_string().contains("SPX-G345"));
    let wrong = format!("sha256:{}", "0".repeat(64));
    for params in [
        json!({"draft_revision":wrong,"new_base_candidate_revision":live["candidate_revision"]}),
        json!({"draft_revision":draft.draft_digest(),"new_base_candidate_revision":wrong}),
    ] {
        assert!(bound(&mut session, "hole/rebase", params)
            .get("error")
            .is_some());
    }
    let stale = call(
        &mut session,
        "hole/rebase",
        json!({"image_revision":wrong,"draft_revision":draft.draft_digest(),"new_base_candidate_revision":live["candidate_revision"]}),
    );
    assert!(stale.get("error").is_some());
    assert_eq!(context(&mut session, draft.draft_digest(), "add"), before);
    assert_eq!(
        session
            .export_draft_archive(&image, draft.draft_digest())
            .unwrap()
            .to_json(),
        saved.to_json()
    );
    let good = payload(bound(
        &mut session,
        "hole/rebase",
        json!({"draft_revision":draft.draft_digest(),"new_base_candidate_revision":live["candidate_revision"]}),
    ));
    assert_eq!(good["report"]["parent_draft_digest"], draft.draft_digest());
    assert_eq!(good["report"]["holes"].as_array().unwrap().len(), 3);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn rebase_requires_v5_candidate_grant_closed_selection_and_explicit_nonquery_schema() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(false);
    assert_eq!(
        bound(&mut readonly, "hole/rebase", json!({}))["error"]["code"],
        -32601
    );
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let frame=json!({"jsonrpc":"2.0","id":1,"method":"hole/rebase","params":{"image_revision":old.image_revision()}}).to_string();
        let response: Value =
            serde_json::from_slice(&old.handle_frame(frame.as_bytes()).unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
    let mut session = fixture.session(true);
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let method = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "hole/rebase")
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    assert_eq!(method["query"], false);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    for key in [
        "image_revision",
        "draft_revision",
        "new_base_candidate_revision",
    ] {
        assert!(params["required"].as_array().unwrap().contains(&json!(key)));
    }
    assert!(!session.parallel_read_methods().contains(&"hole/rebase"));
    let wrong = format!("sha256:{}", "0".repeat(64));
    for params in [
        json!({"draft_revision":wrong}),
        json!({"draft_revision":wrong,"new_base_candidate_revision":wrong,"allow_conflicts":true}),
    ] {
        assert_eq!(
            bound(&mut session, "hole/rebase", params)["error"]["code"],
            -32602
        );
    }
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        assert!(client["source"].as_str().unwrap().contains("hole/rebase"));
        assert_eq!(client["io"], false);
    }
}
