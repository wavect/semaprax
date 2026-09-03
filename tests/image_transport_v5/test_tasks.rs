//! Session-scoped cooperative candidate-test task evidence.
use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use semaprax::project::CandidateTestPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-v5-test-task-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                test_policy: Some(CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap()),
                ..Default::default()
            },
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if method.starts_with("candidate/") {
        params["image_revision"] = json!(session.image_revision());
    }
    let request = json!({"jsonrpc":"2.0","id":"task","method":method,"params":params});
    serde_json::from_slice(
        &session
            .handle_frame(request.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap()
}
fn payload(value: Value) -> Value {
    assert!(value.get("error").is_none(), "{value}");
    value["result"]["payload"].clone()
}
fn open(session: &mut VNextSession) -> String {
    payload(call(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn immediate_cancel_is_sticky_bound_and_releases_no_report() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let mut session = fixture.session();
    let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
    for method in [
        "candidate/test-task-start",
        "candidate/test-task-status",
        "candidate/test-task-cancel",
        "candidate/test-task-result",
    ] {
        assert!(capabilities["methods"]
            .as_array()
            .unwrap()
            .contains(&json!(method)));
    }
    let candidate = open(&mut session);
    let started = payload(call(
        &mut session,
        "candidate/test-task-start",
        json!({"candidate_revision":candidate}),
    ));
    assert_eq!(started["state"], "queued");
    assert_eq!(started["source_authority"], false);
    assert_eq!(started["authority"]["publication"], false);
    assert_eq!(started["blind_spots"].as_array().unwrap().len(), 6);
    let task = started["task_revision"].as_str().unwrap().to_owned();
    let cancelled = payload(call(
        &mut session,
        "candidate/test-task-cancel",
        json!({"task_revision":task}),
    ));
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["terminal"], true);
    assert_eq!(cancelled["before_step"], 1);
    assert_eq!(cancelled["steps_used"], 0);
    assert_eq!(cancelled["report_digest"], Value::Null);
    let repeated = payload(call(
        &mut session,
        "candidate/test-task-status",
        json!({"task_revision":task}),
    ));
    assert_eq!(repeated["state"], "cancelled");
    assert!(call(
        &mut session,
        "candidate/test-task-result",
        json!({"task_revision":task,"offset":0,"max_bytes":4096})
    )["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G365"));
    assert!(call(
        &mut session,
        "candidate/test-task-start",
        json!({"candidate_revision":candidate})
    )["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G365"));
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
    session.finish().unwrap();
}

#[test]
fn source_drift_cancels_the_task_and_terminally_withholds_late_results() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let candidate = open(&mut session);
    let started = payload(call(
        &mut session,
        "candidate/test-task-start",
        json!({"candidate_revision":candidate}),
    ));
    let path = fixture.0.join("src/core.spx");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, bytes).unwrap();
    let rejected = call(
        &mut session,
        "candidate/test-task-status",
        json!({"task_revision":started["task_revision"]}),
    );
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-"));
    assert!(session.is_terminal());
    assert!(session.handle_frame(b"{}").is_none());
    assert!(session.finish().is_err());
}

fn mcp(session: &mut McpSession, id: Value, method: &str, params: Value) -> Option<Value> {
    let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
    session
        .handle_frame(request.to_string().as_bytes())
        .map(|bytes| serde_json::from_slice(&bytes).unwrap())
}
fn mcp_tool(session: &mut McpSession, id: i64, name: &str, arguments: Value) -> Value {
    let outer = mcp(
        session,
        json!(id),
        "tools/call",
        json!({"name":name,"arguments":arguments}),
    )
    .unwrap();
    let inner: Value =
        serde_json::from_str(outer["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    payload(inner)
}

#[test]
fn mcp_tools_schedule_and_cancel_the_real_task_without_new_authority() {
    let fixture = Fixture::new();
    let mut session = McpSession::new(fixture.session()).unwrap();
    mcp(
        &mut session,
        json!(1),
        "initialize",
        json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    )
    .unwrap();
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}});
    assert!(session
        .handle_frame(initialized.to_string().as_bytes())
        .is_none());
    let image = mcp_tool(&mut session, 2, "workspace__open", json!({}))["image_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate = mcp_tool(
        &mut session,
        3,
        "candidate__open",
        json!({"image_revision":image}),
    )["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let started = mcp_tool(
        &mut session,
        4,
        "candidate__test-task-start",
        json!({"image_revision":image,"candidate_revision":candidate}),
    );
    assert_eq!(started["state"], "queued");
    let cancelled = mcp_tool(
        &mut session,
        5,
        "candidate__test-task-cancel",
        json!({"image_revision":image,"task_revision":started["task_revision"]}),
    );
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["authority"]["source_write"], false);
    assert_eq!(cancelled["authority"]["publication"], false);
    session.finish().unwrap();
}
