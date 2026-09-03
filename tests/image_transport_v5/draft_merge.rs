//! Unfinished sibling merge transport evidence, authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
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
            "spx-draft-merge-v5-{}-{}",
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
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
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
fn changed(base: &ProjectCandidate, target: &str, value: i64) -> Arc<ProjectCandidate> {
    let change=SemanticChange::new(base.revision().project_revision(),&json!({"kind":"replace_function_body","target":target,"body":{"kind":"i64","value":value}})).unwrap();
    Arc::new(base.apply(base.candidate_digest(), &change).unwrap())
}
fn body_hole(base: &Arc<ProjectCandidate>, hole: &str) -> ProjectCandidateDraft {
    let draft = ProjectCandidateDraft::open(Arc::clone(base)).unwrap();
    draft
        .with_body_hole(draft.draft_digest(), "calculator.multiply", hole)
        .unwrap()
}
fn archive(draft: &ProjectCandidateDraft) -> ProjectCandidateDraftArchive {
    ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest()).unwrap()
}
fn retain(session: &mut VNextSession, draft: &ProjectCandidateDraft) {
    let saved = archive(draft);
    session
        .restore_draft_archive(
            saved.to_json().as_bytes(),
            saved.archive_digest(),
            saved.draft_digest(),
        )
        .unwrap();
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
fn context(session: &mut VNextSession, draft: &str, hole: &str) -> Value {
    payload(bound(
        session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":hole}),
    ))
}

#[test]
fn merge_unions_all_kinds_with_exact_library_report_and_releases_no_candidate_until_complete() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left_base = changed(&base, "calculator.add", 17);
    let right_base = changed(&base, "calculator.subtract", 23);
    let left = body_hole(&left_base, "multiply");
    let left = left
        .with_expression_hole(
            left.draft_digest(),
            "calculator.is-negative",
            &selected(&left_base, "calculator.is-negative", false, "value < 0"),
            "negative",
        )
        .unwrap();
    let right = body_hole(&right_base, "multiply");
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
    let expected = left
        .merge(left.draft_digest(), &right, right.draft_digest())
        .unwrap();
    let left_saved = archive(&left);
    let right_saved = archive(&right);
    let mut session = fixture.session(true);
    let image = session.image_revision().to_owned();
    retain(&mut session, &left);
    retain(&mut session, &right);
    let response = payload(bound(
        &mut session,
        "hole/merge",
        json!({"draft_revision":left.draft_digest(),"other_draft_revision":right.draft_digest()}),
    ));
    assert_eq!(response["schema"], "semaprax.image-draft-merge.v1");
    assert_eq!(response["left_draft_revision"], left.draft_digest());
    assert_eq!(response["right_draft_revision"], right.draft_digest());
    assert_eq!(
        response["report"],
        serde_json::from_str::<Value>(expected.to_json()).unwrap()
    );
    let handle = &response["draft"];
    assert_eq!(handle["draft_revision"], expected.draft().draft_digest());
    assert_eq!(handle["source_authority"], false);
    assert_eq!(handle["buildable"], false);
    assert!(bound(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":handle["source_candidate_revision"]})
    )
    .get("error")
    .is_some());
    for hole in ["divide", "multiply", "negative", "not"] {
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
            .export_draft_archive(&image, left.draft_digest())
            .unwrap()
            .to_json(),
        left_saved.to_json()
    );
    assert_eq!(
        session
            .export_draft_archive(&image, right.draft_digest())
            .unwrap()
            .to_json(),
        right_saved.to_json()
    );
    let merged_archive = session
        .export_draft_archive(&image, expected.draft().draft_digest())
        .unwrap();
    assert_eq!(
        merged_archive.to_json(),
        archive(expected.draft()).to_json()
    );
    let unresolved = bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":handle["draft_revision"]}),
    );
    assert!(unresolved.to_string().contains("SPX-G232"));
    let mut next = handle.clone();
    for (hole, expression) in [
        ("divide", json!({"kind":"bool","value":true})),
        ("multiply", json!({"kind":"i64","value":42})),
        ("negative", json!({"kind":"bool","value":false})),
        ("not", json!({"kind":"bool","value":false})),
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
    assert_eq!(complete["source_authority"], false);
    assert_eq!(complete["tests"], "not_run");
    assert_eq!(
        complete["base_revision"],
        base.base_revision().project_revision()
    );
    assert!(complete["candidate_revision"].is_string());
    assert_eq!(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":complete["candidate_revision"]})
        )["error"]["code"],
        -32601
    );
    assert_eq!(session.image_revision(), image);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn opposing_history_conflict_and_stale_requests_preserve_both_parents() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = body_hole(&base, "multiply");
    let right = left
        .fill_hole(
            left.draft_digest(),
            "multiply",
            &json!({"kind":"i64","value":9}),
        )
        .unwrap();
    let left_saved = archive(&left);
    let right_saved = archive(&right);
    let mut session = fixture.session(true);
    retain(&mut session, &left);
    retain(&mut session, &right);
    let image = session.image_revision().to_owned();
    let before = context(&mut session, left.draft_digest(), "multiply");
    let conflict = bound(
        &mut session,
        "hole/merge",
        json!({"draft_revision":left.draft_digest(),"other_draft_revision":right.draft_digest()}),
    );
    assert!(conflict.to_string().contains("SPX-G348"));
    let wrong = format!("sha256:{}", "0".repeat(64));
    for params in [
        json!({"draft_revision":wrong,"other_draft_revision":right.draft_digest()}),
        json!({"draft_revision":left.draft_digest(),"other_draft_revision":wrong}),
    ] {
        assert!(bound(&mut session, "hole/merge", params)
            .get("error")
            .is_some());
    }
    assert!(call(&mut session,"hole/merge",json!({"image_revision":wrong,"draft_revision":left.draft_digest(),"other_draft_revision":right.draft_digest()})).get("error").is_some());
    assert_eq!(
        context(&mut session, left.draft_digest(), "multiply"),
        before
    );
    assert_eq!(
        session
            .export_draft_archive(&image, left.draft_digest())
            .unwrap()
            .to_json(),
        left_saved.to_json()
    );
    assert_eq!(
        session
            .export_draft_archive(&image, right.draft_digest())
            .unwrap()
            .to_json(),
        right_saved.to_json()
    );
    let self_merge = payload(bound(
        &mut session,
        "hole/merge",
        json!({"draft_revision":left.draft_digest(),"other_draft_revision":left.draft_digest()}),
    ));
    // `Preserve typed draft branch lineage` records every merge in the recovery
    // capsule's branch ancestry, so even a self-merge advances the revision.
    // Assert the recorded lineage rather than the pre-lineage identity.
    let merged = self_merge["draft"]["draft_revision"].as_str().unwrap();
    assert_ne!(merged, left.draft_digest());
    let archive: Value = serde_json::from_str(
        session
            .export_draft_archive(&image, merged)
            .unwrap()
            .to_json(),
    )
    .unwrap();
    let capsule: Value =
        serde_json::from_str(archive["draft_recovery_capsule"].as_str().unwrap()).unwrap();
    assert_eq!(
        capsule["branch_ancestry"],
        json!([{
            "onto_revision":Value::Null,
            "operation":"merge",
            "parents":[left.draft_digest(), left.draft_digest()],
        }])
    );
    assert_eq!(
        self_merge["report"]["holes"][0]["parents"],
        json!(["left", "right"])
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn merge_is_candidate_granted_v5_nonquery_with_closed_parent_selectors_and_no_batch_admission() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(false);
    assert_eq!(
        bound(&mut readonly, "hole/merge", json!({}))["error"]["code"],
        -32601
    );
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let frame=json!({"jsonrpc":"2.0","id":1,"method":"hole/merge","params":{"image_revision":old.image_revision()}}).to_string();
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
        .find(|method| method["method"] == "hole/merge")
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    assert_eq!(method["query"], false);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    for key in ["image_revision", "draft_revision", "other_draft_revision"] {
        assert!(params["required"].as_array().unwrap().contains(&json!(key)));
    }
    assert!(!session.parallel_read_methods().contains(&"hole/merge"));
    let wrong = format!("sha256:{}", "0".repeat(64));
    for params in [
        json!({"draft_revision":wrong}),
        json!({"draft_revision":wrong,"other_draft_revision":wrong,"discard_conflicts":true}),
    ] {
        assert_eq!(
            bound(&mut session, "hole/merge", params)["error"]["code"],
            -32602
        );
    }
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        assert!(client["source"].as_str().unwrap().contains("hole/merge"));
        assert_eq!(client["io"], false);
    }
}
