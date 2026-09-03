//! Public-library versus provisioned full-host Windows publication evidence.
//! The existing Node consumers still read real published packages on every OS.
//! Building the full CLI uses the existing Windows SEMAPRAX_LINKER/VCTOOLS
//! fixture provisioning; npm publication itself launches no compiler or tool.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::ProjectSnapshot;

// Only `tests/project.rs` includes this file, and it declares both of these
// support modules at its root. Loading them again here would compile each a
// second time into the same crate, which `clippy::duplicate_mod` rejects.
// `full_toolchain` reaches `native_rust_cargo` through `super::`, which now
// resolves at the crate root, so only this one import is needed here.
#[cfg(windows)]
use crate::full_toolchain;

#[cfg(windows)]
const UNAVAILABLE: &str = "Project v8-v11 npm publication requires semaprax-full with safe handle-relative Windows authority";

pub fn publish(
    snapshot: &mut ProjectSnapshot,
    manifest: &Path,
    output: &Path,
    check_aliases: bool,
) -> Result<(), Vec<Diagnostic>> {
    #[cfg(not(windows))]
    {
        let _ = (manifest, check_aliases);
        snapshot.build_npm(output)
    }
    #[cfg(windows)]
    {
        absent(output);
        let parent = output.parent().unwrap();
        let before = names(parent);
        let errors = snapshot.build_npm(output).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-W120");
        assert_eq!(errors[0].message, UNAVAILABLE);
        absent(output);
        assert_eq!(names(parent), before);
        // Missing parents make accidental CLI preparation observable. All
        // aliases must reach the availability error before creating anything.
        let missing = parent.join("standalone-unavailable-parent");
        absent(&missing);
        for target in [Some("npm"), Some("web"), Some("wasm"), None] {
            let result = command(
                Path::new(env!("CARGO_BIN_EXE_semaprax")),
                manifest,
                target,
                &missing.join("package"),
            );
            assert!(
                !result.status.success(),
                "standalone CLI unexpectedly published {target:?}"
            );
            assert!(result.stdout.is_empty());
            let stderr = String::from_utf8(result.stderr).unwrap();
            assert!(stderr.contains("SPX-W120"), "{stderr}");
            assert!(stderr.contains(UNAVAILABLE), "{stderr}");
            absent(&missing);
            assert_eq!(names(parent), before);
        }
        successful_command(manifest, Some("npm"), output);
        if check_aliases {
            assert_full_aliases(manifest, output);
        }
        Ok(())
    }
}

#[cfg(windows)]
fn absent(path: &Path) {
    assert_eq!(
        std::fs::symlink_metadata(path).unwrap_err().kind(),
        std::io::ErrorKind::NotFound,
        "unexpected object at {}",
        path.display()
    );
}

#[cfg(windows)]
fn names(path: &Path) -> std::collections::BTreeSet<std::ffi::OsString> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}

#[cfg(windows)]
fn command(
    binary: &Path,
    manifest: &Path,
    target: Option<&str>,
    output: &Path,
) -> std::process::Output {
    let mut command = std::process::Command::new(binary);
    command.arg("build").arg("--manifest-path").arg(manifest);
    if let Some(target) = target {
        command.args(["--target", target]);
    }
    command
        .arg("-o")
        .arg(output)
        .output()
        .expect("run actual CLI publication route")
}

#[cfg(windows)]
fn successful_command(manifest: &Path, target: Option<&str>, output: &Path) {
    let result = command(full_toolchain::binary(), manifest, target, output);
    assert!(
        result.status.success(),
        "full-host publication {target:?} failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// Called once by the v8 tuple fixture. Every profile already uses the real
/// canonical npm route above; this additionally checks the shared CLI aliases.
#[cfg(windows)]
fn assert_full_aliases(manifest: &Path, canonical: &Path) {
    use std::os::windows::fs::MetadataExt as _;

    const ARTIFACTS: [&str; 6] = [
        "app.wasm",
        "semaprax.js",
        "semaprax.bindings.js",
        "semaprax.bindings.d.ts",
        "semaprax.api.json",
        "package.json",
    ];
    let parent = canonical.parent().unwrap();
    let before = names(parent);
    let missing = parent.join("full-host-missing-parent");
    absent(&missing);
    let rejected = command(
        full_toolchain::binary(),
        manifest,
        Some("npm"),
        &missing.join("package"),
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let stderr = String::from_utf8(rejected.stderr).unwrap();
    assert!(stderr.contains("SPX-W120"), "{stderr}");
    assert!(
        stderr.contains("npm package requires an existing parent"),
        "{stderr}"
    );
    absent(&missing);
    assert_eq!(names(parent), before);
    let expected = ARTIFACTS.map(|name| (name, std::fs::read(canonical.join(name)).unwrap()));
    for (label, target) in [
        ("web", Some("web")),
        ("wasm", Some("wasm")),
        ("default", None),
    ] {
        let output = parent.join(format!("full-host-alias-{label}"));
        absent(&output);
        successful_command(manifest, target, &output);
        let directory = std::fs::symlink_metadata(&output).unwrap();
        assert!(directory.is_dir());
        assert_eq!(directory.file_attributes() & 0x400, 0, "reparse directory");
        assert_eq!(
            names(&output),
            ARTIFACTS.map(std::ffi::OsString::from).into()
        );
        // Authenticate the complete fixed inventory before deleting any file.
        for (name, bytes) in &expected {
            let path = output.join(name);
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file());
            assert_eq!(metadata.file_attributes() & 0x400, 0, "reparse artifact");
            assert_eq!(std::fs::read(path).unwrap(), *bytes);
        }
        for (name, _) in &expected {
            std::fs::remove_file(output.join(name)).unwrap();
        }
        std::fs::remove_dir(output).unwrap();
        assert_eq!(names(parent), before);
    }
}
