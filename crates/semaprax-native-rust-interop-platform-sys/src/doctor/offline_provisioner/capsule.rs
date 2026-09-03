//! Release-anchor wrapper around the effect-free canonical capsule codec.

use semaprax_doctor_capsule::{parse_public_key, parse_signed, Capsule};
pub(super) use semaprax_doctor_capsule::{roles_for_target, Artifact, MAX_CAPSULE_BYTES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    MissingTrustAnchor,
    InvalidTrustAnchor,
    Invalid,
    Limit,
    Signature,
}

/// The release build supplies an Ed25519 public key, never signing material.
/// Absence is a production-disabled state, not permission to trust a capsule.
pub(super) fn parse_with_release_anchor(bytes: &[u8]) -> Result<Capsule, Error> {
    let encoded =
        option_env!("SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX").ok_or(Error::MissingTrustAnchor)?;
    let key = parse_public_key(encoded).map_err(map_error)?;
    parse_signed(bytes, &key).map_err(map_error)
}

fn map_error(error: semaprax_doctor_capsule::Error) -> Error {
    match error {
        semaprax_doctor_capsule::Error::Invalid => Error::Invalid,
        semaprax_doctor_capsule::Error::Limit => Error::Limit,
        semaprax_doctor_capsule::Error::InvalidTrustAnchor => Error::InvalidTrustAnchor,
        semaprax_doctor_capsule::Error::Signature => Error::Signature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_release_anchor_remains_a_distinct_disabled_state() {
        if option_env!("SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX").is_none() {
            assert_eq!(
                parse_with_release_anchor(b"not-a-capsule"),
                Err(Error::MissingTrustAnchor)
            );
        }
    }
}
