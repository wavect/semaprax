//! Authored real publication/Node evidence; not executed during implementation.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{with_authenticated_project, ProjectNpmBuild, MAX_PROJECT_NPM_BUILD_BYTES};

#[path = "support/owned_npm_publication.rs"]
mod owned_npm_publication;
#[path = "support/flat_record_product.rs"]
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
        .map(|name| std::ffi::OsString::from(*name))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
}

fn publish(root: &Path, renamed: bool, retained: &mut Vec<(PathBuf, Vec<u8>)>) -> Vec<u8> {
    fs::create_dir(root).unwrap();
    let manifest = subject::write_project(root, renamed);
    for relative in [
        "semaprax.toml",
        "src/app.spx",
        "src/left.spx",
        "src/right.spx",
        "src/tests.spx",
    ] {
        let path = root.join(relative);
        retained.push((path.clone(), fs::read(path).unwrap()));
    }
    with_authenticated_project(&manifest, |snapshot| {
        let descriptor = snapshot.flat_owned_record_api_descriptor()?;
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        build.verify().unwrap();
        ProjectNpmBuild::inspect_envelope(build.envelope(), MAX_PROJECT_NPM_BUILD_BYTES).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
        assert_eq!(envelope["schema"], "semaprax.project-npm-build.v8");
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), ARTIFACTS.len());
        let output = root.join("package");
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
        Ok(descriptor.canonical_bytes())
    })
    .unwrap()
}

#[test]
fn published_colliding_record_packages_preserve_identity_values_and_recovery() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-v9-consumer-{}-{}-{}",
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
    let before = publish(&root.join("original"), false, &mut retained);
    let after = publish(&root.join("renamed"), true, &mut retained);
    assert_ne!(
        before, after,
        "presentation rename must alter its exact descriptor subject"
    );
    let mut before_facts: serde_json::Value = serde_json::from_slice(&before).unwrap();
    let mut after_facts: serde_json::Value = serde_json::from_slice(&after).unwrap();
    for key in [
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ] {
        assert!(before_facts[key].is_string());
        assert!(after_facts[key].is_string());
        assert_ne!(before_facts[key], after_facts[key]);
        before_facts[key] = serde_json::Value::Null;
        after_facts[key] = serde_json::Value::Null;
    }
    assert_eq!(
        before_facts["exports"][1]["result"]["record_source_name"],
        "Payload"
    );
    assert_eq!(
        after_facts["exports"][1]["result"]["record_source_name"],
        "RenamedPayload"
    );
    after_facts["exports"][1]["result"]["record_source_name"] =
        serde_json::Value::String("Payload".to_owned());
    assert_eq!(
        before_facts, after_facts,
        "no other descriptor fact may change during a display-only rename"
    );
    let probe = root.join("consumer.mjs");
    let bytes = include_bytes!("project_v9_recipe_consumer_v1/consumer.mjs").to_vec();
    fs::write(&probe, &bytes).unwrap();
    retained.push((probe, bytes));
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node is required for the published v9 recipe consumer gate");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"v9-recipe-consumer-ok\n");

    // Failures retain their fixtures. Successful cleanup preflights the whole
    // fixed tree before deleting any file; never recurse through foreign data.
    inventory(&root, &["original", "renamed", "consumer.mjs"]);
    for label in ["original", "renamed"] {
        let project = root.join(label);
        inventory(&project, &["src", "semaprax.toml", "package"]);
        inventory(
            &project.join("src"),
            &["app.spx", "left.spx", "right.spx", "tests.spx"],
        );
        inventory(&project.join("package"), &ARTIFACTS);
    }
    for (path, bytes) in &retained {
        plain(path, false);
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (path, _) in retained {
        fs::remove_file(path).unwrap();
    }
    for label in ["original", "renamed"] {
        let project = root.join(label);
        fs::remove_dir(project.join("src")).unwrap();
        fs::remove_dir(project.join("package")).unwrap();
        fs::remove_dir(project).unwrap();
    }
    fs::remove_dir(root).unwrap();
}
