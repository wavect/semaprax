//! Real compiled inactive-variant cleanup; no forged result carriers.
//! Fixtures are deliberately retained. The runner needs an external process
//! deadline/resource bound; Command::output is not an intrinsic output cap.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use semaprax::project::{with_authenticated_project, MAX_PROJECT_NPM_BUILD_BYTES};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const ARTIFACTS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn write_new(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

#[test]
fn initialized_bytes_settle_before_successful_none_and_err_publication() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-inactive-cleanup-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained inactive-cleanup fixture: {}", root.display());
    fs::create_dir(root.join("src")).unwrap();
    let mut retained = Vec::new();
    for (name, source) in [
        (
            "app.spx",
            include_str!("project_owned_inactive_cleanup_v1/source.spx"),
        ),
        (
            "tests.spx",
            "module inactive.tests; @id(\"inactive.tests.main\") fn main() -> i64 { 0 }",
        ),
    ] {
        let path = root.join("src").join(name);
        let ast = semaprax::check(source, &path).unwrap();
        let canonical = semaprax::format::canonical(&ast);
        let reparsed = semaprax::check(&canonical, &path).unwrap();
        assert_eq!(semaprax::format::canonical(&reparsed), canonical);
        assert_eq!(
            semaprax::graph::to_json(&ast).unwrap(),
            semaprax::graph::to_json(&reparsed).unwrap()
        );
        write_new(&path, canonical.as_bytes());
        retained.push((path, canonical.into_bytes()));
    }
    let manifest = b"schema = \"semaprax.project.v8\"\nname = \"inactive-cleanup\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"inactive.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"inactive.maybe\", \"inactive.result\"]\ntests = [\"inactive.tests\"]\n";
    let manifest_path = root.join("semaprax.toml");
    write_new(&manifest_path, manifest);
    retained.push((manifest_path.clone(), manifest.to_vec()));
    let package = root.join("package");
    fs::create_dir(&package).unwrap();
    with_authenticated_project(&manifest_path, |snapshot| {
        let descriptor = snapshot.public_api_descriptor()?;
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        build.verify().unwrap();
        let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
        assert_eq!(envelope["schema"], "semaprax.project-npm-build.v7");
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), ARTIFACTS.len());
        for (row, name) in rows.iter().zip(ARTIFACTS) {
            assert_eq!(row["path"], name);
            let hex = row["hex"].as_str().unwrap();
            assert_eq!(hex.len() % 2, 0);
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let path = package.join(name);
            write_new(&path, &bytes);
            retained.push((path, bytes));
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(package.join("semaprax.api.json")).unwrap()).unwrap();
        assert_eq!(
            metadata["descriptor"].as_str().unwrap().as_bytes(),
            descriptor.canonical_bytes()
        );
        assert_eq!(metadata["descriptor_digest"], descriptor.digest());
        Ok(())
    })
    .unwrap();
    let script = include_bytes!("project_owned_inactive_cleanup_v1/consumer.mjs");
    write_new(&root.join("consumer.mjs"), script);
    retained.push((root.join("consumer.mjs"), script.to_vec()));
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node is required for the inactive owned-result cleanup gate");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"project-owned-inactive-cleanup-ok\n");
    for (path, bytes) in retained {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}
