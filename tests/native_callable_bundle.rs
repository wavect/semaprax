use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::codegen::{build_native_callable_bundle, emit_c, preflight_native_callable_bundle};
use semaprax::diagnostic::Diagnostic;
use semaprax::parse;
use sha2::{Digest, Sha256};

const SOURCE: &str = include_str!("../examples/native_callable.spx");
const FUNCTION_ID: &str = "example.token.identity";
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn create(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-native-callable-bundle-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create native-callable test directory");
        let root = fs::canonicalize(root).expect("canonical native-callable test directory");
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.root) else {
            return;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            let _ = fs::remove_file(&self.root);
        } else if metadata.is_dir() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("native-callable-bundle.spx")).unwrap()
}

fn require_error<T>(result: Result<T, Diagnostic>, context: &str) -> Diagnostic {
    match result {
        Ok(_) => panic!("{context}"),
        Err(error) => error,
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn bundle_files(directory: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            assert!(metadata.is_file());
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

#[test]
fn public_preflight_is_deterministic_authority_free_and_keeps_native_b104() {
    let program = program();
    let first = preflight_native_callable_bundle(&program, FUNCTION_ID).unwrap();
    let second = preflight_native_callable_bundle(&program, FUNCTION_ID).unwrap();

    assert_eq!(first.module(), "example.native_callable");
    assert_eq!(first.function_id(), FUNCTION_ID);
    assert_eq!(
        first.graph_revision(),
        "sha256:bbe9203bbb86130757a3ba48ccc73d6dba8d1f83a717d0da4e4757bbaab4be8c"
    );
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.getter_symbol(), second.getter_symbol());
    assert_eq!(first.callable_symbol(), second.callable_symbol());
    assert_ne!(first.getter_symbol(), first.callable_symbol());
    assert_eq!(first.call_contract(), second.call_contract());
    assert_eq!(first.max_request_bytes(), 84);
    assert_eq!(first.max_response_bytes(), 88);
    assert_eq!(first.provider_source(), second.provider_source());
    assert_eq!(first.event_dictionary(), second.event_dictionary());
    assert_eq!(
        first.trace_path_certificate(),
        second.trace_path_certificate()
    );
    assert_eq!(first.preflight_sha256(), second.preflight_sha256());
    assert_eq!(first.preflight_sha256().len(), 64);
    assert!(first
        .preflight_sha256()
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));

    let ordinary_native = emit_c(&program).unwrap_err();
    assert_eq!(ordinary_native.code, "SPX-B104");
}

#[test]
fn build_commits_two_byte_identical_hashed_bundles() {
    let fixture = Fixture::create("deterministic");
    let program = program();
    let first = build_native_callable_bundle(&program, FUNCTION_ID, &fixture.path("first"))
        .expect("build first native-callable bundle");
    let second = build_native_callable_bundle(&program, FUNCTION_ID, &fixture.path("second"))
        .expect("build second native-callable bundle");

    assert_eq!(first.manifest_sha256(), second.manifest_sha256());
    let first_files = bundle_files(first.output_directory());
    let second_files = bundle_files(second.output_directory());
    assert_eq!(first_files, second_files);
    assert!(first.library_path().is_file());
    assert!(first.manifest_path().is_file());

    let library_name = first.library_path().file_name().unwrap().to_str().unwrap();
    let mut expected_names = vec![
        "descriptor.bin",
        library_name,
        "provider.c",
        "semantic-event-dictionary.json",
        "semaprax.native-callable.json",
        "semaprax.native-callable.sha256",
        "trace-path-certificate.json",
    ];
    expected_names.sort();
    assert_eq!(
        first_files
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        expected_names
    );

    let manifest = fs::read(first.manifest_path()).unwrap();
    assert_eq!(sha256(&manifest), first.manifest_sha256());
    let manifest_text = String::from_utf8(manifest).unwrap();
    assert!(manifest_text.starts_with(
        "{\"schema\":\"semaprax.native-callable-bundle.v1\",\"abi\":\"semaprax.native-callable.v2\""
    ));
    assert!(manifest_text.contains("\"function_id\":\"example.token.identity\""));

    for name in [
        "descriptor.bin",
        "provider.c",
        "semantic-event-dictionary.json",
        "trace-path-certificate.json",
    ] {
        let bytes = fs::read(first.output_directory().join(name)).unwrap();
        assert!(manifest_text.contains(&format!(
            "\"path\":\"{name}\",\"bytes\":{},\"sha256\":\"{}\"",
            bytes.len(),
            sha256(&bytes)
        )));
    }
    let library = fs::read(first.library_path()).unwrap();
    assert!(manifest_text.contains(&format!(
        "\"path\":\"{library_name}\",\"bytes\":{},\"sha256\":\"{}\"",
        library.len(),
        sha256(&library)
    )));
    let checksum = fs::read_to_string(
        first
            .output_directory()
            .join("semaprax.native-callable.sha256"),
    )
    .unwrap();
    assert_eq!(
        checksum,
        format!(
            "{}  semaprax.native-callable.json\n",
            first.manifest_sha256()
        )
    );
}

#[test]
fn default_feature_external_consumer_cannot_import_native_host_internals() {
    let fixture = Fixture::create("default-surface");
    let consumer = fixture.path("consumer");
    fs::create_dir(&consumer).unwrap();
    let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    fs::write(
        consumer.join("Cargo.toml"),
        format!(
            r#"[package]
name = "semaprax-default-surface-check"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
semaprax = {{ path = "{manifest_root}", default-features = false }}
"#,
        ),
    )
    .unwrap();
    fs::create_dir(consumer.join("src")).unwrap();
    fs::write(
        consumer.join("src/main.rs"),
        r#"use semaprax::codegen::{emit_native_callable_admission, NativeCallableAdmissionArtifact};
use semaprax::trace_path_certificate::TracePathCertificate;

fn main() {
    let _ = std::mem::size_of::<NativeCallableAdmissionArtifact>();
    let _ = std::mem::size_of::<TracePathCertificate>();
    let _ = emit_native_callable_admission;
}
"#,
    )
    .unwrap();

    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["check", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", fixture.path("consumer-target"))
        .output()
        .unwrap();
    assert!(
        !checked.status.success(),
        "default surface exposed host internals"
    );
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(
        stderr.contains("emit_native_callable_admission")
            && stderr.contains("NativeCallableAdmissionArtifact")
            && stderr.contains("trace_path_certificate")
            && (stderr.contains("unresolved import") || stderr.contains("private")),
        "unexpected default-surface compiler diagnostic:\n{stderr}"
    );
}

#[test]
fn build_refuses_existing_files_directories_and_symlinks_without_mutation() {
    let fixture = Fixture::create("no-overwrite");
    let program = program();

    let directory = fixture.path("existing-directory");
    fs::create_dir(&directory).unwrap();
    let sentinel = directory.join("sentinel");
    fs::write(&sentinel, b"preserve-directory").unwrap();
    let error = require_error(
        build_native_callable_bundle(&program, FUNCTION_ID, &directory),
        "existing directory was overwritten",
    );
    assert_eq!(error.code, "SPX-I107");
    assert_eq!(fs::read(&sentinel).unwrap(), b"preserve-directory");

    let file = fixture.path("existing-file");
    fs::write(&file, b"preserve-file").unwrap();
    let error = require_error(
        build_native_callable_bundle(&program, FUNCTION_ID, &file),
        "existing file was overwritten",
    );
    assert_eq!(error.code, "SPX-I107");
    assert_eq!(fs::read(&file).unwrap(), b"preserve-file");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let target = fixture.path("missing-symlink-target");
        let link = fixture.path("output-symlink");
        symlink(&target, &link).unwrap();
        let error = require_error(
            build_native_callable_bundle(&program, FUNCTION_ID, &link),
            "output symlink was followed or replaced",
        );
        assert_eq!(error.code, "SPX-I107");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!target.exists());
    }

    assert!(fs::read_dir(&fixture.root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".semaprax-native-callable-staging-")
    }));
}

#[test]
fn public_preflight_rejects_missing_automatic_and_excluded_functions() {
    let program = program();
    let missing = require_error(
        preflight_native_callable_bundle(&program, "missing.function"),
        "missing function preflight succeeded",
    );
    assert_eq!(missing.code, "SPX-B105");

    let excluded = require_error(
        preflight_native_callable_bundle(&program, "app.main"),
        "resource-free function was admitted as an owned callable",
    );
    assert_eq!(excluded.code, "SPX-B105");
    assert!(excluded
        .message
        .contains("at least one direct `own` trivial-resource parameter"));

    let automatic = parse(
        r#"module test.automatic_callable;
@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}
fn identity(value: own Token) -> Token { value }
@id("app.main")
fn main() -> i64 { 0 }
"#,
        Path::new("automatic-native-callable.spx"),
    )
    .unwrap();
    let automatic = require_error(
        preflight_native_callable_bundle(&automatic, "auto:test.automatic_callable.identity"),
        "automatic function identity was admitted",
    );
    assert_eq!(automatic.code, "SPX-B105");
    assert!(automatic.message.contains("explicit persistent @id"));
}

#[test]
fn cli_builds_selected_bundle_and_requires_function_without_opening_native_run() {
    let fixture = Fixture::create("cli");
    let source = fixture.path("input.spx");
    fs::write(&source, SOURCE).unwrap();
    let output = fixture.path("bundle");
    let binary = env!("CARGO_BIN_EXE_semaprax");

    let built = Command::new(binary)
        .args(["build"])
        .arg(&source)
        .args([
            "--target",
            "native-callable",
            "--function",
            FUNCTION_ID,
            "-o",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "native-callable CLI failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(String::from_utf8_lossy(&built.stdout).contains("manifest sha256:"));
    assert!(output.join("semaprax.native-callable.json").is_file());

    let missing_function = Command::new(binary)
        .args(["build"])
        .arg(&source)
        .args(["--target", "native-callable", "-o"])
        .arg(fixture.path("missing-function"))
        .output()
        .unwrap();
    assert_eq!(missing_function.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing_function.stderr)
        .contains("requires --function <stable-id>"));

    let ordinary_native = Command::new(binary)
        .args(["build"])
        .arg(&source)
        .args(["--target", "native", "-o"])
        .arg(fixture.path("ordinary-native"))
        .output()
        .unwrap();
    assert_eq!(ordinary_native.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&ordinary_native.stderr).contains("SPX-B104"));

    let compiler_failure_output = fixture.path("compiler-failure");
    let compiler_failure = Command::new(binary)
        .args(["build"])
        .arg(&source)
        .args([
            "--target",
            "native-callable",
            "--function",
            FUNCTION_ID,
            "-o",
        ])
        .arg(&compiler_failure_output)
        .env("PATH", &fixture.root)
        .output()
        .unwrap();
    assert_eq!(compiler_failure.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&compiler_failure.stderr).contains("SPX-B105"));
    assert!(!compiler_failure_output.exists());
    assert!(fs::read_dir(&fixture.root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".semaprax-native-callable-staging-")
    }));
}
