//! Compact hole navigation transport evidence, authored and intentionally unrun.
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
            "spx-hole-navigation-v5-{}-{}",
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
        let base = with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap();
        let expression = selected(&base, "calculator.is-negative", false, "value < 0");
        let contract = selected(&base, "calculator.divide", true, "right != 0");
        let draft = ProjectCandidateDraft::open(base).unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.add", "body")
            .unwrap();
        let draft = draft
            .with_expression_hole(
                draft.draft_digest(),
                "calculator.is-negative",
                &expression,
                "expression",
            )
            .unwrap();
        draft
            .with_contract_expression_hole(
                draft.draft_digest(),
                "calculator.divide",
                &contract,
                "contract",
            )
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
fn selected(base: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let catalog: Value = serde_json::from_str(
        &if contract {
            base.contract_expression_catalog(target)
        } else {
            base.expression_catalog(target)
        }
        .unwrap(),
    )
    .unwrap();
    let source = base
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows = catalog["expressions"]
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
fn frame(id: usize, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if !matches!(method, "protocol/schemas" | "protocol/client") {
        params["image_revision"] = json!(session.image_revision());
    }
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn summary(draft: &ProjectCandidateDraft, hole: &str) -> Value {
    serde_json::from_str(&draft.hole_summary(draft.draft_digest(), hole).unwrap()).unwrap()
}
fn reference(summary: &Value, facet: &str) -> String {
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["facet"] == facet)
        .unwrap()["reference"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn summary_and_pages_match_the_library_while_old_full_context_and_draft_stay_unchanged() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let draft = fixture.draft();
    let before = draft.to_json().to_owned();
    let mut session = fixture.session();
    for hole in ["body", "expression", "contract"] {
        let params = json!({"draft_revision":draft.draft_digest(),"hole_id":hole});
        let full = call(&mut session, "hole/query", params.clone());
        let expected = summary(&draft, hole);
        let actual = payload(call(&mut session, "hole/summary", params.clone()));
        assert_eq!(actual, expected);
        assert_eq!(actual["source_authority"], false);
        assert!(actual.get("chunk").is_none());
        for facet in ["scope", "calls", "obligations", "constructors"] {
            let reference = reference(&actual, facet);
            let mut offset = 0usize;
            loop {
                let page = payload(call(
                    &mut session,
                    "hole/page",
                    json!({"draft_revision":draft.draft_digest(),"hole_id":hole,"reference":reference,"offset":offset,"limit":1}),
                ));
                let expected: Value = serde_json::from_str(
                    &draft
                        .hole_page(draft.draft_digest(), hole, &reference, offset, 1)
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(page, expected);
                assert!(serde_json::to_vec(&page).unwrap().len() < 65536);
                assert_eq!(page["source_authority"], false);
                let Some(next) = page["next_offset"].as_u64() else {
                    assert!(page["next_offset"].is_null());
                    break;
                };
                assert!(next as usize > offset);
                offset = next as usize;
            }
        }
        assert_eq!(call(&mut session, "hole/query", params), full);
    }
    let handle: Value = serde_json::from_str(draft.summary(draft.draft_digest()).unwrap()).unwrap();
    assert!(call(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":handle["last_valid_candidate_digest"]})
    )
    .get("error")
    .is_some());
    assert!(call(
        &mut session,
        "hole/complete",
        json!({"draft_revision":draft.draft_digest()})
    )
    .get("error")
    .is_some());
    assert_eq!(draft.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn host_selected_schema_and_all_client_languages_describe_only_available_navigation() {
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
        for method in ["hole/summary", "hole/page"] {
            let found = bundle["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["method"] == method);
            assert_eq!(found.is_some(), prepare);
            if let Some(method) = found {
                assert_eq!(method["query"], true);
                assert_eq!(method["capability"], "candidate_prepare");
                assert_eq!(
                    method["request_schema"]["properties"]["params"]["additionalProperties"],
                    false
                );
            }
        }
        if prepare {
            let descriptor = bundle["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["method"] == "hole/page")
                .unwrap();
            let params = &descriptor["request_schema"]["properties"]["params"];
            assert_eq!(params["properties"]["offset"]["maximum"], 16384);
            assert_eq!(params["properties"]["limit"]["minimum"], 1);
            assert_eq!(params["properties"]["limit"]["maximum"], 64);
            assert!(!params["required"]
                .as_array()
                .unwrap()
                .contains(&json!("offset")));
            assert!(!params["required"]
                .as_array()
                .unwrap()
                .contains(&json!("limit")));
            for schema in [
                "urn:semaprax.project-hole-summary.v1",
                "urn:semaprax.project-hole-page.v1",
            ] {
                let doc = bundle["documents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|doc| doc["$id"] == schema)
                    .unwrap();
                if schema == "urn:semaprax.project-hole-page.v1" {
                    let branches = doc["oneOf"].as_array().unwrap();
                    assert_eq!(branches.len(), 4);
                    for (branch, facet) in
                        branches
                            .iter()
                            .zip(["scope", "calls", "obligations", "constructors"])
                    {
                        assert_eq!(branch["additionalProperties"], false);
                        assert_eq!(branch["properties"]["source_authority"]["const"], false);
                        assert_eq!(branch["properties"]["facet"]["const"], facet);
                        assert_eq!(branch["properties"]["items"]["maxItems"], 64);
                    }
                } else {
                    assert_eq!(doc["additionalProperties"], false);
                    assert_eq!(doc["properties"]["source_authority"]["const"], false);
                }
                assert!(!bundle["unbundled_payload_schemas"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(schema)));
            }
        }
        for language in ["typescript", "python", "rust"] {
            let generated = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = generated["source"].as_str().unwrap();
            for name in [
                "HoleSummaryPayload",
                "HolePagePayload",
                "HoleSummaryTypedParams",
                "HolePageTypedParams",
                "request_hole_summary_typed",
                "request_hole_page_typed",
                "decode_request_hole_summary_typed",
                "decode_request_hole_page_typed",
            ] {
                assert_eq!(source.contains(name), prepare, "{language}: {name}");
            }
        }
        if !prepare {
            assert!(call(&mut session, "hole/summary", json!({"draft_revision":"sha256:0000000000000000000000000000000000000000000000000000000000000000","hole_id":"body"})).get("error").is_some());
        }
        session.finish().unwrap();
    }
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        for method in ["hole/summary", "hole/page"] {
            let response: Value =
                serde_json::from_slice(&old.handle_frame(&frame(1, method, json!({}))).unwrap())
                    .unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
}

#[test]
fn all_three_holes_have_exact_sequential_batch_parity_and_foreign_refs_fail_inside_the_batch() {
    let fixture = Fixture::new();
    let draft = fixture.draft();
    let mut sequential = fixture.session();
    let image = sequential.image_revision().to_owned();
    let mut frames = Vec::new();
    for hole in ["body", "expression", "contract"] {
        let compact = summary(&draft, hole);
        for method in ["hole/summary", "hole/query"] {
            frames.push(frame(frames.len()+1, method, json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":hole})));
        }
        frames.push(frame(frames.len()+1, "hole/page", json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":hole,"reference":reference(&compact,"scope"),"limit":1})));
    }
    let expected = frames
        .iter()
        .map(|frame| sequential.handle_frame(frame))
        .collect::<Vec<_>>();
    for response in &expected {
        let value: Value = serde_json::from_slice(response.as_ref().unwrap()).unwrap();
        assert!(value.get("error").is_none(), "{value}");
    }
    let borrowed = frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        let mut parallel = fixture.session();
        assert_eq!(
            parallel.handle_read_batch(&borrowed, workers).unwrap(),
            expected
        );
        parallel.finish().unwrap();
    }
    let foreign = reference(&summary(&draft, "body"), "scope");
    let bad = frame(
        20,
        "hole/page",
        json!({"image_revision":image,"draft_revision":draft.draft_digest(),"hole_id":"contract","reference":foreign}),
    );
    let expected_bad = sequential.handle_frame(&bad);
    let value: Value = serde_json::from_slice(expected_bad.as_ref().unwrap()).unwrap();
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G232"));
    let mut parallel = fixture.session();
    assert_eq!(
        parallel.handle_read_batch(&[bad.as_slice()], 2).unwrap(),
        vec![expected_bad]
    );
    sequential.finish().unwrap();
    parallel.finish().unwrap();
}

#[test]
fn source_drift_invalidates_even_unknown_draft_navigation_and_cannot_be_hidden_by_restoring_bytes()
{
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let image = session.image_revision().to_owned();
    let request = frame(
        1,
        "hole/summary",
        json!({"image_revision":image,"draft_revision":"sha256:0000000000000000000000000000000000000000000000000000000000000000","hole_id":"unknown"}),
    );
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid source drift\n").unwrap();
    let errors = session
        .handle_read_batch(&[request.as_slice()], 2)
        .unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-J102"),
        "{errors:?}"
    );
    std::fs::write(path, original).unwrap();
    assert!(session.handle_read_batch(&[request.as_slice()], 1).is_err());
    assert!(session.finish().is_err());
}
