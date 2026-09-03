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
    require_exact_inventory(directory)?;
    let paths = [
        (BUNDLE_FILE, MAX_ARTIFACT_BYTES, false),
        (COLLECTOR_FILE, MAX_ARTIFACT_BYTES, true),
        (LAUNCHER_FILE, MAX_ARTIFACT_BYTES, true),
        (PROVISIONER_FILE, MAX_ARTIFACT_BYTES, true),
        (MANIFEST_FILE, MAX_MANIFEST_BYTES, false),
        (MANIFEST_SIGNATURE_FILE, 64, false),
        (CAPSULE_FILE, 341, false),
        (REQUEST_FILE, MAX_ARTIFACT_BYTES, false),
        (WORKER_FILE, MAX_ARTIFACT_BYTES, true),
    ];
    let loaded = paths
        .map(|(name, maximum, executable)| {
            read_once(&directory.join(name), maximum, executable, false)
                .map(|bytes| (name, bytes, executable))
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?;
    let borrowed: Vec<_> = loaded
        .iter()
        .map(|(name, bytes, executable)| (*name, bytes.as_slice(), *executable))
        .collect();
    verify_release_bytes(&borrowed, expected)
}

/// Replays one already handle-authenticated exact inventory. The caller owns
/// file identity and mutation checks; this function performs no filesystem IO.
pub(crate) fn verify_release_bytes(
    files: &[(&str, &[u8], bool)],
    expected: &ReleaseExpectation,
) -> Result<(), String> {
    validate_expectation(expected)?;
    if files.len() != INVENTORY.len() {
        return Err("release artifact inventory is not exact".into());
    }
    for expected_name in INVENTORY {
        if files.iter().filter(|row| row.0 == expected_name).count() != 1 {
            return Err("release artifact inventory is not exact".into());
        }
    }
    let get = |name: &str| {
        files
            .iter()
            .find(|row| row.0 == name)
            .map(|row| row.1)
            .ok_or_else(|| "release artifact inventory is not exact".to_owned())
    };
    let capsule = get(CAPSULE_FILE)?;
    let manifest = get(MANIFEST_FILE)?;
    let signature = get(MANIFEST_SIGNATURE_FILE)?;
    if capsule.len() > 341 || manifest.len() > MAX_MANIFEST_BYTES as usize || signature.len() != 64
    {
        return Err("release signed metadata length is invalid".into());
    }
    verify_outputs(capsule, manifest, signature, &expected.public_key_hex)?;
    let public = parse_public_key(&expected.public_key_hex)?;
    let parsed = parse_signed(capsule, &public).map_err(|_| "signed capsule replay failed")?;
    if parsed.architecture != expected.architecture
        || parsed.target != expected.target
        || parsed.selector != expected.selector
    {
        return Err("capsule disagrees with the independent release identity".into());
    }
    let roles = [
        ("request", REQUEST_FILE, false),
        ("bundle", BUNDLE_FILE, false),
        ("launcher", LAUNCHER_FILE, true),
        ("worker", WORKER_FILE, true),
        ("collector", COLLECTOR_FILE, true),
        ("provisioner", PROVISIONER_FILE, true),
    ];
    let loaded = roles.map(|(role, name, executable)| {
        let bytes = get(name)?;
        let stated_executable = files
            .iter()
            .find(|row| row.0 == name)
            .map(|row| row.2)
            .ok_or_else(|| "release artifact inventory is not exact".to_owned())?;
        if executable != stated_executable {
            return Err("release artifact mode classification disagrees".into());
        }
        if executable {
            validate_static_elf(bytes, expected.architecture)?;
        }
        Ok(InputArtifact {
            role,
            bytes: bytes.to_vec(),
            artifact: Artifact {
                length: u64::try_from(bytes.len()).map_err(|_| "artifact length overflow")?,
                digest: Sha256::digest(bytes).into(),
            },
        })
    });
    let artifacts: [InputArtifact; 6] = loaded
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "release artifact inventory is not exact")?;
    for (actual, capsule_artifact) in artifacts[..5].iter().zip(parsed.artifacts) {
        if actual.artifact.length != capsule_artifact.length
            || actual.artifact.digest != capsule_artifact.digest
        {
            return Err("actual artifact disagrees with the signed capsule".into());
        }
    }
    let canonical = render_manifest_exact(
        &expected.release_version,
        &expected.release_commit,
        &expected.target_triple,
        expected.architecture,
        expected.target,
        &expected.selector,
        &artifacts,
        capsule.len(),
        &digest(capsule),
        &digest(&public.to_bytes()),
        &expected.public_key_hex,
    );
    if manifest != canonical.as_bytes() {
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
