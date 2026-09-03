//! Independent replay of one unpacked signed doctor distribution.

use std::path::Path;

use semaprax_doctor_capsule::parse_signed;
use sha2::{Digest as _, Sha256};

use super::{
    digest, parse_public_key, read_once, render_manifest_exact, validate_release_identity_values,
    validate_selector, validate_static_elf, verify_outputs, Artifact, InputArtifact, BUNDLE_FILE,
    CAPSULE_FILE, COLLECTOR_FILE, LAUNCHER_FILE, MANIFEST_FILE, MANIFEST_SIGNATURE_FILE,
    MAX_ARTIFACT_BYTES, MAX_MANIFEST_BYTES, PROVISIONER_FILE, REQUEST_FILE, WORKER_FILE,
};

const INVENTORY: [&str; 9] = [
    BUNDLE_FILE,
    COLLECTOR_FILE,
    LAUNCHER_FILE,
    PROVISIONER_FILE,
    MANIFEST_FILE,
    MANIFEST_SIGNATURE_FILE,
    CAPSULE_FILE,
    REQUEST_FILE,
    WORKER_FILE,
];

/// Identity supplied independently of the unpacked distribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseExpectation {
    pub release_version: String,
    pub release_commit: String,
    pub target_triple: String,
    pub architecture: u8,
    pub target: u8,
    pub selector: String,
    pub public_key_hex: String,
}

/// Replays signatures, canonical metadata and every actual artifact in an
/// unpacked doctor distribution. No manifest field supplies its own trust.
pub fn verify_release_directory(
    directory: &Path,
    expected: &ReleaseExpectation,
) -> Result<(), String> {
    validate_expectation(expected)?;
    require_exact_inventory(directory)?;

    let capsule = read_once(&directory.join(CAPSULE_FILE), 341, false, false)?;
    let manifest = read_once(
        &directory.join(MANIFEST_FILE),
        MAX_MANIFEST_BYTES,
        false,
        false,
    )?;
    let signature = read_once(&directory.join(MANIFEST_SIGNATURE_FILE), 64, false, false)?;
    if signature.len() != 64 {
        return Err("manifest signature length is invalid".into());
    }
    verify_outputs(&capsule, &manifest, &signature, &expected.public_key_hex)?;
    let public = parse_public_key(&expected.public_key_hex)?;
    let parsed = parse_signed(&capsule, &public).map_err(|_| "signed capsule replay failed")?;
    if parsed.architecture != expected.architecture
        || parsed.target != expected.target
        || parsed.selector != expected.selector
    {
        return Err("capsule disagrees with the independent release identity".into());
    }

    let paths = [
        ("request", REQUEST_FILE, false),
        ("bundle", BUNDLE_FILE, false),
        ("launcher", LAUNCHER_FILE, true),
        ("worker", WORKER_FILE, true),
        ("collector", COLLECTOR_FILE, true),
        ("provisioner", PROVISIONER_FILE, true),
    ];
    let loaded = paths
        .map(|(role, file, executable)| {
            let bytes = read_once(&directory.join(file), MAX_ARTIFACT_BYTES, executable, false)?;
            if executable {
                validate_static_elf(&bytes, expected.architecture)?;
            }
            let length = u64::try_from(bytes.len()).map_err(|_| "artifact length overflow")?;
            let artifact = Artifact {
                length,
                digest: Sha256::digest(&bytes).into(),
            };
            Ok(InputArtifact {
                role,
                bytes,
                artifact,
            })
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?;
    let artifacts: [InputArtifact; 6] = loaded
        .try_into()
        .map_err(|_| "release artifact inventory is not exact")?;
    for (actual, capsule_artifact) in artifacts[..5].iter().zip(parsed.artifacts) {
        if actual.artifact.length != capsule_artifact.length
            || actual.artifact.digest != capsule_artifact.digest
        {
            return Err("actual artifact disagrees with the signed capsule".into());
        }
    }

    let expected_manifest = render_manifest_exact(
        &expected.release_version,
        &expected.release_commit,
        &expected.target_triple,
        expected.architecture,
        expected.target,
        &expected.selector,
        &artifacts,
        capsule.len(),
        &digest(&capsule),
        &digest(&public.to_bytes()),
        &expected.public_key_hex,
    );
    if manifest != expected_manifest.as_bytes() {
        return Err("manifest is not the exact canonical release binding".into());
    }
    Ok(())
}

fn validate_expectation(expected: &ReleaseExpectation) -> Result<(), String> {
    validate_selector(&expected.selector)?;
    parse_public_key(&expected.public_key_hex)?;
    validate_release_identity_values(
        &expected.release_version,
        &expected.release_commit,
        &expected.target_triple,
        expected.architecture,
    )?;
    if !matches!(expected.architecture, 1 | 2) || expected.target > 3 {
        return Err("independent architecture or target is unsupported".into());
    }
    Ok(())
}

fn require_exact_inventory(directory: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|_| "release directory metadata is unavailable")?;
    if !metadata.file_type().is_dir() {
        return Err("release directory is not one physical directory".into());
    }
    let mut actual = Vec::<String>::new();
    actual
        .try_reserve_exact(INVENTORY.len())
        .map_err(|_| "release inventory allocation failed")?;
    for entry in std::fs::read_dir(directory).map_err(|_| "release directory cannot be read")? {
        let entry = entry.map_err(|_| "release directory entry cannot be read")?;
        let kind = entry
            .file_type()
            .map_err(|_| "release directory entry type is unavailable")?;
        if !kind.is_file() || kind.is_symlink() {
            return Err("release inventory contains a non-regular entry".into());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "release inventory name is not UTF-8")?;
        actual.push(name);
        if actual.len() > INVENTORY.len() {
            return Err("release inventory contains surplus entries".into());
        }
    }
    actual.sort();
    if actual != INVENTORY {
        return Err("release inventory is not exact".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_names_are_a_closed_bijection() {
        for (role, file) in [
            ("request", REQUEST_FILE),
            ("bundle", BUNDLE_FILE),
            ("launcher", LAUNCHER_FILE),
            ("worker", WORKER_FILE),
            ("collector", COLLECTOR_FILE),
            ("provisioner", PROVISIONER_FILE),
        ] {
            assert_eq!(crate::artifact_file(role), Some(file));
        }
        assert_eq!(crate::artifact_file("surplus"), None);
    }

    #[test]
    fn exact_inventory_names_are_sorted_and_unique() {
        assert!(INVENTORY.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(INVENTORY.len(), 9);
    }
}
