//! Effect-free admission for the release-signed production profile capsule.
use ed25519_dalek::{Signature, VerifyingKey};

pub(super) const MAX_CAPSULE_BYTES: usize = 341;
const MAGIC: &[u8; 8] = b"SPXDPC1\0";
const SIGNATURE_BYTES: usize = 64;
const ARTIFACTS: usize = 5;
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Invalid,
    Limit,
    MissingTrustAnchor,
    InvalidTrustAnchor,
    Signature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Artifact {
    pub(super) length: u64,
    pub(super) digest: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Capsule {
    pub(super) architecture: u8,
    pub(super) target: u8,
    pub(super) roles: u8,
    pub(super) selector: String,
    pub(super) artifacts: [Artifact; ARTIFACTS],
}

impl Capsule {
    pub(super) fn request(&self) -> Artifact {
        self.artifacts[0]
    }
    pub(super) fn bundle(&self) -> Artifact {
        self.artifacts[1]
    }
    pub(super) fn launcher(&self) -> Artifact {
        self.artifacts[2]
    }
    pub(super) fn worker(&self) -> Artifact {
        self.artifacts[3]
    }
    pub(super) fn collector(&self) -> Artifact {
        self.artifacts[4]
    }
}

/// The release build supplies an Ed25519 public key, never signing material.
/// Absence is a production-disabled state, not permission to trust a capsule.
pub(super) fn parse_with_release_anchor(bytes: &[u8]) -> Result<Capsule, Error> {
    let encoded =
        option_env!("SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX").ok_or(Error::MissingTrustAnchor)?;
    let key = parse_public_key(encoded)?;
    parse(bytes, &key)
}

fn parse_public_key(encoded: &str) -> Result<VerifyingKey, Error> {
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

fn hex(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidTrustAnchor),
    }
}

pub(super) fn parse(bytes: &[u8], key: &VerifyingKey) -> Result<Capsule, Error> {
    if bytes.len() > MAX_CAPSULE_BYTES {
        return Err(Error::Limit);
    }
    if bytes.len() < 8 + 5 + ARTIFACTS * 40 + SIGNATURE_BYTES {
        return Err(Error::Invalid);
    }
    let body_len = bytes.len() - SIGNATURE_BYTES;
    let (body, signature) = bytes.split_at(body_len);
    let signature_bytes: [u8; SIGNATURE_BYTES] =
        signature.try_into().map_err(|_| Error::Invalid)?;
    key.verify_strict(body, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| Error::Signature)?;

    let mut cursor = 0usize;
    if take(body, &mut cursor, MAGIC.len())? != MAGIC || byte(body, &mut cursor)? != 1 {
        return Err(Error::Invalid);
    }
    let architecture = byte(body, &mut cursor)?;
    if !matches!(architecture, 1 | 2) {
        return Err(Error::Invalid);
    }
    let target = byte(body, &mut cursor)?;
    let expected_roles = roles_for_target(target).ok_or(Error::Invalid)?;
    let roles = byte(body, &mut cursor)?;
    if roles != expected_roles {
        return Err(Error::Invalid);
    }
    let selector_len = usize::from(byte(body, &mut cursor)?);
    if selector_len == 0 || selector_len > 64 {
        return Err(Error::Invalid);
    }
    let selector_bytes = take(body, &mut cursor, selector_len)?;
    if !selector_bytes[0].is_ascii_lowercase()
        || !selector_bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        return Err(Error::Invalid);
    }
    let selector = std::str::from_utf8(selector_bytes)
        .map_err(|_| Error::Invalid)?
        .to_owned();
    let mut artifacts = [Artifact {
        length: 0,
        digest: [0; 32],
    }; ARTIFACTS];
    for artifact in &mut artifacts {
        artifact.length = u64::from_le_bytes(array(body, &mut cursor)?);
        artifact.digest = array(body, &mut cursor)?;
        if artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES {
            return Err(Error::Limit);
        }
    }
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

pub(super) fn roles_for_target(target: u8) -> Option<u8> {
    match target {
        0 => Some(4),
        1 => Some(1),
        2 => Some(2),
        3 => Some(7),
        _ => None,
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
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    fn signed(mut body: Vec<u8>, signing: &SigningKey) -> Vec<u8> {
        let signature = signing.sign(&body).to_bytes();
        body.extend_from_slice(&signature);
        body
    }

    fn fixture(signing: &SigningKey) -> Vec<u8> {
        let selector = b"release-linux-v1";
        let mut body = Vec::new();
        body.extend_from_slice(MAGIC);
        body.extend_from_slice(&[1, 1, 3, 7, selector.len() as u8]);
        body.extend_from_slice(selector);
        for index in 0..ARTIFACTS {
            body.extend_from_slice(&(100 + index as u64).to_le_bytes());
            body.extend_from_slice(&[index as u8 + 1; 32]);
        }
        signed(body, signing)
    }

    #[test]
    fn exact_signed_capsule_is_closed_and_bound() {
        let signing = SigningKey::from_bytes(&[17; 32]);
        let bytes = fixture(&signing);
        let capsule = parse(&bytes, &signing.verifying_key()).unwrap();
        assert_eq!(capsule.architecture, 1);
        assert_eq!(capsule.target, 3);
        assert_eq!(capsule.roles, 7);
        assert_eq!(capsule.selector, "release-linux-v1");
        assert_eq!(capsule.request().length, 100);
        assert_eq!(capsule.bundle().length, 101);
        assert_eq!(capsule.launcher().length, 102);
        assert_eq!(capsule.worker().length, 103);
        assert_eq!(capsule.collector().digest, [5; 32]);
    }

    #[test]
    fn every_body_mutation_and_wrong_key_rejects() {
        let signing = SigningKey::from_bytes(&[23; 32]);
        let bytes = fixture(&signing);
        for index in 0..bytes.len() - SIGNATURE_BYTES {
            let mut corrupt = bytes.clone();
            corrupt[index] ^= 1;
            assert_eq!(
                parse(&corrupt, &signing.verifying_key()),
                Err(Error::Signature)
            );
        }
        let wrong = SigningKey::from_bytes(&[24; 32]);
        assert_eq!(parse(&bytes, &wrong.verifying_key()), Err(Error::Signature));
    }

    #[test]
    fn signed_noncanonical_fields_and_trailing_bytes_reject() {
        let signing = SigningKey::from_bytes(&[29; 32]);
        let valid = fixture(&signing);
        let body = &valid[..valid.len() - SIGNATURE_BYTES];
        for (offset, value) in [(9, 0), (10, 9), (11, 1), (12, 0)] {
            let mut invalid = body.to_vec();
            invalid[offset] = value;
            assert!(parse(&signed(invalid, &signing), &signing.verifying_key()).is_err());
        }
        let mut trailing = body.to_vec();
        trailing.push(0);
        assert_eq!(
            parse(&signed(trailing, &signing), &signing.verifying_key()),
            Err(Error::Invalid)
        );
    }

    #[test]
    fn trust_anchor_parser_is_strict_lower_hex_ed25519() {
        if option_env!("SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX").is_none() {
            assert_eq!(
                parse_with_release_anchor(b"not-a-capsule"),
                Err(Error::MissingTrustAnchor)
            );
        }
        assert_eq!(parse_public_key(""), Err(Error::InvalidTrustAnchor));
        assert_eq!(
            parse_public_key(&"A".repeat(64)),
            Err(Error::InvalidTrustAnchor)
        );
        assert_eq!(
            parse_public_key(&"0".repeat(64)),
            Err(Error::InvalidTrustAnchor)
        );
        let signing = SigningKey::from_bytes(&[31; 32]);
        let encoded = signing
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(parse_public_key(&encoded).unwrap(), signing.verifying_key());
    }
}
