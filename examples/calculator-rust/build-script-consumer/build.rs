#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn configured_path(name: &str) -> PathBuf {
    let path =
        PathBuf::from(env::var_os(name).unwrap_or_else(|| panic!("RI10-E001 {name} is required")));
    assert!(path.is_absolute(), "RI10-E002 {name} must be absolute");
    assert!(
        path.to_str()
            .is_some_and(|value| !value.contains(['\r', '\n'])),
        "RI10-E003 {name} is not a safe Unicode path"
    );
    path
}

fn configured_text(name: &str) -> String {
    let value = env::var(name).unwrap_or_else(|_| panic!("RI10-E001 {name} is required"));
    assert!(
        !value.is_empty() && !value.contains(['\r', '\n']),
        "RI10-E004 {name} must be nonempty and contain no line break"
    );
    value
}

fn require_manifest_field(metadata: &str, field: &str, value: &str) {
    let expected = format!("\"{field}\":\"{value}\"");
    assert!(
        metadata.contains(&expected),
        "RI10-E005 prepared SDK {field} does not match its explicit binding"
    );
}

fn copy(input: &Path, output: &Path, track_external: bool) {
    assert!(
        input.is_file(),
        "prepared SDK input is missing: {}",
        input.display()
    );
    fs::copy(input, output)
        .unwrap_or_else(|error| panic!("cannot stage {}: {error}", input.display()));
    if track_external {
        println!("cargo:rerun-if-changed={}", input.display());
    }
}

fn tracked_inputs() -> Vec<PathBuf> {
    let value =
        env::var("SEMAPRAX_RI10_INPUTS").expect("RI10-E001 SEMAPRAX_RI10_INPUTS is required");
    let rows = value.split(';').collect::<Vec<_>>();
    assert!(
        !rows.is_empty() && rows.len() <= 32,
        "RI10-E006 tracked input count is invalid"
    );
    let mut inputs = Vec::with_capacity(rows.len());
    for row in rows {
        let input = PathBuf::from(row);
        assert!(
            input.is_absolute()
                && input.is_file()
                && input
                    .to_str()
                    .is_some_and(|path| path.len() <= 4_096 && !path.contains(['\r', '\n'])),
            "RI10-E006 tracked RI-10 input must be an absolute file"
        );
        println!("cargo:rerun-if-changed={}", input.display());
        inputs.push(input);
    }
    inputs
}

fn build_explicit_project(out_dir: &Path, inputs: &[PathBuf]) -> PathBuf {
    let builder = configured_path("SEMAPRAX_RI10_BUILDER");
    let project = configured_path("SEMAPRAX_RI10_PROJECT_MANIFEST");
    assert!(
        builder.is_file() && project.is_file(),
        "RI10-E007 builder inputs are missing"
    );
    for name in ["RUSTC", "CLANG", "SEMAPRAX_ARCHIVER"] {
        let tool = configured_path(name);
        assert!(tool.is_file(), "RI10-E007 configured tool is missing");
        let resolved = tool
            .canonicalize()
            .expect("RI10-E007 configured tool cannot be resolved");
        assert!(
            resolved
                .to_str()
                .is_some_and(|path| !path.contains(['\r', '\n'])),
            "RI10-E007 configured tool path is unsafe"
        );
        println!("cargo:rerun-if-changed={}", resolved.display());
    }
    assert!(
        inputs.iter().any(|input| input == &project),
        "RI10-E007 Project manifest must be a tracked input"
    );
    let resolved_builder = builder
        .canonicalize()
        .expect("RI10-E007 configured builder cannot be resolved");
    assert!(
        resolved_builder
            .to_str()
            .is_some_and(|path| !path.contains(['\r', '\n'])),
        "RI10-E007 configured builder path is unsafe"
    );
    println!("cargo:rerun-if-changed={}", resolved_builder.display());
    let generated = out_dir.join("semaprax_generated");
    if let Ok(metadata) = fs::symlink_metadata(&generated) {
        assert!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "RI10-E007 generated output has an invalid type"
        );
        fs::remove_dir_all(&generated).expect("RI10-E007 cannot refresh owned output");
    }
    let status = Command::new(&builder)
        .args(["project", "--manifest-path"])
        .arg(&project)
        .arg("--output")
        .arg(&generated)
        .status()
        .expect("RI10-E007 explicit builder could not start");
    assert!(status.success(), "RI10-E007 explicit Project build failed");
    generated
}

fn main() {
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_PREPARED_SDK");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_INPUTS");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_SDK_VERSION");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_DESCRIPTOR_DIGEST");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_BUNDLE_DIGEST");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_BUILDER");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_PROJECT_MANIFEST");
    for tool in ["RUSTC", "CLANG", "SEMAPRAX_ARCHIVER"] {
        println!("cargo:rerun-if-env-changed={tool}");
    }
    let inputs = tracked_inputs();
    let output_root = configured_path("OUT_DIR");
    let explicit_build = env::var_os("SEMAPRAX_RI10_BUILDER").is_some();
    let sdk = if explicit_build {
        build_explicit_project(&output_root, &inputs)
    } else {
        configured_path("SEMAPRAX_RI10_PREPARED_SDK")
    };
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    let manifest = sdk.join("semaprax.native-rust-sdk.json");
    let metadata = fs::read_to_string(&manifest).expect("prepared SDK manifest is required");
    require_manifest_field(&metadata, "target", &target);
    if !explicit_build {
        require_manifest_field(
            &metadata,
            "version",
            &configured_text("SEMAPRAX_RI10_SDK_VERSION"),
        );
        require_manifest_field(
            &metadata,
            "descriptor_digest",
            &configured_text("SEMAPRAX_RI10_DESCRIPTOR_DIGEST"),
        );
        require_manifest_field(
            &metadata,
            "bundle_digest",
            &configured_text("SEMAPRAX_RI10_BUNDLE_DIGEST"),
        );
    }
    let output = output_root.join("semaprax_sdk");
    fs::create_dir_all(&output).unwrap();
    copy(
        &sdk.join("src/lib.rs"),
        &output.join("lib.rs"),
        !explicit_build,
    );
    copy(
        &sdk.join("src/semaprax_native_rust_interop.rs"),
        &output.join("semaprax_native_rust_interop.rs"),
        !explicit_build,
    );
    copy(
        &sdk.join("src/semaprax_native_rust_interop_ffi.rs"),
        &output.join("semaprax_native_rust_interop_ffi.rs"),
        !explicit_build,
    );
    copy(
        &manifest,
        &output.join("semaprax.native-rust-sdk.json"),
        !explicit_build,
    );
    let archive = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let native = sdk.join("native").join(archive);
    copy(&native, &output.join(archive), !explicit_build);
    println!("cargo:rustc-link-search=native={}", output.display());
    println!("cargo:rustc-link-lib=static=semaprax_native_rust_sdk");
}
