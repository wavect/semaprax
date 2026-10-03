#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
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

fn copy(input: &Path, output: &Path) {
    assert!(
        input.is_file(),
        "prepared SDK input is missing: {}",
        input.display()
    );
    fs::copy(input, output)
        .unwrap_or_else(|error| panic!("cannot stage {}: {error}", input.display()));
    println!("cargo:rerun-if-changed={}", input.display());
}

fn main() {
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_PREPARED_SDK");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_INPUTS");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_SDK_VERSION");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_DESCRIPTOR_DIGEST");
    println!("cargo:rerun-if-env-changed=SEMAPRAX_RI10_BUNDLE_DIGEST");
    let sdk = configured_path("SEMAPRAX_RI10_PREPARED_SDK");
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    let manifest = sdk.join("semaprax.native-rust-sdk.json");
    let metadata = fs::read_to_string(&manifest).expect("prepared SDK manifest is required");
    require_manifest_field(&metadata, "target", &target);
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
    let output = configured_path("OUT_DIR").join("semaprax_sdk");
    fs::create_dir_all(&output).unwrap();
    copy(&sdk.join("src/lib.rs"), &output.join("lib.rs"));
    copy(
        &sdk.join("src/semaprax_native_rust_interop.rs"),
        &output.join("semaprax_native_rust_interop.rs"),
    );
    copy(&manifest, &output.join("semaprax.native-rust-sdk.json"));
    let archive = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let native = sdk.join("native").join(archive);
    copy(&native, &output.join(archive));
    println!("cargo:rustc-link-search=native={}", output.display());
    println!("cargo:rustc-link-lib=static=semaprax_native_rust_sdk");
    for input in env::var("SEMAPRAX_RI10_INPUTS")
        .expect("RI10-E001 SEMAPRAX_RI10_INPUTS is required")
        .split(';')
    {
        let input = Path::new(input);
        assert!(
            input.is_absolute() && input.is_file(),
            "RI10-E006 tracked RI-10 input must be an absolute file"
        );
        println!("cargo:rerun-if-changed={}", input.display());
    }
}
