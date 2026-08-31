//! Cross-process cache evidence using a separately installed compiler image.
#![cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
use semaprax::project::{with_authenticated_project, ProjectSemanticImage};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    store: PathBuf,
    compiler: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-hir-cache-cli-{}-{}",
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
        let store = root.join(".semaprax-semantic-cache");
        std::fs::create_dir(&store).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Cargo may hard-link its public binary to debug/deps on Linux. That
        // build artifact is not the immutable single-link installation required
        // by the cache contract. Install identical bytes without altering Cargo's
        // artifact, and use that same installed image for every subprocess.
        let compiler = root.join("semaprax-installed");
        std::fs::copy(env!("CARGO_BIN_EXE_semaprax"), &compiler).unwrap();
        std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o555)).unwrap();
        let metadata = std::fs::metadata(&compiler).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.permissions().mode() & 0o777, 0o555);
        assert!(metadata.len() > 0);
        assert!(
            metadata.len()
                <= semaprax::semantic_cache_store::MAX_SEMANTIC_CACHE_COMPILER_BYTES as u64,
            "cache fixture requires a compiler within the existing 256-MiB bound; build with debug=0"
        );
        Self {
            root,
            store,
            compiler,
        }
    }
    fn initialize(&self) -> Value {
        value(
            Command::new(&self.compiler)
                .arg("semantic-cache-init")
                .arg(&self.store)
                .output()
                .unwrap(),
        )
    }
    fn persist(&self) -> Value {
        value(self.persist_output())
    }
    fn persist_output(&self) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-persist")
            .arg(self.root.join("semaprax.toml"))
            .arg(&self.store)
            .output()
            .unwrap()
    }
    fn load(&self, digest: &str) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-load")
            .arg(&self.store)
            .arg(digest)
            .output()
            .unwrap()
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            ProjectSemanticImage::derive(revision.clone(), revision.project_revision())
        })
        .unwrap()
    }
    fn session(&self, policy: &Value, input: &str) -> Output {
        let policy_path = self.root.join("host.json");
        std::fs::write(&policy_path, policy.to_string()).unwrap();
        let mut process = Command::new(&self.compiler)
            .arg("serve-workspace")
            .arg(self.root.join("semaprax.toml"))
            .arg(policy_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        process
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        process.wait_with_output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn value(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
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
fn policy(version: u8) -> Value {
    let mut value = json!({"schema":format!("semaprax.workspace-host-policy.v{version}"),"candidate_prepare":false,"diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null});
    if version >= 2 {
        value["frontend_cache"] = json!(true);
    }
    if version >= 3 {
        value["candidate_archives"] = json!([]);
    }
    if version >= 4 {
        value["semantic_cache"] = json!(true);
    }
    if version >= 5 {
        value["semantic_cache_entry"] = Value::Null;
    }
    value
}
fn warm(report: &Value) {
    assert_eq!(report["schema"], "semaprax.project-semantic-cache-work.v1");
    assert_eq!(report["work"]["modules_resolved"], 0);
    assert_eq!(report["work"]["checked_HIR_reused"], 3);
    assert_eq!(report["work"]["full_cross_file_checks"], true);
    assert_eq!(report["work"]["full_link_and_profile_admission"], true);
}

#[test]
fn separate_process_load_reuses_hir_and_live_startup_rechecks_edited_source() {
    let fixture = Fixture::new();
    let old_image = fixture.image();
    assert_eq!(fixture.initialize()["source_authority"], false);
    let receipt = fixture.persist();
    assert_eq!(receipt["schema"], "semaprax.semantic-cache-receipt.v1");
    assert_eq!(receipt["source_authority"], false);
    assert_eq!(receipt["current_source_admission"], false);
    assert!(receipt["payload_bytes"].as_u64().unwrap() > 0);
    let digest = receipt["entry_digest"].as_str().unwrap();
    let historical = value(fixture.load(digest));
    warm(&historical);
    let path = fixture.root.join("src/app.spx");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("multiply(6, 7)", "multiply(6, 8)");
    let canonical = semaprax::format::canonical(&semaprax::parse(&changed, "src/app.spx").unwrap());
    std::fs::write(&path, &canonical).unwrap();
    assert_eq!(value(fixture.load(digest)), historical); // Historical cache, not live admission.
    let current = fixture.image();
    assert_ne!(current.image_digest(), old_image.image_digest());
    let mut selected = policy(5);
    selected["semantic_cache_entry"] = json!({"root":fixture.store,"entry_digest":digest});
    let revision: Value = serde_json::from_str(current.to_json()).unwrap();
    let input = [
        json!({"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"workspace/refresh","params":{"image_revision":current.image_digest(),"expected_new_project_revision":revision["project_revision"]}}),
        json!({"jsonrpc":"2.0","id":3,"method":"workspace/open","params":{"semantic_cache_entry":{"root":fixture.store,"entry_digest":digest}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"candidate/commit","params":{}}),
    ].iter().map(|row| format!("{row}\n")).collect::<String>();
    let observed = rows(fixture.session(&selected, &input));
    assert_eq!(observed.len(), 4);
    assert_eq!(
        observed[0]["result"]["image_revision"],
        current.image_digest()
    );
    assert_eq!(observed[1]["result"]["payload"]["source_authority"], false);
    warm(&observed[1]["result"]["payload"]["frontend_work"]);
    assert_eq!(observed[2]["error"]["code"], -32602);
    assert_eq!(observed[3]["error"]["code"], -32601);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), canonical);
    // Cache deletion does not remove canonical sources or prevent explicit cold mode.
    std::fs::remove_dir_all(&fixture.store).unwrap();
    let cold = rows(fixture.session(
        &policy(1),
        &format!(
            "{}\n",
            json!({"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{}})
        ),
    ));
    assert_eq!(cold[0], observed[0]);
}

#[test]
fn reminting_public_digest_does_not_authenticate_changed_private_payload() {
    let fixture = Fixture::new();
    fixture.initialize();
    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();
    let mut bytes = std::fs::read(fixture.store.join(format!("{}.bin", &digest[7..]))).unwrap();
    let context_len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let payload_start = 44 + context_len + 8;
    assert!(payload_start < bytes.len() - 32);
    bytes[payload_start] ^= 1;
    let hex = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let reminted = format!("sha256:{hex}");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(fixture.store.join(format!("{hex}.bin")))
        .unwrap();
    file.write_all(&bytes).unwrap();
    drop(file);
    let rejected = fixture.load(&reminted);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("SPX-G309"));
    warm(&value(fixture.load(digest)));
}

#[test]
fn persisted_cache_is_bound_to_exact_executable_not_only_package_version() {
    let current = std::env::current_exe().unwrap();
    let bound = semaprax::semantic_cache_store::MAX_SEMANTIC_CACHE_COMPILER_BYTES as u64;
    let metadata = std::fs::metadata(current).unwrap();
    assert!(metadata.len() <= bound, "build cache evidence with debug=0");
    assert_eq!(metadata.nlink(), 1);
    let fixture = Fixture::new();
    fixture.initialize();
    let receipt = fixture.persist();
    let errors = semaprax::semantic_cache_store::load(
        &fixture.store,
        receipt["entry_digest"].as_str().unwrap(),
    )
    .err()
    .expect("test harness is not the sealing CLI executable");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-G308");
    assert_eq!(
        errors[0].message,
        "semantic cache version or exact compiler file identity differs"
    );
}

#[test]
fn compiler_hard_links_and_write_authority_reject_before_cache_publication() {
    let fixture = Fixture::new();
    fixture.initialize();
    let key_path = fixture.store.join("compiler-cache.key");
    let key = std::fs::read(&key_path).unwrap();
    let rejected = || {
        let output = fixture.persist_output();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G308"));
        assert_eq!(std::fs::read_dir(&fixture.store).unwrap().count(), 1);
        assert_eq!(std::fs::read(&key_path).unwrap(), key);
    };
    let alias = fixture.root.join("compiler-hard-link");
    std::fs::hard_link(&fixture.compiler, &alias).unwrap();
    assert_eq!(std::fs::metadata(&fixture.compiler).unwrap().nlink(), 2);
    rejected();
    std::fs::remove_file(alias).unwrap();
    assert_eq!(std::fs::metadata(&fixture.compiler).unwrap().nlink(), 1);
    std::fs::set_permissions(&fixture.compiler, std::fs::Permissions::from_mode(0o575)).unwrap();
    rejected();
    std::fs::set_permissions(&fixture.compiler, std::fs::Permissions::from_mode(0o555)).unwrap();
    let receipt = fixture.persist();
    warm(&value(
        fixture.load(receipt["entry_digest"].as_str().unwrap()),
    ));
}

#[test]
fn persisted_selection_is_closed_startup_policy_not_a_legacy_extension() {
    let fixture = Fixture::new();
    for version in 1..=4 {
        let mut selected = policy(version);
        selected["semantic_cache_entry"] = Value::Null;
        let rejected = fixture.session(&selected, "");
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("SPX-G280"));
    }
    let mut selected = policy(5);
    selected
        .as_object_mut()
        .unwrap()
        .remove("semantic_cache_entry");
    assert!(!fixture.session(&selected, "").status.success());
    let mut selected = policy(5);
    selected["semantic_cache"] = json!(false);
    selected["semantic_cache_entry"] =
        json!({"root":fixture.store,"entry_digest":format!("sha256:{}", "0".repeat(64))});
    let rejected = fixture.session(&selected, "");
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("SPX-G280"));
}
