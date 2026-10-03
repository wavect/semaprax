//! Physical single-file indexed scalar package acceptance.

use super::*;
use crate::indexed_binding::SelectedPackage;
use semaprax_rust_api_index::RustApiIndex;
use std::process::Command;

const SOURCE: &str = r#"module interop.fixture;

permit { host.math }

@id("host.math")
interface HostMath permits { host.math } {
    @id("host.add")
    import rust fn host_add(left: i64, right: i64) -> i64 from "fixture_math::add"
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

fn options() -> NativeRustSdkOptions {
    NativeRustSdkOptions {
        exports: vec!["interop.add".into()],
        imports: vec!["host.add".into()],
        capabilities: vec!["host.math".into()],
    }
}

fn reset_build_observer() {
    TEST_BUILD_STATE.with(|state| state.set(TestBuildState::default()));
}

fn index_for(source: &[u8], stable_rustc: &str) -> Vec<u8> {
    let mut envelope: Value = serde_json::from_slice(include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index = &mut envelope["index"];
    index["package"]["name"] = "fixture_math".into();
    index["package"]["version"] = "0.0.1".into();
    index["package"]["source_sha256"] = raw_digest(source).into();
    index["target"] = target_triple().unwrap().into();
    index["stable_rustc_version"] = stable_rustc.into();
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
    let mut bytes = serde_json::to_vec(&envelope).unwrap();
    bytes.push(b'\n');
    RustApiIndex::admit_extractor_output(&bytes)
        .unwrap()
        .canonical_json()
        .as_bytes()
        .to_vec()
}

fn run_published_sdk(rustc: &str, clang: &str, root: &Path, output: &Path) -> i32 {
    let mut library = Command::new(rustc);
    library.current_dir(output).args([
        "--edition=2021",
        "--crate-name",
        "indexed_sdk",
        "--crate-type=rlib",
        "src/lib.rs",
        "-o",
        "libindexed_sdk.rlib",
    ]);
    assert!(library.status().unwrap().success());
    std::fs::write(
        root.join("consumer.rs"),
        "fn main(){let mut sdk=indexed_sdk::indexed_scalar_sdk(&[\"host.math\"]).unwrap_or_else(|_|std::process::exit(13));match sdk.spx_interop_dot_add(20,22){Ok(64)=>{},_=>std::process::exit(12)}}\n",
    )
    .unwrap();
    let archive = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let executable = if cfg!(windows) {
        "consumer.exe"
    } else {
        "consumer"
    };
    let mut consumer = Command::new(rustc);
    consumer.current_dir(root).args([
        "--edition=2021",
        "-C",
        &format!("linker={clang}"),
        "--extern",
        &format!(
            "indexed_sdk={}",
            output.join("libindexed_sdk.rlib").display()
        ),
        "-C",
        &format!("link-arg={}", output.join("native").join(archive).display()),
        "consumer.rs",
        "-o",
        executable,
    ]);
    assert!(consumer.status().unwrap().success());
    Command::new(root.join(executable))
        .status()
        .unwrap()
        .code()
        .unwrap()
}

#[test]
fn indexed_scalar_sdk_publishes_compiled_adapter_and_refuses_signature_drift() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC for physical RI-04 test");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG for physical RI-04 test");
    let archiver = std::env::var("SEMAPRAX_ARCHIVER")
        .expect("configure absolute SEMAPRAX_ARCHIVER for physical RI-04 test");
    for tool in [&rustc, &clang, &archiver] {
        assert!(Path::new(tool).is_absolute());
    }
    let actual_version = Command::new(&rustc).arg("--version").output().unwrap();
    assert!(actual_version.status.success());
    let actual_version = std::str::from_utf8(&actual_version.stdout).unwrap().trim();
    let crate_source = b"pub fn add(left:i64,right:i64)->i64{left+right}\n";
    let crate_source_digest = raw_digest(crate_source);
    let index = index_for(crate_source, actual_version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let package = SelectedPackage {
        cargo_alias: "fixture_math",
        name: "fixture_math",
        version: "0.0.1",
        source_sha256: &crate_source_digest,
        target: target_triple().unwrap(),
        feature_digest: replay.feature_digest(),
        stable_rustc_version: actual_version,
    };
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-ri04-indexed-sdk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("sdk");
    reset_build_observer();
    let bundle = build_indexed_scalar_native_rust_sdk(
        SOURCE,
        Path::new("indexed-sdk.spx"),
        options(),
        &index,
        package,
        crate_source,
        &output,
    )
    .unwrap_or_else(|error| {
        panic!(
            "indexed SDK build failed: {error:?}; stage: {:?}",
            test_build_snapshot()
        )
    });
    assert_eq!(bundle.output_directory(), output);
    let lib = std::fs::read_to_string(output.join("src/lib.rs")).unwrap();
    let manifest = std::fs::read_to_string(output.join("semaprax.native-rust-sdk.json")).unwrap();
    assert!(lib.contains("SEMAPRAX_INDEXED_SCALAR_PROFILE"));
    assert!(lib.contains("let target:fn(i64,i64)->i64=fixture_math::add"));
    assert!(lib.contains(std::str::from_utf8(crate_source).unwrap().trim()));
    assert!(manifest.contains(&raw_digest(lib.as_bytes())));
    assert_eq!(run_published_sdk(&rustc, &clang, &root, &output), 0);

    let flipped = b"pub fn add(left:i64,right:i64)->i64{left+right+1}\n";
    let flipped_digest = raw_digest(flipped);
    let flipped_index = index_for(flipped, actual_version);
    let flipped_package = SelectedPackage {
        source_sha256: &flipped_digest,
        ..package
    };
    let flipped_output = root.join("flipped-sdk");
    reset_build_observer();
    build_indexed_scalar_native_rust_sdk(
        SOURCE,
        Path::new("indexed-sdk.spx"),
        options(),
        &flipped_index,
        flipped_package,
        flipped,
        &flipped_output,
    )
    .unwrap();
    assert_eq!(
        run_published_sdk(&rustc, &clang, &root, &flipped_output),
        12
    );

    let wrong = b"pub fn add(_left:i64,_right:i64)->bool{true}\n";
    let wrong_digest = raw_digest(wrong);
    let wrong_index = index_for(wrong, actual_version);
    let wrong_package = SelectedPackage {
        source_sha256: &wrong_digest,
        ..package
    };
    let wrong_output = root.join("wrong-sdk");
    reset_build_observer();
    let rejected = build_indexed_scalar_native_rust_sdk(
        SOURCE,
        Path::new("indexed-sdk.spx"),
        options(),
        &wrong_index,
        wrong_package,
        wrong,
        &wrong_output,
    )
    .unwrap_err();
    assert!(!rejected.is_empty());
    assert!(!wrong_output.exists());
    let stale_version = "rustc 1.97.1";
    let stale_index = index_for(crate_source, stale_version);
    let stale_package = SelectedPackage {
        stable_rustc_version: stale_version,
        ..package
    };
    let stale_output = root.join("stale-sdk");
    reset_build_observer();
    let rejected = build_indexed_scalar_native_rust_sdk(
        SOURCE,
        Path::new("indexed-sdk.spx"),
        options(),
        &stale_index,
        stale_package,
        crate_source,
        &stale_output,
    )
    .unwrap_err();
    assert!(!rejected.is_empty());
    assert!(!stale_output.exists());
    let hidden = b"pub fn add(left:i64,right:i64)->i64{include!(\"/tmp/other.rs\");left+right}\n";
    let hidden_digest = raw_digest(hidden);
    let hidden_index = index_for(hidden, actual_version);
    let hidden_package = SelectedPackage {
        source_sha256: &hidden_digest,
        ..package
    };
    let hidden_output = root.join("hidden-sdk");
    reset_build_observer();
    assert!(build_indexed_scalar_native_rust_sdk(
        SOURCE,
        Path::new("indexed-sdk.spx"),
        options(),
        &hidden_index,
        hidden_package,
        hidden,
        &hidden_output,
    )
    .is_err());
    assert!(!hidden_output.exists());
    std::fs::remove_dir_all(root).unwrap();
}
