#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt as _};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax_doctor_release::{
    activate, create_release, inspect_active, install_from_verified_directory, key_information,
    open_store, recover, rollback, ReleaseExpectation, ReleaseInputs, StoreExpectation,
    BUNDLE_FILE, COLLECTOR_FILE, LAUNCHER_FILE, PROVISIONER_FILE, REQUEST_FILE, WORKER_FILE,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-doctor-install-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(fs::canonicalize(root).unwrap())
    }

    fn dir(&self, name: &str, mode: u32) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    fn file(&self, name: &str, bytes: &[u8], executable: bool) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
        )
        .unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn elf() -> Vec<u8> {
    let mut bytes = vec![0_u8; 120];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
    bytes[64..68].copy_from_slice(&1_u32.to_le_bytes());
    bytes[96..104].copy_from_slice(&120_u64.to_le_bytes());
    bytes
}

fn release(fixture: &Fixture, label: &str, seed: u8) -> (PathBuf, ReleaseExpectation) {
    let key_bytes = format!("{}\n", format!("{seed:02x}").repeat(32));
    let key = fixture.file(&format!("key-{label}"), key_bytes.as_bytes(), false);
    let output = fixture.dir(&format!("release-{label}"), 0o700);
    let inputs = ReleaseInputs {
        request: fixture.file(&format!("request-{label}"), label.as_bytes(), false),
        bundle: fixture.file(&format!("bundle-{label}"), b"bundle", false),
        launcher: fixture.file(&format!("launcher-{label}"), &elf(), true),
        worker: fixture.file(&format!("worker-{label}"), &elf(), true),
        collector: fixture.file(&format!("collector-{label}"), &elf(), true),
        provisioner: fixture.file(&format!("provisioner-{label}"), &elf(), true),
        selector: "release-linux-v1".into(),
        architecture: 1,
        target: 3,
        release_version: "0.2.0".into(),
        release_commit: format!("{seed:040x}"),
        target_triple: "x86_64-unknown-linux-musl".into(),
        signing_key: key,
        output_directory: output.clone(),
    };
    create_release(&inputs).unwrap();
    for (source, name) in [
        (&inputs.request, REQUEST_FILE),
        (&inputs.bundle, BUNDLE_FILE),
        (&inputs.launcher, LAUNCHER_FILE),
        (&inputs.worker, WORKER_FILE),
        (&inputs.collector, COLLECTOR_FILE),
        (&inputs.provisioner, PROVISIONER_FILE),
    ] {
        fs::copy(source, output.join(name)).unwrap();
    }
    let info = key_information(&inputs.signing_key).unwrap();
    let marker = "\"public_key_hex\":\"";
    let start = info.find(marker).unwrap() + marker.len();
    let expected = ReleaseExpectation {
        release_version: inputs.release_version,
        release_commit: inputs.release_commit,
        target_triple: inputs.target_triple,
        architecture: inputs.architecture,
        target: inputs.target,
        selector: inputs.selector,
        public_key_hex: info[start..start + 64].to_owned(),
    };
    (output, expected)
}

#[test]
fn install_activate_rollback_and_recovery_replay_every_generation() {
    let fixture = Fixture::new();
    let store_path = fixture.dir("store", 0o700);
    let store = open_store(&store_path, StoreExpectation::default()).unwrap();
    let (first_source, first_expected) = release(&fixture, "first", 17);
    let (second_source, second_expected) = release(&fixture, "second", 19);

    let first = install_from_verified_directory(&store, &first_source, &first_expected).unwrap();
    assert!(first.installed_new);
    assert!(install_from_verified_directory(&store, &first_source, &first_expected).is_err());
    activate(&store, &first.generation, None, &first_expected).unwrap();
    assert_eq!(
        inspect_active(&store).unwrap().as_ref(),
        Some(&first.generation)
    );

    let second = install_from_verified_directory(&store, &second_source, &second_expected).unwrap();
    assert!(activate(&store, &second.generation, None, &second_expected).is_err());
    activate(
        &store,
        &second.generation,
        Some(&first.generation),
        &second_expected,
    )
    .unwrap();
    rollback(
        &store,
        &first.generation,
        &second.generation,
        &first_expected,
    )
    .unwrap();
    assert_eq!(
        inspect_active(&store).unwrap().as_ref(),
        Some(&first.generation)
    );

    let generation = format!("generation-{}", second.generation.as_str());
    let stage = format!(".stage-{}", second.generation.as_str());
    fs::rename(store_path.join(generation), store_path.join(stage)).unwrap();
    let receipt = recover(&store, &second_expected).unwrap();
    assert_eq!(
        receipt.removed_generation.as_ref(),
        Some(&second.generation)
    );
}

#[test]
fn substitutions_foreign_bytes_and_cross_identity_fail_closed() {
    let fixture = Fixture::new();
    let store_path = fixture.dir("store", 0o700);
    let store = open_store(&store_path, StoreExpectation::default()).unwrap();
    let (source, expected) = release(&fixture, "source", 23);

    let original = source.join(REQUEST_FILE);
    let external = fixture.file("external", b"source", false);
    fs::remove_file(&original).unwrap();
    fs::hard_link(&external, &original).unwrap();
    assert!(install_from_verified_directory(&store, &source, &expected).is_err());
    assert!(fs::read_dir(&store_path).unwrap().next().is_none());

    fs::remove_file(&original).unwrap();
    fs::write(&original, b"source").unwrap();
    fs::set_permissions(&original, fs::Permissions::from_mode(0o600)).unwrap();
    let installed = install_from_verified_directory(&store, &source, &expected).unwrap();
    let generation = store_path.join(format!("generation-{}", installed.generation.as_str()));
    fs::remove_dir_all(&generation).unwrap();
    symlink(&source, &generation).unwrap();
    assert!(activate(&store, &installed.generation, None, &expected).is_err());
    assert!(inspect_active(&store).is_err());
    assert!(!store_path.join("ACTIVE").exists());
}

#[test]
fn unsafe_source_modes_reject_before_store_effects() {
    let fixture = Fixture::new();
    let store_path = fixture.dir("store", 0o700);
    let store = open_store(&store_path, StoreExpectation::default()).unwrap();
    let (source, expected) = release(&fixture, "modes", 31);

    fs::set_permissions(source.join(REQUEST_FILE), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(install_from_verified_directory(&store, &source, &expected).is_err());
    assert!(fs::read_dir(&store_path).unwrap().next().is_none());

    fs::set_permissions(source.join(REQUEST_FILE), fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(install_from_verified_directory(&store, &source, &expected).is_err());
    assert!(fs::read_dir(&store_path).unwrap().next().is_none());
}

#[test]
fn store_authority_rebinding_and_unauthenticated_stages_reject() {
    let fixture = Fixture::new();
    let store_path = fixture.dir("store", 0o700);
    let store = open_store(&store_path, StoreExpectation::default()).unwrap();
    let replacement = fixture.dir("replacement", 0o700);
    let moved = fixture.0.join("moved-store");
    fs::rename(&store_path, &moved).unwrap();
    fs::rename(&replacement, &store_path).unwrap();
    assert!(inspect_active(&store).is_err());

    fs::rename(&store_path, &replacement).unwrap();
    fs::rename(&moved, &store_path).unwrap();
    let (source, expected) = release(&fixture, "recover", 29);
    let installed = install_from_verified_directory(&store, &source, &expected).unwrap();
    let generation = store_path.join(format!("generation-{}", installed.generation.as_str()));
    let stage = store_path.join(format!(".stage-{}", installed.generation.as_str()));
    fs::rename(&generation, &stage).unwrap();
    fs::write(stage.join("foreign"), b"foreign").unwrap();
    assert!(recover(&store, &expected).is_err());
    assert_eq!(fs::read(stage.join("foreign")).unwrap(), b"foreign");
}

#[test]
fn concurrent_same_generation_install_has_one_winner_and_no_partial_adoption() {
    let fixture = Fixture::new();
    let store_path = fixture.dir("store", 0o700);
    let store = Arc::new(open_store(&store_path, StoreExpectation::default()).unwrap());
    let (source, expected) = release(&fixture, "race", 37);
    let mut workers = Vec::new();
    for _ in 0..2 {
        let store = Arc::clone(&store);
        let source = source.clone();
        let expected = expected.clone();
        workers.push(std::thread::spawn(move || {
            install_from_verified_directory(&store, &source, &expected)
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    let entries: Vec<_> = fs::read_dir(&store_path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].to_string_lossy().starts_with("generation-"));
}

#[test]
fn recovery_preserves_partial_and_active_stages() {
    let fixture = Fixture::new();
    let store_path = fixture.dir("store", 0o700);
    let store = open_store(&store_path, StoreExpectation::default()).unwrap();
    let (_, expected) = release(&fixture, "recovery", 41);
    let stage = store_path.join(format!(".stage-{}", "0".repeat(64)));
    fs::create_dir(&stage).unwrap();
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(stage.join("partial"), b"foreign-or-incomplete").unwrap();
    assert!(recover(&store, &expected).is_err());
    assert_eq!(
        fs::read(stage.join("partial")).unwrap(),
        b"foreign-or-incomplete"
    );

    fs::remove_dir_all(&stage).unwrap();
    let active_stage = store_path.join(".ACTIVE.stage");
    fs::write(&active_stage, format!("{}\n", "0".repeat(64))).unwrap();
    fs::set_permissions(&active_stage, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(recover(&store, &expected).is_err());
    assert!(active_stage.exists());
}
