//! Real compiled inactive-variant cleanup; no forged result carriers.
//! Fixtures are deliberately retained. The runner needs an external process
//! deadline/resource bound; Command::output is not an intrinsic output cap.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use semaprax::project::{
    with_authenticated_project, PublicApiSubject, MAX_PROJECT_NPM_BUILD_BYTES,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "project_owned_inactive_cleanup_v1/native.rs"]
mod native;
#[path = "support/owned_inactive_product.rs"]
mod subject;

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
    let manifest_path = subject::write_project(&root);
    let mut retained = ["semaprax.toml", "src/app.spx", "src/tests.spx"]
        .map(|name| {
            let path = root.join(name);
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .to_vec();
    let package = root.join("package");
    fs::create_dir(&package).unwrap();
    let provider = with_authenticated_project(&manifest_path, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let descriptor = snapshot.public_api_descriptor()?;
        let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
            revision.entry_program(),
            revision.manifest().web_exports(),
            PublicApiSubject {
                project_schema: revision.manifest().schema(),
                project_revision: revision.project_revision(),
                workspace_revision: revision.workspace_revision(),
                project_graph_digest: revision.semantic_graph_digest(),
            },
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])?;
        assert_eq!(provider.descriptor(), descriptor.canonical_bytes());
        assert_eq!(provider.descriptor_digest(), descriptor.digest());
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
        Ok(provider)
    })
    .unwrap();
    native::run(&root, provider.source());
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
