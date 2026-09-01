//! Candidate-bound compact function facets; authored and intentionally unrun.
use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ImageFacet, ImageFacetOptions, ProjectCandidate, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SUMMARY: &str = "candidate/function-summary";
const FACET: &str = "candidate/function-facet";
const FACETS: [&str; 9] = [
    "signature",
    "contracts",
    "callers",
    "ownership",
    "loans",
    "cleanup",
    "relationships",
    "data-access",
    "unsafe-boundaries",
];
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
            "spx-candidate-function-facets-{}-{}",
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
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"candidate-facet","method":method,"params":params})
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
fn open(session: &mut VNextSession) -> String {
    payload(bound(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn apply(candidate: &ProjectCandidate, intent: &Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), intent).unwrap(),
        )
        .unwrap()
}
fn apply_wire(session: &mut VNextSession, candidate: &str, intent: &Value) -> String {
    payload(bound(
        session,
        "candidate/apply-intent",
        json!({"candidate_revision":candidate,"intent":intent}),
    ))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn summary(session: &mut VNextSession, candidate: &str, target: &str) -> Value {
    payload(bound(
        session,
        SUMMARY,
        json!({"candidate_revision":candidate,"target":target}),
    ))
}
#[allow(clippy::too_many_arguments)]
fn facet(
    session: &mut VNextSession,
    candidate: &str,
    target: &str,
    name: &str,
    handle: &str,
    cursor: Option<&str>,
    page_size: usize,
    max_bytes: usize,
) -> Value {
    let mut params = json!({"candidate_revision":candidate,"target":target,"facet":name,
        "handle":handle,"page_size":page_size,"max_bytes":max_bytes});
    if let Some(cursor) = cursor {
        params["cursor"] = json!(cursor);
    }
    payload(bound(session, FACET, params))
}
fn handle(summary: &Value, name: &str) -> String {
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["facet"] == name)
        .unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn error_code(response: Value, code: &str) {
    assert!(response.get("result").is_none(), "{response}");
    assert!(response["error"].to_string().contains(code), "{response}");
}

#[test]
fn changed_and_added_functions_preserve_all_nine_exact_candidate_facet_inventories() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let rename = json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"});
    let added = json!({"kind":"add_declaration","target":"calculator.add","declaration":{
        "id":"calculator.added","name":"added","parameters":[{"name":"value","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[],"body":{"kind":"place","name":"value"}}});
    let record = json!({"kind":"add_declaration","target":"calculator.add","declaration":{
        "kind":"record","id":"calculator.record","name":"AddedRecord",
        "fields":[{"id":"calculator.record.value","name":"value","type":"i64"}]}});
    let changed = apply(&base, &rename);
    let changed = apply(&changed, &added);
    let changed = apply(&changed, &record);
    let changed_json = changed.to_json().to_owned();

    let mut session = fixture.session();
    let root = open(&mut session);
    let selected = apply_wire(&mut session, &root, &rename);
    let selected = apply_wire(&mut session, &selected, &added);
    let selected = apply_wire(&mut session, &selected, &record);
    assert_eq!(selected, changed.candidate_digest());

    for target in ["calculator.add", "calculator.added"] {
        let wire = summary(&mut session, &selected, target);
        let library: Value = serde_json::from_str(
            &changed
                .function_summary(changed.candidate_digest(), target)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(wire, library);
        assert_eq!(wire["candidate_revision"], selected);
        assert_eq!(
            wire["base_project_revision"],
            base.revision().project_revision()
        );
        assert_eq!(
            wire["facets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["facet"].as_str().unwrap())
                .collect::<Vec<_>>(),
            FACETS
        );
        for flag in [
            "source_authority",
            "target_execution",
            "candidate_retained",
            "execution",
            "publication_authority",
        ] {
            assert_eq!(wire[flag], false);
        }
        for name in FACETS {
            let token = handle(&wire, name);
            let facet_kind = ImageFacet::parse(name).unwrap();
            let mut cursor = None::<String>;
            let mut actual = Vec::new();
            loop {
                let page = facet(
                    &mut session,
                    &selected,
                    target,
                    name,
                    &token,
                    cursor.as_deref(),
                    2,
                    1024 * 1024,
                );
                let expected: Value = serde_json::from_str(
                    &changed
                        .expand_function_facet(
                            changed.candidate_digest(),
                            target,
                            facet_kind,
                            &token,
                            cursor.as_deref(),
                            ImageFacetOptions::new(2, 1024 * 1024).unwrap(),
                        )
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(page, expected);
                for flag in [
                    "source_authority",
                    "target_execution",
                    "candidate_retained",
                    "execution",
                    "publication_authority",
                ] {
                    assert_eq!(page[flag], false);
                }
                for item in page["items"].as_array().unwrap() {
                    assert_eq!(
                        item["schema"],
                        "semaprax.project-candidate-function-facet-item.v1"
                    );
                    assert!(!item["value"].is_null());
                    actual.push(item["value"].clone());
                }
                cursor = page["next_cursor"].as_str().map(str::to_owned);
                if cursor.is_none() {
                    assert_eq!(actual.len(), page["total_items"].as_u64().unwrap() as usize);
                    break;
                }
            }
        }
    }
    error_code(
        bound(
            &mut session,
            SUMMARY,
            json!({"candidate_revision":selected,"target":"calculator.record"}),
        ),
        "SPX-G227",
    );
    error_code(
        bound(
            &mut session,
            SUMMARY,
            json!({"candidate_revision":selected,"target":"calculator.removed"}),
        ),
        "SPX-G227",
    );
    assert_eq!(changed.to_json(), changed_json);
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn handles_and_cursors_are_isolated_by_candidate_target_facet_and_page_size() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let left_intent =
        json!({"kind":"rename_declaration","target":"calculator.add","name":"left_add"});
    let right_intent =
        json!({"kind":"rename_declaration","target":"calculator.add","name":"right_add"});
    let left = apply(&base, &left_intent);
    let right = apply(&base, &right_intent);
    let mut session = fixture.session();
    let root = open(&mut session);
    let left_id = apply_wire(&mut session, &root, &left_intent);
    let right_id = apply_wire(&mut session, &root, &right_intent);
    assert_eq!(left_id, left.candidate_digest());
    assert_eq!(right_id, right.candidate_digest());
    let base_summary = summary(&mut session, &root, "calculator.divide");
    let left_summary = summary(&mut session, &left_id, "calculator.divide");
    let right_summary = summary(&mut session, &right_id, "calculator.divide");
    let base_handle = handle(&base_summary, "signature");
    let left_handle = handle(&left_summary, "signature");
    let right_handle = handle(&right_summary, "signature");
    assert_ne!(base_handle, left_handle);
    assert_ne!(left_handle, right_handle);
    for (candidate, target, name, token) in [
        (&left_id, "calculator.divide", "signature", &base_handle),
        (&left_id, "calculator.divide", "signature", &right_handle),
        (&left_id, "calculator.add", "signature", &left_handle),
        (&left_id, "calculator.divide", "contracts", &left_handle),
    ] {
        error_code(
            bound(
                &mut session,
                FACET,
                json!({"candidate_revision":candidate,"target":target,"facet":name,"handle":token}),
            ),
            "SPX-G360",
        );
    }
    let first = facet(
        &mut session,
        &left_id,
        "calculator.divide",
        "signature",
        &left_handle,
        None,
        1,
        65536,
    );
    let cursor = first["next_cursor"].as_str().unwrap();
    error_code(
        bound(
            &mut session,
            FACET,
            json!({"candidate_revision":left_id,"target":"calculator.divide","facet":"signature","handle":left_handle,"cursor":cursor,"page_size":2,"max_bytes":65536}),
        ),
        "SPX-G360",
    );
    let continuation = facet(
        &mut session,
        &left_id,
        "calculator.divide",
        "signature",
        &left_handle,
        Some(cursor),
        1,
        1024 * 1024,
    );
    assert_eq!(continuation["offset"], 1);
    session.finish().unwrap();
}

#[test]
fn discovery_mcp_detached_batches_and_drift_preserve_exact_read_only_results() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut absent = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let absent_capabilities = payload(call(&mut absent, "protocol/capabilities", json!({})));
    assert!(!absent_capabilities["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method
            .as_str()
            .is_some_and(|name| name == SUMMARY || name == FACET)));
    let absent_schemas = payload(call(&mut absent, "protocol/schemas", json!({})));
    assert!(!absent_schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|document| document["$id"]
            .as_str()
            .is_some_and(|id| id.contains("project-candidate-function"))));
    absent.finish().unwrap();

    let mut session = fixture.session().with_read_batch_workers(2).unwrap();
    let candidate = open(&mut session);
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    for (method, schema) in [
        (
            SUMMARY,
            "urn:semaprax.project-candidate-function-summary.v1",
        ),
        (FACET, "urn:semaprax.project-candidate-function-facet.v1"),
    ] {
        let descriptor = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method)
            .unwrap();
        assert_eq!(descriptor["capability"], "candidate_prepare");
        assert_eq!(descriptor["query"], true);
        assert!(schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["$id"] == schema));
        assert!(session.parallel_read_methods().contains(&method));
    }
    assert!(schemas["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(
            "urn:semaprax.project-candidate-function-facet-item.v1"
        )));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        for fragment in [
            "request_candidate_function_summary",
            "request_candidate_function_facet",
            "decode_request_candidate_function_summary",
            "decode_request_candidate_function_facet",
        ] {
            assert!(source.contains(fragment), "{language} missing {fragment}");
        }
    }
    let params = json!({"image_revision":session.image_revision(),"candidate_revision":candidate,"target":"calculator.divide"});
    let request = frame(SUMMARY, params);
    let sequential = session.handle_frame(&request).unwrap();
    let detached = session
        .handle_read_batch(&[request.as_slice()], 2)
        .unwrap()
        .remove(0)
        .unwrap();
    assert_eq!(detached, sequential);
    let rpc = payload(bound(
        &mut session,
        "workspace/read-batch",
        json!({"batch":{"frames":[String::from_utf8(request).unwrap()]}}),
    ));
    assert_eq!(rpc["responses"][0].as_str().unwrap().as_bytes(), sequential);

    let mut mcp = McpSession::new(fixture.session()).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},
        "clientInfo":{"name":"candidate-function-facets","version":"1"}}});
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
    assert!(names.contains(&"candidate__function-summary".to_owned()));
    assert!(names.contains(&"candidate__function-facet".to_owned()));
    mcp.finish().unwrap();

    let source = fixture.0.join("src/core.spx");
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let drifted = fixture.bytes();
    assert_eq!(
        bound(
            &mut session,
            SUMMARY,
            json!({"candidate_revision":candidate,"target":"calculator.divide"})
        )["error"]["code"],
        -32000
    );
    assert!(session.finish().is_err());
    assert_eq!(fixture.bytes(), drifted);
    assert_ne!(fixture.bytes(), disk);
}
