use std::{env, path::PathBuf, process::Command};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let sources = [
        root.join("generated/regex/src/regex_project.c"),
        root.join("generated/url/src/url_project.c"),
        root.join("generated/m2/module.c"),
    ];
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    if sources.iter().any(|source| !source.is_file()) {
        return;
    }
    let clang = env::var_os("CLANG").expect("set absolute CLANG for linked RI-13 consumer");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let objects = [
        out.join("m1_regex.o"),
        out.join("m1_url.o"),
        out.join("m2.o"),
    ];
    let archive = out.join("libri13_linked.a");
    for (source, object) in sources.iter().zip(&objects) {
        assert!(Command::new(&clang)
            .args(["-std=c11", "-O0", "-c"])
            .arg(source)
            .arg("-o")
            .arg(object)
            .status()
            .unwrap()
            .success());
    }
    assert!(
        Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()))
            .arg("crs")
            .arg(&archive)
            .args(&objects)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=ri13_linked");
}
