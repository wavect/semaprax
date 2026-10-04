use std::{env, path::PathBuf, process::Command};

fn quoted_field<'a>(line: &'a str, key: &str, trailing_comma: bool) -> &'a str {
    let prefix = format!("  \"{key}\": \"");
    let suffix = if trailing_comma { "\"," } else { "\"" };
    line.strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .unwrap_or_else(|| panic!("linked subject field {key} is malformed"))
}

fn sha256_field(line: &str, key: &str) -> String {
    let value = quoted_field(line, key, true);
    assert!(
        value.strip_prefix("sha256:").is_some_and(
            |digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        ),
        "linked subject field {key} is not a SHA-256 digest"
    );
    value.to_owned()
}

fn validate_linked_subject(binding: &str) {
    // `bin_prepare` emits this small, fixed envelope. Accept no alternative
    // shape so a stale generated Future module cannot reach C compilation.
    let mut lines = binding.lines();
    assert_eq!(
        lines.next(),
        Some("{"),
        "linked subject envelope is malformed"
    );
    assert_eq!(
        quoted_field(lines.next().expect("linked subject schema"), "schema", true,),
        "semaprax.ri13.linked-subject.v1",
        "linked subject schema is unsupported"
    );
    let _m1 = sha256_field(
        lines.next().expect("linked M1 subject"),
        "m1_project_subject",
    );
    let project_revision = sha256_field(
        lines.next().expect("linked Project revision"),
        "project_revision",
    );
    let _m2 = sha256_field(
        lines.next().expect("linked M2 source revision"),
        "m2_source_revision",
    );
    let m3_revision = sha256_field(
        lines.next().expect("linked M3 Project revision"),
        "m3_project_revision",
    );
    assert_eq!(
        quoted_field(
            lines.next().expect("linked subject candidate"),
            "candidate",
            false,
        ),
        "unified-project/semaprax.toml",
        "linked subject candidate is unsupported"
    );
    assert_eq!(
        lines.next(),
        Some("}"),
        "linked subject envelope is malformed"
    );
    assert!(
        lines.next().is_none(),
        "linked subject envelope has trailing data"
    );
    assert_eq!(
        project_revision, m3_revision,
        "linked Project/M3 revision binding is stale"
    );
}

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
    validate_linked_subject(&binding);
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
