#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn configured_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env::var_os(name).unwrap_or_else(|| panic!("{name} is required")));
    assert!(path.is_absolute(), "{name} must be absolute");
    assert!(
        path.to_str()
            .is_some_and(|value| !value.contains(['\r', '\n'])),
        "{name} is not a safe Unicode path"
    );
    path
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
    let sdk = configured_path("SEMAPRAX_RI10_PREPARED_SDK");
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    let manifest = sdk.join("semaprax.native-rust-sdk.json");
    let metadata = fs::read_to_string(&manifest).expect("prepared SDK manifest is required");
    assert!(
        metadata.contains(&format!("\"target\":\"{target}\"")),
        "prepared SDK target differs from Cargo TARGET"
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
        .expect("SEMAPRAX_RI10_INPUTS is required")
        .split(';')
    {
        let input = Path::new(input);
        assert!(
            input.is_absolute() && input.is_file(),
            "tracked RI-10 input must be an absolute file"
        );
        println!("cargo:rerun-if-changed={}", input.display());
    }
}
