use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax_doctor_release::{
    create_release, key_information, verify_outputs, ReleaseInputs, CAPSULE_FILE, MANIFEST_FILE,
    MANIFEST_SIGNATURE_FILE,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-doctor-release-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self, name: &str, bytes: &[u8], executable: bool) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(
                &path,
                fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
            )
            .unwrap();
        }
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
    bytes[72..80].copy_from_slice(&0_u64.to_le_bytes());
    bytes[96..104].copy_from_slice(&120_u64.to_le_bytes());
    bytes
}

fn inputs(fixture: &Fixture, seed: u8) -> ReleaseInputs {
    let encoded = format!(
        "{}\n",
        std::iter::repeat_n(format!("{seed:02x}"), 32).collect::<String>()
    );
    let key = fixture.file("key", encoded.as_bytes(), false);
    ReleaseInputs {
        request: fixture.file("request", b"request", false),
        bundle: fixture.file("bundle", b"bundle", false),
        launcher: fixture.file("launcher", &elf(), true),
        worker: fixture.file("worker", &elf(), true),
        collector: fixture.file("collector", &elf(), true),
        provisioner: fixture.file("provisioner", &elf(), true),
        selector: "release-linux-v1".into(),
        architecture: 1,
        target: 3,
        release_version: "0.2.0".into(),
        release_commit: "0123456789abcdef0123456789abcdef01234567".into(),
        target_triple: "x86_64-unknown-linux-musl".into(),
        signing_key: key,
        output_directory: fixture.0.clone(),
    }
}

#[test]
fn deterministic_release_is_closed_and_no_clobber() {
    let first = Fixture::new();
    let first_inputs = inputs(&first, 17);
    create_release(&first_inputs).unwrap();
    let capsule = fs::read(first.0.join(CAPSULE_FILE)).unwrap();
    let manifest = fs::read(first.0.join(MANIFEST_FILE)).unwrap();
    let signature = fs::read(first.0.join(MANIFEST_SIGNATURE_FILE)).unwrap();
    assert!(create_release(&first_inputs).is_err());

    let second = Fixture::new();
    create_release(&inputs(&second, 17)).unwrap();
    assert_eq!(fs::read(second.0.join(CAPSULE_FILE)).unwrap(), capsule);
    assert_eq!(fs::read(second.0.join(MANIFEST_FILE)).unwrap(), manifest);
    assert_eq!(
        fs::read(second.0.join(MANIFEST_SIGNATURE_FILE)).unwrap(),
        signature
    );
    let text = String::from_utf8(manifest).unwrap();
    assert!(text.starts_with("{\"schema\":\"semaprax.doctor-release-manifest.v1\""));
    assert!(text.ends_with("}\n"));
    assert!(!text.contains("signing_key"));
}

#[test]
fn every_signed_byte_and_wrong_key_reject() {
    let fixture = Fixture::new();
    let values = inputs(&fixture, 31);
    let info = key_information(&values.signing_key).unwrap();
    let marker = "\"public_key_hex\":\"";
    let start = info.find(marker).unwrap() + marker.len();
    let public = &info[start..start + 64];
    create_release(&values).unwrap();
    let capsule = fs::read(fixture.0.join(CAPSULE_FILE)).unwrap();
    let manifest = fs::read(fixture.0.join(MANIFEST_FILE)).unwrap();
    let signature = fs::read(fixture.0.join(MANIFEST_SIGNATURE_FILE)).unwrap();
    verify_outputs(&capsule, &manifest, &signature, public).unwrap();
    for index in 0..capsule.len() {
        let mut changed = capsule.clone();
        changed[index] ^= 1;
        assert!(verify_outputs(&changed, &manifest, &signature, public).is_err());
    }
    for index in 0..manifest.len() {
        let mut changed = manifest.clone();
        changed[index] ^= 1;
        assert!(verify_outputs(&capsule, &changed, &signature, public).is_err());
    }
    for index in 0..signature.len() {
        let mut changed = signature.clone();
        changed[index] ^= 1;
        assert!(verify_outputs(&capsule, &manifest, &changed, public).is_err());
    }
    let wrong_fixture = Fixture::new();
    let wrong_values = inputs(&wrong_fixture, 37);
    let wrong = key_information(&wrong_values.signing_key).unwrap();
    let wrong_start = wrong.find(marker).unwrap() + marker.len();
    assert!(verify_outputs(
        &capsule,
        &manifest,
        &signature,
        &wrong[wrong_start..wrong_start + 64]
    )
    .is_err());
}

#[test]
fn key_info_contains_only_public_material_and_unsafe_inputs_reject() {
    let fixture = Fixture::new();
    let mut values = inputs(&fixture, 23);
    let info = key_information(&values.signing_key).unwrap();
    assert!(info.contains("public_key_hex"));
    assert!(info.contains("public_key_fingerprint"));
    assert!(!info.contains("signing_key"));
    values.selector = "Not-Canonical".into();
    assert!(create_release(&values).is_err());

    let malformed = fixture.file("malformed-key", b"11\n", false);
    assert!(key_information(&malformed).is_err());
    assert!(verify_outputs(b"x", b"x", &[0; 64], &"00".repeat(32)).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&values.signing_key, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(key_information(&values.signing_key).is_err());
    }
}
