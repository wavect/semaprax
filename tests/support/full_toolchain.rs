use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::native_rust_cargo;

/// Build the explicitly unpublished CLI offline; never substitute the registry CLI.
pub fn binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let executable = std::env::current_exe().expect("integration test executable");
            let profile = executable.parent().unwrap().parent().unwrap();
            let target = profile.parent().unwrap();
            let output = native_rust_cargo::cargo_command()
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .args([
                    "build",
                    "--locked",
                    "--offline",
                    "-p",
                    "semaprax-toolchain",
                    "--bin",
                    "semaprax-full",
                ])
                .arg("--target-dir")
                .arg(target)
                .output()
                .expect("build unpublished full-toolchain CLI");
            assert!(
                output.status.success(),
                "full-toolchain build failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let binary = target
                .join("debug")
                .join(format!("semaprax-full{}", std::env::consts::EXE_SUFFIX));
            assert!(binary.is_file(), "full-toolchain binary is missing");
            binary
        })
        .as_path()
}
