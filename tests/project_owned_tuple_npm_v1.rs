//! Authored npm counterpart to the provisioned native mixed-borrow consumer.
//! Real engine-entry observations are not allocator or native-context evidence.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use semaprax::project::{with_authenticated_project, ProjectNpmBuild, MAX_PROJECT_NPM_BUILD_BYTES};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "support/owned_npm_publication.rs"]
mod owned_npm_publication;
#[path = "support/owned_tuple_product.rs"]
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

fn publish(root: &Path, flat: bool, retained: &mut Vec<(PathBuf, Vec<u8>)>) {
    fs::create_dir(root).unwrap();
    let manifest = subject::write_project(root, flat);
    for relative in ["semaprax.toml", "src/app.spx", "src/tests.spx"] {
        let path = root.join(relative);
        retained.push((path.clone(), fs::read(path).unwrap()));
    }
    with_authenticated_project(&manifest, |snapshot| {
        let (descriptor, digest) = if flat {
            let descriptor = snapshot.flat_owned_record_api_descriptor()?;
            (descriptor.canonical_bytes(), descriptor.digest())
        } else {
            let descriptor = snapshot.public_api_descriptor()?;
            (descriptor.canonical_bytes(), descriptor.digest())
        };
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        build.verify().unwrap();
        ProjectNpmBuild::inspect_envelope(build.envelope(), MAX_PROJECT_NPM_BUILD_BYTES).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
        assert_eq!(
            envelope["schema"],
            if flat {
                "semaprax.project-npm-build.v8"
            } else {
                "semaprax.project-npm-build.v7"
            }
        );
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), ARTIFACTS.len());
        let output = root.join("package");
        owned_npm_publication::publish(snapshot, &manifest, &output, !flat)?;
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
            descriptor
        );
        assert_eq!(metadata["descriptor_digest"], digest);
        Ok(())
    })
    .unwrap();
}

#[test]
fn published_v8_v9_mixed_borrow_tuples_match_native_consumer_boundaries() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-tuple-npm-{}-{}-{}",
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
    publish(&root.join("v8"), false, &mut retained);
    publish(&root.join("v9"), true, &mut retained);
    let probe = root.join("consumer.mjs");
    let bytes = include_bytes!("project_owned_tuple_npm_v1/consumer.mjs").to_vec();
    fs::write(&probe, &bytes).unwrap();
    retained.push((probe, bytes));
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node is required for the published mixed-borrow tuple gate");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"project-owned-tuple-npm-ok\n");

    // Inspect the entire fixed tree and its bytes before removing anything.
    // Failed assertions retain evidence rather than following foreign paths.
    inventory(&root, &["v8", "v9", "consumer.mjs"]);
    for label in ["v8", "v9"] {
        let project = root.join(label);
        inventory(&project, &["src", "semaprax.toml", "package"]);
        inventory(&project.join("src"), &["app.spx", "tests.spx"]);
        inventory(&project.join("package"), &ARTIFACTS);
    }
    for (path, bytes) in &retained {
        plain(path, false);
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (path, _) in retained {
        fs::remove_file(path).unwrap();
    }
    for label in ["v8", "v9"] {
        let project = root.join(label);
        fs::remove_dir(project.join("src")).unwrap();
        fs::remove_dir(project.join("package")).unwrap();
        fs::remove_dir(project).unwrap();
    }
    fs::remove_dir(root).unwrap();
}
