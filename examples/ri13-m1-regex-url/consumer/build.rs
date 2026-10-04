use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let clang = PathBuf::from(env::var_os("CLANG").expect("set an absolute CLANG path"));
    assert!(
        clang.is_absolute() && clang.is_file(),
        "CLANG must resolve to one executable"
    );
    let version = Command::new(&clang)
        .arg("--version")
        .output()
        .expect("resolved CLANG");
    assert!(version.status.success());
    println!(
        "cargo:warning=ri13 M1 C compiler: {}",
        String::from_utf8_lossy(&version.stdout)
            .lines()
            .next()
            .unwrap_or("unknown")
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (name, relative) in [
        ("regex", "generated/regex/src/regex_project.c"),
        ("url", "generated/url/src/url_project.c"),
    ] {
        let source = root.join(relative);
        println!("cargo:rerun-if-changed={}", source.display());
        let object = out.join(format!("{name}_project.o"));
        let result = Command::new(&clang)
            .args(["-std=c11", "-O0", "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .expect("C compilation");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        println!("cargo:rustc-link-arg={}", object.display());
    }
}
