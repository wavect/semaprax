use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project;
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const BUILD_MAX_BYTES: usize = 512 * 1024;

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-workflow-v1-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for relative in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(relative), root.join(relative)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn config_validator(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-workflow-v2-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config-validator-project");
        for relative in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/rules.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(relative), root.join(relative)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

#[test]
fn daemon_v2_web_and_npm_targets_return_the_same_text_carrier() {
    let fixture = Fixture::config_validator("web-alias");
    let mut daemon = Daemon::start(&fixture);
    let opened = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
    let project_revision = opened["result"]["project_revision"].as_str().unwrap();
    let workspace_revision = opened["result"]["workspace_revision"].as_str().unwrap();
    let build = |id, target| {
        json!({
            "jsonrpc":"2.0","id":id,"method":"build","params":{
                "project_revision":project_revision,
                "workspace_revision":workspace_revision,
                "target":target,
                "max_bytes":project::MAX_PROJECT_NPM_BUILD_BYTES
            }
        })
    };
    let web = daemon.call(build(2, "web"));
    let npm = daemon.call(build(3, "npm"));
    assert_eq!(
        web["result"]["build"]["schema"],
        project::PROJECT_NPM_BUILD_SCHEMA
    );
    assert_eq!(web["result"]["build"], npm["result"]["build"]);
    daemon.finish();
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
    fn start(fixture: &Fixture) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semapraxd"))
            .arg("--stdio")
            .arg("--manifest-path")
            .arg(fixture.manifest())
            .arg("--allow-project-workflow")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn call(&mut self, request: Value) -> Value {
        self.input
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
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
        serde_json::from_str(response.trim_end()).unwrap()
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
        let status = self.child.wait().unwrap();
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(status.success(), "daemon failed: {stderr}");
    }

    fn finish_invalidated(mut self, expected: &str) {
        let response = self.call(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}));
        assert_eq!(response["result"]["ok"], true);
        drop(self.input);
        let status = self.child.wait().unwrap();
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(
            !status.success(),
            "invalidated daemon unexpectedly succeeded"
        );
        assert!(
            stderr.contains(expected),
            "missing `{expected}` in: {stderr}"
        );
    }
}

fn subject(project_revision: &str, workspace_revision: &str) -> Value {
    json!({
        "project_revision": project_revision,
        "workspace_revision": workspace_revision
    })
}

fn hex_bytes(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    let encoded = value.as_bytes();
    let mut result = Vec::with_capacity(encoded.len() / 2);
    let mut index = 0usize;
    while index < encoded.len() {
        let high = char::from(encoded[index]).to_digit(16).unwrap();
        let low = char::from(encoded[index + 1]).to_digit(16).unwrap();
        result.push(u8::try_from((high << 4) | low).unwrap());
        index += 2;
    }
    result
}

fn materialize_and_run(build: &Value) {
    let output = std::env::temp_dir().join(format!(
        "semaprax-project-workflow-inline-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&output).unwrap();
    let paths = build["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|artifact| {
            let path = artifact["path"].as_str().unwrap();
            std::fs::write(
                output.join(path),
                hex_bytes(artifact["content_hex"].as_str().unwrap()),
            )
            .unwrap();
            path
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "app.wasm",
            "semaprax.js",
            "semaprax.bindings.js",
            "semaprax.bindings.d.ts",
            "semaprax.scalar-exports.json",
            "package.json",
            "index.html",
        ]
    );
    std::fs::write(
        output.join("workflow-consumer.mjs"),
        r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { instantiateBytes } from "./semaprax.bindings.js";
const runtime = await instantiateBytes(await readFile("./app.wasm"));
assert.deepEqual(runtime.call("calculator.add", 19n, 23n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.divide", 84n, 2n), {ok:true,value:42n});
"#,
    )
    .unwrap();
    let node = Command::new("node")
        .arg("workflow-consumer.mjs")
        .current_dir(&output)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&output);
    assert!(
        node.status.success(),
        "inline stable-ID consumer failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
}

#[test]
fn daemon_derives_reviews_applies_and_rebuilds_the_stable_web_api() {
    let fixture = Fixture::new("vertical");
    let mut daemon = Daemon::start(&fixture);
    let protocol = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"protocol"}));
    assert_eq!(
        protocol["result"]["protocol"],
        "semaprax.agent-transport.v4"
    );
    for method in [
        "rename/derive",
        "change/preview",
        "impact",
        "review",
        "change/apply",
        "build",
    ] {
        assert!(protocol["result"]["methods"]
            .as_array()
            .unwrap()
            .contains(&json!(method)));
    }
    assert!(!protocol["result"]["methods"]
        .as_array()
        .unwrap()
        .contains(&json!("rename/preview")));
    assert!(!protocol["result"]["methods"]
        .as_array()
        .unwrap()
        .contains(&json!("rename/apply")));

    let opened = daemon.call(json!({"jsonrpc":"2.0","id":2,"method":"workspace/open"}));
    let base_project = opened["result"]["project_revision"].as_str().unwrap();
    let base_workspace = opened["result"]["workspace_revision"].as_str().unwrap();
    let held_v1_npm = daemon.call(json!({
        "jsonrpc":"2.0","id":18,"method":"build","params":{
            "project_revision":base_project,"workspace_revision":base_workspace,
            "target":"npm","max_bytes":BUILD_MAX_BYTES
        }
    }));
    assert!(held_v1_npm.get("error").is_some());
    assert!(held_v1_npm.get("result").is_none());
    let legacy = daemon.call(json!({
        "jsonrpc":"2.0","id":19,"method":"rename/preview","params":{
            "project_revision":base_project,"workspace_revision":base_workspace,
            "target_id":"calculator.add","from":"add","to":"bypass"
        }
    }));
    assert_eq!(legacy["error"]["code"], -32601);
    daemon.notify(json!({
        "jsonrpc":"2.0","method":"build","params":{
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "target":"web"
        }
    }));
    daemon.notify(json!({
        "jsonrpc":"2.0","method":"rename/derive","params":{
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "target_id":"calculator.add","from":"add","to":"ignored"
        }
    }));
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":20,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "open");
    let mut derive_params = subject(base_project, base_workspace);
    derive_params.as_object_mut().unwrap().extend([
        ("target_id".to_owned(), json!("calculator.add")),
        ("from".to_owned(), json!("add")),
        ("to".to_owned(), json!("sum")),
    ]);
    let derivation = daemon.call(json!({
        "jsonrpc":"2.0","id":3,"method":"rename/derive","params":derive_params
    }));
    assert_eq!(
        derivation["result"]["derivation"]["schema"],
        "semaprax.project-rename-derivation.v1"
    );
    let derivation_digest = derivation["result"]["derivation"]["artifact_digest"]
        .as_str()
        .unwrap();

    daemon.notify(json!({
        "jsonrpc":"2.0","method":"change/preview","params":{
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "derivation_digest":derivation_digest
        }
    }));
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":23,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "derived");

    let blocked_build = daemon.call(json!({
        "jsonrpc":"2.0","id":21,"method":"build","params":{
            "project_revision":base_project,"workspace_revision":base_workspace,
            "target":"web"
        }
    }));
    assert!(blocked_build["error"]["message"]
        .as_str()
        .unwrap()
        .contains("derived"));
    let stale_preview = daemon.call(json!({
        "jsonrpc":"2.0","id":22,"method":"change/preview","params":{
            "project_revision":base_project,"workspace_revision":base_workspace,
            "derivation_digest":"sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        }
    }));
    assert_eq!(stale_preview["error"]["code"], -32602);

    let preview = daemon.call(json!({
        "jsonrpc":"2.0","id":4,"method":"change/preview",
        "params":{
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "derivation_digest":derivation_digest
        }
    }));
    let change = &preview["result"]["change"];
    assert_eq!(change["schema"], "semaprax.project-change-preview.v1");
    assert_eq!(
        change["impact"]["schema"],
        "semaprax.project-change-impact.v1"
    );
    assert_eq!(
        change["review"]["schema"],
        "semaprax.project-change-review.v1"
    );
    assert_eq!(
        change["impact"]["base_dependency_impact"]["schema"],
        "semaprax.project-semantic-impact.v1"
    );
    assert!(change["impact"]["base_dependency_impact"]
        .get("workspace_manifest_schema")
        .is_none());
    assert_eq!(
        change["impact"]["conclusions"]["stable_identity_preserved"],
        true
    );
    let change_preview_digest = change["artifact_digest"].as_str().unwrap();
    let preview_digest = change["rename_preview"]["preview_digest"].as_str().unwrap();
    let candidate_project = change["rename_preview"]["candidate_project_revision"]
        .as_str()
        .unwrap();
    let candidate_workspace = change["rename_preview"]["candidate_workspace_revision"]
        .as_str()
        .unwrap();

    for method in ["impact", "review", "change/apply"] {
        daemon.notify(json!({
            "jsonrpc":"2.0","method":method,"params":{
                "project_revision":base_project,
                "workspace_revision":base_workspace,
                "change_preview_digest":change_preview_digest
            }
        }));
    }
    daemon.notify(json!({
        "jsonrpc":"2.0","method":"build","params":{
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "target":"web"
        }
    }));
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":24,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "prepared");
    assert!(std::fs::read_to_string(fixture.0.join("src/core.spx"))
        .unwrap()
        .contains("fn add("));

    for (id, method, schema) in [
        (5, "impact", "semaprax.project-change-impact.v1"),
        (6, "review", "semaprax.project-change-review.v1"),
    ] {
        let response = daemon.call(json!({
            "jsonrpc":"2.0","id":id,"method":method,
            "params":{
                "project_revision":base_project,
                "workspace_revision":base_workspace,
                "change_preview_digest":change_preview_digest
            }
        }));
        assert_eq!(response["result"][method]["schema"], schema);
    }

    let applied = daemon.call(json!({
        "jsonrpc":"2.0","id":7,"method":"change/apply",
        "params":{
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "change_preview_digest":change_preview_digest
        }
    }));
    assert_eq!(applied["result"]["applied"], true);
    assert_eq!(
        applied["result"]["change_preview_digest"],
        change_preview_digest
    );
    assert_eq!(applied["result"]["rename_preview_digest"], preview_digest);
    assert_eq!(
        applied["result"]["candidate_project_revision"],
        candidate_project
    );
    assert_eq!(
        applied["result"]["candidate_workspace_revision"],
        candidate_workspace
    );

    let build = daemon.call(json!({
        "jsonrpc":"2.0","id":8,"method":"build",
        "params":{
            "project_revision":candidate_project,
            "workspace_revision":candidate_workspace,
            "target":"web",
            "max_bytes":BUILD_MAX_BYTES
        }
    }));
    let build = &build["result"]["build"];
    assert_eq!(build["schema"], project::PROJECT_WEB_BUILD_SCHEMA);
    assert_eq!(build["project_revision"], candidate_project);
    let direct = project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.build_web_inline(BUILD_MAX_BYTES)
    })
    .unwrap();
    direct.verify().unwrap();
    assert_eq!(
        *build,
        serde_json::from_str::<Value>(direct.envelope()).unwrap()
    );
    materialize_and_run(build);

    let test = daemon.call(json!({
        "jsonrpc":"2.0","id":9,"method":"test",
        "params":{
            "project_revision":candidate_project,
            "workspace_revision":candidate_workspace
        }
    }));
    assert_eq!(test["result"]["command_succeeded"], true);
    daemon.finish();
}

#[test]
fn drift_after_derivation_invalidates_before_change_artifacts_can_render() {
    let fixture = Fixture::new("derived-drift");
    let mut daemon = Daemon::start(&fixture);
    let opened = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
    let project = opened["result"]["project_revision"].as_str().unwrap();
    let workspace = opened["result"]["workspace_revision"].as_str().unwrap();
    let derivation = daemon.call(json!({
        "jsonrpc":"2.0","id":2,"method":"rename/derive","params":{
            "project_revision":project,
            "workspace_revision":workspace,
            "target_id":"calculator.add",
            "from":"add",
            "to":"sum"
        }
    }));
    let derivation_digest = derivation["result"]["derivation"]["artifact_digest"]
        .as_str()
        .unwrap();

    let core = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&core).unwrap();
    std::fs::write(&core, format!("{source}\n// authenticated drift\n")).unwrap();
    let response = daemon.call(json!({
        "jsonrpc":"2.0","id":3,"method":"change/preview","params":{
            "project_revision":project,
            "workspace_revision":workspace,
            "derivation_digest":derivation_digest
        }
    }));
    assert_eq!(response["error"]["code"], -32000);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-J102"));
    assert!(response.get("result").is_none());
    let ping = daemon.call(json!({"jsonrpc":"2.0","id":4,"method":"ping"}));
    assert_eq!(ping["result"]["state"], "invalidated");
    daemon.finish_invalidated("SPX-J102");
}
