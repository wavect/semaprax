//! Framing precedes v9 digest work; provider authentication still precedes both.
use super::*;

const CANONICAL: &[u8] =
    include_bytes!("../../../../tests/fixtures/flat_descriptor_retained_names.json");

fn observed<T>(action: impl FnOnce() -> T) -> (T, (usize, usize)) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FLAT_DESCRIPTOR_HASH_WORK.with(|work| work.set(None));
        }
    }
    FLAT_DESCRIPTOR_HASH_WORK.with(|work| {
        assert!(work.get().is_none());
        work.set(Some((0, 0)));
    });
    let reset = Reset;
    let result = action();
    let work = FLAT_DESCRIPTOR_HASH_WORK.with(|work| work.get().unwrap());
    drop(reset);
    (result, work)
}

fn exact_canonical() -> Vec<u8> {
    // Grow only the presentation name in an existing independently authored
    // canonical oracle; its retained identities/control escaping stay exact.
    let value: Value = serde_json::from_slice(CANONICAL).unwrap();
    let name = value["exports"][0]["result"]["record_source_name"]
        .as_str()
        .unwrap();
    assert!(name.is_ascii());
    let marker = format!("\"record_source_name\":\"{name}\"");
    let text = std::str::from_utf8(CANONICAL).unwrap();
    assert_eq!(text.matches(&marker).count(), 1);
    let replacement = format!(
        "\"record_source_name\":\"{name}{}\"",
        "x".repeat(MAX_DESCRIPTOR_BYTES - CANONICAL.len())
    );
    let bytes = text.replacen(&marker, &replacement, 1).into_bytes();
    assert_eq!(bytes.len(), MAX_DESCRIPTOR_BYTES);
    bytes
}

#[test]
fn canonical_exact_limit_hashes_but_invalid_framing_does_not() {
    let exact = exact_canonical();
    for bytes in [CANONICAL, exact.as_slice()] {
        let digest = flat_descriptor_digest(bytes);
        let (result, work) =
            observed(|| flat_descriptor::replay(bytes, &digest, &["api.value".to_owned()]));
        assert!(result.is_ok());
        assert_eq!(work, (1, bytes.len()));
    }
    let mut oversized = exact;
    oversized.insert(1, b' ');
    assert_eq!(oversized.len(), MAX_DESCRIPTOR_BYTES + 1);
    for bytes in [Vec::new(), b"{}".to_vec(), b"{\0}\n".to_vec(), oversized] {
        let digest = flat_descriptor_digest(&bytes); // Remint outside observation.
        for digest in [digest.as_str(), "wrong"] {
            let (result, work) = observed(|| flat_descriptor::replay(&bytes, digest, &[]));
            assert_eq!(result.unwrap_err().kind(), PackageErrorKind::Descriptor);
            assert_eq!(work, (0, 0));
        }
    }
}

#[test]
fn public_flat_builder_bounds_digest_work_and_preserves_provider_precedence() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "semaprax-flat-input-bounds-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let output = root.join("package");
    let mut oversized = exact_canonical();
    oversized.insert(1, b' ');
    let malformed = [Vec::new(), b"{}".to_vec(), b"{\0}\n".to_vec(), oversized];
    for bytes in malformed {
        let real_digest = flat_descriptor_digest(&bytes);
        for supplied_digest in [real_digest.as_str(), "wrong"] {
            for provider_fault in 0..4 {
                let provider = if provider_fault == 1 {
                    Vec::new()
                } else {
                    format!(
                        "#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{}\"\n",
                        if provider_fault == 3 {
                            "foreign"
                        } else {
                            supplied_digest
                        }
                    )
                    .into_bytes()
                };
                let hash = if provider_fault == 2 {
                    "wrong".to_owned()
                } else {
                    provider_sha256(&provider)
                };
                let plan = PackagePlan::new(
                    bytes.clone(),
                    supplied_digest.to_owned(),
                    vec!["api.value".to_owned()],
                    provider,
                    hash,
                    PackageMode::ProjectV9FlatRecord,
                );
                let (result, work) = observed(|| build_flat_record_and_publish(plan, &output));
                let expected = if HostTarget::current().is_none() {
                    PackageErrorKind::ToolConfiguration
                } else if provider_fault != 0 {
                    PackageErrorKind::Provider
                } else {
                    PackageErrorKind::Descriptor
                };
                assert_eq!(result.unwrap_err().kind(), expected);
                assert_eq!(work, (0, 0));
                assert!(!output.exists());
                assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            }
        }
    }
    // Bounded, correctly framed digest mismatches retain Provider classification.
    let provider = b"#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"wrong\"\n";
    let plan = PackagePlan::new(
        CANONICAL.to_vec(),
        "wrong".to_owned(),
        vec!["api.value".to_owned()],
        provider.to_vec(),
        provider_sha256(provider),
        PackageMode::ProjectV9FlatRecord,
    );
    let (result, work) = observed(|| build_flat_record_and_publish(plan, &output));
    if HostTarget::current().is_some() {
        assert_eq!(result.unwrap_err().kind(), PackageErrorKind::Provider);
        assert_eq!(work, (1, CANONICAL.len()));
    } else {
        assert_eq!(
            result.unwrap_err().kind(),
            PackageErrorKind::ToolConfiguration
        );
        assert_eq!(work, (0, 0));
    }
    // Remove only the exclusively created, still-empty plain directory.
    assert!(std::fs::symlink_metadata(&root)
        .unwrap()
        .file_type()
        .is_dir());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir(root).unwrap();
}
