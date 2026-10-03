//! Real reqwest response through an authenticated checked Semaprax Bytes export.
//! Rust owns the await point; Semaprax source does not yet admit async import.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax::project::with_authenticated_project;
use semaprax_native_rust_interop::render_local_future_bridge;

use super::native_rust_cargo;

fn exact_tool(variable: &str, fallback: &str, version_arg: &str, prefix: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| fallback.into()));
    assert!(
        path.is_absolute() && path.is_file(),
        "{variable} must name an installed absolute tool"
    );
    let output = Command::new(&path).arg(version_arg).output().unwrap();
    assert!(
        output.status.success(),
        "{variable} did not report a version"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).starts_with(prefix),
        "{variable} has an unexpected version"
    );
    path
}

fn project(root: &Path) -> PathBuf {
    fn write_source(root: &Path, path: &str, source: &str) {
        let parsed = semaprax::parse(source, path).unwrap();
        fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
    }
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("semaprax.toml"),
        "schema = \"semaprax.project.v8\"\nname = \"asyncchecked\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"asyncchecked.app\"\nsources = [\"app.spx\", \"core.spx\", \"tests.spx\"]\nweb_exports = [\"asyncchecked.payload\"]\ntests = [\"asyncchecked.tests\"]\n",
    )
    .unwrap();
    write_source(
        &project,
        "core.spx",
        "module asyncchecked.core;\n\n@id(\"asyncchecked.payload\")\nfn payload(input: borrow Slice<u8>) -> Bytes {\n    bytes_copy(byte_range(input, 1usize, byte_len(input)))\n}\n",
    );
    write_source(
        &project,
        "app.spx",
        "module asyncchecked.app;\n\n@id(\"asyncchecked.app.main\")\nfn main() -> i64 { 0 }\n",
    );
    write_source(
        &project,
        "tests.spx",
        "module asyncchecked.tests;\n\n@id(\"asyncchecked.tests.main\")\nfn main() -> i64 { 0 }\n",
    );
    project.join("semaprax.toml")
}

fn consumer_lock() -> String {
    let lock = include_str!("../../../semaprax-native-rust-interop-builder/src/public_sdk/fixtures/ri09-reqwest/Cargo.lock");
    let needle = "[[package]]\nname = \"semaprax-ri09-local-future\"\nversion = \"0.1.0\"\ndependencies = [\n \"reqwest\",\n \"rustls\",\n \"tokio\",\n]\n";
    assert_eq!(lock.matches(needle).count(), 1);
    lock.replace(
        needle,
        "[[package]]\nname = \"semaprax-generated-native-rust-owned-data-sdk\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"semaprax-ri09-local-future\"\nversion = \"0.1.0\"\ndependencies = [\n \"reqwest\",\n \"rustls\",\n \"semaprax-generated-native-rust-owned-data-sdk\",\n \"tokio\",\n]\n",
    )
}

#[test]
#[ignore = "requires explicit checkout-private Cargo target and installed native tools"]
fn locked_reqwest_response_enters_checked_semaprax_bytes_export() {
    let cargo = exact_tool("CARGO", "/opt/homebrew/bin/cargo", "--version", "cargo 1.");
    let _rustc = exact_tool("RUSTC", "/opt/homebrew/bin/rustc", "--version", "rustc 1.");
    let _clang = exact_tool(
        "CLANG",
        "/usr/bin/clang",
        "--version",
        "Apple clang version",
    );
    let _archiver = exact_tool(
        "SEMAPRAX_ARCHIVER",
        "/usr/bin/libtool",
        "-V",
        "Apple Inc. version",
    );
    let target = PathBuf::from(
        std::env::var_os("SEMAPRAX_RI09_TARGET_DIR")
            .expect("set SEMAPRAX_RI09_TARGET_DIR to a checkout-private warm target"),
    )
    .canonicalize()
    .unwrap();
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    assert!(target.starts_with(checkout.join("target")));

    let root = std::env::temp_dir().join(format!("semaprax-ri09-checked-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let manifest = project(&root);
    let sdk = root.join("sdk");
    with_authenticated_project(&manifest, |snapshot| {
        semaprax_toolchain::build_rust(snapshot, &sdk)
    })
    .unwrap();
    assert!(sdk
        .join("semaprax.native-rust-owned-data-sdk.json")
        .is_file());

    let consumer = root.join("consumer");
    fs::create_dir(&consumer).unwrap();
    fs::create_dir(consumer.join("src")).unwrap();
    let toml = format!(
        "{}semaprax-generated-native-rust-owned-data-sdk = {{ path = \"../sdk\", version = \"=0.1.0\" }}\n",
        include_str!("../../../semaprax-native-rust-interop-builder/src/public_sdk/fixtures/ri09-reqwest/Cargo.toml")
    );
    let lock = consumer_lock();
    fs::write(consumer.join("Cargo.toml"), toml).unwrap();
    fs::write(consumer.join("Cargo.lock"), &lock).unwrap();
    fs::write(
        consumer.join("src/main.rs"),
        format!(
            "{}\n{}",
            render_local_future_bridge(),
            include_str!("ri09_checked_consumer.rs.txt")
        ),
    )
    .unwrap();
    let output = native_rust_cargo::cargo_command()
        .arg("run")
        .args(["--quiet", "--locked", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .current_dir(&consumer)
        .env("CARGO", cargo)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ri09-checked-bytes-ok\n");
    assert_eq!(
        fs::read(consumer.join("Cargo.lock")).unwrap(),
        lock.as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}
