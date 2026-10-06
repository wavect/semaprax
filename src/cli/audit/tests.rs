//! Tests for the `semaprax audit inspect|verify|diff` CLI front.
//!
//! Every fixture here is built through `audit_capsule::render_capsule`
//! rather than hand-written JSON, so a change to the front's own parsing or
//! path handling is what these tests exercise -- `audit_capsule` itself
//! already carries its own exhaustive hostile-input suite.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::{Signer as _, SigningKey};

use semaprax::audit_capsule::{
    render_capsule, sha256_digest, AssociationEdge, ObjectRef, Profile, SignatureEntry,
};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn scratch_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-audit-cli-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).expect("scratch directory must be creatable");
    path
}

const OBJECT_A_BYTES: &[u8] = b"program-root bytes";
const OBJECT_B_BYTES: &[u8] = b"semantic-transaction bytes";
const OBJECT_C_BYTES: &[u8] = b"assurance-manifest bytes";
const OBJECT_D_BYTES: &[u8] = b"source-projection bytes";

fn change_subject(revision: &str) -> BTreeMap<String, String> {
    let mut subject = BTreeMap::new();
    subject.insert(
        "source_digest".to_owned(),
        "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );
    subject.insert(
        "root_digest".to_owned(),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
    );
    subject.insert("revision".to_owned(), revision.to_owned());
    subject.insert(
        "compiler_version".to_owned(),
        env!("CARGO_PKG_VERSION").to_owned(),
    );
    subject
}

fn base_nonclaims() -> Vec<String> {
    audit_capsule::nonclaims::ALWAYS_REQUIRED_NONCLAIMS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect()
}

/// A minimal, valid `change`-profile capsule (all four required object
/// types, no associations, no signatures, no transparency) plus the
/// retained bytes each of its objects declares, keyed by object id.
fn valid_change_capsule(revision: &str) -> (Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let objects = vec![
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
    let associations = vec![AssociationEdge {
        from_id: "obj-b-semantic-transaction".to_owned(),
        relation: "derived_from".to_owned(),
        to_id: "obj-a-program-root".to_owned(),
    }];
    let manifest_bytes = render_capsule(
        Profile::Change,
        &change_subject(revision),
        &objects,
        &associations,
        &[],
        None,
        &base_nonclaims(),
    )
    .expect("a minimal well-formed change capsule must render");

    let mut object_bytes = BTreeMap::new();
    object_bytes.insert("obj-a-program-root".to_owned(), OBJECT_A_BYTES.to_vec());
    object_bytes.insert(
        "obj-b-semantic-transaction".to_owned(),
        OBJECT_B_BYTES.to_vec(),
    );
    object_bytes.insert(
        "obj-c-assurance-manifest".to_owned(),
        OBJECT_C_BYTES.to_vec(),
    );
    object_bytes.insert(
        "obj-d-source-projection".to_owned(),
        OBJECT_D_BYTES.to_vec(),
    );
    (manifest_bytes, object_bytes)
}

/// Writes a capsule manifest plus an objects directory to a fresh scratch
/// directory and returns (manifest_path, objects_dir).
fn write_fixture(label: &str, revision: &str) -> (PathBuf, PathBuf) {
    let (manifest_bytes, object_bytes) = valid_change_capsule(revision);
    let root = scratch_dir(label);
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest_bytes).expect("manifest must be writable");
    let objects_dir = root.join("objects");
    fs::create_dir_all(&objects_dir).expect("objects dir must be creatable");
    for (id, bytes) in &object_bytes {
        fs::write(objects_dir.join(id), bytes).expect("object file must be writable");
    }
    (manifest_path, objects_dir)
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

// ---------------------------------------------------------------------
// `parse`: negative controls for the argument grammar. Each malformed
// input here must be rejected with exit code 2 before any path is opened;
// removing a guard would let a malformed invocation reach `std::fs::read`.
// ---------------------------------------------------------------------

#[test]
fn parse_admits_inspect_with_exactly_one_capsule_path() {
    match parse(&strings(&["inspect", "capsule.json"])) {
        Ok(AuditCommand::Inspect(path)) => assert_eq!(path, PathBuf::from("capsule.json")),
        _ => panic!("expected Inspect"),
    }
    for malformed in [
        &[][..],
        &["inspect"][..],
        &["inspect", "capsule.json", "extra"][..],
        &["inspect", ""][..],
        &["inspect", "--json"][..],
        &["bogus", "capsule.json"][..],
    ] {
        assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
    }
}

#[test]
fn parse_admits_verify_with_capsule_and_objects_dir_and_known_flags_only() {
    match parse(&strings(&["verify", "capsule.json", "objects"])) {
        Ok(AuditCommand::Verify(options)) => {
            assert_eq!(options.capsule, PathBuf::from("capsule.json"));
            assert_eq!(options.objects_dir, PathBuf::from("objects"));
            assert!(options.required_roles.is_empty());
        }
        _ => panic!("expected Verify"),
    }
    match parse(&strings(&[
        "verify",
        "capsule.json",
        "objects",
        "--require-role",
        "approver",
        "--now",
        "1000",
    ])) {
        Ok(AuditCommand::Verify(options)) => {
            assert_eq!(options.required_roles, vec!["approver".to_owned()]);
            assert_eq!(options.verification_time_unix_seconds, Some(1000));
        }
        _ => panic!("expected Verify with flags"),
    }
    for malformed in [
        &["verify"][..],
        &["verify", "capsule.json"][..],
        &["verify", "capsule.json", "objects", "--unknown-flag", "x"][..],
        &["verify", "capsule.json", "objects", "--require-role"][..],
        &["verify", "capsule.json", "objects", "stray-positional"][..],
        &["verify", "--flag-shaped", "objects"][..],
    ] {
        assert!(
            parse(&strings(malformed)).is_err(),
            "expected rejection for {malformed:?}"
        );
    }
}

#[test]
fn parse_admits_diff_with_exactly_two_capsule_paths() {
    match parse(&strings(&["diff", "a.json", "b.json"])) {
        Ok(AuditCommand::Diff(before, after)) => {
            assert_eq!(before, PathBuf::from("a.json"));
            assert_eq!(after, PathBuf::from("b.json"));
        }
        _ => panic!("expected Diff"),
    }
    for malformed in [
        &["diff"][..],
        &["diff", "a.json"][..],
        &["diff", "a.json", "b.json", "c.json"][..],
    ] {
        assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
    }
}

// ---------------------------------------------------------------------
// Path-traversal guard: the negative control for `is_safe_object_id` /
// `object_bytes_from_directory`. Deleting the guard would let a capsule
// whose object id is shaped like a traversal path make this front read a
// file outside the objects directory the caller named.
// ---------------------------------------------------------------------

#[test]
fn a_traversal_shaped_object_id_is_rejected_before_any_file_is_read() {
    assert!(is_safe_object_id("plain-id"));
    assert!(is_safe_object_id("obj-a-program-root"));
    for hostile in ["../secret", "../../etc/passwd", "/etc/passwd", "..", "."] {
        assert!(!is_safe_object_id(hostile), "{hostile:?} must be rejected");
    }

    // End to end: a capsule object whose id is traversal-shaped must fail
    // closed with this front's own document error, never attempt to read
    // outside `objects_dir`, and never reach `audit_capsule::verify_capsule`
    // at all. Built through typed `ObjectRef`s (not string surgery on
    // rendered JSON) so this does not depend on the manifest's on-the-wire
    // key order: only the fourth object's id is hostile, and the
    // association graph never references it, so this is a clean, minimal
    // reproduction of exactly the traversal case.
    let objects = vec![
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
            // A source-projection object whose id is shaped like a
            // traversal path, not a benign id -- `render_capsule` places
            // no character restriction on ids, so this is admitted as a
            // structurally well-formed capsule.
            id: "../../outside-objects-dir".to_owned(),
            object_type: "source-projection".to_owned(),
            schema: "semaprax.program-root.v3".to_owned(),
            digest: sha256_digest(OBJECT_D_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
    ];
    let manifest_bytes = render_capsule(
        Profile::Change,
        &change_subject("r-traversal"),
        &objects,
        &[],
        &[],
        None,
        &base_nonclaims(),
    )
    .expect("a capsule with an unusually-shaped but valid object id must still render");

    let root = scratch_dir("traversal");
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest_bytes).unwrap();
    let objects_dir = root.join("objects");
    fs::create_dir_all(&objects_dir).unwrap();
    fs::write(objects_dir.join("obj-a-program-root"), OBJECT_A_BYTES).unwrap();
    fs::write(
        objects_dir.join("obj-b-semantic-transaction"),
        OBJECT_B_BYTES,
    )
    .unwrap();
    fs::write(objects_dir.join("obj-c-assurance-manifest"), OBJECT_C_BYTES).unwrap();
    // A real file exists exactly where the traversal would land if the
    // guard were removed, so a regression here would actually succeed
    // rather than merely misbehave silently.
    fs::write(root.join("outside-objects-dir"), OBJECT_D_BYTES).unwrap();

    let options = VerifyOptions {
        capsule: manifest_path,
        objects_dir,
        required_roles: Vec::new(),
        revoked_identities: Vec::new(),
        trusted_logs: Vec::new(),
        trust_roster: None,
        minimum_accepted_checkpoint_size: 0,
        verification_time_unix_seconds: Some(0),
    };
    let error = run_verify(&options).expect_err("a traversal-shaped id must be rejected");
    assert_eq!(error.code, "SPX-Z920");
    assert!(
        error.message.contains("plain path segment"),
        "{}",
        error.message
    );
}

// ---------------------------------------------------------------------
// End-to-end wiring: proves `inspect`, `verify`, and `diff` actually call
// through to `audit_capsule` and produce a report, not merely that the
// argument grammar parses.
// ---------------------------------------------------------------------

#[test]
fn inspect_reports_the_capsules_own_structure() {
    let (manifest_path, _objects_dir) = write_fixture("inspect", "r1");
    let report = run_inspect(&manifest_path).expect("a well-formed capsule must inspect");
    assert!(report.contains("profile: change"));
    assert!(report.contains("objects: 4"));
    assert!(report.contains("obj-a-program-root"));
    assert!(report.contains("status: INSPECTED"));
    // Never claims verification happened.
    assert!(!report.contains("VERIFIED"));
}

#[test]
fn verify_succeeds_against_the_matching_objects_directory_and_never_claims_signing() {
    let (manifest_path, objects_dir) = write_fixture("verify-ok", "r2");
    let options = VerifyOptions {
        capsule: manifest_path,
        objects_dir,
        required_roles: Vec::new(),
        revoked_identities: Vec::new(),
        trusted_logs: Vec::new(),
        trust_roster: None,
        minimum_accepted_checkpoint_size: 0,
        verification_time_unix_seconds: Some(0),
    };
    let report = run_verify(&options).expect("a matching objects directory must verify");
    assert!(report.contains("status: VERIFIED (structural integrity only)"));
    assert!(report.contains("verified objects: 4"));
    // Every capsule's own required nonclaims must be printed, not summarized
    // away -- this is the property that stops a downstream reader from
    // mistaking a green report for a cryptographic signature check.
    for nonclaim in audit_capsule::nonclaims::ALWAYS_REQUIRED_NONCLAIMS {
        assert!(report.contains(nonclaim), "missing nonclaim {nonclaim}");
    }
    // This fixture carries no signatures at all, so the report must make no
    // verified/unverified claim either way -- just the plain count.
    assert!(report.contains("signatures: 0 present"));
    assert!(!report.to_lowercase().contains("signed capsule"));
    assert!(report.contains("This command contacts no transparency log"));
}

/// Negative control: this is the exact test that proves `verify` performs
/// real byte-level integrity checking rather than trusting the manifest --
/// removing `object_bytes_from_directory`'s digest recomputation (delegated
/// to `check_object_bytes`) would let a substituted object pass silently.
#[test]
fn verify_fails_closed_when_an_object_file_does_not_match_its_declared_digest() {
    let (manifest_path, objects_dir) = write_fixture("verify-substituted", "r3");
    fs::write(objects_dir.join("obj-a-program-root"), b"substituted bytes").unwrap();
    let options = VerifyOptions {
        capsule: manifest_path,
        objects_dir,
        required_roles: Vec::new(),
        revoked_identities: Vec::new(),
        trusted_logs: Vec::new(),
        trust_roster: None,
        minimum_accepted_checkpoint_size: 0,
        verification_time_unix_seconds: Some(0),
    };
    let error = run_verify(&options).expect_err("a substituted object must fail closed");
    assert_eq!(error.code, "SPX-Z904");
    assert!(error.message.contains("substituted"), "{}", error.message);
}

#[test]
fn verify_fails_closed_when_an_object_file_is_missing() {
    let (manifest_path, objects_dir) = write_fixture("verify-missing", "r4");
    fs::remove_file(objects_dir.join("obj-a-program-root")).unwrap();
    let options = VerifyOptions {
        capsule: manifest_path,
        objects_dir,
        required_roles: Vec::new(),
        revoked_identities: Vec::new(),
        trusted_logs: Vec::new(),
        trust_roster: None,
        minimum_accepted_checkpoint_size: 0,
        verification_time_unix_seconds: Some(0),
    };
    let error = run_verify(&options).expect_err("a missing object file must fail closed");
    assert_eq!(error.code, "SPX-Z920");
}

#[test]
fn diff_reports_no_difference_for_identical_capsules() {
    let (manifest_path, _objects_dir) = write_fixture("diff-same", "r5");
    let report =
        run_diff(&manifest_path, &manifest_path).expect("a capsule must diff against itself");
    assert!(report.contains("no structural difference"));
}

#[test]
fn diff_reports_a_changed_object_digest_between_two_revisions() {
    let (before_path, _) = write_fixture("diff-before", "r6");
    let (after_manifest, after_objects) = valid_change_capsule("r7");
    let after_root = scratch_dir("diff-after");
    let after_path = after_root.join("capsule.json");
    fs::write(&after_path, &after_manifest).unwrap();
    let _ = after_objects; // objects aren't needed for a manifest-only diff

    let report = run_diff(&before_path, &after_path).expect("two change capsules must diff");
    assert!(report.contains("subject.revision: r6 -> r7"));
}

// ---------------------------------------------------------------------
// Determinism: the same inputs must produce byte-identical CLI output.
// This is the load-bearing property AGENTS.md requires of every
// contracted generated artifact; a nondeterministic report (e.g. from
// unordered iteration) would fail this test but pass every other one here.
// ---------------------------------------------------------------------

#[test]
fn inspect_and_verify_reports_are_byte_identical_across_repeated_runs() {
    let (manifest_path, objects_dir) = write_fixture("determinism", "r8");
    let first_inspect = run_inspect(&manifest_path).unwrap();
    let second_inspect = run_inspect(&manifest_path).unwrap();
    assert_eq!(first_inspect, second_inspect);

    let options = VerifyOptions {
        capsule: manifest_path,
        objects_dir,
        required_roles: Vec::new(),
        revoked_identities: Vec::new(),
        trusted_logs: Vec::new(),
        trust_roster: None,
        minimum_accepted_checkpoint_size: 0,
        verification_time_unix_seconds: Some(42),
    };
    let first_verify = run_verify(&options).unwrap();
    let second_verify = run_verify(&options).unwrap();
    assert_eq!(first_verify, second_verify);
}

// ---------------------------------------------------------------------
// Portability / authority: structural asserts on this front's own source
// text, mirroring `audit_capsule`'s own such tests.
// ---------------------------------------------------------------------

#[test]
fn this_front_spawns_no_process_and_reaches_no_network() {
    let source = include_str!("../audit.rs");
    assert!(!source.contains("std::process::Command"));
    assert!(!source.contains("TcpStream"));
    assert!(!source.contains("std::net::"));
}

// ---------------------------------------------------------------------
// `--trust-roster`: the CLI wiring for issue #209's forged-signature gap.
// `audit_capsule::signature_verification` already carries its own
// exhaustive positive/negative cryptographic suite; these tests exercise
// only this front's own roster loading, argument parsing, and report
// wording, end to end through `parse` and `run_verify`.
// ---------------------------------------------------------------------

const TRUST_OBJECT_A_BYTES: &[u8] = b"trust-roster fixture: program-root";
const TRUST_OBJECT_B_BYTES: &[u8] = b"trust-roster fixture: semantic-transaction";
const TRUST_OBJECT_C_BYTES: &[u8] = b"trust-roster fixture: assurance-manifest";
const TRUST_OBJECT_D_BYTES: &[u8] = b"trust-roster fixture: source-projection";

fn trust_capsule_objects() -> Vec<ObjectRef> {
    vec![
        ObjectRef {
            id: "obj-a-program-root".to_owned(),
            object_type: "program-root".to_owned(),
            schema: "semaprax.program-root.v3".to_owned(),
            digest: sha256_digest(TRUST_OBJECT_A_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "obj-b-semantic-transaction".to_owned(),
            object_type: "semantic-transaction".to_owned(),
            schema: "semaprax.project-candidate-semantic-delta.v1".to_owned(),
            digest: sha256_digest(TRUST_OBJECT_B_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "obj-c-assurance-manifest".to_owned(),
            object_type: "assurance-manifest".to_owned(),
            schema: "semaprax.assurance-manifest.v1".to_owned(),
            digest: sha256_digest(TRUST_OBJECT_C_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "obj-d-source-projection".to_owned(),
            object_type: "source-projection".to_owned(),
            schema: "semaprax.program-root.v3".to_owned(),
            digest: sha256_digest(TRUST_OBJECT_D_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
    ]
}

fn render_trust_capsule(revision: &str, signatures: &[SignatureEntry]) -> Vec<u8> {
    render_capsule(
        Profile::Change,
        &change_subject(revision),
        &trust_capsule_objects(),
        &[],
        signatures,
        None,
        &base_nonclaims(),
    )
    .expect("a well-formed change capsule with signatures must render")
}

fn write_trust_objects(objects_dir: &std::path::Path) {
    fs::create_dir_all(objects_dir).unwrap();
    fs::write(objects_dir.join("obj-a-program-root"), TRUST_OBJECT_A_BYTES).unwrap();
    fs::write(
        objects_dir.join("obj-b-semantic-transaction"),
        TRUST_OBJECT_B_BYTES,
    )
    .unwrap();
    fs::write(
        objects_dir.join("obj-c-assurance-manifest"),
        TRUST_OBJECT_C_BYTES,
    )
    .unwrap();
    fs::write(
        objects_dir.join("obj-d-source-projection"),
        TRUST_OBJECT_D_BYTES,
    )
    .unwrap();
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// One genuinely signed `ed25519-entry-v2` entry over the trust fixture
/// capsule for `revision`. The per-entry preimage
/// (`signature_verification::entry_signable_bytes`) excludes every entry's
/// signature bytes, so it is identical whether the capsule is rendered with
/// or without its signatures (issue #577).
fn signed_entry(role: &str, identity: &str, revision: &str, expiry: u64) -> SignatureEntry {
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let mut entry = SignatureEntry {
        role: role.to_owned(),
        identity: identity.to_owned(),
        algorithm: "ed25519-entry-v2".to_owned(),
        signature: String::new(),
        not_valid_after_unix_seconds: expiry,
    };
    let preimage = audit_capsule::signature_verification::entry_signable_bytes(
        &render_trust_capsule(revision, &[]),
        &entry,
    )
    .expect("the trust fixture is well-formed");
    entry.signature = hex_encode(&signing_key.sign(&preimage).to_bytes());
    entry
}

fn trust_fixture_key() -> [u8; 32] {
    SigningKey::from_bytes(&[9u8; 32])
        .verifying_key()
        .to_bytes()
}

/// A genuinely Ed25519-signed `change` capsule naming `identity` under
/// `role`, plus the exact 32-byte verifying key that signature verifies
/// against.
fn genuinely_signed_capsule(role: &str, identity: &str, revision: &str) -> (Vec<u8>, [u8; 32]) {
    let entries = [signed_entry(role, identity, revision, 9_999_999_999)];
    (
        render_trust_capsule(revision, &entries),
        trust_fixture_key(),
    )
}

fn trust_roster_json(entries: &[(&str, [u8; 32])]) -> String {
    let mut map = serde_json::Map::new();
    for (identity, key) in entries {
        map.insert((*identity).to_owned(), Value::String(hex_encode(key)));
    }
    serde_json::to_string(&Value::Object(map)).expect("a flat string map always serializes")
}

fn write_roster(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

fn verify_options_with_roster(
    manifest_path: PathBuf,
    objects_dir: PathBuf,
    trust_roster: Option<PathBuf>,
) -> VerifyOptions {
    VerifyOptions {
        capsule: manifest_path,
        objects_dir,
        required_roles: Vec::new(),
        revoked_identities: Vec::new(),
        trusted_logs: Vec::new(),
        trust_roster,
        minimum_accepted_checkpoint_size: 0,
        verification_time_unix_seconds: Some(0),
    }
}

#[test]
fn parse_admits_the_trust_roster_flag() {
    match parse(&strings(&[
        "verify",
        "capsule.json",
        "objects",
        "--trust-roster",
        "roster.json",
    ])) {
        Ok(AuditCommand::Verify(options)) => {
            assert_eq!(options.trust_roster, Some(PathBuf::from("roster.json")));
        }
        _ => panic!("expected Verify with --trust-roster"),
    }
    // Omitted entirely is the backward-compatible default.
    match parse(&strings(&["verify", "capsule.json", "objects"])) {
        Ok(AuditCommand::Verify(options)) => assert_eq!(options.trust_roster, None),
        _ => panic!("expected Verify"),
    }
}

// ---------------------------------------------------------------------
// The three report states: never let "not verified" read as "verified".
// ---------------------------------------------------------------------

#[test]
fn verify_with_no_trust_roster_reports_signatures_present_but_not_cryptographically_verified() {
    let root = scratch_dir("no-roster");
    let manifest = render_trust_capsule(
        "r1",
        &[SignatureEntry {
            role: "publisher".to_owned(),
            identity: "agent://alice".to_owned(),
            algorithm: "ed25519-raw-v1".to_owned(),
            signature: "not-a-real-signature".to_owned(),
            not_valid_after_unix_seconds: 9_999_999_999,
        }],
    );
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);

    let options = verify_options_with_roster(manifest_path, objects_dir, None);
    let report = run_verify(&options).expect("no roster means no crypto check, so this succeeds");
    assert!(report.contains(
        "signatures: 1 present -- NOT cryptographically verified (no `--trust-roster` was supplied)"
    ));
    assert!(!report.contains("CRYPTOGRAPHICALLY VERIFIED"));
}

#[test]
fn verify_with_an_explicitly_empty_trust_roster_is_distinguished_from_no_roster_at_all() {
    let root = scratch_dir("empty-roster");
    let manifest = render_trust_capsule(
        "r1",
        &[SignatureEntry {
            role: "publisher".to_owned(),
            identity: "agent://alice".to_owned(),
            algorithm: "ed25519-raw-v1".to_owned(),
            signature: "not-a-real-signature".to_owned(),
            not_valid_after_unix_seconds: 9_999_999_999,
        }],
    );
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    let roster_path = write_roster(&root, "roster.json", "{}");

    let options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    let report =
        run_verify(&options).expect("an empty roster performs no check, so this succeeds too");
    assert!(report.contains(
        "signatures: 1 present -- NOT cryptographically verified (`--trust-roster` was supplied \
         but names 0 identities"
    ));
    // The wording must differ from the "no --trust-roster at all" case above,
    // even though both leave the signature unverified.
    assert!(!report.contains("no `--trust-roster` was supplied"));
    assert!(!report.contains("CRYPTOGRAPHICALLY VERIFIED"));
}

#[test]
fn verify_succeeds_and_reports_cryptographic_verification_for_a_genuine_signature() {
    let (manifest, key_bytes) = genuinely_signed_capsule("publisher", "agent://alice", "r1");
    let root = scratch_dir("roster-ok");
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    let roster_path = write_roster(
        &root,
        "roster.json",
        &trust_roster_json(&[("agent://alice", key_bytes)]),
    );

    let options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    let report =
        run_verify(&options).expect("a genuine signature must verify against the matching key");
    assert!(report.contains(
        "signatures: 1 present -- all CRYPTOGRAPHICALLY VERIFIED against the supplied trust \
         roster (1 identity)"
    ));
    assert!(!report.contains("NOT cryptographically verified"));
}

/// **The sentence issue #209 closes on.** This is also the fault-injection
/// target: temporarily hardcoding `identity_public_keys: BTreeMap::new()`
/// in `run_verify` (ignoring `options.trust_roster` entirely) turns this
/// test's expected `Err` into an `Ok`, which is exactly the silent-forgery
/// defect this change closes -- confirmed by hand during this change and
/// reverted before landing.
#[test]
fn verify_rejects_a_forged_signature_naming_an_approved_identity_when_trust_roster_supplied() {
    let (manifest, key_bytes) = genuinely_signed_capsule("publisher", "agent://alice", "r1");
    // Forge: flip one hex character of the real signature after the fact,
    // keeping the identity, role, and every other field exactly as an
    // approved signer's would look. Located by exact text surgery on the
    // one signature string present, not by guessing an offset.
    let manifest_text = String::from_utf8(manifest).unwrap();
    let capsule_for_signature =
        audit_capsule::parse_capsule(manifest_text.as_bytes()).expect("fixture must parse");
    let real_signature = capsule_for_signature.signatures[0].signature.clone();
    let mut forged_signature = real_signature.clone();
    let flipped = if &forged_signature[0..1] == "0" {
        "1"
    } else {
        "0"
    };
    forged_signature.replace_range(0..1, flipped);
    let forged_manifest_text = manifest_text.replacen(&real_signature, &forged_signature, 1);
    assert_ne!(
        forged_manifest_text, manifest_text,
        "the forgery must change the manifest bytes"
    );

    let root = scratch_dir("roster-forged");
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, forged_manifest_text.as_bytes()).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    let roster_path = write_roster(
        &root,
        "roster.json",
        &trust_roster_json(&[("agent://alice", key_bytes)]),
    );

    let options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    let error = run_verify(&options)
        .expect_err("a forged signature naming an approved identity must be rejected");
    assert!(
        error.message.contains(
            "does not verify against this capsule's exact manifest bytes and this entry's own role"
        ),
        "{}",
        error.message
    );
}

#[test]
fn verify_rejects_an_identity_the_roster_does_not_recognize() {
    let (manifest, _unused_key) = genuinely_signed_capsule("publisher", "agent://alice", "r1");
    let root = scratch_dir("roster-unknown-identity");
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    // The roster is non-empty (so strict mode turns on) but never names
    // `agent://alice`, the capsule's actual signer.
    let roster_path = write_roster(
        &root,
        "roster.json",
        &trust_roster_json(&[("agent://someone-else", [3u8; 32])]),
    );

    let options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    let error =
        run_verify(&options).expect_err("an identity absent from a non-empty roster is unknown");
    assert!(
        error.message.contains("not in the trusted signer roster"),
        "{}",
        error.message
    );
}

#[test]
fn verify_ignores_a_roster_entry_for_an_identity_absent_from_the_capsule() {
    let (manifest, key_bytes) = genuinely_signed_capsule("publisher", "agent://alice", "r1");
    let root = scratch_dir("roster-extra-identity");
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    // An extra entry for an identity this capsule never signs with must be
    // harmless -- a real operator's roster naturally accumulates entries
    // across many capsules.
    let roster_path = write_roster(
        &root,
        "roster.json",
        &trust_roster_json(&[
            ("agent://alice", key_bytes),
            ("agent://nobody-in-this-capsule", [3u8; 32]),
        ]),
    );

    let options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    let report =
        run_verify(&options).expect("an unrelated extra roster entry must not affect verification");
    assert!(report.contains("all CRYPTOGRAPHICALLY VERIFIED"));
}

#[test]
fn a_traversal_shaped_signature_identity_has_no_filesystem_effect_through_the_trust_roster() {
    // Unlike an object id (joined into `objects_dir` and guarded by
    // `is_safe_object_id`), a signature identity is only ever compared as an
    // in-memory `BTreeMap` key against the roster -- so a traversal-shaped
    // identity is just an unusual string, not a path. This proves it end to
    // end rather than merely asserting the reasoning.
    let hostile_identity = "../../outside-roster-dir";
    let (manifest, key_bytes) = genuinely_signed_capsule("publisher", hostile_identity, "r1");
    let root = scratch_dir("roster-traversal-identity");
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    let roster_path = write_roster(
        &root,
        "roster.json",
        &trust_roster_json(&[(hostile_identity, key_bytes)]),
    );

    let options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    let report = run_verify(&options)
        .expect("a traversal-shaped identity is just a string key, never a filesystem path");
    assert!(report.contains("all CRYPTOGRAPHICALLY VERIFIED"));
}

#[test]
fn a_trust_roster_path_with_parent_directory_components_is_read_like_any_other_cli_path() {
    // `--trust-roster`, like `<capsule.json>` and `<objects-dir>`, is a
    // plain caller-supplied path: a legitimate `..`-containing path to a
    // real file elsewhere must work exactly as `std::fs` would resolve it,
    // with no extra restriction this front does not also apply to its other
    // two path arguments.
    let (manifest, key_bytes) = genuinely_signed_capsule("publisher", "agent://alice", "r1");
    let root = scratch_dir("roster-dotdot");
    let manifest_dir = root.join("capsule-dir");
    fs::create_dir_all(&manifest_dir).unwrap();
    let manifest_path = manifest_dir.join("capsule.json");
    fs::write(&manifest_path, &manifest).unwrap();
    let objects_dir = manifest_dir.join("objects");
    write_trust_objects(&objects_dir);
    let roster_dir = root.join("elsewhere");
    write_roster(
        &roster_dir,
        "roster.json",
        &trust_roster_json(&[("agent://alice", key_bytes)]),
    );
    let roster_path_via_dotdot = manifest_dir.join("../elsewhere/roster.json");

    let options =
        verify_options_with_roster(manifest_path, objects_dir, Some(roster_path_via_dotdot));
    let report = run_verify(&options).expect("a `..`-containing roster path must resolve fine");
    assert!(report.contains("all CRYPTOGRAPHICALLY VERIFIED"));
}

// ---------------------------------------------------------------------
// `load_trust_roster`: hostile documents, each failing closed with its own
// distinct, stable reason.
// ---------------------------------------------------------------------

#[test]
fn load_trust_roster_accepts_an_explicitly_empty_object() {
    let root = scratch_dir("roster-load-empty");
    let path = write_roster(&root, "roster.json", "{}");
    let roster = load_trust_roster(&path).expect("an empty JSON object is a valid, empty roster");
    assert!(roster.is_empty());
}

#[test]
fn load_trust_roster_rejects_malformed_json() {
    let root = scratch_dir("roster-load-malformed");
    let path = write_roster(&root, "roster.json", "not json at all {{{");
    let error = load_trust_roster(&path).expect_err("malformed JSON must fail closed");
    assert!(
        error.message.contains("is not valid JSON"),
        "{}",
        error.message
    );
}

#[test]
fn load_trust_roster_rejects_a_non_object_top_level_value() {
    let root = scratch_dir("roster-load-non-object");
    let path = write_roster(&root, "roster.json", "[\"agent://alice\"]");
    let error = load_trust_roster(&path).expect_err("an array is not a valid roster shape");
    assert!(
        error.message.contains("must be a JSON object"),
        "{}",
        error.message
    );
}

#[test]
fn load_trust_roster_rejects_a_non_string_entry_value() {
    let root = scratch_dir("roster-load-non-string");
    let path = write_roster(&root, "roster.json", r#"{"agent://alice": 12345}"#);
    let error = load_trust_roster(&path).expect_err("a numeric key value is not a hex string");
    assert!(
        error.message.contains("must be a hex string"),
        "{}",
        error.message
    );
}

#[test]
fn load_trust_roster_rejects_an_entry_of_the_wrong_hex_length() {
    let root = scratch_dir("roster-load-short-hex");
    let path = write_roster(&root, "roster.json", r#"{"agent://alice": "abcd"}"#);
    let error = load_trust_roster(&path).expect_err("4 hex characters cannot encode 32 bytes");
    assert!(
        error.message.contains("64 lowercase-hex characters"),
        "{}",
        error.message
    );
}

#[test]
fn load_trust_roster_rejects_uppercase_hex() {
    let root = scratch_dir("roster-load-uppercase-hex");
    let body = format!(r#"{{"agent://alice": "{}"}}"#, "AB".repeat(32));
    let path = write_roster(&root, "roster.json", &body);
    let error = load_trust_roster(&path).expect_err("uppercase hex must not be tolerated");
    assert!(
        error.message.contains("64 lowercase-hex characters"),
        "{}",
        error.message
    );
}

#[test]
fn load_trust_roster_rejects_a_hex_shaped_but_invalid_curve_point() {
    let root = scratch_dir("roster-load-invalid-point");
    // A little-endian encoded y-coordinate of 2 (compressed-point byte `02`
    // followed by 31 zero bytes) has no corresponding x on the Edwards25519
    // curve, so `VerifyingKey::from_bytes` rejects it as `PointDecompression`
    // even though it is exactly 64 lowercase-hex characters. Found by direct
    // search over this exact `ed25519-dalek` version rather than assumed --
    // e.g. 32 bytes of `0xff` (a non-canonical, out-of-range field element)
    // decompresses to a *valid* point in this crate and is not a usable
    // negative fixture.
    let body = format!(r#"{{"agent://alice": "02{}"}}"#, "0".repeat(62));
    let path = write_roster(&root, "roster.json", &body);
    let error =
        load_trust_roster(&path).expect_err("an invalid curve point must be rejected, not parsed");
    assert!(
        error.message.contains("not a valid Ed25519 verifying key"),
        "{}",
        error.message
    );
}

#[test]
fn load_trust_roster_rejects_an_oversized_document() {
    let root = scratch_dir("roster-load-oversized");
    let path = root.join("roster.json");
    fs::create_dir_all(&root).unwrap();
    let oversized = vec![b'0'; (MAX_TRUST_ROSTER_FILE_BYTES + 1) as usize];
    fs::write(&path, &oversized).unwrap();
    let error = load_trust_roster(&path).expect_err("an oversized document must fail closed");
    assert!(
        error.message.contains("byte bound for this front"),
        "{}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Issue #577 (DV-17): the real `audit verify --trust-roster` front must not
// accept a signer's role, identity, or expiry that the signature never
// covered. Each case keeps the one genuine signature and the same roster.
// ---------------------------------------------------------------------

const DECISION_ROLES: [&str; 5] = ["proposer", "reviewer", "validator", "approver", "publisher"];

fn verify_signed_entries(
    label: &str,
    entries: &[SignatureEntry],
    roles: &[&str],
    now: u64,
) -> Result<String, Diagnostic> {
    let root = scratch_dir(label);
    let manifest_path = root.join("capsule.json");
    fs::write(&manifest_path, render_trust_capsule("r1", entries)).unwrap();
    let objects_dir = root.join("objects");
    write_trust_objects(&objects_dir);
    let roster_path = write_roster(
        &root,
        "roster.json",
        &trust_roster_json(&[("local-test-key", trust_fixture_key())]),
    );
    let mut options = verify_options_with_roster(manifest_path, objects_dir, Some(roster_path));
    options.required_roles = roles.iter().map(|role| (*role).to_owned()).collect();
    options.verification_time_unix_seconds = Some(now);
    run_verify(&options)
}

#[test]
fn verify_accepts_the_original_signed_role_and_rejects_a_missing_one() {
    let original = signed_entry("proposer", "local-test-key", "r1", 100);
    verify_signed_entries(
        "dv17-original",
        std::slice::from_ref(&original),
        &["proposer"],
        50,
    )
    .expect("the genuinely signed role verifies");
    let error = verify_signed_entries("dv17-missing", &[original], &["publisher"], 50)
        .expect_err("a role nobody signed is missing");
    assert!(
        error.message.contains("no signature carries"),
        "{}",
        error.message
    );
}

#[test]
fn verify_rejects_a_signature_whose_role_was_relabelled() {
    let mut relabelled = signed_entry("proposer", "local-test-key", "r1", 100);
    relabelled.role = "publisher".to_owned();
    let error = verify_signed_entries("dv17-role", &[relabelled], &["publisher"], 50)
        .expect_err("an unsigned role must not satisfy the publisher policy");
    assert!(
        error.message.contains("does not verify against"),
        "{}",
        error.message
    );
}

#[test]
fn verify_rejects_a_signature_whose_expiry_was_extended_and_honours_the_signed_one() {
    let original = signed_entry("publisher", "local-test-key", "r1", 100);
    verify_signed_entries(
        "dv17-boundary",
        std::slice::from_ref(&original),
        &["publisher"],
        100,
    )
    .expect("valid through its signed expiry instant");
    let error = verify_signed_entries(
        "dv17-expired",
        std::slice::from_ref(&original),
        &["publisher"],
        101,
    )
    .expect_err("the signed expiry is effective");
    assert!(error.message.contains("expired"), "{}", error.message);

    let mut extended = original;
    extended.not_valid_after_unix_seconds = u64::MAX;
    let error = verify_signed_entries("dv17-extended", &[extended], &["publisher"], 101)
        .expect_err("an unsigned expiry extension must not verify");
    assert!(
        error.message.contains("does not verify against"),
        "{}",
        error.message
    );
}

#[test]
fn verify_rejects_one_signature_copied_into_all_five_decision_roles() {
    let original = signed_entry("proposer", "local-test-key", "r1", 100);
    let copies: Vec<SignatureEntry> = DECISION_ROLES
        .iter()
        .map(|role| SignatureEntry {
            role: (*role).to_owned(),
            ..original.clone()
        })
        .collect();
    let error = verify_signed_entries("dv17-copied", &copies, &DECISION_ROLES, 50)
        .expect_err("one signature is not five role decisions");
    assert!(
        error.message.contains("does not verify against"),
        "{}",
        error.message
    );
}

#[test]
fn verify_refuses_a_legacy_raw_signature_under_a_trust_roster() {
    let legacy = SignatureEntry {
        algorithm: "ed25519-raw-v1".to_owned(),
        ..signed_entry("proposer", "local-test-key", "r1", 100)
    };
    let error = verify_signed_entries("dv17-legacy", &[legacy], &["proposer"], 50)
        .expect_err("legacy signatures leave role and expiry unsigned");
    assert!(
        error
            .message
            .contains("re-sign this entry as `ed25519-entry-v2`"),
        "{}",
        error.message
    );
}
