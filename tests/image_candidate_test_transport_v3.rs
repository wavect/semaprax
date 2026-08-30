//! Explicit test authority regression cases; authored, deliberately unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, TEST_PROTOCOL_SCHEMA};
use semaprax::project::CandidateTestPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-test-protocol-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in FILES {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root)
    }
    fn session(&self, capability: ImageHostCapability) -> ImageSession {
        ImageSession::open(&self.0.join("semaprax.toml"), capability).unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
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
fn call(session: &mut ImageSession, method: &str, mut params: Value) -> Value {
    if method.starts_with("candidate/") || method == "validation/catalog" {
        params["image_revision"] = json!(session.image_revision());
    }
    let request = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["protocol"], TEST_PROTOCOL_SCHEMA);
    response["result"]["payload"].clone()
}
fn open(session: &mut ImageSession) -> String {
    payload(call(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn execution_is_explicit_and_old_profiles_cannot_elevate() {
    let fixture = Fixture::new();
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
    ] {
        let mut session = fixture.session(capability);
        let caps = call(&mut session, "protocol/capabilities", json!({}));
        assert_ne!(caps["result"]["payload"]["test_execution"], true);
        for method in ["candidate/test", "candidate/test-plan"] {
            assert_eq!(
                call(
                    &mut session,
                    method,
                    json!({"candidate_revision":"sha256:unknown"})
                )["error"]["code"],
                -32601
            );
        }
    }
    let mut session = fixture.session(ImageHostCapability::TestEnabled);
    let caps = payload(call(&mut session, "protocol/capabilities", json!({})));
    assert_eq!(caps["test_execution"], true);
    assert_eq!(caps["source_authority"], false);
    assert_eq!(caps["target_execution"], false);
    assert_eq!(caps["test_policy"]["max_steps"], 100_000);
    assert!(caps["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method == "candidate/test"));
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let method = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/test")
        .unwrap();
    assert_eq!(method["capability"], "candidate_test");
}

#[test]
fn test_requests_bind_candidate_and_cannot_override_policy_or_write_source() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(ImageHostCapability::TestEnabled);
    let candidate = open(&mut session);
    let plan = payload(call(
        &mut session,
        "candidate/test-plan",
        json!({"candidate_revision":candidate}),
    ));
    assert_eq!(plan["schema"], "semaprax.project-candidate-test-plan.v1");
    assert_eq!(
        call(
            &mut session,
            "candidate/test",
            json!({"candidate_revision":candidate,"max_steps":1_000_000})
        )["error"]["code"],
        -32602
    );
    let report = payload(call(
        &mut session,
        "candidate/test",
        json!({"candidate_revision":candidate}),
    ));
    assert_eq!(
        report["schema"],
        "semaprax.project-candidate-test-report.v1"
    );
    assert_eq!(report["candidate_digest"], candidate);
    assert_eq!(report["passed"], true);
    assert_eq!(report["options"]["max_steps"], 100_000);
    assert_eq!(
        report["candidate_replay"],
        "exact_source_and_evidence_replay_before_execution"
    );
    assert_eq!(
        call(
            &mut session,
            "candidate/test",
            json!({"candidate_revision":format!("sha256:{}","0".repeat(64))})
        )["error"]["code"],
        -32000
    );
    assert_eq!(
        call(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":candidate})
        )["error"]["code"],
        -32601
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn host_policy_is_disclosed_and_manual_source_drift_prevents_a_report() {
    let fixture = Fixture::new();
    let policy = CandidateTestPolicy::new(1, 65_536, 262_144).unwrap();
    let mut session =
        ImageSession::open_test_enabled(&fixture.0.join("semaprax.toml"), policy).unwrap();
    let caps = payload(call(&mut session, "protocol/capabilities", json!({})));
    assert_eq!(caps["test_policy"]["max_steps"], 1);
    let candidate = open(&mut session);
    let exhausted = payload(call(
        &mut session,
        "candidate/test",
        json!({"candidate_revision":candidate}),
    ));
    assert_eq!(exhausted["passed"], false);
    assert_eq!(exhausted["options"]["max_steps"], 1);
    let path = fixture.0.join("src/core.spx");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, &bytes).unwrap();
    let rejected = call(
        &mut session,
        "candidate/test",
        json!({"candidate_revision":candidate}),
    );
    assert!(rejected.get("result").is_none());
    assert_eq!(rejected["error"]["code"], -32000);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
