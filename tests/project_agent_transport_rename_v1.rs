use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project;
use serde_json::{json, Value};

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
            "semaprax-project-rename-v1-{label}-{}-{}",
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

    fn core(&self) -> PathBuf {
        self.0.join("src/core.spx")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Output(PathBuf);

impl Output {
    fn fresh(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "semaprax-project-rename-output-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for Output {
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
    fn start(fixture: &Fixture, rename: bool, extra: &[&str]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_semapraxd"));
        command
            .arg("--stdio")
            .arg("--manifest-path")
            .arg(fixture.manifest());
        if rename {
            command.arg("--allow-project-rename");
        }
        let mut child = command
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

    fn call(&mut self, request: Value) -> Value {
        let request = serde_json::to_vec(&request).unwrap();
        self.input.write_all(&request).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut response = String::new();
        if self.output.read_line(&mut response).unwrap() == 0 {
            let status = self.child.wait().unwrap();
            let mut stderr = String::new();
            self.child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            panic!("daemon closed before response ({status}): {stderr}");
        }
        assert!(response.ends_with('\n'));
        serde_json::from_str(response.trim_end()).unwrap()
    }

    fn notify(&mut self, request: Value) {
        let request = serde_json::to_vec(&request).unwrap();
        self.input.write_all(&request).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
    }

    fn probe_terminal_eof(&mut self) {
        let _ = self
            .input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":999,\"method\":\"ping\"}\n");
        let _ = self.input.flush();
        let mut unexpected = String::new();
        assert_eq!(self.output.read_line(&mut unexpected).unwrap(), 0);
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

fn open(daemon: &mut Daemon, id: u64) -> (String, String) {
    let response = daemon.call(json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "workspace/open"
    }));
    (
        response["result"]["project_revision"]
            .as_str()
            .unwrap()
            .to_owned(),
        response["result"]["workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn subject(project_revision: &str, workspace_revision: &str) -> Value {
    json!({
        "project_revision": project_revision,
        "workspace_revision": workspace_revision
    })
}

fn preview_request(id: u64, project_revision: &str, workspace_revision: &str) -> Value {
    let mut params = subject(project_revision, workspace_revision);
    let params = params.as_object_mut().unwrap();
    params.insert("target_id".to_owned(), json!("calculator.add"));
    params.insert("from".to_owned(), json!("add"));
    params.insert("to".to_owned(), json!("sum"));
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "rename/preview",
        "params": params
    })
}

fn apply_request(
    id: u64,
    project_revision: &str,
    workspace_revision: &str,
    preview_digest: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "rename/apply",
        "params": {
            "project_revision": project_revision,
            "workspace_revision": workspace_revision,
            "preview_digest": preview_digest
        }
    })
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

fn build_web(fixture: &Fixture, output: &Output) {
    project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.build_web(&output.0)
    })
    .unwrap();
}

fn run_node_consumer(output: &Output) {
    std::fs::write(
        output.0.join("verify-rename.mjs"),
        r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { instantiateBytes } from "./semaprax.bindings.js";
const runtime = await instantiateBytes(await readFile("./app.wasm"));
assert.deepEqual(runtime.call("calculator.add", 19n, 23n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.subtract", 84n, 42n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.multiply", 6n, 7n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.divide", 84n, 2n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.is-negative", -1n), {ok:true,value:true});
assert.deepEqual(runtime.call("calculator.not", true), {ok:true,value:false});
"#,
    )
    .unwrap();
    let result = Command::new("node")
        .arg("verify-rename.mjs")
        .current_dir(&output.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stable-ID Node consumer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

struct ProjectRustSdk {
    _root: Fixture,
    project_revision: String,
    workspace_revision: String,
    project_subject_digest: String,
    source_revisions: Vec<String>,
}

fn project_rust_sdk_gate() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_PROJECT_NATIVE_RUST_SDK").as_deref()
        == Some(std::ffi::OsStr::new("1"))
}

fn run_project_rust_sdk(fixture: &Fixture, label: &str) -> ProjectRustSdk {
    let root = Fixture::new(&format!("rust-sdk-{label}"));
    let generated = root.0.join("generated-project-sdk");
    let consumer = root.0.join("project-consumer");
    std::fs::create_dir_all(consumer.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-rust");
    for relative in ["Cargo.toml", "src/main.rs"] {
        std::fs::copy(
            example.join("project-consumer").join(relative),
            consumer.join(relative),
        )
        .unwrap();
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let setup = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
        .arg(example.join("Cargo.toml"))
        .arg("--")
        .arg("project")
        .arg(fixture.manifest())
        .arg(&generated)
        .output()
        .unwrap();
    assert!(
        setup.status.success(),
        "generate {label} Project Rust SDK: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let stdout = String::from_utf8(setup.stdout).unwrap();
    let fields = stdout.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 6, "unexpected Project SDK setup output");

    let lock = Command::new(&cargo)
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "lock {label} Project Rust consumer: {}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let run = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "run {label} Project Rust consumer: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"42\n");

    let manifest: Value = serde_json::from_slice(
        &std::fs::read(generated.join("semaprax.native-rust-sdk.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["schema"], "semaprax.project-native-rust-sdk.v1");
    assert_eq!(manifest["project_subject"]["project_revision"], fields[3]);
    assert_eq!(manifest["project_subject"]["workspace_revision"], fields[4]);
    let sources = manifest["project_subject"]["sources"]
        .as_array()
        .expect("Project SDK manifest must carry exact source facts");
    assert_eq!(sources.len(), PROJECT_FILES.len() - 1);
    let source_revisions = sources
        .iter()
        .map(|source| {
            source["source_revision"]
                .as_str()
                .expect("Project SDK source revision must be a string")
                .to_owned()
        })
        .collect::<Vec<_>>();
    let descriptor: Value =
        serde_json::from_slice(&std::fs::read(generated.join("native/descriptor.json")).unwrap())
            .unwrap();
    assert_eq!(
        descriptor["schema"],
        "semaprax.project-native-rust-interop-descriptor.v1"
    );
    assert_eq!(descriptor["project_subject_digest"], fields[5]);

    ProjectRustSdk {
        _root: root,
        project_revision: fields[3].to_owned(),
        workspace_revision: fields[4].to_owned(),
        project_subject_digest: fields[5].to_owned(),
        source_revisions,
    }
}

#[test]
fn project_rename_transaction_refreshes_the_exact_project_and_preserves_web_api() {
    let fixture = Fixture::new("vertical");
    let baseline_web = Output::fresh("baseline");
    let renamed_web = Output::fresh("renamed");
    build_web(&fixture, &baseline_web);
    run_node_consumer(&baseline_web);
    let baseline_rust = project_rust_sdk_gate().then(|| run_project_rust_sdk(&fixture, "baseline"));

    let mut daemon = Daemon::start(&fixture, true, &[]);
    let protocol = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"protocol"}));
    assert_eq!(
        protocol["result"]["protocol"],
        "semaprax.agent-transport.v3"
    );
    let methods = protocol["result"]["methods"].as_array().unwrap();
    assert!(methods.contains(&json!("rename/preview")));
    assert!(methods.contains(&json!("rename/apply")));
    assert!(!methods.contains(&json!("change/apply")));

    let (base_project, base_workspace) = open(&mut daemon, 2);
    if let Some(baseline_rust) = &baseline_rust {
        assert_eq!(baseline_rust.project_revision, base_project);
        assert_eq!(baseline_rust.workspace_revision, base_workspace);
    }
    let preview = daemon.call(preview_request(3, &base_project, &base_workspace));
    let preview = &preview["result"]["preview"];
    assert_eq!(preview["schema"], "semaprax.project-rename-preview.v1");
    assert_eq!(preview["target"]["stable_id"], "calculator.add");
    assert_eq!(preview["target"]["from"], "add");
    assert_eq!(preview["target"]["to"], "sum");
    assert_eq!(preview["target"]["path"], "src/core.spx");
    assert_ne!(
        preview["base_project_revision"],
        preview["candidate_project_revision"]
    );
    assert_ne!(
        preview["base_workspace_revision"],
        preview["candidate_workspace_revision"]
    );
    let preview_digest = preview["preview_digest"].as_str().unwrap().to_owned();
    let candidate_project = preview["candidate_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate_workspace = preview["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate_source = preview["candidate_source"]["source_revision"]
        .as_str()
        .unwrap()
        .to_owned();

    let receipt = daemon.call(apply_request(
        4,
        &base_project,
        &base_workspace,
        &preview_digest,
    ));
    assert_eq!(receipt["result"]["applied"], true);
    assert_eq!(receipt["result"]["preview_digest"], preview_digest);
    assert_eq!(
        receipt["result"]["candidate_project_revision"],
        candidate_project
    );
    assert_eq!(
        receipt["result"]["candidate_workspace_revision"],
        candidate_workspace
    );
    assert_eq!(
        receipt["result"]["candidate_source_revision"],
        candidate_source
    );

    let core = std::fs::read_to_string(fixture.core()).unwrap();
    assert!(core.contains("@id(\"calculator.add\")\nfn sum("));
    assert!(!core.contains("fn add("));

    let graph = daemon.call(json!({
        "jsonrpc":"2.0", "id":5, "method":"graph",
        "params": subject(&candidate_project, &candidate_workspace)
    }));
    assert_eq!(
        graph["result"]["graph"]["project_revision"],
        candidate_project
    );
    assert_eq!(
        graph["result"]["graph"]["workspace_revision"],
        candidate_workspace
    );
    assert!(graph["result"]["graph"]["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|declaration| declaration["id"] == "calculator.add"));

    let context = daemon.call(json!({
        "jsonrpc":"2.0", "id":6, "method":"context",
        "params": {
            "project_revision": candidate_project,
            "workspace_revision": candidate_workspace,
            "target_kind":"declaration",
            "target":"calculator.add"
        }
    }));
    assert_eq!(
        context["result"]["context"]["schema"],
        "semaprax.project-semantic-context.v1"
    );
    assert_eq!(
        context["result"]["context"]["project_revision"],
        candidate_project
    );

    let test = daemon.call(json!({
        "jsonrpc":"2.0", "id":7, "method":"test",
        "params": subject(&candidate_project, &candidate_workspace)
    }));
    assert_eq!(test["result"]["command_succeeded"], true);

    let snapshot = daemon.call(json!({
        "jsonrpc":"2.0", "id":8, "method":"workspace/snapshot",
        "params": subject(&candidate_project, &candidate_workspace)
    }));
    assert!(snapshot["result"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| {
            source["path"] == "src/core.spx" && source["source_revision"] == candidate_source
        }));
    let stale = daemon.call(json!({
        "jsonrpc":"2.0", "id":9, "method":"check",
        "params": subject(&base_project, &base_workspace)
    }));
    assert_eq!(stale["error"]["code"], -32602);

    let shutdown = daemon.call(json!({"jsonrpc":"2.0","id":10,"method":"shutdown"}));
    assert_eq!(shutdown["result"]["ok"], true);
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");
    assert!(stderr.is_empty());

    build_web(&fixture, &renamed_web);
    for artifact in ["app.wasm", "semaprax.bindings.js", "semaprax.bindings.d.ts"] {
        assert_eq!(
            std::fs::read(baseline_web.0.join(artifact)).unwrap(),
            std::fs::read(renamed_web.0.join(artifact)).unwrap(),
            "display rename changed stable-ID Web artifact {artifact}"
        );
    }
    run_node_consumer(&renamed_web);
    if let Some(baseline_rust) = baseline_rust {
        let renamed_rust = run_project_rust_sdk(&fixture, "renamed");
        assert_eq!(renamed_rust.project_revision, candidate_project);
        assert_eq!(renamed_rust.workspace_revision, candidate_workspace);
        assert_ne!(
            baseline_rust.project_revision,
            renamed_rust.project_revision
        );
        assert_ne!(
            baseline_rust.workspace_revision,
            renamed_rust.workspace_revision
        );
        assert_ne!(
            baseline_rust.project_subject_digest,
            renamed_rust.project_subject_digest
        );
        assert_ne!(
            baseline_rust.source_revisions,
            renamed_rust.source_revisions
        );
        assert!(renamed_rust.source_revisions.contains(&candidate_source));
    }
}

#[test]
fn rename_methods_are_opt_in_and_unknown_change_authority_stays_closed() {
    let fixture = Fixture::new("method-isolation");
    let before = inventory(&fixture.0);
    let mut daemon = Daemon::start(&fixture, false, &[]);
    let protocol = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"protocol"}));
    assert_eq!(
        protocol["result"]["protocol"],
        "semaprax.agent-transport.v2"
    );
    assert!(!protocol["result"]["methods"]
        .as_array()
        .unwrap()
        .contains(&json!("rename/preview")));
    let (project, workspace) = open(&mut daemon, 2);
    let rejected = daemon.call(preview_request(3, &project, &workspace));
    assert_eq!(rejected["error"]["code"], -32601);
    let unknown = daemon.call(json!({
        "jsonrpc":"2.0", "id":4, "method":"change/apply", "params":{}
    }));
    assert_eq!(unknown["error"]["code"], -32601);
    daemon.call(json!({"jsonrpc":"2.0","id":5,"method":"shutdown"}));
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");
    assert!(stderr.is_empty());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn stale_notifications_and_preview_digest_mismatch_are_no_write() {
    let fixture = Fixture::new("no-write-rejections");
    let before = inventory(&fixture.0);
    let mut daemon = Daemon::start(&fixture, true, &[]);
    let (project, workspace) = open(&mut daemon, 1);

    let stale = daemon.call(preview_request(2, "sha256:stale", &workspace));
    assert_eq!(stale["error"]["code"], -32602);
    assert_eq!(inventory(&fixture.0), before);

    let mut notification = preview_request(0, &project, &workspace);
    notification.as_object_mut().unwrap().remove("id");
    daemon.notify(notification);
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":3,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "open");
    assert_eq!(inventory(&fixture.0), before);

    let preview = daemon.call(preview_request(4, &project, &workspace));
    let digest = preview["result"]["preview"]["preview_digest"]
        .as_str()
        .unwrap();
    let mismatch = daemon.call(apply_request(5, &project, &workspace, "sha256:mismatch"));
    assert_eq!(mismatch["error"]["code"], -32602);
    assert_eq!(inventory(&fixture.0), before);

    let mut apply_notification = apply_request(0, &project, &workspace, digest);
    apply_notification.as_object_mut().unwrap().remove("id");
    daemon.notify(apply_notification);
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":6,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "prepared");
    assert_eq!(inventory(&fixture.0), before);

    let reopen = daemon.call(json!({"jsonrpc":"2.0","id":7,"method":"workspace/open"}));
    assert_eq!(reopen["error"]["code"], -32000);
    let still_prepared = daemon.call(json!({"jsonrpc":"2.0","id":8,"method":"ping"}));
    assert_eq!(still_prepared["result"]["state"], "prepared");
    assert_eq!(inventory(&fixture.0), before);

    daemon.call(json!({"jsonrpc":"2.0","id":9,"method":"shutdown"}));
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");
    assert!(stderr.is_empty());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn preview_response_overflow_is_terminal_and_never_writes() {
    let fixture = Fixture::new("response-cap");
    let before = inventory(&fixture.0);
    let mut daemon = Daemon::start(&fixture, true, &["--max-response-bytes", "512"]);
    let (project, workspace) = open(&mut daemon, 1);
    let overflow = daemon.call(preview_request(2, &project, &workspace));
    assert_eq!(overflow["error"]["code"], -32001);
    daemon.probe_terminal_eof();
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");
    assert!(stderr.is_empty());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn a0_lock_contention_rejects_apply_without_source_or_plan_loss() {
    let fixture = Fixture::new("lock-contention");
    let before = inventory(&fixture.0);
    let mut daemon = Daemon::start(&fixture, true, &[]);
    let (project, workspace) = open(&mut daemon, 1);
    let preview = daemon.call(preview_request(2, &project, &workspace));
    let digest = preview["result"]["preview"]["preview_digest"]
        .as_str()
        .unwrap();
    let lock = fixture.0.join("src/.core.spx.semaprax-patch.lock");
    std::fs::write(&lock, b"foreign lock").unwrap();

    let rejected = daemon.call(apply_request(3, &project, &workspace, digest));
    assert_eq!(rejected["error"]["code"], -32000);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-I205"));
    assert_eq!(
        std::fs::read_to_string(fixture.core()).unwrap(),
        String::from_utf8(before["src/core.spx"].clone()).unwrap()
    );
    let state = daemon.call(json!({"jsonrpc":"2.0","id":4,"method":"ping"}));
    assert_eq!(state["result"]["state"], "prepared");

    std::fs::remove_file(lock).unwrap();
    daemon.call(json!({"jsonrpc":"2.0","id":5,"method":"shutdown"}));
    let (status, stderr) = daemon.finish();
    assert!(status.success(), "daemon failed: {stderr}");
    assert!(stderr.is_empty());
    assert_eq!(inventory(&fixture.0), before);
}

#[cfg(unix)]
#[test]
fn same_byte_target_substitution_after_preview_invalidates_without_commit() {
    let fixture = Fixture::new("same-byte-target-substitution");
    let before = std::fs::read(fixture.core()).unwrap();
    let mut daemon = Daemon::start(&fixture, true, &[]);
    let (project, workspace) = open(&mut daemon, 1);
    let preview = daemon.call(preview_request(2, &project, &workspace));
    let digest = preview["result"]["preview"]["preview_digest"]
        .as_str()
        .unwrap();

    let replacement = fixture.0.join("src/core.replacement");
    std::fs::write(&replacement, &before).unwrap();
    std::fs::rename(&replacement, fixture.core()).unwrap();
    let rejected = daemon.call(apply_request(3, &project, &workspace, digest));
    assert_eq!(rejected["error"]["code"], -32000);
    assert!(rejected["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J102"));
    assert_eq!(std::fs::read(fixture.core()).unwrap(), before);
    let absorbing = daemon.call(
        json!({"jsonrpc":"2.0","id":4,"method":"check","params":subject(&project,&workspace)}),
    );
    assert_eq!(absorbing["error"]["code"], -32000);
    daemon.notify(json!({"jsonrpc":"2.0","method":"shutdown"}));
    let (status, stderr) = daemon.finish();
    assert!(!status.success());
    assert!(stderr.contains("SPX-J102"));
}
