//! Real Project admission/publication and Node String-output boundary evidence.
//! Authored but unrun; oversized source is rejected, not executed by a runtime.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use semaprax::project::{with_authenticated_project, ProjectNpmBuild, MAX_PROJECT_NPM_BUILD_BYTES};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::owned_npm_publication;
#[path = "../support/owned_utf8_capacity.rs"]
mod subject;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const ARTIFACTS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];

fn plain(path: &Path, directory: bool) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.is_dir(), directory);
    if !directory {
        assert!(metadata.is_file());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
}

fn inventory(path: &Path, expected: &[&str]) {
    plain(path, true);
    let mut actual = fs::read_dir(path)
        .unwrap()
        .map(|row| row.unwrap().file_name())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = expected
        .iter()
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
}

fn prepare(root: &Path, byte_len: usize, retained: &mut Vec<(PathBuf, Vec<u8>)>) {
    fs::create_dir(root).unwrap();
    let manifest = subject::write_project(root, byte_len);
    for name in ["semaprax.toml", "src/app.spx", "src/tests.spx"] {
        let path = root.join(name);
        retained.push((path.clone(), fs::read(path).unwrap()));
    }
    let output = root.join("package");
    let mut entered = false;
    let result = with_authenticated_project(&manifest, |snapshot| {
        entered = true;
        assert!(
            byte_len <= 65_536,
            "oversized literal reached live Project authority"
        );
        let descriptor = snapshot.owned_utf8_api_descriptor()?;
        let facts: serde_json::Value =
            serde_json::from_slice(&descriptor.canonical_bytes()).unwrap();
        assert_eq!(facts["schema"], "semaprax.public-owned-utf8-api.v1");
        let exports = facts["exports"].as_array().unwrap();
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0]["stable_id"], "utf8.maximum");
        assert_eq!(exports[0]["result"], "owned-utf8");
        assert!(exports[0]["parameters"].as_array().unwrap().is_empty());
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        build.verify().unwrap();
        ProjectNpmBuild::inspect_envelope(build.envelope(), MAX_PROJECT_NPM_BUILD_BYTES).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
        assert_eq!(envelope["schema"], "semaprax.project-npm-build.v9");
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), ARTIFACTS.len());
        owned_npm_publication::publish(snapshot, &manifest, &output, false)?;
        inventory(&output, &ARTIFACTS);
        for (row, name) in rows.iter().zip(ARTIFACTS) {
            assert_eq!(row["path"], name);
            let hex = row["hex"].as_str().unwrap();
            assert_eq!(hex.len() % 2, 0);
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let path = output.join(name);
            plain(&path, false);
            assert_eq!(fs::read(&path).unwrap(), bytes);
            retained.push((path, bytes));
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("semaprax.api.json")).unwrap()).unwrap();
        assert_eq!(
            metadata["descriptor"].as_str().unwrap().as_bytes(),
            descriptor.canonical_bytes()
        );
        assert_eq!(metadata["descriptor_digest"], descriptor.digest());
        Ok(())
    });
    if byte_len == 65_537 {
        let errors = result.unwrap_err();
        assert!(!entered, "plus-one must fail at Project admission");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-W110");
        assert_eq!(
            errors[0].message,
            "owned UTF-8 literal table exceeds 65536 bytes"
        );
        assert!(!output.exists());
        inventory(root, &["semaprax.toml", "src"]);
    } else {
        result.unwrap();
        assert!(entered);
    }
}

#[test]
fn actual_owned_string_minus_one_exact_and_plus_one_boundaries() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-utf8-capacity-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let mut retained = Vec::new();
    for (label, length) in [
        ("minus-one", 65_535),
        ("exact", 65_536),
        ("plus-one", 65_537),
    ] {
        prepare(&root.join(label), length, &mut retained);
    }
    let script = include_bytes!("../project_owned_utf8_capacity_v1/consumer.mjs").to_vec();
    let probe = root.join("consumer.mjs");
    fs::write(&probe, &script).unwrap();
    retained.push((probe, script));
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node is required for the real Project UTF-8 output-capacity gate");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"project-owned-utf8-capacity-ok\n");
    // Validate the entire fixed tree before bounded nonrecursive cleanup.
    // Any earlier assertion failure retains its fixture for diagnosis.
    inventory(&root, &["minus-one", "exact", "plus-one", "consumer.mjs"]);
    for label in ["minus-one", "exact", "plus-one"] {
        let project = root.join(label);
        inventory(
            &project,
            if label == "plus-one" {
                &["semaprax.toml", "src"]
            } else {
                &["semaprax.toml", "src", "package"]
            },
        );
        inventory(&project.join("src"), &["app.spx", "tests.spx"]);
        if label != "plus-one" {
            inventory(&project.join("package"), &ARTIFACTS);
        }
    }
    for (path, bytes) in &retained {
        plain(path, false);
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (path, _) in retained {
        fs::remove_file(path).unwrap();
    }
    for label in ["minus-one", "exact", "plus-one"] {
        let project = root.join(label);
        fs::remove_dir(project.join("src")).unwrap();
        if label != "plus-one" {
            fs::remove_dir(project.join("package")).unwrap();
        }
        fs::remove_dir(project).unwrap();
    }
    fs::remove_dir(root).unwrap();
}
