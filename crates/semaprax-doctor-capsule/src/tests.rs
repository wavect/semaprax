use super::*;
use ed25519_dalek::{Signer as _, SigningKey};

fn artifacts() -> [Artifact; ARTIFACT_COUNT] {
    std::array::from_fn(|index| Artifact {
        length: 100 + index as u64,
        digest: [index as u8 + 1; 32],
    })
}

fn spec() -> CapsuleSpec {
    CapsuleSpec {
        architecture: 1,
        target: 3,
        selector: "release-linux-v1".to_owned(),
        artifacts: artifacts(),
    }
}

fn signed(mut body: Vec<u8>, signing: &SigningKey) -> Vec<u8> {
    body.extend_from_slice(&signing.sign(&body).to_bytes());
    body
}

fn fixture(signing: &SigningKey) -> Vec<u8> {
    signed(encode_body(&spec()).unwrap(), signing)
}

#[test]
fn encoder_preserves_v1_wire_and_parser_accessors() {
    let selector = b"release-linux-v1";
    let mut expected = Vec::new();
    expected.extend_from_slice(MAGIC);
    expected.extend_from_slice(&[1, 1, 3, 7, selector.len() as u8]);
    expected.extend_from_slice(selector);
    for (index, artifact) in artifacts().iter().enumerate() {
        expected.extend_from_slice(&(100 + index as u64).to_le_bytes());
        expected.extend_from_slice(&artifact.digest);
    }
    assert_eq!(encode_body(&spec()).unwrap(), expected);
    let signing = SigningKey::from_bytes(&[17; 32]);
    let capsule = parse_signed(&signed(expected, &signing), &signing.verifying_key()).unwrap();
    assert_eq!(
        (capsule.architecture, capsule.target, capsule.roles),
        (1, 3, 7)
    );
    assert_eq!(capsule.selector, "release-linux-v1");
    assert_eq!(capsule.request().length, 100);
    assert_eq!(capsule.bundle().length, 101);
    assert_eq!(capsule.launcher().length, 102);
    assert_eq!(capsule.worker().length, 103);
    assert_eq!(capsule.collector().digest, [5; 32]);
}

#[test]
fn every_target_derives_exact_roles_and_unknown_target_rejects() {
    let signing = SigningKey::from_bytes(&[19; 32]);
    for (target, roles) in [(0, 4), (1, 1), (2, 2), (3, 7)] {
        let mut value = spec();
        value.target = target;
        let capsule = parse_signed(
            &signed(encode_body(&value).unwrap(), &signing),
            &signing.verifying_key(),
        )
        .unwrap();
        assert_eq!(capsule.roles, roles);
    }
    let mut value = spec();
    value.target = 4;
    assert_eq!(encode_body(&value), Err(Error::Invalid));
}

#[test]
fn every_body_mutation_and_wrong_key_rejects() {
    let signing = SigningKey::from_bytes(&[23; 32]);
    let bytes = fixture(&signing);
    for index in 0..bytes.len() - SIGNATURE_BYTES {
        let mut corrupt = bytes.clone();
        corrupt[index] ^= 1;
        assert_eq!(
            parse_signed(&corrupt, &signing.verifying_key()),
            Err(Error::Signature)
        );
    }
    let wrong = SigningKey::from_bytes(&[24; 32]);
    assert_eq!(
        parse_signed(&bytes, &wrong.verifying_key()),
        Err(Error::Signature)
    );
}

#[test]
fn signed_noncanonical_fields_and_trailing_bytes_reject() {
    let signing = SigningKey::from_bytes(&[29; 32]);
    let valid = fixture(&signing);
    let body = &valid[..valid.len() - SIGNATURE_BYTES];
    for (offset, value) in [(8, 0), (9, 0), (10, 9), (11, 1), (12, 0)] {
        let mut invalid = body.to_vec();
        invalid[offset] = value;
        assert!(parse_signed(&signed(invalid, &signing), &signing.verifying_key()).is_err());
    }
    let mut trailing = body.to_vec();
    trailing.push(0);
    assert_eq!(
        parse_signed(&signed(trailing, &signing), &signing.verifying_key()),
        Err(Error::Invalid)
    );
}

#[test]
fn selector_and_artifact_limits_are_exact() {
    for invalid in ["", "Upper", "1first", "has_underscore"] {
        let mut value = spec();
        value.selector = invalid.to_owned();
        assert_eq!(encode_body(&value), Err(Error::Invalid));
    }
    let mut exact = spec();
    exact.selector = format!("a{}", "z".repeat(63));
    exact.artifacts[0].length = MAX_ARTIFACT_BYTES;
    let body = encode_body(&exact).unwrap();
    assert_eq!(body.len() + SIGNATURE_BYTES, MAX_CAPSULE_BYTES);
    let mut selector_over = exact.clone();
    selector_over.selector.push('z');
    assert_eq!(encode_body(&selector_over), Err(Error::Invalid));
    for length in [0, MAX_ARTIFACT_BYTES + 1] {
        let mut invalid = spec();
        invalid.artifacts[2].length = length;
        assert_eq!(encode_body(&invalid), Err(Error::Limit));
    }
}

#[test]
fn parser_distinguishes_size_shape_signature_and_semantic_limits() {
    let signing = SigningKey::from_bytes(&[30; 32]);
    assert_eq!(
        parse_signed(&vec![0; MAX_CAPSULE_BYTES + 1], &signing.verifying_key()),
        Err(Error::Limit)
    );
    assert_eq!(
        parse_signed(&[], &signing.verifying_key()),
        Err(Error::Invalid)
    );
    let mut body = encode_body(&spec()).unwrap();
    let length_offset = MAGIC.len() + 5 + spec().selector.len();
    body[length_offset..length_offset + 8].fill(0);
    assert_eq!(
        parse_signed(&signed(body, &signing), &signing.verifying_key()),
        Err(Error::Limit)
    );
}

#[test]
fn trust_anchor_parser_is_strict_lower_hex_nonweak_ed25519() {
    assert_eq!(parse_public_key(""), Err(Error::InvalidTrustAnchor));
    assert_eq!(
        parse_public_key(&"A".repeat(64)),
        Err(Error::InvalidTrustAnchor)
    );
    assert_eq!(
        parse_public_key(&"0".repeat(64)),
        Err(Error::InvalidTrustAnchor)
    );
    assert_eq!(
        parse_public_key(&format!("{}g", "0".repeat(63))),
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
