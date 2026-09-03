//! Pure canonical codec for release-signed production doctor capsules.
#![forbid(unsafe_code)]

use ed25519_dalek::{Signature, VerifyingKey};

pub const ARTIFACT_COUNT: usize = 5;
pub const MAX_CAPSULE_BYTES: usize = 341;
pub const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

const MAGIC: &[u8; 8] = b"SPXDPC1\0";
const VERSION: u8 = 1;
const SIGNATURE_BYTES: usize = 64;
const MAX_SELECTOR_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Limit,
    InvalidTrustAnchor,
    Signature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Artifact {
    pub length: u64,
    pub digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapsuleSpec {
    pub architecture: u8,
    pub target: u8,
    pub selector: String,
    pub artifacts: [Artifact; ARTIFACT_COUNT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capsule {
    pub architecture: u8,
    pub target: u8,
    pub roles: u8,
    pub selector: String,
    pub artifacts: [Artifact; ARTIFACT_COUNT],
}

impl Capsule {
    pub fn request(&self) -> Artifact {
        self.artifacts[0]
    }
    pub fn bundle(&self) -> Artifact {
        self.artifacts[1]
    }
    pub fn launcher(&self) -> Artifact {
        self.artifacts[2]
    }
    pub fn worker(&self) -> Artifact {
        self.artifacts[3]
    }
    pub fn collector(&self) -> Artifact {
        self.artifacts[4]
    }
}

pub fn encode_body(spec: &CapsuleSpec) -> Result<Vec<u8>, Error> {
    validate_architecture(spec.architecture)?;
    let roles = roles_for_target(spec.target).ok_or(Error::Invalid)?;
    validate_selector(spec.selector.as_bytes())?;
    validate_artifacts(&spec.artifacts)?;
    let capacity = MAGIC
        .len()
        .checked_add(5)
        .and_then(|length| length.checked_add(spec.selector.len()))
        .and_then(|length| length.checked_add(ARTIFACT_COUNT.checked_mul(40)?))
        .ok_or(Error::Limit)?;
    let mut body = Vec::with_capacity(capacity);
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(&[
        VERSION,
        spec.architecture,
        spec.target,
        roles,
        u8::try_from(spec.selector.len()).map_err(|_| Error::Limit)?,
    ]);
    body.extend_from_slice(spec.selector.as_bytes());
    for artifact in spec.artifacts {
        body.extend_from_slice(&artifact.length.to_le_bytes());
        body.extend_from_slice(&artifact.digest);
    }
    if body
        .len()
        .checked_add(SIGNATURE_BYTES)
        .ok_or(Error::Limit)?
        > MAX_CAPSULE_BYTES
    {
        return Err(Error::Limit);
    }
    Ok(body)
}

pub fn parse_signed(bytes: &[u8], key: &VerifyingKey) -> Result<Capsule, Error> {
    if bytes.len() > MAX_CAPSULE_BYTES {
        return Err(Error::Limit);
    }
    if bytes.len() < MAGIC.len() + 5 + ARTIFACT_COUNT * 40 + SIGNATURE_BYTES {
        return Err(Error::Invalid);
    }
    let body_len = bytes.len() - SIGNATURE_BYTES;
    let (body, signature) = bytes.split_at(body_len);
    let signature_bytes: [u8; SIGNATURE_BYTES] =
        signature.try_into().map_err(|_| Error::Invalid)?;
    key.verify_strict(body, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| Error::Signature)?;

    let mut cursor = 0usize;
    if take(body, &mut cursor, MAGIC.len())? != MAGIC || byte(body, &mut cursor)? != VERSION {
        return Err(Error::Invalid);
    }
    let architecture = byte(body, &mut cursor)?;
    validate_architecture(architecture)?;
    let target = byte(body, &mut cursor)?;
    let expected_roles = roles_for_target(target).ok_or(Error::Invalid)?;
    let roles = byte(body, &mut cursor)?;
    if roles != expected_roles {
        return Err(Error::Invalid);
    }
    let selector_len = usize::from(byte(body, &mut cursor)?);
    let selector_bytes = take(body, &mut cursor, selector_len)?;
    validate_selector(selector_bytes)?;
    let selector = std::str::from_utf8(selector_bytes)
        .map_err(|_| Error::Invalid)?
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
        return Err(Error::Invalid);
    }
    Ok(Capsule {
        architecture,
        target,
        roles,
        selector,
        artifacts,
    })
}

pub fn parse_public_key(encoded: &str) -> Result<VerifyingKey, Error> {
    if encoded.len() != 64 {
        return Err(Error::InvalidTrustAnchor);
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index.checked_mul(2).ok_or(Error::InvalidTrustAnchor)?;
        *byte = hex(encoded.as_bytes()[offset])?
            .checked_mul(16)
            .and_then(|high| high.checked_add(hex(encoded.as_bytes()[offset + 1]).ok()?))
            .ok_or(Error::InvalidTrustAnchor)?;
    }
    let key = VerifyingKey::from_bytes(&bytes).map_err(|_| Error::InvalidTrustAnchor)?;
    if key.is_weak() {
        Err(Error::InvalidTrustAnchor)
    } else {
        Ok(key)
    }
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

fn validate_architecture(architecture: u8) -> Result<(), Error> {
    if matches!(architecture, 1 | 2) {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}

fn validate_selector(selector: &[u8]) -> Result<(), Error> {
    if selector.is_empty()
        || selector.len() > MAX_SELECTOR_BYTES
        || !selector[0].is_ascii_lowercase()
        || !selector
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}

fn validate_artifacts(artifacts: &[Artifact; ARTIFACT_COUNT]) -> Result<(), Error> {
    if artifacts
        .iter()
        .any(|artifact| artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES)
    {
        Err(Error::Limit)
    } else {
        Ok(())
    }
}

fn hex(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidTrustAnchor),
    }
}

fn byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, Error> {
    Ok(take(bytes, cursor, 1)?[0])
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, count: usize) -> Result<&'a [u8], Error> {
    let end = cursor.checked_add(count).ok_or(Error::Limit)?;
    let value = bytes.get(*cursor..end).ok_or(Error::Invalid)?;
    *cursor = end;
    Ok(value)
}

fn array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], Error> {
    take(bytes, cursor, N)?
        .try_into()
        .map_err(|_| Error::Invalid)
}

#[cfg(test)]
mod tests;
