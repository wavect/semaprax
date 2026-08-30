//! Select Cargo's reported binary, never a guessed target/profile pathname.
//!
//! Cargo's successful build and freshness decision remain trusted inputs. This
//! does not attest executable contents or defend against a hostile Cargo process.

use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn select_artifact(stdout: &[u8], expected_manifest: &Path) -> Result<PathBuf, String> {
    let expected_manifest = expected_manifest
        .canonicalize()
        .map_err(|error| format!("cannot identify full-toolchain manifest: {error}"))?;
    let mut executable = None;
    let mut finished = false;
    for (index, line) in stdout.split(|byte| *byte == b'\n').enumerate() {
        let line = line.trim_ascii_start();
        // Cargo documents that other tools may print non-JSON stdout. They
        // cannot contribute artifact identity or a build-finished receipt.
        if !line.starts_with(b"{") {
            continue;
        }
        let message: Value = serde_json::from_slice(line)
            .map_err(|error| format!("invalid Cargo JSON on line {}: {error}", index + 1))?;
        if finished {
            return Err("Cargo emitted JSON after build-finished".to_owned());
        }
        match message["reason"].as_str() {
            Some("build-finished") => {
                if message["success"].as_bool() != Some(true) {
                    return Err("Cargo did not report a successful build".to_owned());
                }
                finished = true;
            }
            Some("compiler-artifact") => {
                let target = &message["target"];
                if target["name"].as_str() != Some("semaprax-full")
                    || !target["kind"]
                        .as_array()
                        .is_some_and(|kinds| kinds.len() == 1 && kinds[0].as_str() == Some("bin"))
                    || message["profile"]["test"].as_bool() != Some(false)
                {
                    continue;
                }
                let Some(manifest) = message["manifest_path"].as_str().map(Path::new) else {
                    continue;
                };
                if !manifest.is_absolute()
                    || manifest.canonicalize().ok().as_ref() != Some(&expected_manifest)
                {
                    continue;
                }
                if executable.is_some() {
                    return Err("Cargo reported multiple full-toolchain executables".to_owned());
                }
                let binary = message["executable"]
                    .as_str()
                    .map(PathBuf::from)
                    .ok_or_else(|| "Cargo omitted the full-toolchain executable".to_owned())?;
                if !binary.is_absolute() || !binary.is_file() {
                    return Err(
                        "Cargo's full-toolchain executable is not an absolute file".to_owned()
                    );
                }
                // `fresh: true` is legitimate: Cargo has validated cached output.
                // Do not replace that decision with timestamps or force a rebuild.
                executable = Some(binary);
            }
            _ => {}
        }
    }
    if !finished {
        return Err("Cargo omitted build-finished".to_owned());
    }
    executable.ok_or_else(|| "Cargo did not report the full-toolchain executable".to_owned())
}
