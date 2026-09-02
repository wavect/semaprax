use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{self, ProjectNpmBuild};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-transport-v5-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            format!(
                "schema = \"semaprax.project.v8\"\nname = \"agent-owned-{label}\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"agent_owned.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"agent-owned.payload\"]\ntests = [\"agent_owned.tests\"]\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("src/app.spx"),
            "module agent_owned.app;\n\n@id(\"agent-owned.payload\")\nfn payload(input: borrow Slice<u8>) -> Bytes\n{\n    bytes_copy(input)\n}\n\n@id(\"agent-owned.app.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/tests.spx"),
            "module agent_owned.tests;\n\n@id(\"agent-owned.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn legacy(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-transport-v5-legacy-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/app.spx"),
            "module legacy.app;\n\n@id(\"legacy.value\")\nfn value() -> i64\n{\n    1\n}\n\n@id(\"legacy.app.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/tests.spx"),
            "module legacy.tests;\n\n@id(\"legacy.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn inventory(&self) -> Vec<(String, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, rows: &mut Vec<(String, Vec<u8>)>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, rows);
                } else {
                    rows.push((
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        std::fs::read(path).unwrap(),
                    ));
                }
            }
        }
        let mut rows = Vec::new();
        visit(&self.0, &self.0, &mut rows);
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        rows
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
    error: BufReader<ChildStderr>,
}

impl Daemon {
    fn start(fixture: &Fixture, profile: bool, max_response_bytes: usize) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_semapraxd"));
        command
            .arg("--stdio")
            .arg("--manifest-path")
            .arg(fixture.manifest())
            .arg("--max-response-bytes")
            .arg(max_response_bytes.to_string());
        if profile {
            command.arg("--allow-project-owned-data");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            error: BufReader::new(child.stderr.take().unwrap()),
            child,
        }
    }

    fn raw(&mut self, request: Value) -> String {
        self.input
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        let bytes = self.output.read_line(&mut line).unwrap();
        if bytes == 0 {
            let status = self.child.wait().unwrap();
            let mut error = String::new();
            self.error.read_to_string(&mut error).unwrap();
            panic!("daemon exited before a response: status={status} stderr={error}");
        }
        assert!(line.ends_with('\n'));
        line.pop();
        assert!(!line.bytes().any(|byte| matches!(byte, b'\n' | b'\r')));
        line
    }

    fn call(&mut self, request: Value) -> Value {
        serde_json::from_str(&self.raw(request)).unwrap()
    }

    fn notify(&mut self, request: Value) {
        self.input
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
    }

    fn finish(mut self) {
        let response = self.call(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert_eq!(response["result"]["ok"], true);
        drop(self.input);
        assert!(self.child.wait().unwrap().success());
    }
}

fn open(daemon: &mut Daemon) -> (String, String) {
    let opened = daemon.call(json!({"jsonrpc":"2.0","id":2,"method":"workspace/open"}));
    (
        opened["result"]["project_revision"]
            .as_str()
            .unwrap()
            .to_owned(),
        opened["result"]["workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn subject(project: &str, workspace: &str) -> Value {
    json!({"project_revision":project,"workspace_revision":workspace})
}

fn extract_build(raw: &str) -> &str {
    let marker = "\"build\":";
    let start = raw.find(marker).unwrap() + marker.len();
    assert!(raw.ends_with("}}"));
    &raw[start..raw.len() - 2]
}

fn direct_descriptor(fixture: &Fixture) -> project::PublicApiDescriptor {
    project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.public_api_descriptor()
    })
    .unwrap()
}

fn direct_descriptor_and_build(
    fixture: &Fixture,
) -> (project::PublicApiDescriptor, ProjectNpmBuild) {
    project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        let descriptor = snapshot.public_api_descriptor()?;
        let build = snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)?;
        Ok((descriptor, build))
    })
    .unwrap()
}

#[test]
fn v5_returns_one_exact_descriptor_and_replayable_carrier_without_authority() {
    let fixture = Fixture::new("primary");
    let before = fixture.inventory();
    let (direct_descriptor, direct_build) = direct_descriptor_and_build(&fixture);
    direct_build
        .verify_public_api_descriptor(&direct_descriptor)
        .unwrap();

    let mut daemon = Daemon::start(&fixture, true, MAX_RESPONSE_BYTES);
    let protocol = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"protocol"}));
    assert_eq!(
        protocol["result"]["protocol"],
        "semaprax.agent-transport.v5"
    );
    assert_eq!(
        protocol["result"]["methods"],
        json!([
            "check",
            "context",
            "graph",
            "ping",
            "project/api-describe",
            "project/npm-build-inline",
            "protocol",
            "shutdown",
            "test",
            "workspace/open",
            "workspace/snapshot",
            "workspace/status"
        ])
    );
    for forbidden in [
        "build",
        "rename/preview",
        "rename/apply",
        "rename/derive",
        "change/preview",
        "change/apply",
        "impact",
        "review",
    ] {
        assert!(!protocol["result"]["methods"]
            .as_array()
            .unwrap()
            .contains(&json!(forbidden)));
    }

    let (project_revision, workspace_revision) = open(&mut daemon);
    let described = daemon.call(json!({
        "jsonrpc":"2.0","id":3,"method":"project/api-describe",
        "params":subject(&project_revision, &workspace_revision)
    }));
    assert_eq!(
        described["result"]["descriptor"],
        serde_json::from_slice::<Value>(&direct_descriptor.canonical_bytes()).unwrap()
    );
    assert_eq!(
        described["result"]["descriptor_digest"],
        direct_descriptor.digest()
    );

    let exact_carrier_bytes = direct_build.envelope().len();
    let too_small = daemon.call(json!({
        "jsonrpc":"2.0","id":4,"method":"project/npm-build-inline",
        "params":{
            "project_revision":project_revision,
            "workspace_revision":workspace_revision,
            "max_bytes":exact_carrier_bytes - 1
        }
    }));
    assert!(too_small.get("error").is_some());

    let raw = daemon.raw(json!({
        "jsonrpc":"2.0","id":5,"method":"project/npm-build-inline",
        "params":{
            "project_revision":project_revision,
            "workspace_revision":workspace_revision,
            "max_bytes":exact_carrier_bytes
        }
    }));
    let parsed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        parsed["result"]["descriptor"],
        described["result"]["descriptor"]
    );
    assert_eq!(
        parsed["result"]["descriptor_digest"],
        direct_descriptor.digest()
    );
    let returned = extract_build(&raw);
    ProjectNpmBuild::inspect_envelope(returned, exact_carrier_bytes).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(returned).unwrap(),
        serde_json::from_str::<Value>(direct_build.envelope()).unwrap()
    );

    for extra in [
        ("target", json!("rust")),
        ("output", json!("dist/npm")),
        ("path", json!("foreign/semaprax.toml")),
    ] {
        let mut params = subject(&project_revision, &workspace_revision);
        params
            .as_object_mut()
            .unwrap()
            .insert(extra.0.into(), extra.1);
        let rejected = daemon.call(json!({
            "jsonrpc":"2.0","id":6,"method":"project/npm-build-inline","params":params
        }));
        assert_eq!(rejected["error"]["code"], -32602);
    }
    let stale = daemon.call(json!({
        "jsonrpc":"2.0","id":7,"method":"project/api-describe","params":{
            "project_revision":"sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "workspace_revision":workspace_revision
        }
    }));
    assert_eq!(stale["error"]["code"], -32602);
    let forbidden = daemon.call(json!({
        "jsonrpc":"2.0","id":8,"method":"change/apply",
        "params":subject(&project_revision, &workspace_revision)
    }));
    assert_eq!(forbidden["error"]["code"], -32601);
    daemon.notify(json!({
        "jsonrpc":"2.0","method":"project/npm-build-inline",
        "params":subject(&project_revision, &workspace_revision)
    }));
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":9,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "open");
    daemon.finish();
    assert_eq!(fixture.inventory(), before);
}

#[test]
fn v5_rejects_decoys_foreign_remints_and_legacy_profile_confusion() {
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    let (first_descriptor, first_build) = direct_descriptor_and_build(&first);
    let (second_descriptor, second_build) = direct_descriptor_and_build(&second);
    assert!(first_build
        .verify_public_api_descriptor(&second_descriptor)
        .is_err());
    assert!(second_build
        .verify_public_api_descriptor(&first_descriptor)
        .is_err());

    let duplicate = first_build.envelope().replacen(
        "{\"schema\":",
        "{\"schema\":\"semaprax.project-npm-build.v7\",\"schema\":",
        1,
    );
    assert!(ProjectNpmBuild::inspect_envelope(&duplicate, duplicate.len()).is_err());
    let decoy = first_build.envelope().replacen(
        "{\"schema\":",
        &format!(
            "{{\"descriptor_digest\":{},\"schema\":",
            serde_json::to_string(&second_descriptor.digest()).unwrap()
        ),
        1,
    );
    assert!(ProjectNpmBuild::inspect_envelope(&decoy, decoy.len()).is_err());

    let mut legacy = Daemon::start(&first, false, MAX_RESPONSE_BYTES);
    let (project_revision, workspace_revision) = open(&mut legacy);
    for method in ["project/api-describe", "project/npm-build-inline"] {
        let rejected = legacy.call(json!({
            "jsonrpc":"2.0","id":10,"method":method,
            "params":subject(&project_revision, &workspace_revision)
        }));
        assert_eq!(rejected["error"]["code"], -32601);
    }
    legacy.finish();

    let legacy_project = Fixture::legacy("startup-confusion");
    let output = Command::new(env!("CARGO_BIN_EXE_semapraxd"))
        .arg("--stdio")
        .arg("--manifest-path")
        .arg(legacy_project.manifest())
        .arg("--allow-project-owned-data")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("Agent Transport v5 requires Project v8 owned-data-api.v1"));
}

#[test]
fn v5_descriptor_response_uses_the_exact_framed_capacity_boundary() {
    let fixture = Fixture::new("response-boundary");
    let descriptor = direct_descriptor(&fixture);
    let canonical = String::from_utf8(descriptor.canonical_bytes()).unwrap();
    let canonical = canonical.strip_suffix('\n').unwrap();
    let result = format!(
        "{{\"descriptor\":{},\"descriptor_digest\":{}}}",
        canonical,
        serde_json::to_string(&descriptor.digest()).unwrap(),
    );
    let exact_response = format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{result}}}").len() + 1;

    let mut exact = Daemon::start(&fixture, true, exact_response);
    let (project_revision, workspace_revision) = open(&mut exact);
    let raw = exact.raw(json!({
        "jsonrpc":"2.0","id":2,"method":"project/api-describe",
        "params":subject(&project_revision, &workspace_revision)
    }));
    assert_eq!(raw.len() + 1, exact_response);
    assert_eq!(
        serde_json::from_str::<Value>(&raw).unwrap()["result"],
        serde_json::from_str::<Value>(&result).unwrap()
    );
    exact.finish();

    let mut short = Daemon::start(&fixture, true, exact_response - 1);
    let (project_revision, workspace_revision) = open(&mut short);
    let raw = short.raw(json!({
        "jsonrpc":"2.0","id":2,"method":"project/api-describe",
        "params":subject(&project_revision, &workspace_revision)
    }));
    assert_eq!(raw.as_bytes(), b"{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32001,\"message\":\"response exceeds configured byte limit\"}}");
    drop(short.input);
    assert!(short.child.wait().unwrap().success());
}
