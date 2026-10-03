//! Real rustc refusal controls for the local-mirror coherence boundary.
//! The generator only implements foreign traits for its generated local type.
//! These deliberate downstream mutations prove rustc rejects a foreign target
//! or an overlapping implementation instead of silently accepting either.
use std::{ffi::OsStr, path::Path, process::Command};

pub(super) fn verify(root: &Path, cargo: &OsStr, consumer: &str) {
    for (label, implementation, expected) in [
        (
            "foreign-target",
            r#"
impl serde::Serialize for std::sync::Barrier {
    fn serialize<S>(&self, serializer:S)->Result<S::Ok,S::Error>
    where S:serde::Serializer { serializer.serialize_unit() }
}
"#,
            "E0117",
        ),
        (
            "overlapping-local",
            r#"
impl serde::Serialize for SpxMirrorri07record {
    fn serialize<S>(&self, serializer:S)->Result<S::Ok,S::Error>
    where S:serde::Serializer { serializer.serialize_unit() }
}
"#,
            "E0119",
        ),
    ] {
        std::fs::write(
            root.join("src/main.rs"),
            format!("{consumer}\n{implementation}"),
        )
        .unwrap();
        let output = Command::new(cargo)
            .args([
                "check",
                "--offline",
                "--locked",
                "--message-format=json",
                "--manifest-path",
            ])
            .arg(root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{label} unexpectedly compiled");
        let diagnostics = output
            .stdout
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
            .filter(|row| row["reason"] == "compiler-message")
            .collect::<Vec<_>>();
        assert!(
            diagnostics
                .iter()
                .any(|row| row["message"]["level"] == "error"
                    && row["message"]["code"]["code"] == expected),
            "{label} did not produce {expected}: {:?}; {}",
            diagnostics,
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!("RI-07 {label}: rustc {expected} refused");
    }
}
