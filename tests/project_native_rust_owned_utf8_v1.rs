#[path = "support/full_toolchain.rs"]
mod full_toolchain;

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;
#[path = "support/native_rust_target.rs"]
mod native_rust_target;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
static PACKAGE_PUBLICATION: Mutex<()> = Mutex::new(());

fn publication_guard() -> std::sync::MutexGuard<'static, ()> {
    // These tests exercise the same real compiler and archiver authorities.
    // Keep their package-publication lifetimes disjoint within this test binary.
    PACKAGE_PUBLICATION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn configured_tool(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(configured) = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
    {
        #[cfg(windows)]
        if variable == "SEMAPRAX_ARCHIVER" {
            return configured;
        }
        if let Ok(canonical) = configured.canonicalize() {
            return canonical;
        }
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| panic!("{variable} must name an installed absolute tool"))
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-v10-rust-route-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v10\"\nname = \"utf8-route\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.count\", \"utf8.greeting\"]\ntests = [\"utf8.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module utf8.app;\n\n@id(\"utf8.count\")\nfn count() -> i64\n{\n    7\n}\n\n@id(\"utf8.greeting\")\nfn greeting() -> string\n{\n    \"hello\\u{0}世界\"\n}\n\n@id(\"utf8.app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module utf8.tests;\n\n@id(\"utf8.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    Fixture(root.canonicalize().unwrap())
}

#[test]
fn project_v10_rust_route_emits_a_distinct_mixed_safe_string_package() {
    let _publication = publication_guard();
    let fixture = fixture();
    let output = fixture.0.join("rust");
    let clang = configured_tool("CLANG", &["/usr/bin/clang"]);
    let archiver = if cfg!(target_os = "macos") {
        configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/libtool"])
    } else {
        configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/ar", "/bin/ar"])
    };
    let built = Command::new(full_toolchain::binary())
        .args([
            "build",
            "--manifest-path",
            "semaprax.toml",
            "--target",
            "rust",
            "-o",
        ])
        .arg(&output)
        .current_dir(&fixture.0)
        .env("CLANG", clang)
        .env("SEMAPRAX_ARCHIVER", archiver)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert_eq!(
        String::from_utf8(built.stdout).unwrap(),
        format!(
            "built Project v10 Native Rust owned-data package {}\n",
            output.display()
        )
    );

    let descriptor: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("descriptor.json")).unwrap()).unwrap();
    assert_eq!(descriptor["schema"], "semaprax.public-owned-utf8-api.v1");
    assert_eq!(descriptor["exports"][0]["result"], "i64");
    assert_eq!(descriptor["exports"][1]["result"], "owned-utf8");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(output.join("semaprax.native-rust-owned-utf8-sdk.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["schema"], "semaprax.native-rust-owned-utf8-sdk.v1");
    let safe = std::fs::read_to_string(output.join("lib.rs")).unwrap();
    let settle = safe.find("copy_and_settle(raw.handle)").unwrap();
    let validate = safe.find("String::from_utf8(bytes)").unwrap();
    assert!(settle < validate);
    assert!(safe.contains("pub fn spx_utf8_dot_count(&mut self)->Result<i64,CallError>"));
    assert!(safe.contains("pub fn spx_utf8_dot_greeting(&mut self)->Result<String,CallError>"));
}

#[test]
fn unsupported_rust_profile_rejection_precedes_explicit_parent_creation() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-rust-profile-rejection-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v2\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"legacy.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"legacy.add\"]\ntests = [\"legacy.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module legacy.app;\n\n@id(\"legacy.add\")\nfn add(value: borrow str) -> i64\n{\n    str_len_bytes(value)\n}\n\n@id(\"legacy.app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module legacy.tests;\n\n@id(\"legacy.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    let fixture = Fixture(root.canonicalize().unwrap());
    let output = fixture.0.join("missing-parent/rust");
    let rejected = Command::new(full_toolchain::binary())
        .args([
            "build",
            "--manifest-path",
            "semaprax.toml",
            "--target",
            "rust",
            "-o",
        ])
        .arg(&output)
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("SPX-J114"), "stderr was: {stderr}");
    assert!(!fixture.0.join("missing-parent").exists());
}

#[test]
fn real_v10_generated_rust_consumer_recovers_after_string_semantic_failures() {
    let _publication = publication_guard();
    // This is the actual Project -> provider archive -> generated safe package
    // route, not the ABI double used by the separate hostile guard tests.
    // Physical C heap counts belong to native_owned_utf8_settlement_v1.
    let root = std::env::temp_dir().join(format!(
        "semaprax-v10-string-consumer-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v10\"\nname = \"string-consumer\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"consumer.late\", \"consumer.local\", \"consumer.text\"]\ntests = [\"consumer.tests\"]\n",
    )
    .unwrap();
    for (path, source) in [
        (
            "src/app.spx",
            r#"module consumer.app;
@id("consumer.sink") fn sink(value: string, number: i64) -> string { "done" }
@id("consumer.late") fn late(zero: i64) -> string { sink("argument", 1 / zero) }
@id("consumer.local") fn local(zero: i64) -> string {
    sink({ let kept = "kept"; let divided = 1 / zero; kept }, 0)
}
@id("consumer.text") fn text() -> string { "hello\u{0}世界" }
@id("consumer.main") fn main() -> i64 { 0 }
"#,
        ),
        (
            "src/tests.spx",
            "module consumer.tests; @id(\"consumer.tests.main\") fn main() -> i64 { 0 }",
        ),
    ] {
        let canonical = semaprax::format::canonical(
            &semaprax::parse(source, std::path::Path::new(path)).unwrap(),
        );
        std::fs::write(root.join(path), canonical).unwrap();
    }
    let clang = configured_tool("CLANG", &["/usr/bin/clang"]);
    let archiver = if cfg!(target_os = "macos") {
        configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/libtool"])
    } else {
        configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/ar", "/bin/ar"])
    };
    let built = Command::new(full_toolchain::binary())
        .args([
            "build",
            "--manifest-path",
            "semaprax.toml",
            "--target",
            "rust",
            "-o",
            "generated",
        ])
        .current_dir(&root)
        .env("CLANG", clang)
        .env("SEMAPRAX_ARCHIVER", archiver)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}: {}",
        root.display(),
        String::from_utf8_lossy(&built.stderr)
    );
    let consumer = root.join("consumer");
    std::fs::create_dir(&consumer).unwrap();
    std::fs::write(
        consumer.join("Cargo.toml"),
        r#"[package]
name = "semaprax-string-consumer"
version = "0.0.0"
edition = "2021"
rust-version = "1.85"
publish = false
[workspace]
[dependencies]
semaprax-generated-native-rust-owned-data-sdk = { path = "../generated" }
[[bin]]
name = "string-consumer"
path = "main.rs"
"#,
    )
    .unwrap();
    let lock = r#"# Authored standalone lock: neither package has registry dependencies.
version = 4
[[package]]
name = "semaprax-generated-native-rust-owned-data-sdk"
version = "0.1.0"
[[package]]
name = "semaprax-string-consumer"
version = "0.0.0"
dependencies = ["semaprax-generated-native-rust-owned-data-sdk"]
"#;
    std::fs::write(consumer.join("Cargo.lock"), lock).unwrap();
    std::fs::write(
        consumer.join("main.rs"),
        r#"#![forbid(unsafe_code)]
use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};
fn main() {
    let mut sdk = NativeRustOwnedDataSdk::new().unwrap();
    for _ in 0..32 {
        assert_eq!(sdk.spx_consumer_dot_late(0), Err(CallError::SemanticFailure));
        assert_eq!(sdk.spx_consumer_dot_late(1).unwrap(), "done");
        assert_eq!(sdk.spx_consumer_dot_local(0), Err(CallError::SemanticFailure));
        assert_eq!(sdk.spx_consumer_dot_local(1).unwrap(), "done");
        assert_eq!(sdk.spx_consumer_dot_text().unwrap(), "hello\0世界");
    }
    drop(sdk);
    println!("v10-string-consumer-ok");
}
"#,
    )
    .unwrap();
    let cargo_target = native_rust_target::CargoTarget::new();
    let output = native_rust_cargo::cargo_command()
        .current_dir(&consumer)
        .args(["run", "--quiet", "--locked", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", cargo_target.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}: stdout={} stderr={}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"v10-string-consumer-ok\n");
    assert_eq!(
        std::fs::read(consumer.join("Cargo.lock")).unwrap(),
        lock.as_bytes()
    );
    remove_consumer_fixture(&root);
}

fn remove_consumer_fixture(root: &std::path::Path) {
    // Enumerate and validate the complete fresh fixture before deleting any
    // entry. Do not follow compiler-created links, Windows reparse points, or
    // an unexpectedly large/deep tree; preserve it for inspection instead.
    fn inventory(path: &std::path::Path, depth: usize, entries: &mut Vec<(PathBuf, bool)>) {
        assert!(depth <= 32 && entries.len() < 10_000);
        let metadata = std::fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
        assert!(metadata.is_dir() || metadata.is_file());
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                inventory(&entry.unwrap().path(), depth + 1, entries);
            }
        }
        entries.push((path.to_owned(), metadata.is_dir()));
        assert!(entries.len() <= 10_000);
    }
    assert!(root.is_absolute());
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("semaprax-v10-string-consumer-"));
    let mut entries = Vec::new();
    inventory(root, 0, &mut entries);
    for (path, directory) in entries {
        assert!(path.starts_with(root));
        if directory {
            std::fs::remove_dir(path).unwrap();
        } else {
            std::fs::remove_file(path).unwrap();
        }
    }
}
