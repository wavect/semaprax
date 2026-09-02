//! Durable candidate CLI scenarios, authored and intentionally unrun.
#![cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectSemanticImage, SemanticChange,
};
use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    store: PathBuf,
    candidate: ProjectCandidate,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-archive-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let revision = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        let base = ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap();
        let change = SemanticChange::new(
            revision.project_revision(),
            &json!({"kind":"rename_declaration","target":"calculator.add","name":"plus"}),
        )
        .unwrap();
        let candidate = base.apply(base.candidate_digest(), &change).unwrap();
        std::fs::write(
            root.join("capsule.json"),
            candidate.recovery_capsule().unwrap(),
        )
        .unwrap();
        let store = root.join(".semaprax-candidates");
        std::fs::create_dir(&store).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            root,
            store,
            candidate,
        }
    }
    fn persist(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("project-candidate-persist")
            .arg(self.root.join("semaprax.toml"))
            .arg(self.root.join("capsule.json"))
            .arg(&self.store)
            .output()
            .unwrap()
    }
    fn load(&self, receipt: &Value) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("project-candidate-load")
            .arg(&self.store)
            .arg(receipt["archive_digest"].as_str().unwrap())
            .arg(receipt["candidate_digest"].as_str().unwrap())
            .output()
            .unwrap()
    }
    fn run_session(&self, policy: &Value, input: &[u8]) -> Output {
        let path = self.root.join("host.json");
        std::fs::write(&path, policy.to_string()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve-workspace")
            .arg(self.root.join("semaprax.toml"))
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn receipt(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.candidate-archive-store-receipt.v1"
    );
    assert_eq!(value["source_authority"], false);
    assert_eq!(value["commit_approval"], false);
    assert_eq!(value["current_source_admission"], false);
    value
}

#[test]
fn explicit_archive_store_survives_removed_original_sources_and_refuses_overwrite() {
    let fixture = Fixture::new();
    let saved = receipt(fixture.persist());
    assert_eq!(
        saved["candidate_digest"],
        fixture.candidate.candidate_digest()
    );
    let entry = fixture.store.join(format!(
        "{}.json",
        &saved["archive_digest"].as_str().unwrap()[7..]
    ));
    let before = std::fs::read(&entry).unwrap();
    let duplicate = fixture.persist();
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert_eq!(std::fs::read(&entry).unwrap(), before);
    std::fs::remove_dir_all(fixture.root.join("src")).unwrap();
    std::fs::remove_file(fixture.root.join("semaprax.toml")).unwrap();
    std::fs::remove_file(fixture.root.join("capsule.json")).unwrap();
    let loaded = fixture.load(&saved);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert_eq!(loaded.stdout, fixture.candidate.to_json().as_bytes());
    assert!(!fixture.root.join("src").exists());
    assert!(!fixture.root.join("semaprax.toml").exists());
    assert_eq!(std::fs::read(&entry).unwrap(), before);
}

#[test]
fn v3_host_policy_restores_historical_candidate_without_publication_authority() {
    let fixture = Fixture::new();
    let saved = receipt(fixture.persist());
    let path = fixture.root.join("src/app.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let ast = semaprax::parse(
        &source.replace("multiply(6, 7)", "multiply(6, 8)"),
        "src/app.spx",
    )
    .unwrap();
    let changed = semaprax::format::canonical(&ast);
    std::fs::write(&path, &changed).unwrap();
    let image = with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
    })
    .unwrap();
    let policy = json!({"schema":"semaprax.workspace-host-policy.v3","candidate_prepare":true,
        "diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null,"frontend_cache":true,
        "candidate_archives":[{"root":fixture.store,"archive_digest":saved["archive_digest"],"candidate_digest":saved["candidate_digest"]}]});
    let input = [
        json!({"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"candidate/query","params":{"image_revision":image.image_digest(),"candidate_revision":fixture.candidate.candidate_digest()}}),
        json!({"jsonrpc":"2.0","id":3,"method":"candidate/commit","params":{}}),
    ].iter().map(|value| format!("{value}\n")).collect::<String>();
    let output = fixture.run_session(&policy, input.as_bytes());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["result"]["image_revision"], image.image_digest());
    assert_eq!(
        rows[1]["result"]["payload"]["candidate_revision"],
        fixture.candidate.candidate_digest()
    );
    assert_eq!(rows[2]["error"]["code"], -32601);
    assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
}

#[test]
fn archive_policy_is_closed_bounded_and_not_an_rpc_grant() {
    let fixture = Fixture::new();
    let plain = json!({"schema":"semaprax.workspace-host-policy.v3","candidate_prepare":true,
        "diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null,"frontend_cache":false,"candidate_archives":[]});
    let selection = json!({"root":fixture.store,"archive_digest":format!("sha256:{}","0".repeat(64)),"candidate_digest":format!("sha256:{}","1".repeat(64))});
    let mut invalid = Vec::new();
    for schema in [
        "semaprax.workspace-host-policy.v1",
        "semaprax.workspace-host-policy.v2",
    ] {
        let mut value = plain.clone();
        value["schema"] = json!(schema);
        if schema.ends_with("v1") {
            value.as_object_mut().unwrap().remove("frontend_cache");
        }
        invalid.push(value);
    }
    let mut null = plain.clone();
    null["candidate_archives"] = Value::Null;
    invalid.push(null);
    let mut too_many = plain.clone();
    too_many["candidate_archives"] = json!(vec![selection.clone(); 17]);
    invalid.push(too_many);
    let mut duplicate = plain.clone();
    duplicate["candidate_archives"] = json!([selection, selection]);
    invalid.push(duplicate);
    let mut denied = plain.clone();
    denied["candidate_prepare"] = json!(false);
    denied["candidate_archives"] = json!([selection]);
    invalid.push(denied);
    for value in invalid {
        let output = fixture.run_session(&value, b"");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("SPX-G280"));
    }
    let frame = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"workspace/open\",\"params\":{\"candidate_archives\":[]}}\n";
    let output = fixture.run_session(&plain, frame);
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["error"]["code"], -32602);
}
