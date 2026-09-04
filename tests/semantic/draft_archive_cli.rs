//! Durable draft CLI and startup-policy evidence, authored and intentionally unrun.
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
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, ProjectSemanticImage,
};
use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    store: PathBuf,
    draft: ProjectCandidateDraft,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("project/src")).unwrap();
        let root = root.canonicalize().unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join("project").join(path)).unwrap();
        }
        let base = with_authenticated_project(&root.join("project/semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap();
        let draft = ProjectCandidateDraft::open(base).unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.add", "add")
            .unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.subtract", "subtract")
            .unwrap();
        let draft = draft
            .fill_hole(
                draft.draft_digest(),
                "add",
                &json!({"kind":"i64","value":17}),
            )
            .unwrap();
        std::fs::write(
            root.join("draft-capsule.json"),
            draft.recovery_capsule().unwrap(),
        )
        .unwrap();
        let store = root.join("archives");
        std::fs::create_dir(&store).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, store, draft }
    }
    fn manifest(&self) -> PathBuf {
        self.root.join("project/semaprax.toml")
    }
    fn persist(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("project-draft-persist")
            .arg(self.manifest())
            .arg(self.root.join("draft-capsule.json"))
            .arg(&self.store)
            .output()
            .unwrap()
    }
    fn load(&self, receipt: &Value) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("project-draft-load")
            .arg(&self.store)
            .arg(receipt["archive_digest"].as_str().unwrap())
            .arg(receipt["draft_digest"].as_str().unwrap())
            .output()
            .unwrap()
    }
    fn session(&self, policy: &Value, input: &str) -> Output {
        let policy_path = self.root.join("host.json");
        std::fs::write(&policy_path, policy.to_string()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve-workspace")
            .arg(self.manifest())
            .arg(policy_path)
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
    fn sources(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.root.join("project").join(path)).unwrap())
        .collect()
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
        "semaprax.candidate-draft-archive-store-receipt.v1"
    );
    assert_eq!(value["historical_source_snapshot"], true);
    for key in [
        "current_source_admission",
        "source_authority",
        "commit_approval",
    ] {
        assert_eq!(value[key], false);
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
fn policy() -> Value {
    json!({"schema":"semaprax.workspace-host-policy.v6","candidate_prepare":true,"diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null,"frontend_cache":false,"candidate_archives":[],"semantic_cache":false,"semantic_cache_entry":null,"draft_archives":[]})
}
fn selection(fixture: &Fixture, saved: &Value) -> Value {
    json!({"root":fixture.store,"archive_digest":saved["archive_digest"],"draft_digest":saved["draft_digest"]})
}

#[test]
fn help_describes_both_explicit_draft_store_commands_exactly_once() {
    // The bare invocation prints the guided page; the catalog is `help all`.
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["help", "all"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    for line in [
        "semaprax project-draft-persist <manifest> <draft-capsule.json> <store-root>\n",
        "semaprax project-draft-load <store-root> <archive-digest> <draft-digest>\n",
    ] {
        assert_eq!(stdout.matches(line).count(), 1);
    }
}

#[test]
fn explicit_draft_persist_and_load_survive_origin_removal_without_overwrite_or_source_recreation() {
    let fixture = Fixture::new();
    let source_bytes = fixture.sources();
    let saved = receipt(fixture.persist());
    assert_eq!(saved["draft_digest"], fixture.draft.draft_digest());
    assert_eq!(fixture.sources(), source_bytes);
    let entry = fixture.store.join(format!(
        "{}.json",
        &saved["archive_digest"].as_str().unwrap()[7..]
    ));
    let archive_bytes = std::fs::read(&entry).unwrap();
    let duplicate = fixture.persist();
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("SPX-G302"));
    assert_eq!(std::fs::read(&entry).unwrap(), archive_bytes);
    std::fs::remove_dir_all(fixture.root.join("project")).unwrap();
    std::fs::remove_file(fixture.root.join("draft-capsule.json")).unwrap();
    let loaded = fixture.load(&saved);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert_eq!(
        loaded.stdout,
        fixture
            .draft
            .summary(fixture.draft.draft_digest())
            .unwrap()
            .as_bytes()
    );
    assert!(!fixture.root.join("project").exists());
    assert!(!fixture.root.join("draft-capsule.json").exists());
    assert_eq!(std::fs::read(entry).unwrap(), archive_bytes);
}

#[test]
fn v6_startup_rebuilds_sibling_historical_partial_draft_and_completes_without_publication_grant() {
    let original = Fixture::new();
    let saved = receipt(original.persist());
    let ready = original
        .draft
        .fill_hole(
            original.draft.draft_digest(),
            "subtract",
            &json!({"kind":"i64","value":23}),
        )
        .unwrap();
    let complete = ready.complete(ready.draft_digest()).unwrap();
    let context: Value = serde_json::from_str(
        &original
            .draft
            .hole_context(original.draft.draft_digest(), "subtract")
            .unwrap(),
    )
    .unwrap();
    let summary: Value = serde_json::from_str(
        original
            .draft
            .summary(original.draft.draft_digest())
            .unwrap(),
    )
    .unwrap();
    std::fs::remove_dir_all(original.root.join("project")).unwrap();
    std::fs::remove_file(original.root.join("draft-capsule.json")).unwrap();
    let sibling = Fixture::new();
    let path = sibling.root.join("project/src/app.spx");
    let before = std::fs::read_to_string(&path).unwrap();
    let changed = before.replace("multiply(6, 7)", "multiply(6, 8)");
    assert_ne!(before, changed);
    let parsed = semaprax::parse(&changed, "src/app.spx").unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let sources = sibling.sources();
    let image = with_authenticated_project(&sibling.manifest(), |snapshot| {
        ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
    })
    .unwrap();
    let mut host = policy();
    host["draft_archives"] = json!([selection(&original, &saved)]);
    let input=[
        json!({"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"hole/query","params":{"image_revision":image.image_digest(),"draft_revision":saved["draft_digest"],"hole_id":"subtract"}}),
        json!({"jsonrpc":"2.0","id":3,"method":"candidate/query","params":{"image_revision":image.image_digest(),"candidate_revision":summary["last_valid_candidate_digest"]}}),
        json!({"jsonrpc":"2.0","id":4,"method":"hole/complete","params":{"image_revision":image.image_digest(),"draft_revision":saved["draft_digest"]}}),
        json!({"jsonrpc":"2.0","id":5,"method":"hole/fill","params":{"image_revision":image.image_digest(),"draft_revision":saved["draft_digest"],"hole_id":"subtract","expression":{"kind":"i64","value":23}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"hole/complete","params":{"image_revision":image.image_digest(),"draft_revision":ready.draft_digest()}}),
        json!({"jsonrpc":"2.0","id":7,"method":"candidate/commit","params":{}}),
        json!({"jsonrpc":"2.0","id":8,"method":"workspace/open","params":{}}),
    ].iter().map(|value|format!("{value}\n")).collect::<String>();
    let responses = rows(sibling.session(&host, &input));
    assert_eq!(responses.len(), 8);
    assert_eq!(
        responses[0]["result"]["image_revision"],
        image.image_digest()
    );
    assert_eq!(responses[1]["result"]["payload"], context);
    assert!(responses[2].get("error").is_some());
    assert!(responses[3].to_string().contains("SPX-G232"));
    assert_eq!(
        responses[4]["result"]["payload"]["draft_revision"],
        ready.draft_digest()
    );
    assert_eq!(
        responses[5]["result"]["payload"]["candidate_revision"],
        complete.candidate_digest()
    );
    assert_eq!(
        responses[5]["result"]["payload"]["base_revision"],
        saved["base_revision"]
    );
    assert_eq!(responses[5]["result"]["payload"]["source_authority"], false);
    assert_eq!(responses[5]["result"]["payload"]["tests"], "not_run");
    assert_eq!(responses[6]["error"]["code"], -32601);
    assert_eq!(
        responses[7]["result"]["image_revision"],
        image.image_digest()
    );
    assert_eq!(sibling.sources(), sources);
    assert!(!original.root.join("project").exists());
}

#[test]
fn v6_draft_selection_is_closed_bounded_explicit_and_cannot_be_requested_by_rpc() {
    let fixture = Fixture::new();
    let saved = receipt(fixture.persist());
    let selected = selection(&fixture, &saved);
    let plain = policy();
    let mut invalid = Vec::new();
    for version in 1..=5 {
        let mut prior = plain.clone();
        prior["schema"] = json!(format!("semaprax.workspace-host-policy.v{version}"));
        prior.as_object_mut().unwrap().remove("draft_archives");
        for (introduced, field) in [
            (2, "frontend_cache"),
            (3, "candidate_archives"),
            (4, "semantic_cache"),
            (5, "semantic_cache_entry"),
        ] {
            if version < introduced {
                prior.as_object_mut().unwrap().remove(field);
            }
        }
        let healthy = fixture.session(&prior, "");
        assert!(
            healthy.status.success(),
            "{}",
            String::from_utf8_lossy(&healthy.stderr)
        );
        assert!(healthy.stdout.is_empty());
        prior["draft_archives"] = json!([]);
        invalid.push(prior);
    }
    let mut missing = plain.clone();
    missing.as_object_mut().unwrap().remove("draft_archives");
    invalid.push(missing);
    let mut null = plain.clone();
    null["draft_archives"] = Value::Null;
    invalid.push(null);
    let mut excess = plain.clone();
    excess["draft_archives"] = json!(vec![selected.clone(); 17]);
    invalid.push(excess);
    let mut duplicate = plain.clone();
    duplicate["draft_archives"] = json!([selected.clone(), selected.clone()]);
    invalid.push(duplicate);
    let mut denied = plain.clone();
    denied["candidate_prepare"] = json!(false);
    denied["draft_archives"] = json!([selected.clone()]);
    invalid.push(denied);
    for (key, value) in [
        ("unknown", json!(true)),
        ("root", json!("relative")),
        ("archive_digest", json!("bad")),
        ("draft_digest", json!("bad")),
    ] {
        let mut wrong = selected.clone();
        wrong[key] = value;
        let mut host = plain.clone();
        host["draft_archives"] = json!([wrong]);
        invalid.push(host);
    }
    for host in invalid {
        let output = fixture.session(&host, "");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("SPX-G280"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let input=json!({"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{"draft_archives":[selected]}}).to_string()+"\n";
    let responses = rows(fixture.session(&plain, &input));
    assert_eq!(responses[0]["error"]["code"], -32602);
    let image = with_authenticated_project(&fixture.manifest(), |snapshot| {
        ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
    })
    .unwrap();
    let input=json!({"jsonrpc":"2.0","id":1,"method":"hole/query","params":{"image_revision":image.image_digest(),"draft_revision":saved["draft_digest"],"hole_id":"subtract"}}).to_string()+"\n";
    let responses = rows(fixture.session(&plain, &input));
    assert!(responses[0].get("error").is_some());
}

#[test]
fn explicit_load_rejects_wrong_draft_selector_without_output_or_store_mutation() {
    let fixture = Fixture::new();
    let saved = receipt(fixture.persist());
    let entry = fixture.store.join(format!(
        "{}.json",
        &saved["archive_digest"].as_str().unwrap()[7..]
    ));
    let bytes = std::fs::read(&entry).unwrap();
    let sources = fixture.sources();
    let mut wrong = saved.clone();
    wrong["draft_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    let output = fixture.load(&wrong);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G342"));
    assert_eq!(std::fs::read(entry).unwrap(), bytes);
    assert_eq!(fixture.sources(), sources);
}
