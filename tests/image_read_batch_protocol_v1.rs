//! Optional protocol read batching: regression source authored without execution.
use semaprax::image_transport::{
    McpSession, VNextPolicy, VNextSession, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
use semaprax::project::CandidateTestPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-read-batch-protocol-{}-{}",
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
    fn session(&self, policy: VNextPolicy) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), policy).unwrap()
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
fn policy() -> VNextPolicy {
    VNextPolicy {
        candidate_prepare: true,
        diagnostics: true,
        build_enabled: true,
        test_policy: Some(CandidateTestPolicy::new(100, 4096, 16384).unwrap()),
    }
}
fn frame(id: Value, method: &str, params: Value) -> String {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(
        &session
            .handle_frame(frame(json!("outer"), method, params).as_bytes())
            .unwrap(),
    )
    .unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn batch(session: &mut VNextSession, frames: &[String]) -> Value {
    let image = session.image_revision().to_owned();
    call(
        session,
        "workspace/read-batch",
        json!({"image_revision":image,"batch":{"frames":frames}}),
    )
}
fn rows(response: Value, expected_len: usize) -> Vec<Option<String>> {
    let result = payload(response);
    assert_eq!(result["schema"], "semaprax.image-read-batch.v1");
    assert_eq!(result["source_authority"], false);
    assert_eq!(result.as_object().unwrap().len(), 3);
    let rows = result["responses"].as_array().unwrap();
    assert_eq!(rows.len(), expected_len);
    rows.iter()
        .map(|row| match row {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            _ => panic!("inner replies must remain raw strings or null"),
        })
        .collect()
}
fn error_code(response: &Value, code: &str) {
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
fn exact_sequential_and_host_batch_parity_preserves_order_ids_errors_and_notifications() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    for workers in [1, 2, 4] {
        let mut ordinary = fixture
            .session(policy())
            .with_read_batch_workers(workers)
            .unwrap();
        let mut remote = fixture
            .session(policy())
            .with_read_batch_workers(workers)
            .unwrap();
        let mut host = fixture
            .session(policy())
            .with_read_batch_workers(workers)
            .unwrap();
        let image = remote.image_revision().to_owned();
        assert_eq!(ordinary.image_revision(), image);
        let requests = vec![
            frame(json!(8), "workspace/open", json!({})),
            frame(
                json!("unicode-λ"),
                "image/symbol",
                json!({"image_revision":image,"stable_id":"calculator.add"}),
            ),
            frame(
                json!(2),
                "image/function-summary",
                json!({"image_revision":image,"target":"calculator.multiply"}),
            ),
            frame(
                json!(3),
                "image/symbol",
                json!({"image_revision":image,"stable_id":"missing"}),
            ),
            frame(
                json!(4),
                "image/symbol",
                json!({"image_revision":format!("sha256:{}","0".repeat(64)),"stable_id":"calculator.add"}),
            ),
            frame(json!(5), "workspace/status", json!({"unknown":true})),
            "{\"jsonrpc\":\"2.0\",\"id\":7,\"id\":8,\"method\":\"workspace/status\"}".to_owned(),
            "{".to_owned(),
            String::new(),
            "{\"jsonrpc\":\"2.0\",\"method\":\"candidate/open\",\"params\":{}}".to_owned(),
            frame(json!(6), "protocol/capabilities", json!({})),
            format!("{}\n", frame(json!(10), "workspace/status", json!({}))),
        ];
        let expected = requests
            .iter()
            .map(|request| {
                ordinary
                    .handle_frame(request.as_bytes())
                    .map(|bytes| String::from_utf8(bytes).unwrap())
            })
            .collect::<Vec<_>>();
        let actual = rows(batch(&mut remote, &requests), requests.len());
        assert_eq!(actual, expected);
        let borrowed = requests
            .iter()
            .map(|request| request.as_bytes())
            .collect::<Vec<_>>();
        let local = host
            .handle_read_batch(&borrowed, workers)
            .unwrap()
            .into_iter()
            .map(|row| row.map(|bytes| String::from_utf8(bytes).unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(actual, local);
        assert!(actual[8].is_none());
        assert!(actual[9].is_none());
        assert_eq!(
            serde_json::from_str::<Value>(actual[1].as_ref().unwrap()).unwrap()["id"],
            "unicode-λ"
        );
        assert_eq!(
            serde_json::from_str::<Value>(actual[7].as_ref().unwrap()).unwrap()["error"]["code"],
            -32700
        );
        ordinary.finish().unwrap();
        remote.finish().unwrap();
        host.finish().unwrap();
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn batching_does_not_enable_or_execute_registry_refresh_build_test_or_nested_batch_actions() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture
        .session(policy())
        .with_read_batch_workers(4)
        .unwrap();
    let image = session.image_revision().to_owned();
    let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
    for grant in [
        "parallel_read",
        "candidate_prepare",
        "candidate_build",
        "candidate_test",
    ] {
        assert!(capabilities["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!(grant)));
    }
    let methods = [
        "candidate/open",
        "candidate/apply-intent",
        "candidate/discard",
        "hole/open",
        "hole/fill",
        "hole/complete",
        "workspace/refresh-preview",
        "workspace/refresh",
        "candidate/build",
        "candidate/artifact-delta",
        "candidate/test",
        "candidate/commit",
        "workspace/read-batch",
        "no/such-method",
    ];
    let requests = methods
        .iter()
        .enumerate()
        .map(|(id, method)| frame(json!(id), method, json!({"image_revision":image})))
        .collect::<Vec<_>>();
    for row in rows(batch(&mut session, &requests), requests.len()) {
        let row: Value = serde_json::from_str(row.as_ref().unwrap()).unwrap();
        assert_eq!(row["error"]["code"], -32601, "{row}");
    }
    // A notifications-only outer request must never run its enclosed calls.
    let notification = json!({"jsonrpc":"2.0","method":"workspace/read-batch","params":{
        "image_revision":image,"batch":{"frames":[frame(json!(0),"candidate/open",json!({"image_revision":image}))]}}}).to_string();
    assert!(session.handle_frame(notification.as_bytes()).is_none());
    let project =
        payload(call(&mut session, "workspace/open", json!({})))["project_revision"].clone();
    let refreshed = payload(call(
        &mut session,
        "workspace/refresh",
        json!({"image_revision":image,"expected_new_project_revision":project}),
    ));
    assert_eq!(refreshed["retained_candidates"], json!([]));
    // Ordinary authorized preparation still works; batch exclusion never
    // changes the host policy or creates an implicit candidate.
    let image = session.image_revision().to_owned();
    let candidate = payload(call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image}),
    ));
    let query = frame(
        json!(21),
        "candidate/query",
        json!({"image_revision":image,"candidate_revision":candidate["candidate_revision"]}),
    );
    let expected = session.handle_frame(query.as_bytes()).unwrap();
    assert_eq!(
        rows(batch(&mut session, &[query]), 1)[0].as_deref(),
        Some(std::str::from_utf8(&expected).unwrap())
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn startup_selection_is_once_only_and_cannot_be_requested_or_added_after_any_frame_attempt() {
    let fixture = Fixture::new();
    for workers in [0, 5] {
        let errors = fixture
            .session(VNextPolicy::default())
            .with_read_batch_workers(workers)
            .err()
            .unwrap();
        assert_eq!(errors[0].code, "SPX-G294");
    }
    let selected = fixture
        .session(VNextPolicy::default())
        .with_read_batch_workers(1)
        .unwrap();
    assert_eq!(
        selected.with_read_batch_workers(2).err().unwrap()[0].code,
        "SPX-G294"
    );
    for attempted in [
        "",
        "{",
        "{\"jsonrpc\":\"2.0\",\"method\":\"workspace/status\"}",
    ] {
        let mut session = fixture.session(VNextPolicy::default());
        let _ = session.handle_frame(attempted.as_bytes());
        assert_eq!(
            session.with_read_batch_workers(1).err().unwrap()[0].code,
            "SPX-G294"
        );
    }
    let mut session = fixture.session(VNextPolicy::default());
    assert!(session.handle_read_batch(&[], 0).is_err());
    assert_eq!(
        session.with_read_batch_workers(1).err().unwrap()[0].code,
        "SPX-G294"
    );
    let mut disabled = fixture.session(VNextPolicy::default());
    let image = disabled.image_revision().to_owned();
    let denied = call(
        &mut disabled,
        "workspace/read-batch",
        json!({"image_revision":image,"batch":{"frames":[""]}}),
    );
    assert_eq!(denied["error"]["code"], -32601);
    let mut enabled = fixture
        .session(VNextPolicy::default())
        .with_read_batch_workers(1)
        .unwrap();
    let image = enabled.image_revision().to_owned();
    let override_workers = call(
        &mut enabled,
        "workspace/read-batch",
        json!({"image_revision":image,"batch":{"frames":[""]},"workers":4}),
    );
    assert_eq!(override_workers["error"]["code"], -32602);
    assert_eq!(enabled.policy(), VNextPolicy::default());
}

#[test]
fn malformed_outer_inputs_and_stale_bindings_never_return_partial_rows() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture
        .session(VNextPolicy::default())
        .with_read_batch_workers(2)
        .unwrap();
    let image = session.image_revision().to_owned();
    for invalid in [
        json!({"frames":[]}),
        json!({"frames":vec!["";17]}),
        json!({"frames":[7]}),
        json!({"frames":[null]}),
        json!({"frames":[{}]}),
        json!({"frames":[""],"workers":4}),
        json!({}),
    ] {
        let response = call(
            &mut session,
            "workspace/read-batch",
            json!({"image_revision":image,"batch":invalid}),
        );
        error_code(&response, "SPX-G294");
    }
    let stale = call(
        &mut session,
        "workspace/read-batch",
        json!({"image_revision":format!("sha256:{}","0".repeat(64)),"batch":{"frames":["", "{"]}}),
    );
    error_code(&stale, "SPX-G282");
    assert_eq!(
        rows(batch(&mut session, &vec![String::new(); 16]), 16),
        vec![None; 16]
    );
    // The outer NDJSON limit is not widened to fit a maximal escaped inner
    // frame: its existing terminal transport rejection owns this boundary.
    let oversized = frame(
        json!(1),
        "workspace/read-batch",
        json!({"image_revision":image,"batch":{"frames":["λ".repeat(MAX_REQUEST_BYTES / 2 + 1)]}}),
    );
    assert!(oversized.len() > MAX_REQUEST_BYTES);
    let response: Value =
        serde_json::from_slice(&session.handle_frame(oversized.as_bytes()).unwrap()).unwrap();
    assert!(response.get("result").is_none());
    assert!(session.is_terminal());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn live_source_drift_is_checked_even_for_all_invalid_or_notification_inner_frames() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/app.spx");
    let source = std::fs::read(&path).unwrap();
    for requests in [
        vec![String::new(), "{".to_owned()],
        vec!["{\"jsonrpc\":\"2.0\",\"method\":\"workspace/status\"}".to_owned()],
        vec![frame(json!(1), "workspace/status", json!({}))],
    ] {
        let mut session = fixture
            .session(VNextPolicy::default())
            .with_read_batch_workers(2)
            .unwrap();
        let image = session.image_revision().to_owned();
        std::fs::write(&path, b"manual source drift\n").unwrap();
        let response = batch(&mut session, &requests);
        assert!(
            response.get("result").is_none(),
            "no partial replies on source drift: {response}"
        );
        assert!(response.get("error").is_some());
        assert_eq!(session.image_revision(), image);
        std::fs::write(&path, &source).unwrap();
        assert!(
            batch(&mut session, &requests).get("result").is_none(),
            "restoring bytes cannot revive lost source authentication"
        );
        assert!(session.finish().is_err());
    }
}

#[test]
fn aggregate_response_overflow_rejects_the_whole_envelope_without_partial_results() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture
        .session(policy())
        .with_read_batch_workers(4)
        .unwrap();
    let inner = frame(json!(7), "protocol/schemas", json!({}));
    let single = session.handle_frame(inner.as_bytes()).unwrap();
    let decoded: Value = serde_json::from_slice(&single).unwrap();
    assert!(
        decoded.get("result").is_some(),
        "the individual schema query must fit: {decoded}"
    );
    assert!(
        single.len() * 16 > MAX_RESPONSE_BYTES,
        "selected schema inventory must exercise aggregate overflow"
    );
    let response = batch(&mut session, &vec![inner; 16]);
    assert_eq!(response["error"]["code"], -32001);
    assert!(response.get("result").is_none());
    assert!(serde_json::to_vec(&response).unwrap().len() <= MAX_RESPONSE_BYTES);
    assert!(!session.is_terminal());
    assert_eq!(rows(batch(&mut session, &[String::new()]), 1), vec![None]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn selected_discovery_and_typed_clients_describe_only_the_outer_container() {
    let fixture = Fixture::new();
    let mut disabled = fixture.session(VNextPolicy::default());
    let mut enabled = fixture
        .session(VNextPolicy::default())
        .with_read_batch_workers(2)
        .unwrap();
    for (session, selected) in [(&mut disabled, false), (&mut enabled, true)] {
        let capabilities = payload(call(session, "protocol/capabilities", json!({})));
        assert_eq!(
            capabilities["capabilities"]
                .as_array()
                .unwrap()
                .contains(&json!("parallel_read")),
            selected
        );
        assert_eq!(capabilities["source_authority"], false);
        assert_eq!(capabilities["test_execution"], false);
        let bundle = payload(call(session, "protocol/schemas", json!({})));
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "workspace/read-batch");
        assert_eq!(method.is_some(), selected);
        for id in [
            "urn:semaprax.image-read-batch-request.v1",
            "urn:semaprax.image-read-batch.v1",
        ] {
            let document = bundle["documents"]
                .as_array()
                .unwrap()
                .iter()
                .find(|document| document["$id"] == id);
            assert_eq!(document.is_some(), selected);
            if let Some(document) = document {
                assert_eq!(document["additionalProperties"], false);
            }
        }
        if let Some(method) = method {
            assert_eq!(method["capability"], "parallel_read");
            assert_eq!(method["query"], true);
            let params = &method["request_schema"]["properties"]["params"];
            assert_eq!(
                params["properties"]["batch"]["$ref"],
                "urn:semaprax.image-read-batch-request.v1"
            );
            assert!(params["properties"].get("workers").is_none());
            let docs = bundle["documents"].as_array().unwrap();
            let request = docs
                .iter()
                .find(|d| d["$id"] == "urn:semaprax.image-read-batch-request.v1")
                .unwrap();
            assert_eq!(request["properties"]["frames"]["maxItems"], 16);
            assert_eq!(
                request["properties"]["frames"]["items"]["x-max-utf8-bytes"],
                MAX_REQUEST_BYTES
            );
            let report = docs
                .iter()
                .find(|d| d["$id"] == "urn:semaprax.image-read-batch.v1")
                .unwrap();
            let arms = report["properties"]["responses"]["items"]["anyOf"]
                .as_array()
                .unwrap();
            assert!(arms.iter().any(|arm| arm["type"] == "null"));
            assert!(arms.iter().any(
                |arm| arm["type"] == "string" && arm["x-max-utf8-bytes"] == MAX_RESPONSE_BYTES
            ));
            assert_eq!(report["properties"]["source_authority"]["const"], false);
        }
        for language in ["typescript", "python", "rust"] {
            let generated = payload(call(
                session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = generated["source"].as_str().unwrap();
            for name in [
                "WorkspaceReadBatchTypedParams",
                "WorkspaceReadBatchPayload",
                "WorkspaceReadBatchResult",
                "decode_request_workspace_read_batch_typed",
                "request_workspace_read_batch_typed",
            ] {
                assert_eq!(source.contains(name), selected, "{language}: {name}");
            }
            assert_eq!(generated["io"], false);
        }
    }
}

#[test]
fn mcp_selection_exposes_the_same_bounded_method_and_embeds_exact_v5_batch_response() {
    let fixture = Fixture::new();
    for selected in [false, true] {
        let host = fixture.session(VNextPolicy::default());
        let host = if selected {
            host.with_read_batch_workers(2).unwrap()
        } else {
            host
        };
        let image = host.image_revision().to_owned();
        let mut mcp = McpSession::new(host).unwrap();
        let initialize = frame(
            json!(1),
            "initialize",
            json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"batch-evidence","version":"1"}}),
        );
        let reply: Value =
            serde_json::from_slice(&mcp.handle_frame(initialize.as_bytes()).unwrap()).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-11-25");
        assert!(mcp
            .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
        let mut params = json!({});
        let mut names = Vec::new();
        loop {
            let page: Value = serde_json::from_slice(
                &mcp.handle_frame(frame(json!(2), "tools/list", params).as_bytes())
                    .unwrap(),
            )
            .unwrap();
            names.extend(
                page["result"]["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|tool| tool["name"].as_str().unwrap().to_owned()),
            );
            match page["result"]["nextCursor"].as_str() {
                Some(cursor) => params = json!({"cursor":cursor}),
                None => break,
            }
        }
        assert_eq!(
            names.iter().any(|name| name == "workspace__read-batch"),
            selected
        );
        let arguments = json!({"image_revision":image,"batch":{"frames":[frame(json!(9),"workspace/status",json!({})),"", "{"]}});
        let tool = frame(
            json!("mcp-outer"),
            "tools/call",
            json!({"name":"workspace__read-batch","arguments":arguments}),
        );
        let reply: Value =
            serde_json::from_slice(&mcp.handle_frame(tool.as_bytes()).unwrap()).unwrap();
        assert_eq!(reply["id"], "mcp-outer");
        if selected {
            let mut direct = fixture
                .session(VNextPolicy::default())
                .with_read_batch_workers(2)
                .unwrap();
            let expected = direct
                .handle_frame(frame(json!(0), "workspace/read-batch", arguments).as_bytes())
                .unwrap();
            assert_eq!(reply["result"]["isError"], false);
            assert_eq!(
                reply["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .as_bytes(),
                expected
            );
        } else {
            assert!(reply.get("error").is_some() || reply["result"]["isError"] == true);
        }
        mcp.finish().unwrap();
    }
}
