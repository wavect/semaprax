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
use semaprax::project::{
    with_authenticated_project, ProjectFrontendCache, ProjectFrontendSource, ProjectSemanticImage,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, MutexGuard,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
static FIXTURE_LIFETIME: Mutex<()> = Mutex::new(());
struct Fixture {
    _fixture_lifetime: MutexGuard<'static, ()>,
    root: PathBuf,
    store: PathBuf,
    compiler: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        Self::from_example("calculator-project")
    }

    fn from_example(example_name: &str) -> Self {
        // One case deliberately changes the installed image's link and write
        // authority. Keep every test-local install/mutate/execute lifetime
        // disjoint so Linux never observes a writable executable image.
        let fixture_lifetime = FIXTURE_LIFETIME
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "spx-hir-cache-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example_name);
        let paths = if example_name == "frame-payload-project" {
            [
                "semaprax.toml",
                "src/app.spx",
                "src/frame.spx",
                "src/tests.spx",
            ]
        } else {
            [
                "semaprax.toml",
                "src/app.spx",
                "src/core.spx",
                "src/tests.spx",
            ]
        };
        for path in paths {
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
        let compiler_stage = root.join(".semaprax-installed.stage");
        let source = std::fs::File::open(env!("CARGO_BIN_EXE_semaprax")).unwrap();
        let expected_length = source.metadata().unwrap().len();
        let mut source = std::io::BufReader::new(source);
        let mut staged = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&compiler_stage)
            .unwrap();
        assert_eq!(
            std::io::copy(&mut source, &mut staged).unwrap(),
            expected_length
        );
        staged.sync_all().unwrap();
        drop(staged);
        drop(source);
        std::fs::rename(&compiler_stage, &compiler).unwrap();
        std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!compiler_stage.exists());
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
            _fixture_lifetime: fixture_lifetime,
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
    fn evict(&self, digest: &str) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-evict")
            .arg(&self.store)
            .arg(digest)
            .output()
            .unwrap()
    }
    fn cold_open(&self) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-cold-open")
            .arg(self.root.join("semaprax.toml"))
            .output()
            .unwrap()
    }
    fn warm_open(&self, digest: &str) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-warm-open")
            .arg(self.root.join("semaprax.toml"))
            .arg(&self.store)
            .arg(digest)
            .output()
            .unwrap()
    }
    fn refresh(&self, digest: &str) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-refresh")
            .arg(self.root.join("semaprax.toml"))
            .arg(&self.store)
            .arg(digest)
            .output()
            .unwrap()
    }
    fn lifecycle(&self) -> Output {
        Command::new(&self.compiler)
            .arg("semantic-cache-lifecycle")
            .arg(self.root.join("semaprax.toml"))
            .arg(&self.store)
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
    fn semantic_cache_image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            let authenticated = snapshot.retain_revision();
            let sources = authenticated
                .sources()
                .iter()
                .map(|source| ProjectFrontendSource::new(source.path(), source.source()))
                .collect::<Result<Vec<_>, _>>()?;
            // Match the cold CLI's staged semantic-frontend retention route.
            // An ordinary uncached graph may use a different construction peak.
            let revision = ProjectFrontendCache::new_with_semantic_cache()
                .build(authenticated.manifest(), &sources)?
                .into_revision();
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
fn task_service_project_persists_and_restarts_with_checked_hir_reuse() {
    let fixture = Fixture::from_example("task-service-project");
    assert_eq!(fixture.initialize()["source_authority"], false);
    let receipt = fixture.persist();
    assert_eq!(receipt["schema"], "semaprax.semantic-cache-receipt.v1");
    assert_eq!(receipt["source_authority"], false);
    let digest = receipt["entry_digest"].as_str().unwrap();
    let restarted = value(fixture.load(digest));
    assert_eq!(
        restarted["schema"],
        "semaprax.project-semantic-cache-work.v1"
    );
    assert_eq!(restarted["work"]["modules_resolved"], 0);
    assert!(restarted["work"]["checked_HIR_reused"].as_u64().unwrap() >= 3);
    assert_eq!(restarted["work"]["full_cross_file_checks"], true);
    assert_eq!(restarted["work"]["full_link_and_profile_admission"], true);
}

#[test]
fn persisted_nonempty_loan_plan_replays_with_exact_zero_resolution_warm_accounting() {
    let fixture = Fixture::from_example("frame-payload-project");
    let frame_path = fixture.root.join("src/frame.spx");
    let frame = std::fs::read_to_string(&frame_path).unwrap()
        + r#"
@id("loan.consume-bytes")
fn consume_bytes(value: own Bytes) -> i64 { 7 }
@id("loan.projected")
fn projected() -> i64 {
    let source = [7u8, 8u8, 9u8];
    let owned = bytes_copy(array_as_slice(source));
    let parent = bytes_as_slice(owned);
    let child = byte_range(parent, 1usize, byte_len(parent));
    let sibling = bytes_as_slice(owned);
    let byte_observed = if byte_len(child) + byte_len(sibling) > 0usize { 1 } else { 0 };
    consume_bytes(owned) + byte_observed
}
"#;
    let frame_ast = semaprax::parse(&frame, "src/frame.spx").unwrap();
    std::fs::write(&frame_path, semaprax::format::canonical(&frame_ast)).unwrap();

    let tests_path = fixture.root.join("src/tests.spx");
    let tests = std::fs::read_to_string(&tests_path).unwrap();
    assert_eq!(tests.matches("if valid_ok && mismatch_ok").count(), 1);
    let tests = tests
        .replace(
            "module frame_payload.tests;",
            "module frame_payload.tests;\nuse function @id(\"loan.projected\") from frame_payload.frame as loan_projected;",
        )
        .replace(
            "if valid_ok && mismatch_ok",
            "if valid_ok && mismatch_ok && loan_projected() == 8",
        );
    let tests_ast = semaprax::parse(&tests, "src/tests.spx").unwrap();
    std::fs::write(&tests_path, semaprax::format::canonical(&tests_ast)).unwrap();

    assert_eq!(fixture.initialize()["source_authority"], false);
    let cold = value(fixture.cold_open());
    assert_eq!(cold["frontend_work"]["work"]["modules_resolved"], 3);
    assert_eq!(cold["frontend_work"]["work"]["checked_HIR_reused"], 0);
    let cold_image = fixture.semantic_cache_image();
    assert_eq!(
        cold_image.revision().project_revision(),
        cold["project_revision"].as_str().unwrap()
    );
    let projected = cold_image
        .revision()
        .test_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.projected")
        .expect("fixture test imports the loan-bearing function");
    assert!(projected.loan_plan.loans.len() >= 4);
    assert!(projected
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.parent.is_some()));

    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();
    let loaded = value(fixture.load(digest));
    warm(&loaded);
    let warm_report = value(fixture.warm_open(digest));
    assert_eq!(
        warm_report["schema"],
        "semaprax.semantic-cache-warm-open.v1"
    );
    assert_eq!(warm_report["store_effect"], "entry_read_only");
    warm(&warm_report["frontend_work"]);
    assert_eq!(cold["project_revision"], warm_report["project_revision"]);
    assert_eq!(cold["image_revision"], warm_report["image_revision"]);
    assert_eq!(
        cold_image.image_digest(),
        warm_report["image_revision"].as_str().unwrap()
    );
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
fn exact_eviction_preserves_source_and_rebuilds_the_same_warm_entry() {
    let fixture = Fixture::new();
    fixture.initialize();
    let paths = [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ];
    let source_before = paths.map(|path| std::fs::read(fixture.root.join(path)).unwrap());
    let first = fixture.persist();
    let digest = first["entry_digest"].as_str().unwrap();
    warm(&value(fixture.load(digest)));

    let removed = value(fixture.evict(digest));
    assert_eq!(removed["schema"], "semaprax.semantic-cache-eviction.v1");
    assert_eq!(removed["entry_digest"], digest);
    assert!(removed["envelope_bytes"].as_u64().unwrap() > first["payload_bytes"].as_u64().unwrap());
    assert_eq!(removed["entries_remaining"], 0);
    assert_eq!(removed["source_authority"], false);
    assert_eq!(removed["canonical_source_mutation"], false);
    assert_eq!(removed["publication_authority"], false);
    assert_eq!(removed["cache_management_effect"], "selected_entry_removed");
    let absent = fixture.load(digest);
    assert!(!absent.status.success());
    assert!(String::from_utf8_lossy(&absent.stderr).contains("SPX-G308"));
    let repeated = fixture.evict(digest);
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("SPX-G308"));
    assert_eq!(
        source_before,
        paths.map(|path| std::fs::read(fixture.root.join(path)).unwrap())
    );

    let rebuilt = fixture.persist();
    assert_eq!(rebuilt["entry_digest"], digest);
    assert_eq!(rebuilt["compiler_digest"], first["compiler_digest"]);
    assert_eq!(rebuilt["payload_bytes"], first["payload_bytes"]);
    warm(&value(fixture.load(digest)));
}

#[test]
fn lifecycle_receipt_binds_cold_restore_refresh_evict_and_identical_rebuild() {
    let fixture = Fixture::new();
    let paths = [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ];
    let source_before = paths.map(|path| std::fs::read(fixture.root.join(path)).unwrap());
    let report = value(fixture.lifecycle());
    assert_eq!(report["schema"], "semaprax.semantic-cache-lifecycle.v1");
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["canonical_source_mutation"], false);
    assert_eq!(report["publication_authority"], false);
    assert_eq!(report["execution"], false);
    assert_eq!(report["equivalence"]["project_revision_preserved"], true);
    assert_eq!(report["equivalence"]["image_revision_preserved"], true);
    assert_eq!(report["equivalence"]["cold_rebuild_work_identical"], true);
    assert!(report["payload_bytes"].as_u64().unwrap() > 0);
    assert!(report["envelope_bytes"].as_u64().unwrap() > report["payload_bytes"].as_u64().unwrap());
    let stages = report["stages"].as_array().unwrap();
    assert_eq!(stages.len(), 5);
    assert_eq!(stages[0]["stage"], "cold_open");
    assert_eq!(stages[0]["frontend_work"]["work"]["modules_resolved"], 3);
    assert_eq!(stages[1]["stage"], "authenticated_store_restore");
    warm(&stages[1]["frontend_work"]);
    assert_eq!(stages[2]["stage"], "same_revision_refresh");
    warm(&stages[2]["frontend_work"]);
    assert_eq!(stages[3]["stage"], "exact_eviction");
    assert_eq!(stages[3]["entries_remaining"], 0);
    assert_eq!(stages[4]["stage"], "cold_rebuild_after_eviction");
    assert_eq!(stages[0]["frontend_work"], stages[4]["frontend_work"]);
    assert_eq!(
        source_before,
        paths.map(|path| std::fs::read(fixture.root.join(path)).unwrap())
    );
    assert_eq!(
        std::fs::read_dir(&fixture.store).unwrap().count(),
        1,
        "successful lifecycle retains only the initialized store key"
    );
}

/// `semantic-cache-cold-open` and `semantic-cache-warm-open` are the two
/// fresh-process halves of the lifecycle: each is one standalone compiler
/// invocation on the same manifest, differing only in whether a persisted
/// cache is threaded in. Same product on unchanged source is the guarantee
/// that a caller timing them externally is comparing cache state alone, not
/// two different operations.
#[test]
fn cold_open_and_warm_open_agree_on_unchanged_source_and_isolate_cache_state() {
    let fixture = Fixture::new();
    fixture.initialize();
    let cold_before_persist = value(fixture.cold_open());
    assert_eq!(
        cold_before_persist["schema"],
        "semaprax.semantic-cache-cold-open.v1"
    );
    assert_eq!(cold_before_persist["store_effect"], "none");
    assert_eq!(
        cold_before_persist["frontend_work"]["work"]["modules_resolved"],
        3
    );
    assert_eq!(
        cold_before_persist["frontend_work"]["work"]["checked_HIR_reused"],
        0
    );

    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();
    let warm_report = value(fixture.warm_open(digest));
    assert_eq!(
        warm_report["schema"],
        "semaprax.semantic-cache-warm-open.v1"
    );
    assert_eq!(warm_report["store_effect"], "entry_read_only");
    warm(&warm_report["frontend_work"]);
    assert_eq!(
        warm_report["frontend_work"]["invalidated_sources"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    // Same manifest, same store entry, cache state is the only input that
    // differs between the two commands' processes.
    assert_eq!(
        cold_before_persist["project_revision"],
        warm_report["project_revision"]
    );
    assert_eq!(
        cold_before_persist["image_revision"],
        warm_report["image_revision"]
    );
    // Warm hits coexist with substantial remaining total work: the four
    // full_* phases still ran, so a warm report is not zero-cost reuse.
    for phase in [
        "full_source_verification",
        "full_HIR_validation",
        "full_cross_file_checks",
        "full_link_and_profile_admission",
    ] {
        assert_eq!(warm_report["frontend_work"]["work"][phase], true);
    }
}

/// A body-only edit to a module nothing else imports invalidates exactly
/// that module; its unrelated siblings remain a whole-module checked-HIR hit.
#[test]
fn warm_open_after_local_body_edit_reuses_unaffected_modules() {
    let fixture = Fixture::new();
    fixture.initialize();
    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();

    let path = fixture.root.join("src/app.spx");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("multiply(6, 7)", "multiply(7, 6)");
    let canonical = semaprax::format::canonical(&semaprax::parse(&changed, "src/app.spx").unwrap());
    std::fs::write(&path, &canonical).unwrap();

    let warm_report = value(fixture.warm_open(digest));
    let invalidated: Vec<&str> = warm_report["frontend_work"]["invalidated_sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(invalidated, ["src/app.spx"]);
    assert_eq!(warm_report["frontend_work"]["work"]["modules_resolved"], 1);
    assert_eq!(
        warm_report["frontend_work"]["work"]["checked_HIR_reused"],
        2
    );

    // A fresh cold open of the edited manifest lands on the same admitted
    // identity, but with strictly more resolution work: the warm path really
    // did skip work the cold path repeated, on the exact edited source.
    let cold_report = value(fixture.cold_open());
    assert_eq!(
        cold_report["project_revision"],
        warm_report["project_revision"]
    );
    assert_eq!(cold_report["image_revision"], warm_report["image_revision"]);
    assert_eq!(cold_report["frontend_work"]["work"]["modules_resolved"], 3);
    assert_eq!(
        cold_report["frontend_work"]["work"]["checked_HIR_reused"],
        0
    );
}

/// Editing the shared provider invalidates exactly the provider's own
/// AST-cache entry (#130/#131). `src/app.spx` and `src/tests.spx` import
/// `add` from `src/core.spx` but their own source bytes are untouched, so
/// their cached `Program` is reused instead of reparsed -- parsing a file is
/// a pure function of that file's own bytes, so a reused AST here is
/// bit-identical to a fresh reparse regardless of what the provider did.
/// Whether their checked HIR is *also* reused is a separate, independently
/// exact question (the checked-module cache's own `synthetic` equality
/// gate, unaffected by this AST-level narrowing); this test does not assume
/// an answer there, only that the AST-level reuse is exactly this narrow and
/// that the admitted product still matches an independent cold build on the
/// exact edited source.
#[test]
fn warm_open_after_provider_edit_reuses_unaffected_consumers_ast() {
    let fixture = Fixture::new();
    fixture.initialize();
    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();

    let path = fixture.root.join("src/core.spx");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("left + right", "right + left");
    let canonical =
        semaprax::format::canonical(&semaprax::parse(&changed, "src/core.spx").unwrap());
    std::fs::write(&path, &canonical).unwrap();

    let warm_report = value(fixture.warm_open(digest));
    let invalidated: Vec<&str> = warm_report["frontend_work"]["invalidated_sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(invalidated, ["src/core.spx"]);
    assert_eq!(warm_report["frontend_work"]["work"]["modules_parsed"], 1);
    assert_eq!(warm_report["frontend_work"]["work"]["modules_reused"], 2);
    // Function-level reuse for every call whose exact monomorphic
    // environment is unchanged still holds, so this stays strictly cheaper
    // than a cold open on the same edited source.
    assert!(
        warm_report["frontend_work"]["work"]["monomorphic_function_HIR_reused"]
            .as_u64()
            .unwrap()
            > 0
    );
    let cold_report = value(fixture.cold_open());
    // The two builds must land on the identical admitted identity: the
    // reused ASTs for app.spx/tests.spx did not hide the provider's change.
    assert_eq!(
        cold_report["project_revision"],
        warm_report["project_revision"]
    );
    assert_eq!(cold_report["image_revision"], warm_report["image_revision"]);
    assert_eq!(cold_report["frontend_work"]["work"]["modules_resolved"], 3);
    assert_eq!(
        cold_report["frontend_work"]["work"]["checked_HIR_reused"],
        0
    );
}

/// A changed Project can continue the explicit derived-cache chain without a
/// cold re-persist. The new entry is built only after current sources have
/// passed ordinary authenticated admission; the historical entry remains an
/// immutable retry/recovery input rather than a mutable cache slot.
#[test]
fn refresh_persists_the_authenticated_changed_generation_for_the_next_restart() {
    let fixture = Fixture::new();
    fixture.initialize();
    let first = fixture.persist();
    let predecessor = first["entry_digest"].as_str().unwrap();

    let path = fixture.root.join("src/app.spx");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("multiply(6, 7)", "multiply(7, 6)");
    let canonical = semaprax::format::canonical(&semaprax::parse(&changed, "src/app.spx").unwrap());
    std::fs::write(&path, &canonical).unwrap();

    let refreshed = value(fixture.refresh(predecessor));
    assert_eq!(refreshed["schema"], "semaprax.semantic-cache-refresh.v1");
    assert_eq!(refreshed["predecessor_entry_digest"], predecessor);
    assert_eq!(refreshed["source_authority"], false);
    assert_eq!(refreshed["live_source_admission"], true);
    assert_eq!(refreshed["canonical_source_mutation"], false);
    assert_eq!(refreshed["publication_authority"], false);
    assert_eq!(
        refreshed["store_effect"],
        "authenticated_entry_read_then_new_immutable_entry_persisted"
    );
    let successor = refreshed["entry_digest"].as_str().unwrap();
    assert_ne!(successor, predecessor);
    assert_eq!(
        refreshed["frontend_work"]["invalidated_sources"],
        json!(["src/app.spx"])
    );
    assert_eq!(refreshed["frontend_work"]["work"]["modules_resolved"], 1);
    assert_eq!(refreshed["frontend_work"]["work"]["checked_HIR_reused"], 2);

    // The refreshed entry is a full next-restart cache: it reaches zero
    // resolution on the exact changed sources. The predecessor remains valid
    // historical input, but cannot conceal the changed module when reopened.
    let warm_successor = value(fixture.warm_open(successor));
    warm(&warm_successor["frontend_work"]);
    assert_eq!(
        warm_successor["project_revision"],
        refreshed["project_revision"]
    );
    assert_eq!(
        warm_successor["image_revision"],
        refreshed["image_revision"]
    );
    let warm_predecessor = value(fixture.warm_open(predecessor));
    assert_eq!(
        warm_predecessor["frontend_work"]["invalidated_sources"],
        json!(["src/app.spx"])
    );
    assert_eq!(
        warm_predecessor["frontend_work"]["work"]["modules_resolved"],
        1
    );
}

/// A stale/evicted entry digest fails closed rather than silently falling
/// back to a cold open, and the caller's own explicit `cold_open` afterward
/// is the recovery path: same product as the original cold open, since
/// canonical source was never touched by the failed warm attempt or by
/// eviction.
#[test]
fn stale_or_evicted_entry_fails_closed_then_cold_open_recovers_the_same_product() {
    let fixture = Fixture::new();
    fixture.initialize();
    let original_cold = value(fixture.cold_open());
    let receipt = fixture.persist();
    let digest = receipt["entry_digest"].as_str().unwrap();
    warm(&value(fixture.warm_open(digest))["frontend_work"].clone());

    value(fixture.evict(digest));
    let stale = fixture.warm_open(digest);
    assert!(!stale.status.success());
    assert!(stale.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("SPX-G308"));

    let recovered = value(fixture.cold_open());
    assert_eq!(recovered, original_cold);
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
