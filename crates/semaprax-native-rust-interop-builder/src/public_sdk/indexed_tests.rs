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

fn method_index_for(
    source: &[u8],
    stable_rustc: &str,
    alias: &str,
    receiver: &str,
    visible: bool,
) -> Vec<u8> {
    method_index_with_signature(source, stable_rustc, alias, receiver, visible, None)
}

fn method_index_with_signature(
    source: &[u8],
    stable_rustc: &str,
    alias: &str,
    receiver: &str,
    visible: bool,
    signature: Option<&str>,
) -> Vec<u8> {
    let mut envelope: Value = serde_json::from_slice(include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index = &mut envelope["index"];
    index["package"]["name"] = "fixture_math".into();
    index["package"]["version"] = "0.0.1".into();
    index["package"]["source_sha256"] = raw_digest(source).into();
    if alias != "fixture_math" {
        index["package"]["renamed_from"] = alias.into();
    }
    index["target"] = target_triple().unwrap().into();
    index["stable_rustc_version"] = stable_rustc.into();
    let mut item = index["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "local_api_fixture::MacroGenerated::answer")
        .unwrap()
        .clone();
    let method_path = format!("{alias}::Meter::add");
    let type_path = format!("{alias}::Meter");
    item["path"] = method_path.into();
    item["receiver"] = receiver.into();
    item["signature"] = signature
        .unwrap_or(if receiver == "mutable" {
            "fn add(&mut self, delta: i64) -> i64"
        } else {
            "fn add(&self, delta: i64) -> i64"
        })
        .into();
    if !visible {
        item["visibility"] = "private".into();
        item["support"] = "rejected".into();
        item["reason"] = "private".into();
    }
    item["type_roots"] = serde_json::json!([type_path.clone()]);
    item["reachable_types"] = serde_json::json!([type_path.clone()]);
    let mut ty = index["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ty| ty["path"] == "local_api_fixture::MacroGenerated")
        .unwrap()
        .clone();
    ty["path"] = type_path.into();
    index["items"] = serde_json::json!([item]);
    index["types"] = serde_json::json!([ty]);
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
    let (selected_program, selected_hir, selected_plan, _) = indexed::prepare_indexed_scalar(
        SOURCE,
        Path::new("indexed-sdk.spx"),
        &options(),
        &index,
        package,
        crate_source,
    )
    .unwrap();
    let unbound = semaprax::check(SOURCE, Path::new("indexed-sdk.spx")).unwrap_err();
    assert_eq!(unbound[0].code, "SPX-B147");
    assert_eq!(unbound[0].path.as_deref(), Some("indexed-sdk.spx"));
    assert!(unbound[0].span.is_some());
    let canonical = semaprax::format::canonical(&selected_program);
    assert!(canonical.contains("import rust selected fn host_add from \"fixture_math::add\""));
    let reparsed = semaprax::parse(&canonical, Path::new("indexed-sdk.spx")).unwrap();
    assert!(reparsed.interfaces[0].imports[0].index_selected);
    assert_eq!(selected_hir.interfaces[0].imports[0].parameters.len(), 2);
    let graph = semaprax::graph::to_json(&selected_program).unwrap();
    assert!(graph.contains("\"schema\":\"semaprax.graph.v53\""));
    assert!(graph.contains(&selected_plan.index_digest));
    let (roundtrip, _, roundtrip_plan, _) = indexed::prepare_indexed_scalar(
        &canonical,
        Path::new("indexed-sdk.spx"),
        &options(),
        &index,
        package,
        crate_source,
    )
    .unwrap();
    assert_eq!(roundtrip_plan, selected_plan);
    assert_eq!(semaprax::graph::to_json(&roundtrip).unwrap(), graph);
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

#[test]
fn indexed_shared_method_executes_and_refuses_inaccessible_or_unsupported_receivers() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC for physical RI-04 test");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG for physical RI-04 test");
    let archiver = std::env::var("SEMAPRAX_ARCHIVER").expect("configure absolute archiver");
    for tool in [&rustc, &clang, &archiver] {
        assert!(Path::new(tool).is_absolute());
    }
    let actual_version = Command::new(&rustc).arg("--version").output().unwrap();
    assert!(actual_version.status.success());
    let actual_version = std::str::from_utf8(&actual_version.stdout).unwrap().trim();
    let source = SOURCE.replace("fixture_math::add", "fixture_math::Meter::add");
    let crate_source = b"pub struct Meter{base:i64} impl From<i64> for Meter{fn from(value:i64)->Self{Self{base:value}}} impl Meter{pub fn add(&self,delta:i64)->i64{self.base+delta}}\n";
    let crate_digest = raw_digest(crate_source);
    let index = method_index_for(crate_source, actual_version, "fixture_math", "shared", true);
    let replay = RustApiIndex::replay(&index).unwrap();
    let package = SelectedPackage {
        cargo_alias: "fixture_math",
        name: "fixture_math",
        version: "0.0.1",
        source_sha256: &crate_digest,
        target: target_triple().unwrap(),
        feature_digest: replay.feature_digest(),
        stable_rustc_version: actual_version,
    };
    let (program, resolved, plan, _) = indexed::prepare_indexed_scalar(
        &source,
        Path::new("indexed-method.spx"),
        &options(),
        &index,
        package,
        crate_source,
    )
    .unwrap();
    assert_eq!(plan.receiver, "shared");
    assert_eq!(resolved.interfaces[0].imports[0].parameters.len(), 2);
    assert_eq!(
        resolved.interfaces[0].imports[0]
            .selected_receiver
            .as_deref(),
        Some("shared")
    );
    let graph = semaprax::graph::to_json(&program).unwrap();
    assert!(graph.contains("\"schema\":\"semaprax.graph.v54\""));
    assert!(graph.contains("\"rust_receiver\":\"shared\""));
    let canonical = semaprax::format::canonical(&program);
    let (rebound, _, rebound_plan, _) = indexed::prepare_indexed_scalar(
        &canonical,
        Path::new("indexed-method.spx"),
        &options(),
        &index,
        package,
        crate_source,
    )
    .unwrap();
    assert_eq!(rebound_plan, plan);
    assert_eq!(semaprax::graph::to_json(&rebound).unwrap(), graph);
    assert_eq!(
        semaprax::wasm::emit_module(&program).unwrap_err().code,
        "SPX-W114"
    );
    assert_eq!(
        semaprax::wasm::emit_resolved_module(&resolved)
            .unwrap_err()
            .code,
        "SPX-W114"
    );

    for malformed_call in [
        source.replace("host_add(left, right)", "host_add(left)"),
        source.replace("host_add(left, right)", "host_add(true, right)"),
    ] {
        let failure = indexed::prepare_indexed_scalar(
            &malformed_call,
            Path::new("indexed-method.spx"),
            &options(),
            &index,
            package,
            crate_source,
        )
        .unwrap_err();
        assert_eq!(failure[0].code, "SPX-B107");
        assert!(failure[0].span.is_some());
    }
    let wrong_feature = SelectedPackage {
        feature_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ..package
    };
    let feature_failure = indexed::prepare_indexed_scalar(
        &source,
        Path::new("indexed-method.spx"),
        &options(),
        &index,
        wrong_feature,
        crate_source,
    )
    .unwrap_err();
    assert_eq!(feature_failure[0].code, "SPX-B142");
    assert!(feature_failure[0].span.is_some());
    let unsupported_index = method_index_with_signature(
        crate_source,
        actual_version,
        "fixture_math",
        "shared",
        true,
        Some("fn add(&self, delta: u8) -> i64"),
    );
    let signature_failure = indexed::prepare_indexed_scalar(
        &source,
        Path::new("indexed-method.spx"),
        &options(),
        &unsupported_index,
        package,
        crate_source,
    )
    .unwrap_err();
    assert_eq!(signature_failure[0].code, "SPX-B145");
    assert!(signature_failure[0].span.is_some());
    let domain_result_index = method_index_with_signature(
        crate_source,
        actual_version,
        "fixture_math",
        "shared",
        true,
        Some("fn add(&self, delta: i64) -> core::result::Result<i64, bool>"),
    );
    let domain_result_failure = indexed::prepare_indexed_scalar(
        &source,
        Path::new("indexed-method.spx"),
        &options(),
        &domain_result_index,
        package,
        crate_source,
    )
    .unwrap_err();
    assert_eq!(domain_result_failure[0].code, "SPX-B145");
    assert!(domain_result_failure[0].span.is_some());

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-ri04-indexed-method-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let interpreter_source = root.join("selected.spx");
    std::fs::write(&interpreter_source, &source).unwrap();
    let interpreter_failure = semaprax::interpreter::interpret(
        &interpreter_source,
        "interop.add",
        &["20".into(), "22".into()],
        &semaprax::interpreter::InterpreterOptions::default(),
    )
    .unwrap_err();
    assert!(!interpreter_failure.is_empty());
    assert!(interpreter_failure
        .iter()
        .any(|diagnostic| diagnostic.span.is_some()));
    let output = root.join("sdk");
    reset_build_observer();
    build_indexed_scalar_native_rust_sdk(
        &source,
        Path::new("indexed-method.spx"),
        options(),
        &index,
        package,
        crate_source,
        &output,
    )
    .unwrap();
    let lib = std::fs::read_to_string(output.join("src/lib.rs")).unwrap();
    let manifest = std::fs::read_to_string(output.join("semaprax.native-rust-sdk.json")).unwrap();
    assert!(lib.contains("let target:fn(&fixture_math::Meter,i64)->i64=fixture_math::Meter::add"));
    assert!(manifest.contains(&raw_digest(lib.as_bytes())));
    assert_eq!(run_published_sdk(&rustc, &clang, &root, &output), 0);

    let flipped = b"pub struct Meter{base:i64} impl From<i64> for Meter{fn from(value:i64)->Self{Self{base:value}}} impl Meter{pub fn add(&self,delta:i64)->i64{self.base+delta+1}}\n";
    let flipped_digest = raw_digest(flipped);
    let flipped_index = method_index_for(flipped, actual_version, "fixture_math", "shared", true);
    let flipped_package = SelectedPackage {
        source_sha256: &flipped_digest,
        ..package
    };
    let flipped_output = root.join("flipped-sdk");
    reset_build_observer();
    build_indexed_scalar_native_rust_sdk(
        &source,
        Path::new("indexed-method.spx"),
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

    let private_index = method_index_for(
        crate_source,
        actual_version,
        "fixture_math",
        "shared",
        false,
    );
    let private_output = root.join("private-sdk");
    reset_build_observer();
    let inaccessible = build_indexed_scalar_native_rust_sdk(
        &source,
        Path::new("indexed-method.spx"),
        options(),
        &private_index,
        package,
        crate_source,
        &private_output,
    )
    .unwrap_err();
    assert_eq!(inaccessible[0].code, "SPX-B141");
    assert!(inaccessible[0].span.is_some());
    assert!(!private_output.exists());

    let mutable_index = method_index_for(
        crate_source,
        actual_version,
        "fixture_math",
        "mutable",
        true,
    );
    let mutable_output = root.join("mutable-sdk");
    reset_build_observer();
    let unsupported = build_indexed_scalar_native_rust_sdk(
        &source,
        Path::new("indexed-method.spx"),
        options(),
        &mutable_index,
        package,
        crate_source,
        &mutable_output,
    )
    .unwrap_err();
    assert_eq!(unsupported[0].code, "SPX-B144");
    assert!(unsupported[0].span.is_some());
    assert!(!mutable_output.exists());

    let wrong_alias = SelectedPackage {
        cargo_alias: "other_alias",
        ..package
    };
    let alias_output = root.join("wrong-alias-sdk");
    reset_build_observer();
    let rejected = build_indexed_scalar_native_rust_sdk(
        &source,
        Path::new("indexed-method.spx"),
        options(),
        &index,
        wrong_alias,
        crate_source,
        &alias_output,
    )
    .unwrap_err();
    assert_eq!(rejected[0].code, "SPX-B142");
    assert!(rejected[0].span.is_some());
    assert!(!alias_output.exists());

    let renamed_index = method_index_for(
        crate_source,
        actual_version,
        "fixture_alias",
        "shared",
        true,
    );
    let renamed_source = source.replace("fixture_math::Meter::add", "fixture_alias::Meter::add");
    let renamed_package = SelectedPackage {
        cargo_alias: "fixture_alias",
        ..package
    };
    let (_, _, renamed_plan, _) = indexed::prepare_indexed_scalar(
        &renamed_source,
        Path::new("indexed-method.spx"),
        &options(),
        &renamed_index,
        renamed_package,
        crate_source,
    )
    .unwrap();
    assert_ne!(plan.physical_symbol, renamed_plan.physical_symbol);

    let keyword_index = method_index_for(crate_source, actual_version, "type", "shared", true);
    let keyword_source = source.replace("fixture_math::Meter::add", "type::Meter::add");
    let keyword_package = SelectedPackage {
        cargo_alias: "type",
        ..package
    };
    let (_, _, keyword_plan, _) = indexed::prepare_indexed_scalar(
        &keyword_source,
        Path::new("indexed-method.spx"),
        &options(),
        &keyword_index,
        keyword_package,
        crate_source,
    )
    .unwrap();
    assert_ne!(plan.physical_symbol, keyword_plan.physical_symbol);
    let keyword_output = root.join("keyword-sdk");
    reset_build_observer();
    build_indexed_scalar_native_rust_sdk(
        &keyword_source,
        Path::new("indexed-method.spx"),
        options(),
        &keyword_index,
        keyword_package,
        crate_source,
        &keyword_output,
    )
    .unwrap();
    let keyword_lib = std::fs::read_to_string(keyword_output.join("src/lib.rs")).unwrap();
    assert!(keyword_lib.contains("mod r#type{"));
    assert!(keyword_lib.contains("r#type::Meter::add"));
    assert_eq!(run_published_sdk(&rustc, &clang, &root, &keyword_output), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "indexed_multiple_tests.rs"]
mod indexed_multiple;

#[test]
fn indexed_result_domain_round_trips_ok_and_err_separately_from_bridge_failure() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG");
    let _archiver = std::env::var("SEMAPRAX_ARCHIVER").expect("configure absolute archiver");
    let actual_version = Command::new(&rustc).arg("--version").output().unwrap();
    assert!(actual_version.status.success());
    let actual_version = std::str::from_utf8(&actual_version.stdout).unwrap().trim();
    let crate_source = b"pub fn divide(left:i64,right:i64)->core::result::Result<i64,i64>{if right==0{Err(7)}else{Ok(left/right)}}\n";
    let mut envelope: Value = serde_json::from_slice(include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index_row = &mut envelope["index"];
    index_row["package"]["name"] = "fixture_math".into();
    index_row["package"]["version"] = "0.0.1".into();
    index_row["package"]["source_sha256"] = raw_digest(crate_source).into();
    index_row["target"] = target_triple().unwrap().into();
    index_row["stable_rustc_version"] = actual_version.into();
    let mut item = index_row["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "local_api_fixture::cfg_selected")
        .unwrap()
        .clone();
    item["path"] = "fixture_math::divide".into();
    item["signature"] =
        "fn divide(left: i64, right: i64) -> core::result::Result<i64, i64>".into();
    index_row["items"] = serde_json::json!([item]);
    index_row["types"] = serde_json::json!([]);
    let mut extractor_bytes = serde_json::to_vec(&envelope).unwrap();
    extractor_bytes.push(b'\n');
    let index = RustApiIndex::admit_extractor_output(&extractor_bytes)
        .unwrap()
        .canonical_json()
        .as_bytes()
        .to_vec();
    let replay = RustApiIndex::replay(&index).unwrap();
    let source_digest = raw_digest(crate_source);
    let package = SelectedPackage {
        cargo_alias: "fixture_math",
        name: "fixture_math",
        version: "0.0.1",
        source_sha256: &source_digest,
        target: target_triple().unwrap(),
        feature_digest: replay.feature_digest(),
        stable_rustc_version: actual_version,
    };
    let source = r#"module result.fixture;
permit { host.math }
@id("host.math") interface HostMath permits { host.math } {
    @id("host.divide") import rust selected fn divide from "fixture_math::divide"
        effects { host.math } failure status "host.math.v1";
}
@id("result.forward") fn forward(left: i64, right: i64) -> Result<i64, i64> uses { host.math } {
    divide(left, right)
}
@id("result.main") fn main() -> i64 { 0 }
"#;
    let options = NativeRustSdkOptions {
        exports: vec!["result.forward".into()],
        imports: vec!["host.divide".into()],
        capabilities: vec!["host.math".into()],
    };
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-ri04-result-sdk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("sdk");
    build_indexed_scalar_native_rust_sdk(
        source,
        Path::new("result-sdk.spx"),
        options,
        &index,
        package,
        crate_source,
        &output,
    )
    .unwrap_or_else(|error| panic!("Result SDK build failed: {error:?}"));
    let lib = std::fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(lib.contains("let target:fn(i64,i64)->core::result::Result<i64,i64>=fixture_math::divide"));
    let mut library = Command::new(&rustc);
    library.current_dir(&output).args([
        "--edition=2021", "--crate-name", "indexed_sdk", "--crate-type=rlib", "src/lib.rs",
        "-o", "libindexed_sdk.rlib",
    ]);
    assert!(library.status().unwrap().success());
    std::fs::write(root.join("consumer.rs"),
        "fn main(){let mut sdk=indexed_sdk::indexed_scalar_sdk(&[\"host.math\"]).unwrap();if !matches!(sdk.spx_result_dot_forward(8,2),Ok(Ok(4))){std::process::exit(11)}if !matches!(sdk.spx_result_dot_forward(8,0),Ok(Err(7))){std::process::exit(12)}if indexed_sdk::indexed_scalar_sdk(&[]).is_ok(){std::process::exit(13)}}\n").unwrap();
    let archive = if cfg!(windows) { "semaprax_native_rust_sdk.lib" } else { "libsemaprax_native_rust_sdk.a" };
    let executable = if cfg!(windows) { "consumer.exe" } else { "consumer" };
    let status = Command::new(&rustc)
        .current_dir(&root)
        .args(["--edition=2021", "-C", &format!("linker={clang}"), "--extern",
            &format!("indexed_sdk={}", output.join("libindexed_sdk.rlib").display()),
            "-C", &format!("link-arg={}", output.join("native").join(archive).display()),
            "consumer.rs", "-o", executable])
        .status().unwrap();
    assert!(status.success());
    assert!(Command::new(root.join(executable)).status().unwrap().success());
    std::fs::remove_dir_all(root).unwrap();
}
