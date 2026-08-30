//! Host-selected v5 CLI regressions, authored and deliberately unrun.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-v5-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root)
    }
    fn run(&self, policy: &Value, input: &str) -> std::process::Output {
        let path = self.0.join("host.json");
        std::fs::write(&path, policy.to_string()).unwrap();
        let manifest = std::fs::canonicalize(self.0.join("semaprax.toml")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve-workspace")
            .arg(manifest)
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn policy() -> Value {
    json!({"schema":"semaprax.workspace-host-policy.v1","candidate_prepare":false,"diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null})
}
#[test]
fn host_read_only_profile_is_discovered_and_cannot_be_elevated_by_rpc() {
    let fixture = Fixture::new();
    let input=concat!("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"protocol/capabilities\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"candidate/open\",\"params\":{\"candidate_prepare\":true}}\n");
    let output = fixture.run(&policy(), input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[0]["result"]["protocol"],
        "semaprax.image-agent-protocol.v5"
    );
    assert_eq!(responses[1]["error"]["code"], -32601);
    let methods = responses[0]["result"]["payload"]["methods"]
        .as_array()
        .unwrap();
    assert!(methods.iter().any(|method| method == "workspace/refresh"));
    assert!(!methods
        .iter()
        .any(|method| method == "candidate/commit" || method == "candidate/build"));
}
#[test]
fn startup_policy_rejects_unknown_fields_and_dependent_capabilities() {
    let fixture = Fixture::new();
    let mut unknown = policy();
    unknown["grant_all"] = json!(true);
    for policy in [unknown, {
        let mut value = policy();
        value["build_enabled"] = json!(true);
        value
    }] {
        let output = fixture.run(&policy, "");
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("SPX-G280"));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn cached_host_policy_preserves_v1_and_cannot_be_selected_by_request() {
    let fixture = Fixture::new();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"protocol/capabilities\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"workspace/open\",\"params\":{\"frontend_cache\":true}}\n"
    );
    let cold = fixture.run(&policy(), input);
    assert!(cold.status.success());
    for enabled in [false, true] {
        let mut selected = policy();
        selected["schema"] = json!("semaprax.workspace-host-policy.v2");
        selected["frontend_cache"] = json!(enabled);
        let output = fixture.run(&selected, input);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // This performance selection grants no methods and changes no image
        // identity. A request cannot toggle it even in the cached session.
        assert_eq!(output.stdout, cold.stdout);
        let rows = String::from_utf8(output.stdout).unwrap();
        let rejected: Value = serde_json::from_str(rows.lines().nth(1).unwrap()).unwrap();
        assert_eq!(rejected["error"]["code"], -32602);
    }
}

#[test]
fn cache_policy_is_versioned_closed_and_strictly_boolean() {
    let fixture = Fixture::new();
    let mut v1_extension = policy();
    v1_extension["frontend_cache"] = json!(true);
    let mut missing = policy();
    missing["schema"] = json!("semaprax.workspace-host-policy.v2");
    let mut values = vec![v1_extension, missing.clone()];
    for invalid in [Value::Null, json!(1), json!("true"), json!({})] {
        let mut value = missing.clone();
        value["frontend_cache"] = invalid;
        values.push(value);
    }
    let mut unknown = missing;
    unknown["frontend_cache"] = json!(true);
    unknown["cache_root"] = json!("/tmp/agent-selected");
    values.push(unknown);
    for value in values {
        let output = fixture.run(&value, "");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("SPX-G280"));
    }
}
