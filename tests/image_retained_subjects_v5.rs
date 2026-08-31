//! Selected v5 retained-subject lifecycle evidence; authored and intentionally unrun.
use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use semaprax::project::with_authenticated_project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "workspace/retained-subjects";
const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-retained-subjects-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in FILES {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self, candidates: bool, diagnostics: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: candidates,
                diagnostics,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
            .iter()
            .map(|path| std::fs::read(self.0.join(path)).unwrap())
            .collect()
    }
    fn revision(&self) -> String {
        with_authenticated_project(&self.manifest(), |snapshot| {
            Ok(snapshot.project_revision().to_owned())
        })
        .unwrap()
    }
    fn edit(&self) {
        let path = self.0.join("src/app.spx");
        let source = std::fs::read_to_string(&path)
            .unwrap()
            .replace("multiply(6, 7)", "multiply(6, 8)");
        let parsed = semaprax::parse(&source, "src/app.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"retained","method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(method, params)).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn inventory(session: &mut VNextSession) -> Value {
    let value = payload(bound(session, METHOD, json!({})));
    assert_eq!(value["schema"], "semaprax.image-retained-subjects.v1");
    assert_eq!(value["image_revision"], session.image_revision());
    assert_eq!(value.as_object().unwrap().len(), 12);
    for field in [
        "source_authority",
        "artifact_materialization",
        "execution",
        "publication_authority",
    ] {
        assert_eq!(value[field], false);
    }
    assert!(value["retained_report_bytes"].as_u64().is_some());
    assert_eq!(
        value["limits"],
        json!({"max_candidates":16,"max_drafts":16,"max_attempts":16,"max_retained_report_bytes":268435456,"max_inventory_bytes":65536})
    );
    assert_eq!(
        value["nonclaims"],
        json!([
            "session_inventory_is_not_persistent_storage",
            "registry_association_is_not_ownership_or_current_candidate_validity",
            "drafts_and_rejected_attempts_are_not_checked_candidates",
            "references_grant_no_source_execution_materialization_or_publication_authority"
        ])
    );
    let sum = ["candidates", "drafts", "attempts"]
        .into_iter()
        .flat_map(|name| value[name].as_array().unwrap())
        .map(|row| row["retained_report_bytes"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(value["retained_report_bytes"], json!(sum));
    value
}
fn open(session: &mut VNextSession) -> String {
    payload(bound(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn changed(session: &mut VNextSession, candidate: &str) -> String {
    payload(bound(
        session,
        "candidate/apply-intent",
        json!({"candidate_revision":candidate,"intent":{"kind":"rename_declaration","target":"calculator.add","name":"addition"}}),
    ))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn draft(session: &mut VNextSession, candidate: &str) -> String {
    payload(bound(
        session,
        "hole/open",
        json!({"candidate_revision":candidate,"target":"calculator.add","hole_id":"body"}),
    ))["draft_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn rejected(session: &mut VNextSession, candidate: &str) -> String {
    let result = payload(bound(
        session,
        "candidate/attempt",
        json!({"candidate_revision":candidate,"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":1}}}),
    ));
    assert_eq!(result["status"], "rejected");
    result["attempt"]["attempt_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn selected_inventory_starts_empty_and_tracks_candidates_drafts_attempts_and_orphans() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true, true);
    let empty = inventory(&mut session);
    assert_eq!(empty["candidates"], json!([]));
    assert_eq!(empty["drafts"], json!([]));
    assert_eq!(empty["attempts"], json!([]));

    let root = open(&mut session);
    let child = changed(&mut session, &root);
    let draft = draft(&mut session, &root);
    let attempt = rejected(&mut session, &root);
    let first = inventory(&mut session);
    assert_eq!(first, inventory(&mut session));
    assert_eq!(
        first["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["candidate_revision"].as_str().unwrap())
            .collect::<Vec<_>>(),
        {
            let mut values = vec![root.as_str(), child.as_str()];
            values.sort();
            values
        }
    );
    for row in first["candidates"].as_array().unwrap() {
        assert!(row["retained_report_bytes"].as_u64().unwrap() > 0);
        assert_eq!(row["detail_method"], "candidate/query");
        assert_eq!(row["discard_method"], "candidate/discard");
        let is_root = row["candidate_revision"] == root;
        assert_eq!(row["has_retained_drafts"], is_root);
        assert_eq!(row["has_retained_attempts"], is_root);
    }
    assert_eq!(first["drafts"].as_array().unwrap().len(), 1);
    assert_eq!(first["drafts"][0]["draft_revision"], draft);
    assert_eq!(first["drafts"][0]["source_candidate_revision"], root);
    assert_eq!(first["drafts"][0]["source_candidate_retained"], true);
    assert_eq!(first["drafts"][0]["state"], "incomplete");
    assert_eq!(first["drafts"][0]["unresolved_hole_count"], 1);
    assert_eq!(first["drafts"][0]["detail_method"], "hole/recovery-export");
    assert_eq!(first["drafts"][0]["discard_method"], "hole/discard");
    assert!(
        first["drafts"][0]["retained_report_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(first["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(first["attempts"][0]["attempt_revision"], attempt);
    assert_eq!(first["attempts"][0]["base_candidate_revision"], root);
    assert_eq!(first["attempts"][0]["base_candidate_retained"], true);
    assert_eq!(first["attempts"][0]["state"], "rejected");
    assert!(first["attempts"][0]["diagnostic_count"].as_u64().unwrap() > 0);
    assert!(
        first["attempts"][0]["retained_report_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(first["attempts"][0]["detail_method"], "attempt/query");
    assert_eq!(first["attempts"][0]["discard_method"], "attempt/discard");

    payload(bound(
        &mut session,
        "candidate/discard",
        json!({"candidate_revision":root}),
    ));
    let orphaned = inventory(&mut session);
    assert_eq!(orphaned["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(orphaned["candidates"][0]["candidate_revision"], child);
    assert_eq!(orphaned["candidates"][0]["has_retained_drafts"], false);
    assert_eq!(orphaned["candidates"][0]["has_retained_attempts"], false);
    assert_eq!(orphaned["drafts"][0]["source_candidate_retained"], false);
    assert_eq!(orphaned["attempts"][0]["base_candidate_retained"], false);

    payload(bound(
        &mut session,
        "hole/discard",
        json!({"draft_revision":draft}),
    ));
    payload(bound(
        &mut session,
        "attempt/discard",
        json!({"attempt_revision":attempt}),
    ));
    let retired = inventory(&mut session);
    assert!(retired["drafts"].as_array().unwrap().is_empty());
    assert!(retired["attempts"].as_array().unwrap().is_empty());
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn refresh_clears_drafts_and_attempts_retains_candidates_and_rebinds_the_inventory_image() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true, true);
    let root = open(&mut session);
    let child = changed(&mut session, &root);
    let draft = draft(&mut session, &root);
    let attempt = rejected(&mut session, &root);
    let old_image = session.image_revision().to_owned();
    fixture.edit();
    assert!(call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_some());
    let preview = payload(bound(&mut session, "workspace/refresh-preview", json!({})));
    let expected = preview["observed_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(expected, fixture.revision());
    let refreshed = payload(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":expected}),
    ));
    assert_eq!(refreshed["old_image_revision"], old_image);
    assert_eq!(refreshed["cleared_drafts"], 1);
    assert_eq!(refreshed["cleared_attempts"], 1);
    assert!(refreshed["retained_candidates"]
        .as_array()
        .unwrap()
        .contains(&json!(root)));
    assert!(refreshed["retained_candidates"]
        .as_array()
        .unwrap()
        .contains(&json!(child)));
    let current = inventory(&mut session);
    assert_eq!(current["image_revision"], session.image_revision());
    assert!(current["drafts"].as_array().unwrap().is_empty());
    assert!(current["attempts"].as_array().unwrap().is_empty());
    assert_eq!(current["candidates"].as_array().unwrap().len(), 2);
    assert!(bound(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":"body"})
    )
    .get("error")
    .is_some());
    assert!(bound(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":attempt})
    )
    .get("error")
    .is_some());
    session.finish().unwrap();
}

#[test]
fn discovery_clients_mcp_batch_exclusion_and_live_drift_preserve_the_read_only_boundary() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut absent = fixture.session(false, false);
    let methods = payload(call(&mut absent, "protocol/capabilities", json!({})))["methods"]
        .as_array()
        .unwrap()
        .clone();
    assert!(!methods.contains(&json!(METHOD)));
    assert_eq!(
        call(&mut absent, METHOD, json!({}))["error"]["code"],
        -32601
    );
    let absent_schemas = payload(call(&mut absent, "protocol/schemas", json!({})));
    assert!(!absent_schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["$id"] == "urn:semaprax.image-retained-subjects.v1"));
    absent.finish().unwrap();

    let mut session = fixture
        .session(true, false)
        .with_read_batch_workers(2)
        .unwrap();
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let descriptor = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == METHOD)
        .unwrap();
    assert_eq!(descriptor["capability"], "candidate_prepare");
    assert_eq!(descriptor["query"], true);
    assert_eq!(
        descriptor["request_schema"]["properties"]["params"]["additionalProperties"],
        false
    );
    assert_eq!(
        descriptor["request_schema"]["properties"]["params"]["required"],
        json!(["image_revision"])
    );
    let document = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.image-retained-subjects.v1")
        .unwrap();
    assert_eq!(document["additionalProperties"], false);
    assert_eq!(document["properties"]["source_authority"]["const"], false);
    assert_eq!(
        document["properties"]["artifact_materialization"]["const"],
        false
    );
    assert_eq!(document["properties"]["execution"]["const"], false);
    assert_eq!(
        document["properties"]["publication_authority"]["const"],
        false
    );
    assert!(!session.parallel_read_methods().contains(&METHOD));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        for fragment in [
            "WorkspaceRetainedSubjectsPayload",
            "request_workspace_retained_subjects",
            "decode_request_workspace_retained_subjects",
        ] {
            assert!(source.contains(fragment), "{language} missing {fragment}");
        }
    }
    let inner = String::from_utf8(frame(
        METHOD,
        json!({"image_revision":session.image_revision()}),
    ))
    .unwrap();
    let batch = payload(bound(
        &mut session,
        "workspace/read-batch",
        json!({"batch":{"frames":[inner]}}),
    ));
    let response: Value = serde_json::from_str(batch["responses"][0].as_str().unwrap()).unwrap();
    assert_eq!(response["error"]["code"], -32601);

    let mut mcp = McpSession::new(fixture.session(true, false)).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},
        "clientInfo":{"name":"retained-subject-evidence","version":"1"}}});
    mcp.handle_frame(initialize.to_string().as_bytes()).unwrap();
    assert!(mcp
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    let mut names = Vec::new();
    let mut cursor = None;
    loop {
        let params = cursor
            .as_ref()
            .map_or_else(|| json!({}), |value| json!({"cursor":value}));
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":params});
        let page: Value =
            serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        names.extend(
            page["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_owned()),
        );
        cursor = page["result"]["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(names.contains(&"workspace__retained-subjects".to_owned()));
    mcp.finish().unwrap();

    let source = fixture.0.join("src/core.spx");
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let drifted = fixture.bytes();
    assert_eq!(
        bound(&mut session, METHOD, json!({}))["error"]["code"],
        -32000
    );
    assert!(session.finish().is_err());
    assert_eq!(fixture.bytes(), drifted);
    assert_ne!(fixture.bytes(), disk);
}
