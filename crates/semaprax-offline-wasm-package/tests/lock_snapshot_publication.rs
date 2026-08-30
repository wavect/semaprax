#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolution_snapshot;
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax_offline_wasm_package::{
    publish_lock_snapshot, INPUT_FILE, LOCK_FILE, RESOLUTION_FILE,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn snapshot() -> package_resolution_snapshot::ResolutionSnapshot {
    let root = std::env::temp_dir().join(format!(
        "semaprax-lock-snapshot-source-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(
        &root,
        "module app.main;\n@id(\"app.main.main\")\nfn main() -> i64 { 41 }\n",
    )
    .unwrap();
    let report = package_report_v2::generate(&root, &PackageReportV2Options::default()).unwrap();
    fs::remove_file(root).unwrap();
    let subject = package_lock_v2::create_subject(
        &Coordinate {
            package: "app.main".to_owned(),
            version: "1.0.0".to_owned(),
        },
        &report,
        &[],
        &[],
    )
    .unwrap();
    let input = ResolutionInput {
        requirements: vec![Requirement {
            package: "app.main".to_owned(),
            range: "=1.0.0".to_owned(),
        }],
        subjects: vec![subject],
        target: "wasm32".to_owned(),
        allowed_capabilities: vec![],
    };
    let options = ResolutionOptions::default();
    let evidence = package_resolver::generate(&input, &options).unwrap();
    package_resolution_snapshot::generate(&input, &options, &evidence).unwrap()
}

fn private_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-lock-snapshot-publish-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    }
    root
}

#[test]
fn public_facade_publishes_exact_snapshot_inventory_without_extras() {
    let snapshot = snapshot();
    let root = private_root();
    let output = root.join("lock");
    let published = publish_lock_snapshot(&output, snapshot.clone()).unwrap();
    assert_eq!(published.output, output);
    assert_eq!(
        fs::read_to_string(output.join(INPUT_FILE)).unwrap(),
        snapshot.input_json
    );
    assert_eq!(
        fs::read_to_string(output.join(RESOLUTION_FILE)).unwrap(),
        snapshot.resolution_evidence_json
    );
    assert_eq!(
        fs::read_to_string(output.join(LOCK_FILE)).unwrap(),
        snapshot.lock_json
    );
    let mut names = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    let mut expected = [INPUT_FILE, RESOLUTION_FILE, LOCK_FILE]
        .map(std::ffi::OsString::from)
        .to_vec();
    expected.sort();
    assert_eq!(names, expected);
}

#[test]
fn existing_destination_is_never_replaced() {
    let root = private_root();
    let output = root.join("lock");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("sentinel"), b"foreign").unwrap();
    let error = publish_lock_snapshot(&output, snapshot()).unwrap_err();
    assert_eq!(error.code, "SPX-PP503");
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign");
}

#[test]
fn invalid_snapshot_is_rejected_before_filesystem_authority() {
    let root = private_root();
    let output = root.join("absent-parent").join("lock");
    let mut candidate = snapshot();
    candidate.lock_json.push(' ');
    let error = publish_lock_snapshot(&output, candidate).unwrap_err();
    assert_eq!(error.code, "SPX-PP502");
    assert_eq!(error.compiler_code, Some("SPX-PK505"));
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn unix_snapshot_publication_requires_exact_private_parent_mode() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = private_root();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    let error = publish_lock_snapshot(&root.join("lock"), snapshot()).unwrap_err();
    assert_eq!(error.code, "SPX-PP501");
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}
