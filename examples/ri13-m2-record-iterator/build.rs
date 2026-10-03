use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=generated/module.c");
    println!("cargo:rerun-if-changed=generated/semaprax_native_rust_interop.h");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = root.join("generated/module.c");
    if !source.is_file() {
        return; // The prepare binary creates the authenticated projection first.
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("module.o");
    let archive = out.join("libri13_m2_callback.a");
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let ar = env::var_os("AR").unwrap_or_else(|| "ar".into());
    assert!(Command::new(clang)
        .args(["-std=c11", "-O0", "-c"])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .status()
        .expect("resolved C compiler")
        .success());
    assert!(Command::new(ar)
        .arg("crs")
        .arg(&archive)
        .arg(&object)
        .status()
        .expect("resolved archiver")
        .success());
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=ri13_m2_callback");
}
