//! Real native companion; the invoking runner supplies the external deadline
//! and resource limits. Command::output is not an intrinsic capture bound.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn run(root: &Path, provider: &str) {
    let source = format!(
        "{}\n{}\n{}\n{}",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_tuple_admission_v1/allocations.c"),
        provider,
        include_str!("native.c"),
    );
    let directory = root.join("native");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("provider.c");
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    for optimization in ["-O0", "-O2"] {
        let executable = directory.join(format!(
            "inactive{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        assert!(!executable.exists());
        let compiled = Command::new(&compiler)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for native inactive-cleanup evidence");
        assert!(
            compiled.status.success(),
            "{optimization}: {}\n{}",
            String::from_utf8_lossy(&compiled.stdout),
            String::from_utf8_lossy(&compiled.stderr)
        );
        let observed = Command::new(&executable).output().unwrap();
        assert!(
            observed.status.success(),
            "{optimization}: {}\n{}",
            String::from_utf8_lossy(&observed.stdout),
            String::from_utf8_lossy(&observed.stderr)
        );
        assert_eq!(observed.stdout, b"project-owned-inactive-native-ok\n");
        assert!(observed.stderr.is_empty());
        assert_eq!(fs::read(&path).unwrap(), source.as_bytes());
    }
    // Fixed, caller-owned fixture retained on success and failure.
}
