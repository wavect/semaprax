use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[path = "project_sdk_cli/indexed_diagnostics.rs"]
mod indexed_diagnostics;
#[path = "project_sdk_cli/indexed_prepare.rs"]
mod indexed_prepare;

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = parent.join(format!(
            "semaprax-project-sdk-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let metadata = fs::symlink_metadata(&self.0).unwrap();
        assert_eq!(self.0.parent(), Some(parent.as_path()));
        assert!(self
            .0
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("semaprax-project-sdk-cli-"));
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_semaprax-native-rust-sdk"))
}

fn invoke(arguments: &[&str]) -> Output {
    binary().args(arguments).output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn absolute_missing(label: &str) -> (TestRoot, PathBuf) {
    let root = TestRoot::new();
    let path = root.0.join(label);
    (root, path)
}

#[test]
fn closed_parser_rejects_arity_duplicates_unknowns_and_missing_values_exactly() {
    for arguments in [
        vec![],
        vec!["project"],
        vec!["other", "--manifest-path", "unused", "--output", "unused"],
        vec!["project", "--manifest-path", "unused"],
    ] {
        let output = invoke(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "SPX-B112: expected `project --manifest-path <path> --output <fresh-absolute-path>`\n"
        );
        assert!(output.stdout.is_empty());
    }

    for arguments in [
        vec!["project", "--manifest-path"],
        vec!["project", "--manifest-path", "--output", "unused"],
        vec!["project", "--output"],
        vec!["project", "--manifest-path", "", "--output", "unused"],
        vec!["project", "--manifest-path", "unused", "--output", ""],
    ] {
        let output = invoke(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "SPX-B112: Native Rust SDK option requires a value\n"
        );
        assert!(output.stdout.is_empty());
    }

    for arguments in [
        vec![
            "project",
            "--manifest-path",
            "a",
            "--manifest-path",
            "b",
            "--output",
            "unused",
        ],
        vec![
            "project",
            "--output",
            "a",
            "--output",
            "b",
            "--manifest-path",
            "unused",
        ],
    ] {
        let output = invoke(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "SPX-B112: Native Rust SDK option may not be repeated\n"
        );
        assert!(output.stdout.is_empty());
    }

    for arguments in [
        vec![
            "project",
            "--manifest-path",
            "unused",
            "--unknown",
            "value",
            "--output",
            "unused",
        ],
        vec![
            "project",
            "--manifest-path",
            "unused",
            "positional",
            "--output",
            "unused",
        ],
        vec![
            "project",
            "--manifest-path",
            "unused",
            "--output",
            "unused",
            "trailing",
        ],
    ] {
        let output = invoke(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "SPX-B112: unknown Native Rust SDK option\n"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn relative_output_fails_before_project_or_tool_activity() {
    let relative = invoke(&[
        "project",
        "--manifest-path",
        "missing.toml",
        "--output",
        "relative-output",
    ]);
    assert_eq!(relative.status.code(), Some(2));
    assert_eq!(
        stderr(&relative),
        "SPX-B112: Native Rust SDK output must be absolute\n"
    );
    assert!(relative.stdout.is_empty());
}

#[test]
fn existing_output_is_rejected_by_builder_without_clobbering_its_inventory() {
    let root = TestRoot::new();
    let existing = root.0.join("existing");
    fs::create_dir(&existing).unwrap();
    let sentinel = existing.join("foreign-sentinel");
    fs::write(&sentinel, b"foreign bytes\n").unwrap();
    let manifest = fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/calculator-project/semaprax.toml"),
    )
    .unwrap();
    let nonfresh = binary()
        .args(["project", "--manifest-path"])
        .arg(&manifest)
        .arg("--output")
        .arg(&existing)
        .output()
        .unwrap();
    assert_eq!(nonfresh.status.code(), Some(1));
    assert_eq!(
        stderr(&nonfresh),
        "SPX-I233: Project Native Rust SDK build failed\n"
    );
    assert!(nonfresh.stdout.is_empty());
    assert!(existing.is_dir());
    assert_eq!(fs::read(&sentinel).unwrap(), b"foreign bytes\n");
    assert_eq!(fs::read_dir(&existing).unwrap().count(), 1);
}

#[test]
fn valid_grammar_reaches_project_authentication_without_tool_activity() {
    let (root, output) = absolute_missing("fresh-output");
    let missing_manifest = root.0.join("guaranteed-missing.toml");
    let result = binary()
        .arg("project")
        .arg("--manifest-path")
        .arg(&missing_manifest)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(
        stderr(&result),
        "SPX-J102: Project Native Rust SDK build failed\n"
    );
    assert!(!output.exists());
}

#[test]
fn command_surface_is_single_call_bounded_and_has_no_tool_or_process_defaults() {
    let root_source = include_str!("../src/bin/semaprax-native-rust-sdk.rs");
    let indexed_source = include_str!("../src/bin/semaprax-native-rust-sdk/indexed_project_cli.rs");
    let source = format!("{root_source}{indexed_source}");
    assert_eq!(source.matches("build_project_native_rust_sdk(").count(), 1);
    for required in [
        "semaprax.project-native-rust-sdk-result.v1",
        "bundle.crate_name()",
        "bundle.manifest_digest()",
        "bundle.project_revision()",
        "bundle.workspace_revision()",
        "bundle.subject_digest()",
        "bundle.target_triple()",
        "Project Native Rust SDK build failed",
    ] {
        assert!(source.contains(required), "CLI surface lost `{required}`");
    }
    for forbidden in [
        "Command::new",
        "std::process::Command",
        "std::fs",
        "std::env::set_var",
        "build_native_rust_sdk(",
        "canonicalize(",
        "symlink_metadata(",
        "create_dir",
        "remove_dir",
        "RUSTC",
        "CLANG",
        "SEMAPRAX_ARCHIVER",
    ] {
        assert!(
            !root_source.contains(forbidden),
            "CLI surface admitted forbidden authority/default `{forbidden}`"
        );
    }
    for forbidden in ["Command::new", "std::process::Command", "std::env::set_var"] {
        assert!(
            !indexed_source.contains(forbidden),
            "indexed CLI admitted forbidden tool authority `{forbidden}`"
        );
    }
    for required in [
        "read_bounded(",
        "MAX_RUSTC_JSON_BYTES",
        "indexed-diagnostics",
        "indexed-prepare",
        "admit_extractor_output(",
        "create_new(true)",
    ] {
        assert!(
            indexed_source.contains(required),
            "indexed CLI lost bounded diagnostic admission `{required}`"
        );
    }
}

#[test]
fn indexed_project_cli_rejects_relative_output_and_escaping_source_before_build() {
    let root = TestRoot::new();
    let selections = root.0.join("selections.json");
    let manifest = root.0.join("semaprax.toml");
    let relative = binary()
        .arg("indexed-project")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--selections")
        .arg(&selections)
        .args(["--output", "relative"])
        .output()
        .unwrap();
    assert_eq!(relative.status.code(), Some(1));
    assert_eq!(
        stderr(&relative),
        "SPX-B112: indexed Project SDK output must be absolute\n"
    );

    fs::write(
        &selections,
        serde_json::json!({
            "schema": "semaprax.indexed-project-selection.v1",
            "selections": [{
                "source_path": "../outside.spx",
                "import_id": "host.add",
                "index_path": "/missing/index.json",
                "package_source_path": "/missing/lib.rs"
            }]
        })
        .to_string(),
    )
    .unwrap();
    let output = root.0.join("sdk");
    let escaped = binary()
        .arg("indexed-project")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--selections")
        .arg(&selections)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(escaped.status.code(), Some(1));
    assert_eq!(
        stderr(&escaped),
        "SPX-B112: indexed Project source path is invalid\n"
    );
    assert!(!output.exists());
}

#[test]
fn indexed_project_cli_publishes_a_callable_selected_rust_import() {
    use semaprax_rust_api_index::RustApiIndex;
    use sha2::{Digest as _, Sha256};

    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG");
    let _archiver =
        std::env::var("SEMAPRAX_ARCHIVER").expect("configure absolute SEMAPRAX_ARCHIVER");
    let version = Command::new(&rustc).arg("--version").output().unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    let host = Command::new(&rustc).arg("-vV").output().unwrap();
    assert!(host.status.success());
    let host = String::from_utf8(host.stdout).unwrap();
    let target = host
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap();

    let crate_source = b"pub fn add(left:i64,right:i64)->i64{left+right}\n";
    let crate_digest = format!(
        "sha256:{}",
        Sha256::digest(crate_source)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let mut envelope: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index = &mut envelope["index"];
    index["package"]["name"] = "fixture_math".into();
    index["package"]["version"] = "0.0.1".into();
    index["package"]["source_sha256"] = crate_digest.into();
    index["target"] = target.into();
    index["stable_rustc_version"] = version.trim().into();
    let mut item = index["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "local_api_fixture::cfg_selected")
        .unwrap()
        .clone();
    item["path"] = "fixture_math::add".into();
    item["signature"] = "fn add(left: i64, right: i64) -> i64".into();
    index["items"] = serde_json::json!([item]);
    index["types"] = serde_json::json!([]);
    let mut index_bytes = serde_json::to_vec(&envelope).unwrap();
    index_bytes.push(b'\n');
    let prepared = RustApiIndex::admit_extractor_output(&index_bytes)
        .unwrap()
        .canonical_json()
        .to_owned();

    let root = TestRoot::new();
    fs::create_dir(root.0.join("src")).unwrap();
    let source = r#"module interop.fixture;
permit { host.math }
@id("host.math")
interface HostMath permits { host.math } {
    @id("host.add")
    import rust selected fn host_add from "fixture_math::add"
        effects { host.math }
        failure status "host.math.v1";
}
@id("interop.add")
fn add(left: i64, right: i64) -> i64 uses { host.math } {
    host_add(left, right) + right
}
@id("interop.main")
fn main() -> i64 { 0 }
"#;
    let source =
        semaprax::format::canonical(&semaprax::parse(source, Path::new("src/app.spx")).unwrap());
    fs::write(root.0.join("src/app.spx"), source).unwrap();
    let test_source = "module interop.tests; @id(\"interop.tests.main\") fn main() -> i64 { 0 }";
    let test_source = semaprax::format::canonical(
        &semaprax::parse(test_source, Path::new("src/tests.spx")).unwrap(),
    );
    fs::write(root.0.join("src/tests.spx"), test_source).unwrap();
    fs::write(root.0.join("semaprax.toml"), "schema = \"semaprax.project.v1\"\nname = \"indexed\"\nentry = \"interop.fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"interop.add\"]\ntests = [\"interop.tests\"]\n").unwrap();
    let index_path = root.0.join("index.json");
    let package_path = root.0.join("lib.rs");
    let selections_path = root.0.join("selections.json");
    fs::write(&index_path, prepared).unwrap();
    fs::write(&package_path, crate_source).unwrap();
    fs::write(
        &selections_path,
        serde_json::json!({
            "schema": "semaprax.indexed-project-selection.v1",
            "selections": [{
                "source_path": "src/app.spx",
                "import_id": "host.add",
                "index_path": index_path,
                "package_source_path": package_path
            }]
        })
        .to_string(),
    )
    .unwrap();
    let output = root.0.join("sdk");
    let result = binary()
        .arg("indexed-project")
        .arg("--manifest-path")
        .arg(root.0.join("semaprax.toml"))
        .arg("--selections")
        .arg(&selections_path)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    let receipt: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        receipt["schema"],
        "semaprax.indexed-project-native-rust-sdk-result.v1"
    );
    assert_eq!(receipt["target_triple"], target);
    assert!(output.join("src/lib.rs").is_file());
    let archive_name = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let archive = output.join("native").join(archive_name);
    assert!(archive.is_file());

    let library = Command::new(&rustc)
        .current_dir(&output)
        .args([
            "--edition=2021",
            "--crate-name",
            "indexed_sdk",
            "--crate-type=rlib",
            "src/lib.rs",
            "-o",
            "libindexed_sdk.rlib",
        ])
        .output()
        .unwrap();
    assert!(
        library.status.success(),
        "{}",
        String::from_utf8_lossy(&library.stderr)
    );
    fs::write(root.0.join("consumer.rs"), "fn main(){let mut sdk=indexed_sdk::indexed_scalar_sdk(&[\"host.math\"]).unwrap();assert_eq!(sdk.spx_interop_dot_add(20,22).unwrap(),64)}\n").unwrap();
    let consumer = Command::new(&rustc)
        .current_dir(&root.0)
        .args([
            "--edition=2021",
            "-C",
            &format!("linker={clang}"),
            "--extern",
            &format!(
                "indexed_sdk={}",
                output.join("libindexed_sdk.rlib").display()
            ),
            "-C",
            &format!("link-arg={}", archive.display()),
            "consumer.rs",
            "-o",
            if cfg!(windows) {
                "consumer.exe"
            } else {
                "consumer"
            },
        ])
        .output()
        .unwrap();
    assert!(
        consumer.status.success(),
        "{}",
        String::from_utf8_lossy(&consumer.stderr)
    );
    assert!(Command::new(root.0.join(if cfg!(windows) {
        "consumer.exe"
    } else {
        "consumer"
    }))
    .status()
    .unwrap()
    .success());
}

fn effectful_tools_available() -> bool {
    ["RUSTC", "CLANG", "SEMAPRAX_ARCHIVER"]
        .iter()
        .all(|name| std::env::var_os(name).is_some())
        && (!cfg!(windows)
            || ["SEMAPRAX_VCTOOLS", "SEMAPRAX_LINKER"]
                .iter()
                .all(|name| std::env::var_os(name).is_some()))
}

#[test]
fn configured_effectful_project_build_emits_one_canonical_result() {
    if !effectful_tools_available() {
        return;
    }
    let root = TestRoot::new();
    let output = root.0.join("generated-sdk");
    let manifest = fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/calculator-project/semaprax.toml"),
    )
    .unwrap();
    let result = binary()
        .args(["project", "--manifest-path"])
        .arg(&manifest)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(result.stderr.is_empty());
    assert_eq!(
        result.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let mut canonical = serde_json::to_vec(&value).unwrap();
    canonical.push(b'\n');
    assert_eq!(result.stdout, canonical);
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        [
            "crate_name",
            "manifest_digest",
            "project_revision",
            "schema",
            "subject_digest",
            "target_triple",
            "workspace_revision",
        ]
    );
    assert_eq!(
        value["schema"],
        "semaprax.project-native-rust-sdk-result.v1"
    );
    assert_eq!(value["crate_name"], "semaprax-generated-native-rust-sdk");
    for field in [
        "manifest_digest",
        "project_revision",
        "subject_digest",
        "workspace_revision",
    ] {
        assert!(value[field].as_str().unwrap().starts_with("sha256:"));
    }
    assert!(output.join("semaprax.native-rust-sdk.json").is_file());
}

#[test]
fn configured_project_rust_dependency_compiles_offline_from_an_exact_lock() {
    if !effectful_tools_available() || std::env::var_os("CARGO").is_none() {
        return;
    }
    let root = TestRoot::new();
    let project = root.0.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/calculator-project");
    for source in ["app.spx", "core.spx", "tests.spx"] {
        fs::copy(
            example.join("src").join(source),
            project.join("src").join(source),
        )
        .unwrap();
    }
    fs::write(
        project.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n\n[rust-dependencies]\nsame-file = [\"=1.0.6\"]\n",
    )
    .unwrap();
    let generated = root.0.join("generated-sdk");
    let result = binary()
        .args(["project", "--manifest-path"])
        .arg(project.join("semaprax.toml"))
        .arg("--output")
        .arg(&generated)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    let cargo_toml = fs::read_to_string(generated.join("Cargo.toml")).unwrap();
    assert!(cargo_toml
        .contains("spx_rust_dependency_0 = { package = \"same-file\", version = \"=1.0.6\" }"));
    let lib = fs::read_to_string(generated.join("src/lib.rs")).unwrap();
    assert!(lib.contains("pub use ::spx_rust_dependency_0 as rust_dependency_same_file;"));

    let cargo = std::env::var_os("CARGO").unwrap();
    let lock = Command::new(&cargo)
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(generated.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(lock.status.success(), "{}", stderr(&lock));
    let check = Command::new(cargo)
        .args(["check", "--locked", "--offline", "--manifest-path"])
        .arg(generated.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.0.join("cargo-target"))
        .output()
        .unwrap();
    assert!(check.status.success(), "{}", stderr(&check));
}

#[test]
fn arbitrary_exact_crate_is_invoked_by_semaprax_through_the_typed_adapter() {
    if !effectful_tools_available() || std::env::var_os("CARGO").is_none() {
        return;
    }
    let root = TestRoot::new();
    let project = root.0.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"callback\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"callback.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"callback.tests\"]\n\n[exports]\nweb = [\"callback.apply\"]\n\n[rust-dependencies]\nsame-file = [\"=1.0.6\"]\n",
    )
    .unwrap();
    for (relative, source) in [
        ("src/app.spx", CALLBACK_APP),
        ("src/tests.spx", CALLBACK_TESTS),
    ] {
        let program = semaprax::parse(source, Path::new(relative)).unwrap();
        fs::write(
            project.join(relative),
            semaprax::format::canonical(&program),
        )
        .unwrap();
    }

    let generated = root.0.join("generated-sdk");
    let result = binary()
        .args(["project", "--manifest-path"])
        .arg(project.join("semaprax.toml"))
        .arg("--output")
        .arg(&generated)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));

    let consumer = root.0.join("consumer");
    fs::create_dir_all(consumer.join("src")).unwrap();
    fs::write(
        consumer.join("Cargo.toml"),
        "[package]\nname = \"arbitrary-crate-callback\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\nsemaprax-generated-native-rust-sdk = { path = \"../generated-sdk\" }\n\n[lints.rust]\nunsafe_code = \"forbid\"\n",
    )
    .unwrap();
    fs::write(
        consumer.join("src/main.rs"),
        r#"use semaprax_generated_native_rust_sdk::{
    rust_dependency_same_file, NativeRustSdk, NativeRustSdkImportResult, NativeRustSdkImports,
};

struct Host;

impl NativeRustSdkImports for Host {
    fn spx_callback_dot_host_dot_adjust(
        &mut self,
        value: i64,
    ) -> NativeRustSdkImportResult<i64> {
        match rust_dependency_same_file::is_same_file(".", ".") {
            Ok(true) => NativeRustSdkImportResult::Success(value + 1),
            Ok(false) => NativeRustSdkImportResult::Success(value),
            Err(_) => NativeRustSdkImportResult::HostFailure,
        }
    }
}

fn main() {
    let mut sdk = NativeRustSdk::new(Host, &["host.adjust"]).unwrap();
    assert_eq!(sdk.spx_callback_dot_apply(19, 22), Ok(42));
}
"#,
    )
    .unwrap();

    let cargo = std::env::var_os("CARGO").unwrap();
    let lock = Command::new(&cargo)
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(lock.status.success(), "{}", stderr(&lock));
    let run = Command::new(cargo)
        .args(["run", "--locked", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.0.join("cargo-target"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", stderr(&run));
}

const CALLBACK_MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"callback\"\nentry = \"callback.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"callback.apply\"]\ntests = [\"callback.tests\"]\n";

const CALLBACK_APP: &str = r#"
module callback.app;

permit { host.adjust }

@id("callback.host")
interface CallbackHost permits { host.adjust } {
    @id("callback.host.adjust")
    import rust fn adjust(value: i64) -> i64
        effects { host.adjust }
        failure status "callback.host.v1";
}

@id("callback.apply")
fn apply(left: i64, right: i64) -> i64 uses { host.adjust } { adjust(left + right) }

@id("callback.main")
fn main() -> i64 uses { host.adjust } { apply(19, 23) }
"#;

const CALLBACK_TESTS: &str =
    "module callback.tests;\n\n@id(\"callback.tests.main\")\nfn main() -> i64 { 0 }\n";

/// The Project route is bidirectional: a Project declaring an `import rust fn`
/// publishes a package whose generated surface carries both the selected
/// SEMAPRAX export a Rust caller invokes and the Rust callback the export
/// calls back into.
#[test]
fn configured_effectful_project_build_publishes_both_call_directions() {
    if !effectful_tools_available() {
        return;
    }
    let root = TestRoot::new();
    let project = root.0.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("semaprax.toml"), CALLBACK_MANIFEST).unwrap();
    for (relative, source) in [
        ("src/app.spx", CALLBACK_APP),
        ("src/tests.spx", CALLBACK_TESTS),
    ] {
        let program = semaprax::parse(source, Path::new(relative)).unwrap();
        fs::write(
            project.join(relative),
            semaprax::format::canonical(&program),
        )
        .unwrap();
    }

    let output = root.0.join("generated-sdk");
    let result = binary()
        .args(["project", "--manifest-path"])
        .arg(project.join("semaprax.toml"))
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));

    // Both directions reach the published package.
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("semaprax.native-rust-sdk.json")).unwrap())
            .unwrap();
    let exports = manifest["exports"].as_array().unwrap();
    let imports = manifest["imports"].as_array().unwrap();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0]["id"], "callback.apply");
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0]["id"], "callback.host.adjust");
    assert_eq!(imports[0]["failure"]["domain_id"], "callback.host.v1");

    let facade = fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(
        facade.contains("fn spx_callback_dot_host_dot_adjust"),
        "the generated host trait lost the Rust callback"
    );
    assert!(
        facade.contains("pub fn spx_callback_dot_apply"),
        "the generated facade lost the SEMAPRAX export"
    );
}
