//! Shared release-signed capsule admission for Windows, plus a retained
//! host-independent structural hostile-input corpus. Runtime admission uses
//! the shared `semaprax_doctor_capsule::parse_signed` codec through
//! [`parse_with_release_anchor`]; the structural decoder is non-authoritative.
//!
//! The structural result below does **not** verify the release Ed25519
//! signature. Runtime admission uses the shared signed parser, and requires
//! the trusted release key compiled through
//! `SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX`; missing or invalid key material
//! refuses before token, job, or filesystem effects. This structural parser
//! is retained only as a host-independent hostile-input corpus. Historical
//! implementation notes below no longer describe the runtime path.
//!
//! Do not interpret [`CapsuleBody`] as verified data: only the shared
//! `parse_signed` path checks its signature. The structural decoder below
//! remains solely for cross-platform malformed-wire tests.

pub const ARTIFACT_COUNT: usize = 5;
pub const MAX_CAPSULE_BYTES: usize = 341;
/// Held equal to the shared codec's `MAX_ARTIFACT_BYTES`: this bound is a
/// wire-format field width sanity check, not an independent capacity policy.
pub const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;

const MAGIC: &[u8; 8] = b"SPXDPC1\0";
const VERSION: u8 = 1;
const SIGNATURE_BYTES: usize = 64;
const MAX_SELECTOR_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapsuleError {
    Invalid,
    Limit,
    MissingTrustAnchor,
    InvalidTrustAnchor,
    Signature,
    ArchitectureMismatch,
    /// The selected signed image cannot be bound to a stable local file.
    ArtifactBinding,
}

#[cfg(windows)]
pub type VerifiedCapsule = semaprax_doctor_capsule::Capsule;

#[cfg(all(test, windows))]
pub(super) fn signed_test_fixture(architecture: u8) -> (Vec<u8>, String) {
    signed_test_fixture_with_artifacts(
        architecture,
        std::array::from_fn(|index| semaprax_doctor_capsule::Artifact {
            length: index as u64 + 1,
            digest: [0x42; 32],
        }),
    )
}

#[cfg(all(test, windows))]
pub(super) fn signed_test_fixture_with_artifacts(
    architecture: u8,
    artifacts: [semaprax_doctor_capsule::Artifact; ARTIFACT_COUNT],
) -> (Vec<u8>, String) {
    use ed25519_dalek::{Signer as _, SigningKey};

    let signing = SigningKey::from_bytes(&[0x6d; 32]);
    let specification = semaprax_doctor_capsule::CapsuleSpec {
        architecture,
        target: 0,
        selector: "runtime-test".to_owned(),
        artifacts,
    };
    let mut bytes =
        semaprax_doctor_capsule::encode_body(&specification).expect("test capsule spec is valid");
    bytes.extend_from_slice(&signing.sign(&bytes).to_bytes());
    let public_key_hex: String = signing
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let parsed =
        parse_signed_with_key(&bytes, &public_key_hex).expect("test fixture signature verifies");
    assert_eq!(parsed.architecture, specification.architecture);
    assert_eq!(parsed.target, specification.target);
    assert_eq!(parsed.roles, 4);
    assert_eq!(parsed.selector, specification.selector);
    assert_eq!(parsed.artifacts, specification.artifacts);
    (bytes, public_key_hex)
}

/// Verify the canonical signed capsule using an explicit trusted public-key
/// input. Runtime admission uses [`parse_with_release_anchor`] instead; this
/// explicit-key form exists for tests that construct a clearly test-only
/// signing key and never constitutes release trust.
#[cfg(windows)]
pub fn parse_signed_with_key(
    bytes: &[u8],
    public_key_hex: &str,
) -> Result<semaprax_doctor_capsule::Capsule, CapsuleError> {
    use semaprax_doctor_capsule::{parse_public_key, parse_signed};

    let key = parse_public_key(public_key_hex).map_err(map_signed_error)?;
    parse_signed(bytes, &key).map_err(map_signed_error)
}

/// Parse with the release key supplied by the trusted build/review process.
/// An absent key is a disabled production path, never a structural fallback.
#[cfg(windows)]
pub fn parse_with_release_anchor(
    bytes: &[u8],
) -> Result<semaprax_doctor_capsule::Capsule, CapsuleError> {
    parse_with_anchor(bytes, option_env!("SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX"))
}

#[cfg(windows)]
pub(super) fn parse_with_anchor(
    bytes: &[u8],
    public_key_hex: Option<&str>,
) -> Result<semaprax_doctor_capsule::Capsule, CapsuleError> {
    let public_key_hex = public_key_hex.ok_or(CapsuleError::MissingTrustAnchor)?;
    parse_windows_signed_with_key(bytes, public_key_hex)
}

#[cfg(windows)]
pub fn parse_windows_signed_with_key(
    bytes: &[u8],
    public_key_hex: &str,
) -> Result<semaprax_doctor_capsule::Capsule, CapsuleError> {
    let parsed = parse_signed_with_key(bytes, public_key_hex)?;
    let expected = windows_architecture_code().ok_or(CapsuleError::ArchitectureMismatch)?;
    if parsed.architecture != expected {
        return Err(CapsuleError::ArchitectureMismatch);
    }
    Ok(parsed)
}

#[cfg(windows)]
pub(super) fn windows_architecture_code() -> Option<u8> {
    if cfg!(target_arch = "x86_64") {
        Some(semaprax_doctor_capsule::ARCHITECTURE_WINDOWS_X86_64)
    } else if cfg!(target_arch = "aarch64") {
        Some(semaprax_doctor_capsule::ARCHITECTURE_WINDOWS_AARCH64)
    } else {
        None
    }
}

#[cfg(windows)]
fn map_signed_error(error: semaprax_doctor_capsule::Error) -> CapsuleError {
    match error {
        semaprax_doctor_capsule::Error::Invalid => CapsuleError::Invalid,
        semaprax_doctor_capsule::Error::Limit => CapsuleError::Limit,
        semaprax_doctor_capsule::Error::InvalidTrustAnchor => CapsuleError::InvalidTrustAnchor,
        semaprax_doctor_capsule::Error::Signature => CapsuleError::Signature,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Artifact {
    pub length: u64,
    pub digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapsuleBody {
    pub architecture: u8,
    pub target: u8,
    pub roles: u8,
    pub selector: String,
    pub artifacts: [Artifact; ARTIFACT_COUNT],
    /// Copied verbatim from the trailing 64 bytes; never checked by this
    /// module. See the module documentation: signature verification is the
    /// reused `semaprax-doctor-capsule` crate's job, once it is available on
    /// Windows.
    pub unverified_signature: [u8; SIGNATURE_BYTES],
}

/// Structurally decode a capsule's body fields without verifying its
/// signature. Field order, bounds, and error classification are identical to
/// `semaprax_doctor_capsule::parse_signed`'s body stage; see the module
/// documentation for why the signature step is absent here.
pub fn parse_capsule_body(bytes: &[u8]) -> Result<CapsuleBody, CapsuleError> {
    if bytes.len() > MAX_CAPSULE_BYTES {
        return Err(CapsuleError::Limit);
    }
    if bytes.len() < MAGIC.len() + 5 + ARTIFACT_COUNT * 40 + SIGNATURE_BYTES {
        return Err(CapsuleError::Invalid);
    }
    let body_len = bytes.len() - SIGNATURE_BYTES;
    let (body, signature) = bytes.split_at(body_len);
    let unverified_signature: [u8; SIGNATURE_BYTES] =
        signature.try_into().map_err(|_| CapsuleError::Invalid)?;

    let mut cursor = 0usize;
    if take(body, &mut cursor, MAGIC.len())? != MAGIC || byte(body, &mut cursor)? != VERSION {
        return Err(CapsuleError::Invalid);
    }
    let architecture = byte(body, &mut cursor)?;
    validate_architecture(architecture)?;
    let target = byte(body, &mut cursor)?;
    let expected_roles = roles_for_target(target).ok_or(CapsuleError::Invalid)?;
    let roles = byte(body, &mut cursor)?;
    if roles != expected_roles {
        return Err(CapsuleError::Invalid);
    }
    let selector_len = usize::from(byte(body, &mut cursor)?);
    let selector_bytes = take(body, &mut cursor, selector_len)?;
    validate_selector(selector_bytes)?;
    let selector = std::str::from_utf8(selector_bytes)
        .map_err(|_| CapsuleError::Invalid)?
        .to_owned();
    let mut artifacts = [Artifact {
        length: 0,
        digest: [0; 32],
    }; ARTIFACT_COUNT];
    for artifact in &mut artifacts {
        artifact.length = u64::from_le_bytes(array(body, &mut cursor)?);
        artifact.digest = array(body, &mut cursor)?;
    }
    validate_artifacts(&artifacts)?;
    if cursor != body.len() {
        return Err(CapsuleError::Invalid);
    }
    Ok(CapsuleBody {
        architecture,
        target,
        roles,
        selector,
        artifacts,
        unverified_signature,
    })
}

pub fn roles_for_target(target: u8) -> Option<u8> {
    match target {
        0 => Some(4),
        1 => Some(1),
        2 => Some(2),
        3 => Some(7),
        _ => None,
    }
}

fn validate_architecture(architecture: u8) -> Result<(), CapsuleError> {
    if matches!(architecture, 1..=4) {
        Ok(())
    } else {
        Err(CapsuleError::Invalid)
    }
}

fn validate_selector(selector: &[u8]) -> Result<(), CapsuleError> {
    if selector.is_empty()
        || selector.len() > MAX_SELECTOR_BYTES
        || !selector[0].is_ascii_lowercase()
        || !selector
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        Err(CapsuleError::Invalid)
    } else {
        Ok(())
    }
}

fn validate_artifacts(artifacts: &[Artifact; ARTIFACT_COUNT]) -> Result<(), CapsuleError> {
    if artifacts
        .iter()
        .any(|artifact| artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES)
    {
        Err(CapsuleError::Limit)
    } else {
        Ok(())
    }
}

fn byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, CapsuleError> {
    Ok(take(bytes, cursor, 1)?[0])
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, count: usize) -> Result<&'a [u8], CapsuleError> {
    let end = cursor.checked_add(count).ok_or(CapsuleError::Limit)?;
    let value = bytes.get(*cursor..end).ok_or(CapsuleError::Invalid)?;
    *cursor = end;
    Ok(value)
}

fn array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], CapsuleError> {
    take(bytes, cursor, N)?
        .try_into()
        .map_err(|_| CapsuleError::Invalid)
}

#[cfg(test)]
mod tests;
