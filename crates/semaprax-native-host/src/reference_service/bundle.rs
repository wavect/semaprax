//! The digest-bound local run bundle.
//!
//! The bundle is a directory holding the exact decoded deployment inputs --
//! the operator's `service.config.json` bytes and the derived canonical
//! `service-host-adapter-request.json` bytes -- plus a manifest binding each
//! file's SHA-256 digest. Writing and verifying follow the durable store's
//! own discipline (atomic create-new, byte-identical replay, file sync,
//! named recheck). The bundle names no base image, registry, signature, or
//! publication: OCI runtime execution remains open, and this bundle does
//! not claim it.

use std::ffi::OsStr;

use semaprax_native_rust_interop_platform as platform;
use semaprax_native_rust_interop_platform::HeldDirectory;

use super::json::{self, JsonValue};

pub const BUNDLE_SCHEMA: &str = "semaprax.reference-service.bundle.v1";
pub const BUNDLE_MANIFEST: &str = "bundle-manifest.json";
pub const BUNDLE_CONFIG: &str = "service.config.json";
pub const BUNDLE_REQUEST: &str = "service-host-adapter-request.json";
pub const MAX_BUNDLE_FILE_BYTES: usize = 64 * 1024;

/// Stable refusal categories for bundle writing and verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleRefusal {
    InvalidName,
    TooLarge,
    Unavailable,
    Mismatch,
}

/// Write the deployment bundle and return its manifest digest. Every file
/// is created atomically; a byte-identical replay is accepted, while any
/// differing content under the same name is refused.
pub fn write_bundle(
    directory: &HeldDirectory,
    files: &[(&str, &[u8])],
) -> Result<String, BundleRefusal> {
    for (name, _) in files {
        if !matches!(name, &BUNDLE_CONFIG | &BUNDLE_REQUEST) {
            return Err(BundleRefusal::InvalidName);
        }
    }
    if files.len() != 2 {
        return Err(BundleRefusal::InvalidName);
    }
    let mut entries = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        if bytes.is_empty() || bytes.len() > MAX_BUNDLE_FILE_BYTES {
            return Err(BundleRefusal::TooLarge);
        }
        commit_file(directory, name, bytes)?;
        entries.push(((*name).to_owned(), digest(bytes)));
    }
    entries.sort();
    let manifest = json::render(&JsonValue::Object(vec![
        (
            "files".to_owned(),
            JsonValue::Array(
                entries
                    .iter()
                    .map(|(name, sha256)| {
                        JsonValue::Object(vec![
                            ("name".to_owned(), JsonValue::Str(name.clone())),
                            ("sha256".to_owned(), JsonValue::Str(sha256.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "schema".to_owned(),
            JsonValue::Str(BUNDLE_SCHEMA.to_owned()),
        ),
    ]));
    commit_file(directory, BUNDLE_MANIFEST, manifest.as_bytes())?;
    Ok(digest(manifest.as_bytes()))
}

/// Verify the bundle against an expected manifest digest: the manifest must
/// address exactly its listed files, and every file must match.
pub fn verify_bundle(
    directory: &HeldDirectory,
    expected_manifest_digest: &str,
) -> Result<(), BundleRefusal> {
    let manifest = load_file(directory, BUNDLE_MANIFEST)?;
    if digest(&manifest) != expected_manifest_digest {
        return Err(BundleRefusal::Mismatch);
    }
    let value =
        json::parse(&manifest, MAX_BUNDLE_FILE_BYTES).map_err(|_| BundleRefusal::Mismatch)?;
    let root = value
        .closed(&["files", "schema"])
        .ok_or(BundleRefusal::Mismatch)?;
    let schema = root
        .iter()
        .find(|(key, _)| key == "schema")
        .and_then(|(_, value)| value.as_str())
        .ok_or(BundleRefusal::Mismatch)?;
    if schema != BUNDLE_SCHEMA {
        return Err(BundleRefusal::Mismatch);
    }
    let files = root
        .iter()
        .find(|(key, _)| key == "files")
        .and_then(|(_, value)| value.as_array())
        .ok_or(BundleRefusal::Mismatch)?;
    if files.len() != 2 {
        return Err(BundleRefusal::Mismatch);
    }
    for file in files {
        let entry = file
            .closed(&["name", "sha256"])
            .ok_or(BundleRefusal::Mismatch)?;
        let name = entry
            .iter()
            .find(|(key, _)| key == "name")
            .and_then(|(_, value)| value.as_str())
            .ok_or(BundleRefusal::Mismatch)?;
        let sha256 = entry
            .iter()
            .find(|(key, _)| key == "sha256")
            .and_then(|(_, value)| value.as_str())
            .ok_or(BundleRefusal::Mismatch)?;
        if !matches!(name, BUNDLE_CONFIG | BUNDLE_REQUEST) {
            return Err(BundleRefusal::Mismatch);
        }
        let bytes = load_file(directory, name)?;
        if digest(&bytes) != sha256 {
            return Err(BundleRefusal::Mismatch);
        }
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    super::content_digest(bytes)
}

fn commit_file(directory: &HeldDirectory, name: &str, bytes: &[u8]) -> Result<(), BundleRefusal> {
    platform::recheck_directory(directory).map_err(|_| BundleRefusal::Unavailable)?;
    match platform::write_file_new(directory, OsStr::new(name), bytes, 0o600) {
        Ok(_) => {
            let existing = platform::hold_regular_file_bounded_for_sync(
                directory,
                OsStr::new(name),
                MAX_BUNDLE_FILE_BYTES,
            )
            .map_err(|_| BundleRefusal::Unavailable)?;
            if platform::sync_regular_file(&existing).is_err()
                || platform::recheck_regular_file_named_bounded(
                    directory,
                    OsStr::new(name),
                    &existing,
                    MAX_BUNDLE_FILE_BYTES,
                )
                .is_err()
            {
                return Err(BundleRefusal::Unavailable);
            }
            Ok(())
        }
        Err(platform::Error::Exists) => {
            let existing = load_file(directory, name)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(BundleRefusal::Mismatch)
            }
        }
        Err(_) => Err(BundleRefusal::Unavailable),
    }
}

fn load_file(directory: &HeldDirectory, name: &str) -> Result<Vec<u8>, BundleRefusal> {
    platform::recheck_directory(directory).map_err(|_| BundleRefusal::Unavailable)?;
    let file =
        platform::hold_regular_file_bounded(directory, OsStr::new(name), MAX_BUNDLE_FILE_BYTES)
            .map_err(|_| BundleRefusal::Unavailable)?;
    let bytes = platform::read_exact(&file, MAX_BUNDLE_FILE_BYTES)
        .map_err(|_| BundleRefusal::Unavailable)?;
    platform::recheck_regular_file(&file).map_err(|_| BundleRefusal::Unavailable)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_service::test_support::TempDir;

    fn inputs() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (BUNDLE_CONFIG, br#"{"mode":"host"}"#.to_vec()),
            (BUNDLE_REQUEST, br#"{"mode":"host"}"#.to_vec()),
        ]
    }

    #[test]
    fn write_verify_and_replay_round_trip() {
        let (_guard, directory) = TempDir::hold("reference-bundle");
        let inputs = inputs();
        let refs: Vec<(&str, &[u8])> = inputs
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect();
        let digest = write_bundle(&directory, &refs).unwrap();
        assert!(digest.starts_with("sha256:"));
        verify_bundle(&directory, &digest).unwrap();
        // A byte-identical replay is accepted and yields the same digest.
        assert_eq!(write_bundle(&directory, &refs).unwrap(), digest);
    }

    #[test]
    fn tamper_and_wrong_names_refuse() {
        let (guard, directory) = TempDir::hold("reference-bundle-tamper");
        let inputs = inputs();
        let refs: Vec<(&str, &[u8])> = inputs
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect();
        let digest = write_bundle(&directory, &refs).unwrap();
        std::fs::write(guard.join(BUNDLE_CONFIG), br#"{"mode":"tampered"}"#).unwrap();
        assert_eq!(
            verify_bundle(&directory, &digest),
            Err(BundleRefusal::Mismatch)
        );
        assert_eq!(
            write_bundle(&directory, &refs),
            Err(BundleRefusal::Mismatch)
        );
        assert_eq!(
            write_bundle(&directory, &[("elsewhere.json", b"{}".as_slice())]),
            Err(BundleRefusal::InvalidName)
        );
        assert_eq!(
            write_bundle(&directory, &refs[..1]),
            Err(BundleRefusal::InvalidName)
        );
    }
}
