use std::{env, path::PathBuf, process::Command};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let sources = [
        root.join("generated/regex/src/regex_project.c"),
        root.join("generated/url/src/url_project.c"),
        root.join("generated/m2/module.c"),
    ];
    let linked_subject = root.join("generated/linked-subject.json");
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    println!("cargo:rerun-if-changed={}", linked_subject.display());
    if sources.iter().any(|source| !source.is_file()) || !linked_subject.is_file() {
        return;
    }
    let binding = std::fs::read_to_string(&linked_subject).expect("linked subject binding");
    for fragment in [
        "\"schema\": \"semaprax.ri13.linked-subject.v1\"",
        "\"m1_project_subject\": \"sha256:",
        "\"m2_source_revision\": \"sha256:",
        "\"m3_project_revision\": \"sha256:",
        "\"candidate\": \"unified-project/semaprax.toml\"",
    ] {
        assert!(
            binding.contains(fragment),
            "linked subject binding is incomplete"
        );
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
