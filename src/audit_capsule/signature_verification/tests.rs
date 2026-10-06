//! Reproduction and regression tests for the forged-signature gap
//! `signature_verification` closes: a structurally well-formed signature
//! naming an approved, unexpired, unrevoked identity must not be accepted
//! unless it is *actually* an Ed25519 signature over this exact capsule's
//! bytes, once the caller has opted in with a trust roster.
//!
//! `an_empty_trust_roster_preserves_the_legacy_policy_only_behavior` is the
//! reproduction case: it asserts today's (pre-fix, and still roster-off)
//! behavior accepts a garbage `signature` string outright, which is the
//! defect the issue named. Every other test here exercises the fix with the
//! roster populated.

use std::collections::BTreeMap;

use ed25519_dalek::{Signer as _, SigningKey};

use super::{entry_signable_bytes, signable_bytes, verify_against_roster};
use crate::audit_capsule::{
    nonclaims, render_capsule, sha256_digest, AssociationEdge, ObjectRef, Profile, SignatureEntry,
    SignaturePolicyContext, TransparencyEntry,
};

const OBJECT_A_BYTES: &[u8] = b"signature-verification fixture: program-root";
const OBJECT_B_BYTES: &[u8] = b"signature-verification fixture: semantic-transaction";
const OBJECT_C_BYTES: &[u8] = b"signature-verification fixture: assurance-manifest";
const OBJECT_D_BYTES: &[u8] = b"signature-verification fixture: source-projection";

/// A minimal, otherwise-well-formed `change` capsule carrying exactly
/// `signatures`, built through the public [`render_capsule`] API rather than
/// hand-written JSON so every fixture here stays valid if the wire format
/// ever changes shape.
fn minimal_signed_capsule(signatures: &[SignatureEntry]) -> Vec<u8> {
    let mut subject = BTreeMap::new();
    subject.insert("source_digest".to_owned(), "sha256:fixture".to_owned());
    subject.insert("root_digest".to_owned(), "sha256:fixture".to_owned());
    subject.insert("revision".to_owned(), "r1".to_owned());
    subject.insert("compiler_version".to_owned(), "0.0.0-fixture".to_owned());

    let objects = [
        ObjectRef {
            id: "obj-a-program-root".to_owned(),
            object_type: "program-root".to_owned(),
            schema: "semaprax.program-root.v3".to_owned(),
            digest: sha256_digest(OBJECT_A_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "obj-b-semantic-transaction".to_owned(),
            object_type: "semantic-transaction".to_owned(),
            schema: "semaprax.project-candidate-semantic-delta.v1".to_owned(),
            digest: sha256_digest(OBJECT_B_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "obj-c-assurance-manifest".to_owned(),
            object_type: "assurance-manifest".to_owned(),
            schema: "semaprax.assurance-manifest.v1".to_owned(),
            digest: sha256_digest(OBJECT_C_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "obj-d-source-projection".to_owned(),
            object_type: "source-projection".to_owned(),
            schema: "semaprax.program-root.v3".to_owned(),
            digest: sha256_digest(OBJECT_D_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
    ];
    let associations: [AssociationEdge; 0] = [];
    let nonclaims: Vec<String> = nonclaims::ALWAYS_REQUIRED_NONCLAIMS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    let transparency: Option<&TransparencyEntry> = None;

    render_capsule(
        Profile::Change,
        &subject,
        &objects,
        &associations,
        signatures,
        transparency,
        &nonclaims,
    )
    .expect("fixture capsule is well-formed")
}

fn signature(role: &str, identity: &str, algorithm: &str, signature: &str) -> SignatureEntry {
    SignatureEntry {
        role: role.to_owned(),
        identity: identity.to_owned(),
        algorithm: algorithm.to_owned(),
        signature: signature.to_owned(),
        not_valid_after_unix_seconds: 9_999_999_999,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ---------------------------------------------------------------------
// Reproduction: with no trust roster configured, a garbage `signature`
// naming an otherwise-approved identity is accepted -- this is the exact
// defect issue #209 names. It stays true after the fix (by design: an
// empty roster is the strictly-backward-compatible opt-out), so this test
// also documents the boundary of what strict mode actually changes.
// ---------------------------------------------------------------------

#[test]
fn an_empty_trust_roster_preserves_the_legacy_policy_only_behavior() {
    let entries = [signature(
        "publisher",
        "agent://alice",
        "ed25519-raw-v1",
        "not-a-real-signature-just-forged-text",
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");
    let ctx = SignaturePolicyContext::default();
    assert!(
        ctx.identity_public_keys.is_empty(),
        "the reproduction relies on the roster being empty by default"
    );
    verify_against_roster(&capsule, &ctx, &manifest)
        .expect("an empty roster must not reject a forged signature -- it never looks at one");
}

// ---------------------------------------------------------------------
// Strict mode: a real key pair, a real signature, and each of the four
// distinct failure conditions the issue's Definition of Done names.
// ---------------------------------------------------------------------

#[test]
fn a_genuine_signature_over_the_exact_capsule_bytes_verifies_against_the_roster() {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();

    // The signature is computed after the manifest exists, so it must be
    // over the manifest's *own* signable bytes -- chicken-and-egg, solved
    // the same way real capsule production would: render once with a
    // placeholder signature to learn the signable bytes, sign those bytes,
    // then render again with the real signature in place.
    let placeholder = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        &"0".repeat(128),
    )];
    let placeholder_manifest = minimal_signed_capsule(&placeholder);
    let bytes_to_sign = entry_signable_bytes(&placeholder_manifest, &placeholder[0])
        .expect("placeholder manifest is well-formed");
    let real_signature = signing_key.sign(&bytes_to_sign);

    let entries = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        &hex_encode(&real_signature.to_bytes()),
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), verifying_key.to_bytes());
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        ..SignaturePolicyContext::default()
    };
    verify_against_roster(&capsule, &ctx, &manifest)
        .expect("a genuine signature over this exact capsule's bytes must verify");
}

#[test]
fn a_forged_signature_naming_an_approved_identity_is_rejected_once_a_roster_is_active() {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();

    // Structurally perfect: correct length, correct hex, and an identity the
    // roster genuinely approves -- but never actually produced by the key.
    let forged = "ab".repeat(64);
    let entries = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        &forged,
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), verifying_key.to_bytes());
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        ..SignaturePolicyContext::default()
    };
    let error = verify_against_roster(&capsule, &ctx, &manifest).unwrap_err();
    assert_eq!(error.code, "SPX-Z905");
    assert!(
        error.message.contains("does not verify against"),
        "{}",
        error.message
    );
}

#[test]
fn a_signature_genuinely_valid_over_a_different_capsules_bytes_is_rejected() {
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();

    // Sign a *different* manifest (a different revision) for real, then
    // paste that genuine signature into the capsule under test with the
    // same role and identity. The bytes are authentic Ed25519 output --
    // just not over this capsule.
    let placeholder = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        &"0".repeat(128),
    )];
    let other_manifest = minimal_signed_capsule(&placeholder);
    // Tamper the *other* manifest's revision so its signable bytes differ
    // from the capsule under test while staying well-formed JSON.
    let other_manifest_text = String::from_utf8(other_manifest)
        .unwrap()
        .replacen("\"r1\"", "\"r2\"", 1);
    let signed_elsewhere = signing_key.sign(
        &entry_signable_bytes(other_manifest_text.as_bytes(), &placeholder[0])
            .expect("tampered manifest stays well-formed JSON"),
    );

    let entries = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        &hex_encode(&signed_elsewhere.to_bytes()),
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), verifying_key.to_bytes());
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        ..SignaturePolicyContext::default()
    };
    let error = verify_against_roster(&capsule, &ctx, &manifest).unwrap_err();
    assert_eq!(error.code, "SPX-Z905");
    assert!(
        error.message.contains("does not verify against"),
        "{}",
        error.message
    );
}

#[test]
fn an_identity_absent_from_the_roster_is_rejected_as_unknown_not_forged() {
    let entries = [signature(
        "publisher",
        "agent://mallory",
        "ed25519-entry-v2",
        &"c".repeat(128),
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    // The roster is non-empty (strict mode is on) but knows nobody named
    // "agent://mallory".
    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), [1u8; 32]);
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        ..SignaturePolicyContext::default()
    };
    let error = verify_against_roster(&capsule, &ctx, &manifest).unwrap_err();
    assert_eq!(error.code, "SPX-Z905");
    assert!(
        error.message.contains("not in the trusted signer roster"),
        "{}",
        error.message
    );
}

#[test]
fn an_algorithm_with_no_local_verifier_is_rejected_as_unverifiable_under_strict_mode() {
    let entries = [signature(
        "publisher",
        "agent://alice",
        "sigstore-cosign-bundle-v0.3",
        "irrelevant-opaque-bundle-text",
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), [1u8; 32]);
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        ..SignaturePolicyContext::default()
    };
    let error = verify_against_roster(&capsule, &ctx, &manifest).unwrap_err();
    assert_eq!(error.code, "SPX-Z905");
    assert!(
        error.message.contains("cannot verify locally"),
        "{}",
        error.message
    );
}

#[test]
fn a_malformed_signature_encoding_is_rejected_as_unverifiable_under_strict_mode() {
    let entries = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        "not-hex-at-all",
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), [1u8; 32]);
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        ..SignaturePolicyContext::default()
    };
    let error = verify_against_roster(&capsule, &ctx, &manifest).unwrap_err();
    assert_eq!(error.code, "SPX-Z905");
    assert!(error.message.contains("unverifiable"), "{}", error.message);
}

// ---------------------------------------------------------------------
// `check_signature_policy` end to end: strict mode composes with the
// pre-existing role/expiry/revocation checks rather than replacing them.
// ---------------------------------------------------------------------

#[test]
fn check_signature_policy_still_enforces_a_missing_required_role_before_any_crypto_check() {
    let entries = [signature(
        "publisher",
        "agent://alice",
        "ed25519-entry-v2",
        &"0".repeat(128),
    )];
    let manifest = minimal_signed_capsule(&entries);
    let capsule = crate::audit_capsule::parse_capsule(&manifest).expect("fixture parses");

    let mut roster = BTreeMap::new();
    roster.insert("agent://alice".to_owned(), [1u8; 32]);
    let ctx = SignaturePolicyContext {
        identity_public_keys: roster,
        required_roles: vec!["approver".to_owned()],
        ..SignaturePolicyContext::default()
    };
    let error =
        crate::audit_capsule::check_signature_policy(&capsule, &manifest, &ctx).unwrap_err();
    assert_eq!(error.code, "SPX-Z905");
    assert!(
        error.message.contains("no signature carries"),
        "{}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Issue #577 (DV-17): role, identity, algorithm, and expiry are policy
// inputs, so a strict roster must never accept them unsigned. The legacy
// `ed25519-raw-v1` preimage omitted the whole `signatures` array; these
// reproduce the published attack with exactly one genuine signature.
// ---------------------------------------------------------------------

const ALL_ROLES: [&str; 5] = ["proposer", "reviewer", "validator", "approver", "publisher"];

fn strict_ctx(identity: &str, key: [u8; 32], roles: &[&str], now: u64) -> SignaturePolicyContext {
    let mut roster = BTreeMap::new();
    roster.insert(identity.to_owned(), key);
    SignaturePolicyContext {
        identity_public_keys: roster,
        required_roles: roles.iter().map(|role| (*role).to_owned()).collect(),
        verification_time_unix_seconds: now,
        ..SignaturePolicyContext::default()
    }
}

fn policy(
    manifest: &[u8],
    ctx: &SignaturePolicyContext,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let capsule = crate::audit_capsule::parse_capsule(manifest).expect("fixture parses");
    crate::audit_capsule::check_signature_policy(&capsule, manifest, ctx)
}

/// One legacy raw signature over the metadata-free preimage, as the issue's
/// probe produced it: role `proposer`, expiry 100.
fn legacy_raw_signature(key: &SigningKey) -> String {
    let unsigned = minimal_signed_capsule(&[]);
    hex_encode(
        &key.sign(&signable_bytes(&unsigned).expect("well-formed"))
            .to_bytes(),
    )
}

fn entry(role: &str, identity: &str, algorithm: &str, sig: &str, expiry: u64) -> SignatureEntry {
    SignatureEntry {
        role: role.to_owned(),
        identity: identity.to_owned(),
        algorithm: algorithm.to_owned(),
        signature: sig.to_owned(),
        not_valid_after_unix_seconds: expiry,
    }
}

#[test]
fn a_legacy_raw_signature_relabelled_to_another_role_is_refused_under_a_strict_roster() {
    let key = SigningKey::from_bytes(&[11u8; 32]);
    let sig = legacy_raw_signature(&key);
    let public = key.verifying_key().to_bytes();
    let relabelled = minimal_signed_capsule(&[entry(
        "publisher",
        "local-test-key",
        "ed25519-raw-v1",
        &sig,
        100,
    )]);
    let error = policy(
        &relabelled,
        &strict_ctx("local-test-key", public, &["publisher"], 50),
    )
    .expect_err("a role the signer never signed must not satisfy a strict policy");
    assert_eq!(error.code, "SPX-Z905");
}

#[test]
fn a_legacy_raw_signature_with_an_extended_expiry_is_refused_under_a_strict_roster() {
    let key = SigningKey::from_bytes(&[11u8; 32]);
    let sig = legacy_raw_signature(&key);
    let public = key.verifying_key().to_bytes();
    let extended = minimal_signed_capsule(&[entry(
        "proposer",
        "local-test-key",
        "ed25519-raw-v1",
        &sig,
        u64::MAX,
    )]);
    let error = policy(
        &extended,
        &strict_ctx("local-test-key", public, &["proposer"], 101),
    )
    .expect_err("an expiry the signer never signed must not satisfy a strict policy");
    assert_eq!(error.code, "SPX-Z905");
}

#[test]
fn one_legacy_raw_signature_copied_into_all_five_roles_is_refused_under_a_strict_roster() {
    let key = SigningKey::from_bytes(&[11u8; 32]);
    let sig = legacy_raw_signature(&key);
    let public = key.verifying_key().to_bytes();
    let entries: Vec<SignatureEntry> = ALL_ROLES
        .iter()
        .map(|role| entry(role, "local-test-key", "ed25519-raw-v1", &sig, 100))
        .collect();
    let copied = minimal_signed_capsule(&entries);
    let error = policy(
        &copied,
        &strict_ctx("local-test-key", public, &ALL_ROLES, 50),
    )
    .expect_err("one signature must not count as five role decisions");
    assert_eq!(error.code, "SPX-Z905");
}

// ---------------------------------------------------------------------
// Issue #577: `ed25519-entry-v2` binds each entry's role, identity,
// algorithm, and expiry. Every mutation below keeps one genuine signature
// and the same roster; only metadata or covered payload changes.
// ---------------------------------------------------------------------

const V2: &str = "ed25519-entry-v2";

/// Independently signs one v2 entry over the fixture capsule. The preimage
/// excludes every entry's signature bytes, so it is the same whether the
/// capsule is rendered with or without the other entries.
fn signed_v2(key: &SigningKey, role: &str, identity: &str, expiry: u64) -> SignatureEntry {
    let mut unsigned = entry(role, identity, V2, "", expiry);
    let preimage =
        entry_signable_bytes(&minimal_signed_capsule(&[]), &unsigned).expect("well-formed");
    unsigned.signature = hex_encode(&key.sign(&preimage).to_bytes());
    unsigned
}

fn assert_unverified(manifest: &[u8], ctx: &SignaturePolicyContext) {
    let error = policy(manifest, ctx).expect_err("tampered metadata must not verify");
    assert_eq!(error.code, "SPX-Z905");
    assert!(
        error.message.contains("does not verify against"),
        "{}",
        error.message
    );
}

#[test]
fn independently_signed_distinct_roles_verify_under_a_strict_roster() {
    let alice = SigningKey::from_bytes(&[21u8; 32]);
    let bob = SigningKey::from_bytes(&[22u8; 32]);
    let manifest = minimal_signed_capsule(&[
        signed_v2(&alice, "proposer", "agent://alice", 100),
        signed_v2(&alice, "reviewer", "agent://alice", 100),
        signed_v2(&bob, "approver", "agent://bob", 100),
    ]);
    let mut ctx = strict_ctx(
        "agent://alice",
        alice.verifying_key().to_bytes(),
        &["proposer", "reviewer", "approver"],
        50,
    );
    ctx.identity_public_keys
        .insert("agent://bob".to_owned(), bob.verifying_key().to_bytes());
    policy(&manifest, &ctx).expect("genuine per-role signatures verify");
}

#[test]
fn a_signed_expiry_is_effective_exactly_at_its_boundary() {
    let key = SigningKey::from_bytes(&[23u8; 32]);
    let public = key.verifying_key().to_bytes();
    let manifest = minimal_signed_capsule(&[signed_v2(&key, "publisher", "k", 100)]);
    policy(&manifest, &strict_ctx("k", public, &["publisher"], 100))
        .expect("valid through its signed expiry instant");
    let error = policy(&manifest, &strict_ctx("k", public, &["publisher"], 101))
        .expect_err("expired one second later");
    assert!(error.message.contains("expired"), "{}", error.message);
}

#[test]
fn a_v2_signature_relabelled_to_another_role_does_not_verify() {
    let key = SigningKey::from_bytes(&[24u8; 32]);
    let public = key.verifying_key().to_bytes();
    let mut relabelled = signed_v2(&key, "proposer", "k", 100);
    relabelled.role = "publisher".to_owned();
    let manifest = minimal_signed_capsule(&[relabelled]);
    assert_unverified(&manifest, &strict_ctx("k", public, &["publisher"], 50));
}

#[test]
fn a_v2_signature_with_an_extended_expiry_does_not_verify() {
    let key = SigningKey::from_bytes(&[25u8; 32]);
    let public = key.verifying_key().to_bytes();
    let mut extended = signed_v2(&key, "proposer", "k", 100);
    extended.not_valid_after_unix_seconds = u64::MAX;
    let manifest = minimal_signed_capsule(&[extended]);
    assert_unverified(&manifest, &strict_ctx("k", public, &["proposer"], 101));
}

#[test]
fn one_v2_signature_copied_into_every_role_does_not_verify() {
    let key = SigningKey::from_bytes(&[26u8; 32]);
    let public = key.verifying_key().to_bytes();
    let original = signed_v2(&key, "proposer", "k", 100);
    let entries: Vec<SignatureEntry> = ALL_ROLES
        .iter()
        .map(|role| entry(role, "k", V2, &original.signature, 100))
        .collect();
    let manifest = minimal_signed_capsule(&entries);
    assert_unverified(&manifest, &strict_ctx("k", public, &ALL_ROLES, 50));
}

#[test]
fn a_v2_signature_moved_to_another_identity_with_the_same_key_does_not_verify() {
    // Both identities map to the same key, so key lookup alone would succeed;
    // only the signed identity field can tell them apart.
    let key = SigningKey::from_bytes(&[27u8; 32]);
    let public = key.verifying_key().to_bytes();
    let mut moved = signed_v2(&key, "publisher", "agent://alice", 100);
    moved.identity = "agent://bob".to_owned();
    let manifest = minimal_signed_capsule(&[moved]);
    let mut ctx = strict_ctx("agent://alice", public, &["publisher"], 50);
    ctx.identity_public_keys
        .insert("agent://bob".to_owned(), public);
    assert_unverified(&manifest, &ctx);
}

#[test]
fn a_v2_signature_over_an_altered_payload_does_not_verify() {
    let key = SigningKey::from_bytes(&[28u8; 32]);
    let public = key.verifying_key().to_bytes();
    let manifest = minimal_signed_capsule(&[signed_v2(&key, "publisher", "k", 100)]);
    let altered = String::from_utf8(manifest)
        .unwrap()
        .replacen("\"r1\"", "\"r2\"", 1);
    assert_unverified(
        altered.as_bytes(),
        &strict_ctx("k", public, &["publisher"], 50),
    );
}

#[test]
fn a_legacy_raw_signature_relabelled_as_v2_does_not_verify() {
    // Domain separation: a genuine signature over the bare legacy payload is
    // never a valid per-entry v2 signature.
    let key = SigningKey::from_bytes(&[11u8; 32]);
    let public = key.verifying_key().to_bytes();
    let sig = legacy_raw_signature(&key);
    let manifest = minimal_signed_capsule(&[entry("proposer", "k", V2, &sig, 100)]);
    assert_unverified(&manifest, &strict_ctx("k", public, &["proposer"], 50));
}

#[test]
fn a_legacy_raw_signature_is_refused_with_a_re_sign_instruction_under_a_strict_roster() {
    let key = SigningKey::from_bytes(&[11u8; 32]);
    let public = key.verifying_key().to_bytes();
    let sig = legacy_raw_signature(&key);
    let manifest = minimal_signed_capsule(&[entry("proposer", "k", "ed25519-raw-v1", &sig, 100)]);
    let error = policy(&manifest, &strict_ctx("k", public, &["proposer"], 50))
        .expect_err("even an unmodified legacy signature leaves its metadata unsigned");
    assert!(
        error
            .message
            .contains("re-sign this entry as `ed25519-entry-v2`"),
        "{}",
        error.message
    );
}
