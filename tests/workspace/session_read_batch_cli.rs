//! Host-selected batching through the actual CLI; authored and unrun locally.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-read-batch-cli-{}-{}",
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
        Self(root.canonicalize().unwrap())
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), VNextPolicy::default()).unwrap()
    }
    fn run(&self, policy: &Value, frames: &[Value]) -> Output {
        let path = self.0.join("host.json");
        std::fs::write(&path, policy.to_string()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve-workspace")
            .arg(self.0.join("semaprax.toml"))
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            let mut input = child.stdin.take().unwrap();
            for frame in frames {
                writeln!(input, "{frame}").unwrap();
            }
        }
        child.wait_with_output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn policy(version: u8) -> Value {
    let mut value = json!({"schema":format!("semaprax.workspace-host-policy.v{version}"),
        "candidate_prepare":false,"diagnostics":false,"build_enabled":false,
        "test_policy":null,"git_commit":null});
    for (first, name, field) in [
        (2, "frontend_cache", json!(false)),
        (3, "candidate_archives", json!([])),
        (4, "semantic_cache", json!(false)),
        (5, "semantic_cache_entry", Value::Null),
        (6, "draft_archives", json!([])),
        (7, "read_batch_workers", Value::Null),
    ] {
        if version >= first {
            value[name] = field;
        }
    }
    value
}
fn rows(output: Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn capabilities() -> Value {
    json!({"jsonrpc":"2.0","id":"caps","method":"protocol/capabilities","params":{}})
}

#[test]
fn startup_workers_enable_ordered_ndjson_batch_without_request_elevation() {
    let fixture = Fixture::new();
    let mut sequential = fixture.session();
    let revision = sequential.image_revision().to_owned();
    let inner = json!({"jsonrpc":"2.0","id":"inner","method":"workspace/open","params":{}});
    let encoded = format!("{inner}\n");
    let expected = String::from_utf8(sequential.handle_frame(encoded.as_bytes()).unwrap()).unwrap();
    let batch = json!({"jsonrpc":"2.0","id":"batch","method":"workspace/read-batch",
        "params":{"image_revision":revision,"batch":{"frames":[encoded, "", "{"]}}});
    let mut override_workers = batch.clone();
    override_workers["params"]["workers"] = json!(4);
    for workers in [1, 4] {
        let mut selected = policy(7);
        selected["read_batch_workers"] = json!(workers);
        let output = rows(fixture.run(
            &selected,
            &[capabilities(), batch.clone(), override_workers.clone()],
        ));
        assert_eq!(output.len(), 3);
        assert!(output[0]["result"]["payload"]["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("parallel_read")));
        assert_eq!(output[0]["result"]["payload"]["source_authority"], false);
        let result = &output[1]["result"]["payload"];
        assert_eq!(result["schema"], "semaprax.image-read-batch.v1");
        assert_eq!(result["source_authority"], false);
        assert_eq!(result["responses"].as_array().unwrap().len(), 3);
        assert_eq!(result["responses"][0], expected);
        assert!(result["responses"][1].is_null());
        let malformed: Value =
            serde_json::from_str(result["responses"][2].as_str().unwrap()).unwrap();
        assert_eq!(malformed["error"]["code"], -32700);
        assert_eq!(output[2]["error"]["code"], -32602);
    }
}

#[test]
fn null_selection_preserves_older_discovery_and_batch_is_unavailable() {
    let fixture = Fixture::new();
    let revision = fixture.session().image_revision().to_owned();
    let batch = json!({"jsonrpc":"2.0","id":2,"method":"workspace/read-batch",
        "params":{"image_revision":revision,"batch":{"frames":["{}"]}}});
    let expected = rows(fixture.run(&policy(1), &[capabilities(), batch.clone()]));
    assert_eq!(expected[1]["error"]["code"], -32601);
    for version in 2..=7 {
        assert_eq!(
            rows(fixture.run(&policy(version), &[capabilities(), batch.clone()])),
            expected
        );
    }
}

#[test]
fn closed_startup_policy_rejects_invalid_worker_shapes_and_old_version_extensions() {
    let fixture = Fixture::new();
    let mut rejected = Vec::new();
    for workers in [
        json!(0),
        json!(5),
        json!(-1),
        json!(1.5),
        json!(true),
        json!("2"),
        json!({}),
    ] {
        let mut selected = policy(7);
        selected["read_batch_workers"] = workers;
        rejected.push(selected);
    }
    let mut missing = policy(7);
    missing
        .as_object_mut()
        .unwrap()
        .remove("read_batch_workers");
    rejected.push(missing);
    for version in 1..=6 {
        let mut selected = policy(version);
        selected["read_batch_workers"] = Value::Null;
        rejected.push(selected);
    }
    for selected in rejected {
        let output = fixture.run(&selected, &[]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G280"));
    }
}
