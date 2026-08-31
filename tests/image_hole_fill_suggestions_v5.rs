//! Read-only hole-fill preview transport evidence, authored and unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, ProjectCandidateDraft};
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
            "spx-fill-suggestions-v5-{}-{}",
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
    fn draft(&self) -> ProjectCandidateDraft {
        let candidate = with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap();
        let draft = ProjectCandidateDraft::open(candidate).unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.add", "body")
            .unwrap();
        draft
            .with_body_hole(draft.draft_digest(), "calculator.multiply", "other")
            .unwrap()
    }
    fn session(&self) -> VNextSession {
        let mut session = VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap()
        .with_read_batch_workers(2)
        .unwrap();
        let draft = self.draft();
        let digest = draft.draft_digest().to_owned();
        session.retain_archived_draft(draft, &digest).unwrap();
        session
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
fn frame(id: usize, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if !method.starts_with("protocol/") && method != "query/catalog" {
        params["image_revision"] = json!(session.image_revision());
    }
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn failure(response: &Value, code: &str) {
    assert!(response.get("result").is_none(), "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains(code),
        "{response}"
    );
}

#[test]
fn pure_suggestion_queries_match_library_and_both_batch_paths_without_retaining_preview_drafts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let draft = fixture.draft();
    let before = draft.to_json().to_owned();
    let expected: Value = serde_json::from_str(
        &draft
            .hole_fill_suggestions(draft.draft_digest(), "body")
            .unwrap(),
    )
    .unwrap();
    let mut session = fixture.session();
    let original = call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft.draft_digest(),"hole_id":"body"}),
    );
    let params = json!({"draft_revision":draft.draft_digest(),"hole_id":"body"});
    assert_eq!(
        payload(call(&mut session, "hole/fill-suggestions", params.clone())),
        expected
    );
    assert!(session
        .parallel_read_methods()
        .contains(&"hole/fill-suggestions"));
    let image = session.image_revision().to_owned();
    let requests = [
        frame(
            3,
            "hole/fill-suggestions",
            json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":"body"}),
        ),
        frame(
            2,
            "hole/summary",
            json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":"other"}),
        ),
    ];
    let sequential = requests
        .iter()
        .map(|request| session.handle_frame(request))
        .collect::<Vec<_>>();
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            session.handle_read_batch(&refs, workers).unwrap(),
            sequential
        );
    }
    let raw = requests
        .iter()
        .map(|request| String::from_utf8(request.clone()).unwrap())
        .collect::<Vec<_>>();
    let batch = payload(call(
        &mut session,
        "workspace/read-batch",
        json!({"batch":{"frames":raw}}),
    ));
    for (row, expected) in batch["responses"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&sequential)
    {
        assert_eq!(row.as_str().unwrap().as_bytes(), expected.as_ref().unwrap());
    }
    assert!(!expected["suggestions"].as_array().unwrap().is_empty());
    for suggestion in expected["suggestions"].as_array().unwrap() {
        let preview = draft
            .fill_hole(draft.draft_digest(), "body", &suggestion["expression"])
            .unwrap();
        assert_eq!(suggestion["preview_draft_revision"], preview.draft_digest());
        // The unfilled second hole would be queryable if this preview had been
        // retained. A missing draft selector must fail before inspecting it.
        failure(
            &call(
                &mut session,
                "hole/query",
                json!({"draft_revision":preview.draft_digest(),"hole_id":"other"}),
            ),
            "SPX-G232",
        );
        let summary: Value = serde_json::from_str(preview.to_json()).unwrap();
        failure(
            &call(
                &mut session,
                "candidate/query",
                json!({"candidate_revision":summary["last_valid_candidate_digest"]}),
            ),
            "SPX-G224",
        );
    }
    assert_eq!(call(&mut session, "hole/query", params.clone()), original);
    failure(
        &call(
            &mut session,
            "hole/complete",
            json!({"draft_revision":draft.draft_digest()}),
        ),
        "SPX-G232",
    );
    // Applying a selected expression is a separate explicit mutation. Only
    // this call installs its preview, and completion remains separately gated.
    let selected = &expected["suggestions"][0];
    let filled = payload(call(
        &mut session,
        "hole/fill",
        json!({"draft_revision":draft.draft_digest(),"hole_id":"body","expression":selected["expression"]}),
    ));
    assert_eq!(filled["draft_revision"], selected["preview_draft_revision"]);
    payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":filled["draft_revision"],"hole_id":"other"}),
    ));
    assert_eq!(draft.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn selection_schemas_and_generated_helpers_are_narrow_and_candidate_granted_only() {
    let fixture = Fixture::new();
    for prepare in [false, true] {
        let mut session = VNextSession::open(
            &fixture.manifest(),
            VNextPolicy {
                candidate_prepare: prepare,
                ..Default::default()
            },
        )
        .unwrap();
        let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == "hole/fill-suggestions");
        assert_eq!(method.is_some(), prepare);
        let report = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["$id"] == "urn:semaprax.project-hole-fill-suggestions.v1");
        assert_eq!(report.is_some(), prepare);
        if let Some(method) = method {
            assert_eq!(method["capability"], "candidate_prepare");
            assert_eq!(method["query"], true);
            let report = report.unwrap();
            assert_eq!(report["additionalProperties"], false);
            assert_eq!(report["required"].as_array().unwrap().len(), 15);
            assert_eq!(report["properties"]["source_authority"]["const"], false);
            assert_eq!(report["properties"]["draft_retained"]["const"], false);
            assert_eq!(report["properties"]["considered"]["maximum"], 32);
            assert_eq!(report["properties"]["suggestions"]["maxItems"], 32);
            let expression =
                &report["properties"]["suggestions"]["items"]["properties"]["expression"];
            let branches = expression["oneOf"].as_array().unwrap();
            assert_eq!(branches.len(), 2);
            assert_eq!(
                branches
                    .iter()
                    .map(|row| row["properties"]["kind"]["const"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["place", "call"]
            );
            for branch in branches {
                assert_eq!(branch["additionalProperties"], false);
            }
            let call = branches
                .iter()
                .find(|row| row["properties"]["kind"]["const"] == "call")
                .unwrap();
            assert_eq!(call["properties"]["arguments"]["maxItems"], 64);
            assert_eq!(
                call["properties"]["arguments"]["items"]["properties"]["kind"]["const"],
                "place"
            );
        }
        for language in ["rust", "typescript", "python"] {
            let generated = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = generated["source"].as_str().unwrap();
            for name in [
                "HoleFillSuggestionsPayload",
                "HoleFillSuggestionsResult",
                "request_hole_fill_suggestions_typed",
                "decode_request_hole_fill_suggestions_typed",
            ] {
                assert_eq!(source.contains(name), prepare, "{language}: {name}");
            }
            assert_eq!(generated["io"], false);
        }
        if !prepare {
            assert_eq!(
                call(
                    &mut session,
                    "hole/fill-suggestions",
                    json!({"draft_revision":fixture.draft().draft_digest(),"hole_id":"body"})
                )["error"]["code"],
                -32601
            );
        }
    }
    let mut old =
        ImageSession::open(&fixture.manifest(), ImageHostCapability::read_only()).unwrap();
    let request = frame(1, "hole/fill-suggestions", json!({}));
    let response: Value = serde_json::from_slice(&old.handle_frame(&request).unwrap()).unwrap();
    assert_eq!(response["error"]["code"], -32601);
}

#[test]
fn stale_foreign_malformed_and_drifted_suggestion_queries_fail_without_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let draft = fixture.draft();
    let mut session = fixture.session();
    let before = call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft.draft_digest(),"hole_id":"body"}),
    );
    let stale = format!("sha256:{}", "0".repeat(64));
    failure(
        &call(
            &mut session,
            "hole/fill-suggestions",
            json!({"draft_revision":stale,"hole_id":"body"}),
        ),
        "SPX-G232",
    );
    failure(
        &call(
            &mut session,
            "hole/fill-suggestions",
            json!({"draft_revision":draft.draft_digest(),"hole_id":"unknown"}),
        ),
        "SPX-G230",
    );
    for extra in [
        json!({"limit":33}),
        json!({"expression":{"kind":"i64","value":0}}),
        json!({"execute":true}),
    ] {
        let mut params = json!({"draft_revision":draft.draft_digest(),"hole_id":"body"});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(
            call(&mut session, "hole/fill-suggestions", params)["error"]["code"],
            -32602
        );
    }
    let stale_image = frame(
        1,
        "hole/fill-suggestions",
        json!({"image_revision":stale,"draft_revision":draft.draft_digest(),"hole_id":"body"}),
    );
    let response: Value =
        serde_json::from_slice(&session.handle_frame(&stale_image).unwrap()).unwrap();
    failure(&response, "SPX-G282");
    assert_eq!(
        call(
            &mut session,
            "hole/query",
            json!({"draft_revision":draft.draft_digest(),"hole_id":"body"})
        ),
        before
    );
    assert_eq!(fixture.bytes(), disk);
    let path = fixture.0.join("src/app.spx");
    std::fs::write(&path, b"manual source drift\n").unwrap();
    let response = call(
        &mut session,
        "hole/fill-suggestions",
        json!({"draft_revision":draft.draft_digest(),"hole_id":"body"}),
    );
    assert!(response.get("result").is_none());
    assert!(response.get("error").is_some());
    std::fs::write(&path, &disk[1]).unwrap();
    assert!(session.finish().is_err());
}
