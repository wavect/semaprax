//! Comparative task-economics transport regressions, authored and unrun.

use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-task-economics-{}-{}",
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
    fn session(&self) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), VNextPolicy::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    serde_json::from_slice(&session.handle_frame(frame.to_string().as_bytes()).unwrap()).unwrap()
}

fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}

fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn embedded(lane: &str, value: u64) -> Value {
    let metrics = [
        "model_input_tokens",
        "model_output_tokens",
        "presented_context_bytes",
        "tool_calls",
        "tool_request_bytes",
        "tool_response_bytes",
        "failed_attempts",
        "stale_failures",
        "stale_recovery_actions",
        "validation_wall_ms",
        "review_wall_ms",
        "human_interventions",
    ]
    .into_iter()
    .map(|metric| {
        (
            metric.to_owned(),
            json!({
                "status":"observed","value":value,"method":"external_counter","evidence":["ledger"]
            }),
        )
    })
    .collect::<serde_json::Map<_, _>>();
    let observation = canonical(json!({
        "schema":"semaprax.agent-task-comparison-observation.v1",
        "plan_sha256":format!("sha256:{}","1".repeat(64)),
        "task":"signature-migration-v1","lane":lane,"trial":1,"state":"cold",
        "model":"model:v1","tokenizer":"provider:v1","model_configuration":"fixed:v1",
        "harness":"harness:v1","host":"host:v1","toolchain":"shared-toolchain:v1",
        "prompt_sha256":format!("sha256:{}","2".repeat(64)),
        "artifacts":[{"id":"ledger","path":"ledger.json","bytes":1,
            "sha256":format!("sha256:{}","3".repeat(64)),"kind":"typed_event_ledger"}],
        "metrics":metrics,"acceptance":[{"id":"correct","outcome":"passed","evidence":["ledger"]}],
        "outcome":"completed"
    }));
    json!({
        "schema":"semaprax.agent-task-comparison-embedded-observation.v1",
        "lane":lane,"task":"signature-migration-v1","corpus":"agent-task-comparison-v1",
        "tool":"shared-toolchain:v1","model":"model:v1",
        "source_revision":format!("sha256:{}","4".repeat(64)),
        "image_revision":if lane=="semaprax-graph-operational"{json!(format!("sha256:{}","5".repeat(64)))}else{Value::Null},
        "candidate_revision":if lane=="semaprax-graph-operational"{json!(format!("sha256:{}","6".repeat(64)))}else{Value::Null},
        "wall_time_ms":value,"protocol_bytes":value,"source_bytes":value,
        "observation_sha256":digest(observation.as_bytes()),"observation":observation
    })
}

fn valid_observations() -> String {
    canonical(json!({
        "schema":"semaprax.agent-task-comparison-observation-set.v1",
        "plan_sha256":format!("sha256:{}","1".repeat(64)),
        "repository_head":"a".repeat(40),"task":"signature-migration-v1",
        "corpus":"agent-task-comparison-v1","model":"model:v1",
        "observations":[embedded("semaprax-graph-operational",4),embedded("semaprax-source-first",7)]
    }))
}

#[test]
fn task_comparison_is_closed_generated_and_mcp_catalogued_without_execution() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let image = session.image_revision().to_owned();
    let schemas = call(&mut session, "protocol/schemas", json!({}))["result"]["payload"].clone();
    let descriptor = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == "agent/task-comparison")
        .unwrap();
    assert_eq!(descriptor["capability"], "semantic_read");
    assert_eq!(
        descriptor["request_schema"]["properties"]["params"]["additionalProperties"],
        false
    );
    assert_eq!(
        descriptor["request_schema"]["properties"]["params"]["properties"]["observations"]
            ["x-max-utf8-bytes"],
        28 * 1024
    );
    for language in ["typescript", "python", "rust"] {
        let client = call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        );
        assert!(client["result"]["payload"]["source"]
            .as_str()
            .unwrap()
            .contains("request_agent_task_comparison"));
    }
    let observations = "{}\n";
    let digest = digest(observations.as_bytes());
    assert_eq!(
        call(
            &mut session,
            "agent/task-comparison",
            json!({"image_revision":image,
        "observations":observations,"observations_sha256":digest})
        )["error"]["data"]["diagnostics"][0]["code"],
        "SPX-G485"
    );
    let valid = valid_observations();
    assert!(valid.len() < 28 * 1024);
    let valid_digest = digest(valid.as_bytes());
    let arguments = json!({"image_revision":image,"observations":valid,
        "observations_sha256":valid_digest});
    let result = call(&mut session, "agent/task-comparison", arguments.clone());
    let result = &result["result"]["payload"];
    assert_eq!(
        result["schema"],
        "semaprax.image-agent-task-comparison-report.v1"
    );
    assert_eq!(result["superiority"], "not_assessed");
    assert_eq!(result["source_authority"], false);
    assert_eq!(
        result["report_sha256"],
        digest(result["report"].as_str().unwrap().as_bytes())
    );
    let report: Value = serde_json::from_str(result["report"].as_str().unwrap()).unwrap();
    assert_eq!(
        report["comparisons"][0]["normalized_deltas"]["tool_calls"]["left_minus_right"],
        -3
    );
    assert_eq!(
        report["comparisons"][1]["status"],
        "not_assessed_missing_observation"
    );
    let mut stale = arguments.clone();
    stale["image_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert_eq!(
        call(&mut session, "agent/task-comparison", stale)["error"]["data"]["diagnostics"][0]
            ["code"],
        "SPX-G489"
    );
    session.finish().unwrap();

    let mcp_host = fixture.session();
    let mcp_image = mcp_host.image_revision().to_owned();
    let mut mcp = McpSession::new(mcp_host).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"economics","version":"1"}}});
    mcp.handle_frame(initialize.to_string().as_bytes()).unwrap();
    mcp.handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let mut cursor = None;
    let mut found = false;
    loop {
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":
            cursor.as_ref().map_or_else(||json!({}),|value|json!({"cursor":value}))});
        let page: Value =
            serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        found |= page["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "agent__task-comparison");
        cursor = page["result"]["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(found);
    let mut mcp_arguments = arguments;
    mcp_arguments["image_revision"] = json!(mcp_image);
    let invoked = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"agent__task-comparison","arguments":mcp_arguments}});
    let invoked: Value =
        serde_json::from_slice(&mcp.handle_frame(invoked.to_string().as_bytes()).unwrap()).unwrap();
    let inner: Value =
        serde_json::from_str(invoked["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        inner["result"]["payload"]["schema"],
        "semaprax.image-agent-task-comparison-report.v1"
    );
    assert_eq!(inner["result"]["payload"]["superiority"], "not_assessed");
    mcp.finish().unwrap();
}
