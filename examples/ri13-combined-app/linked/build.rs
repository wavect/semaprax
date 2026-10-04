use std::{env, path::PathBuf, process::Command};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = root.join("generated/m2/module.c");
    println!("cargo:rerun-if-changed={}", source.display());
    if !source.is_file() {
        return;
    }
    let clang = env::var_os("CLANG").expect("set absolute CLANG for linked RI-13 consumer");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("m2.o");
    let archive = out.join("libri13_linked_m2.a");
    assert!(Command::new(clang)
        .args(["-std=c11", "-O0", "-c"])
        .arg(source)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    assert!(
        Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()))
            .arg("crs")
            .arg(&archive)
            .arg(&object)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=ri13_linked_m2");
}
