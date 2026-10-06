//! Optional, caller-opt-in Ed25519 signature verification for
//! `semaprax.audit-capsule.v1` (issue #209's forged-signature gap).
//!
//! [`super::check_signature_policy`] never cryptographically verified a
//! [`super::SignatureEntry`]: a well-formed but forged `signature` naming an
//! approved, unexpired, unrevoked identity passed silently -- the module doc
//! and `nonclaims::ALWAYS_REQUIRED_NONCLAIMS`'s
//! `"signatures-not-cryptographically-verified"` entry both said so
//! plainly, attributing the gap to "no signing key ... exists in this
//! repository" (issue #168). That reasoning only ever covers *producing* a
//! signature. A caller who already holds a trusted verifying key for a
//! signer -- the ordinary "I already know Alice's public key" case, which
//! needs no signing key at all -- had no way to make this module check it.
//!
//! This closes that gap for `ed25519-raw-v1`, the one
//! [`super::KNOWN_SIGNATURE_ALGORITHMS`] entry a pure verifier can check with
//! no network access and no external tool:
//! [`super::SignaturePolicyContext::identity_public_keys`] lets a caller
//! supply a roster of `identity -> Ed25519 verifying key`. An empty roster
//! (the default, and every caller before this capability existed) preserves
//! the original opaque, policy-only behavior exactly -- this is strictly
//! additive. A non-empty roster switches on strict mode: every signature in
//! the capsule is then required to (a) name an identity the roster
//! recognizes, (b) use an algorithm this module can verify locally, (c)
//! carry a structurally valid signature encoding, and (d) verify against the
//! capsule's own exact signable bytes -- four independently diagnosable
//! failures, so "unverifiable", "unknown identity", "wrong bytes", and
//! "missing" (still `check_signature_policy`'s own role-presence check)
//! never collapse into one message.
//!
//! `sigstore-cosign-bundle-v0.3` remains unverifiable here: checking a
//! Sigstore bundle needs Rekor, which needs network access this module must
//! never touch (see the crate module doc). A capsule using that algorithm
//! under strict mode fails closed as unverifiable, exactly like a
//! structurally malformed signature -- never silently accepted just because
//! its role, identity, expiry, and revocation status all looked fine.
//!
//! ## Signed entry metadata (issue #577)
//!
//! `ed25519-raw-v1` signed only the capsule payload ([`signable_bytes`]
//! removes the whole `signatures` array), so an entry's `role`, `identity`,
//! `algorithm`, and `not_valid_after_unix_seconds` -- the very fields
//! `check_signature_policy` decides on -- were outside cryptographic
//! coverage: one genuine signature could be relabelled to another role, given
//! a later expiry, or copied into every role. Strict mode now verifies only
//! `ed25519-entry-v2`, whose domain-separated per-entry preimage
//! ([`entry_signable_bytes`]) binds all four fields plus the payload, and it
//! refuses `ed25519-raw-v1` as an explicit compatibility boundary rather
//! than reinterpreting old signatures as authenticating metadata they never
//! signed. The empty-roster, policy-only mode is unchanged for both.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::Value;

use crate::diagnostic::Diagnostic;

use super::{
    parse_json, signature_error, sorted_json, ParsedCapsule, SignatureEntry, SignaturePolicyContext,
};

const ED25519_RAW_V1: &str = "ed25519-raw-v1";

/// The per-entry signature algorithm whose preimage binds this entry's own
/// policy metadata (issue #577). See [`entry_signable_bytes`].
pub const ED25519_ENTRY_V2: &str = "ed25519-entry-v2";

/// Domain separator prefixed to every `ed25519-entry-v2` preimage, so a
/// signature over it can never be replayed as a signature over any other
/// SEMAPRAX message (including a legacy `ed25519-raw-v1` capsule payload,
/// which always starts with `{`).
const ENTRY_V2_DOMAIN: &[u8] = b"semaprax.audit-capsule.v1/signature-entry/ed25519-entry-v2\n";

/// The exact bytes an `ed25519-raw-v1` [`SignatureEntry::signature`] must be
/// an Ed25519 signature over: `manifest_bytes` with its `signatures` field
/// replaced by an empty array and object keys sorted, LF-terminated --
/// mirroring `super::transparency_leaf_digest_bytes`'s fixed-point avoidance
/// (a signature cannot honestly cover the very bytes that carry it). Every
/// other field -- `profile`, `subject`, `objects`, `associations`,
/// `transparency`, `nonclaims` -- is covered, so tampering any of them
/// changes what a valid signature must have been computed over.
pub(super) fn signable_bytes(manifest_bytes: &[u8]) -> Result<Vec<u8>, Diagnostic> {
    let mut value = parse_json(manifest_bytes, "audit capsule")?;
    let Some(map) = value.as_object_mut() else {
        return Err(signature_error(
            "audit capsule must be a JSON object to compute its signable bytes".to_owned(),
        ));
    };
    map.insert("signatures".to_owned(), Value::Array(Vec::new()));
    let mut text = serde_json::to_string(&sorted_json(&value)).map_err(|_| {
        signature_error("audit capsule cannot be canonically re-serialized".to_owned())
    })?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// The exact bytes an `ed25519-entry-v2` [`SignatureEntry::signature`] must
/// be an Ed25519 signature over (issue #577): [`ENTRY_V2_DOMAIN`], then one
/// LF-terminated canonical JSON line carrying **this entry's** `role`,
/// `identity`, `algorithm`, and `not_valid_after_unix_seconds`, then the
/// capsule payload [`signable_bytes`] defines. Only the entry's own
/// `signature` bytes are omitted, so relabelling a role, extending an
/// expiry, renaming the identity, or copying one signature into another
/// role all change what a valid signature must have covered. Each entry is
/// signed independently, so an early signer never needs to know later
/// signatures. Producers sign exactly these bytes; the verifier recomputes
/// them from the manifest under test and never trusts a supplied preimage.
pub fn entry_signable_bytes(
    manifest_bytes: &[u8],
    entry: &SignatureEntry,
) -> Result<Vec<u8>, Diagnostic> {
    let payload = signable_bytes(manifest_bytes)?;
    Ok(entry_preimage(&payload, entry))
}

fn entry_preimage(payload: &[u8], entry: &SignatureEntry) -> Vec<u8> {
    let metadata = serde_json::json!({
        "algorithm": entry.algorithm,
        "identity": entry.identity,
        "not_valid_after_unix_seconds": entry.not_valid_after_unix_seconds,
        "role": entry.role,
    });
    // `sorted_json` canonicalizes key order, and compact JSON escapes every
    // LF inside a string, so the metadata line holds no raw newline and the
    // domain line, metadata line, and payload stay unambiguous.
    let line = serde_json::to_string(&sorted_json(&metadata))
        .expect("a flat map of strings and an integer always serializes");
    let mut bytes = Vec::with_capacity(ENTRY_V2_DOMAIN.len() + line.len() + 1 + payload.len());
    bytes.extend_from_slice(ENTRY_V2_DOMAIN);
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(payload);
    bytes
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Decodes exactly `2 * N` lowercase hexadecimal characters into `N` bytes.
/// Uppercase, short, long, or non-hex input is rejected rather than
/// tolerated -- the same "strict lower hex" discipline
/// `super::is_sha256_wire_form` already applies to digests.
fn decode_lower_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; N];
    let bytes = text.as_bytes();
    for (index, slot) in out.iter_mut().enumerate() {
        let high = hex_nibble(bytes[index * 2])?;
        let low = hex_nibble(bytes[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    Some(out)
}

/// Cryptographically checks every signature in `capsule` against
/// `ctx.identity_public_keys` when that roster is non-empty; a no-op when it
/// is empty (see the module doc: empty is the strictly-backward-compatible
/// default). Called from [`super::check_signature_policy`] after its own
/// role/expiry/revocation checks.
pub(super) fn verify_against_roster(
    capsule: &ParsedCapsule,
    ctx: &SignaturePolicyContext,
    manifest_bytes: &[u8],
) -> Result<(), Diagnostic> {
    if ctx.identity_public_keys.is_empty() {
        return Ok(());
    }
    let payload = signable_bytes(manifest_bytes)?;
    for entry in &capsule.signatures {
        verify_one(entry, &ctx.identity_public_keys, &payload)?;
    }
    Ok(())
}

fn verify_one(
    entry: &SignatureEntry,
    roster: &BTreeMap<String, [u8; 32]>,
    payload: &[u8],
) -> Result<(), Diagnostic> {
    let Some(public_key_bytes) = roster.get(&entry.identity) else {
        return Err(signature_error(format!(
            "signature role `{}` identity `{}` is not in the trusted signer roster; an identity \
             absent from the roster cannot be cryptographically approved, so its signature -- \
             forged or not -- is rejected as unknown",
            entry.role, entry.identity
        )));
    };
    if entry.algorithm == ED25519_RAW_V1 {
        // Explicit compatibility boundary (issue #577): a legacy raw
        // signature covers only the capsule payload, never this entry's
        // role, identity, algorithm, or expiry -- exactly the fields strict
        // policy relies on. Treating it as authenticating them would let
        // anyone relabel, extend, or copy it, so strict mode refuses it.
        return Err(signature_error(format!(
            "signature role `{}` identity `{}` uses legacy algorithm `{ED25519_RAW_V1}`, whose \
             preimage does not cover the entry's role, identity, algorithm, or expiry; a strict \
             trust roster cannot accept those fields unsigned, so re-sign this entry as \
             `{ED25519_ENTRY_V2}`",
            entry.role, entry.identity
        )));
    }
    if entry.algorithm != ED25519_ENTRY_V2 {
        return Err(signature_error(format!(
            "signature role `{}` identity `{}` uses algorithm `{}`, which this module cannot \
             verify locally; only `{ED25519_ENTRY_V2}` has a local verifier, so this signature \
             is unverifiable rather than trusted",
            entry.role, entry.identity, entry.algorithm
        )));
    }
    let Some(signature_bytes) = decode_lower_hex::<64>(&entry.signature) else {
        return Err(signature_error(format!(
            "signature role `{}` identity `{}` is not 128 lowercase-hex characters encoding a \
             64-byte `{ED25519_ENTRY_V2}` signature; it is unverifiable",
            entry.role, entry.identity
        )));
    };
    let verifying_key = VerifyingKey::from_bytes(public_key_bytes).map_err(|_| {
        signature_error(format!(
            "signature role `{}` identity `{}`'s configured trust-roster public key is not a \
             valid Ed25519 verifying key; it is unverifiable",
            entry.role, entry.identity
        ))
    })?;
    verifying_key
        .verify_strict(
            &entry_preimage(payload, entry),
            &Signature::from_bytes(&signature_bytes),
        )
        .map_err(|_| {
            signature_error(format!(
                "signature role `{}` identity `{}` does not verify against this capsule's exact \
                 manifest bytes and this entry's own role, identity, algorithm, and expiry -- it \
                 was not produced over this entry of this capsule, whether forged from nothing, \
                 relabelled, or copied from a different entry or capsule",
                entry.role, entry.identity
            ))
        })
}

#[cfg(test)]
mod tests;
