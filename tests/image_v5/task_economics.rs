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
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(observations.as_bytes()))
    );
    assert_eq!(
        call(
            &mut session,
            "agent/task-comparison",
            json!({"image_revision":image,
        "observations":observations,"observations_sha256":digest})
        )["error"]["data"]["diagnostics"][0]["code"],
        "SPX-G485"
    );
    session.finish().unwrap();

    let mut mcp = McpSession::new(fixture.session()).unwrap();
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
    mcp.finish().unwrap();
}
