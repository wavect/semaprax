use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PROJECT_FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-agent-v2-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in PROJECT_FILES {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Daemon {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Daemon {
    fn start(fixture: &Fixture, extra: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semapraxd"))
            .arg("--stdio")
            .arg("--manifest-path")
            .arg(fixture.manifest())
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }

    fn call(&mut self, request: &str) -> Value {
        self.input.write_all(request.as_bytes()).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut response = String::new();
        assert_ne!(self.output.read_line(&mut response).unwrap(), 0);
        assert!(response.ends_with('\n'));
        serde_json::from_str(response.trim_end_matches('\n')).unwrap()
    }

    fn notify(&mut self, request: &str) {
        self.input.write_all(request.as_bytes()).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
    }

    fn probe_terminal_eof(&mut self) {
        let _ = self
            .input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"ping\"}\n");
        let _ = self.input.flush();
        let mut unexpected = String::new();
        assert_eq!(self.output.read_line(&mut unexpected).unwrap(), 0);
        assert!(unexpected.is_empty());
    }

    fn finish(mut self) -> (std::process::ExitStatus, String) {
        drop(self.input);
        let status = self.child.wait().unwrap();
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        (status, stderr)
    }
}

fn revisions(open: &Value) -> (&str, &str) {
    (
        open["result"]["project_revision"].as_str().unwrap(),
        open["result"]["workspace_revision"].as_str().unwrap(),
    )
}

fn params(project: &str, workspace: &str) -> String {
    format!(
        "\"project_revision\":{},\"workspace_revision\":{}",
        serde_json::to_string(project).unwrap(),
        serde_json::to_string(workspace).unwrap()
    )
}

fn inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, path: &Path, facts: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), facts);
            } else {
                facts.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut facts = BTreeMap::new();
    visit(root, root, &mut facts);
    facts
}

#[test]
fn persistent_project_session_serves_retained_graph_context_and_test_without_writes() {
    let fixture = Fixture::new("vertical-slice");
    let before = inventory(&fixture.0);
    let mut daemon = Daemon::start(&fixture, &[]);

    let protocol = daemon.call(r#"{"jsonrpc":"2.0","id":1,"method":"protocol"}"#);
    assert_eq!(
        protocol["result"]["protocol"],
        "semaprax.agent-transport.v2"
    );
    assert_eq!(protocol["result"]["state"], "configured");
    let configured_status = daemon.call(r#"{"jsonrpc":"2.0","id":10,"method":"workspace/status"}"#);
    assert!(configured_status["result"]["last_successful_project_revision"].is_null());
    assert!(configured_status["result"]["last_successful_workspace_revision"].is_null());

    let before_open = daemon.call(r#"{"jsonrpc":"2.0","id":2,"method":"check"}"#);
    assert_eq!(before_open["error"]["code"], -32000);

    let open = daemon.call(r#"{"jsonrpc":"2.0","id":3,"method":"workspace/open"}"#);
    let (project, workspace) = revisions(&open);
    let revision_params = params(project, workspace);

    let snapshot = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":4,"method":"workspace/snapshot","params":{{{revision_params}}}}}"#
    ));
    assert_eq!(snapshot["result"]["schema"], "semaprax.project-snapshot.v1");
    assert_eq!(snapshot["result"]["sources"].as_array().unwrap().len(), 3);

    let graph = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":5,"method":"graph","params":{{{revision_params}}}}}"#
    ));
    assert_eq!(
        graph["result"]["graph"]["schema"],
        "semaprax.project-semantic-graph.v1"
    );

    let context = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":6,"method":"context","params":{{{revision_params},"target_kind":"declaration","target":"calculator.add"}}}}"#
    ));
    assert_eq!(
        context["result"]["context"]["schema"],
        "semaprax.project-semantic-context.v1"
    );

    let test = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":7,"method":"test","params":{{{revision_params}}}}}"#
    ));
    assert_eq!(test["result"]["command_succeeded"], true);
    assert_eq!(
        test["result"]["execution"]["schema"],
        "semaprax.project-execution.v1"
    );

    let stale = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":8,"method":"check","params":{{"project_revision":"sha256:stale","workspace_revision":{}}}}}"#,
        serde_json::to_string(workspace).unwrap()
    ));
    assert_eq!(stale["error"]["code"], -32602);

    for request in [
        format!(
            r#"{{"jsonrpc":"2.0","id":11,"method":"workspace/snapshot","params":{{{revision_params},"unknown":1}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":12,"method":"check","params":{{{revision_params},"unknown":1}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":13,"method":"graph","params":{{{revision_params},"unknown":1}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":14,"method":"context","params":{{{revision_params},"target_kind":"declaration"}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":15,"method":"test","params":{{{revision_params},"max_steps":0}}}}"#
        ),
        r#"{"jsonrpc":"2.0","id":16,"method":"graph","params":{}}"#.to_owned(),
    ] {
        let rejected = daemon.call(&request);
        assert_eq!(rejected["error"]["code"], -32602, "{request}");
    }

    let shutdown = daemon.call(r#"{"jsonrpc":"2.0","id":9,"method":"shutdown"}"#);
    assert_eq!(shutdown["result"]["ok"], true);
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");
    assert!(stderr.is_empty());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn strict_framing_notifications_and_response_overflow_fail_closed() {
    let fixture = Fixture::new("framing");
    let mut daemon = Daemon::start(&fixture, &[]);
    let duplicate =
        daemon.call(r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{"x":1,"x":2}}"#);
    assert_eq!(duplicate["error"]["code"], -32600);
    let carriage_return = daemon.call("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\r");
    assert_eq!(carriage_return["error"]["code"], -32700);
    daemon.notify(r#"{"jsonrpc":"2.0","method":"ping"}"#);
    let ping = daemon.call(r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#);
    assert_eq!(ping["result"]["pong"], true);
    daemon.notify(r#"{"jsonrpc":"2.0","method":"shutdown"}"#);
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");

    let mut capped = Daemon::start(&fixture, &["--max-response-bytes", "104"]);
    let response = capped.call(r#"{"jsonrpc":"2.0","id":4,"method":"protocol"}"#);
    assert_eq!(response["error"]["code"], -32001);
    assert_eq!(response["id"], 4);
    capped.probe_terminal_eof();
    let (status, stderr) = capped.finish();
    assert!(status.success(), "daemon failed: {stderr}");
}

#[test]
fn input_drift_invalidates_the_session_before_cached_meaning_can_escape() {
    let fixture = Fixture::new("drift");
    let mut daemon = Daemon::start(&fixture, &[]);
    let open = daemon.call(r#"{"jsonrpc":"2.0","id":1,"method":"workspace/open"}"#);
    let (project, workspace) = revisions(&open);
    let revision_params = params(project, workspace);

    let source = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, original.replace("left + right", "left - right")).unwrap();

    let rejected = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"graph","params":{{{revision_params}}}}}"#
    ));
    assert_eq!(rejected["error"]["code"], -32000);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J102"));

    let absorbing = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"check","params":{{{revision_params}}}}}"#
    ));
    assert_eq!(absorbing["error"]["code"], -32000);
    assert!(absorbing["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J104"));
    daemon.notify(r#"{"jsonrpc":"2.0","method":"shutdown"}"#);
    let (status, stderr) = daemon.finish();
    assert!(!status.success());
    assert!(stderr.contains("SPX-J102"));
}

#[cfg(unix)]
#[test]
fn same_byte_path_substitution_invalidates_retained_authority() {
    let fixture = Fixture::new("same-byte-substitution");
    let mut daemon = Daemon::start(&fixture, &[]);
    let open = daemon.call(r#"{"jsonrpc":"2.0","id":1,"method":"workspace/open"}"#);
    let (project, workspace) = revisions(&open);
    let revision_params = params(project, workspace);

    let source = fixture.0.join("src/core.spx");
    let replacement = fixture.0.join("src/core.replacement");
    std::fs::write(&replacement, std::fs::read(&source).unwrap()).unwrap();
    std::fs::rename(&replacement, &source).unwrap();

    let rejected = daemon.call(&format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"check","params":{{{revision_params}}}}}"#
    ));
    assert_eq!(rejected["error"]["code"], -32000);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J102"));
    daemon.notify(r#"{"jsonrpc":"2.0","method":"shutdown"}"#);
    let (status, stderr) = daemon.finish();
    assert!(!status.success());
    assert!(stderr.contains("SPX-J102"));
}
