//! Independent, filesystem-light verification of release provenance and
//! signature *binding* (#168). This module signs nothing and cannot: the
//! compiler and generated code carry no ambient signing authority (see
//! `AGENTS.md`), no signing key or keyless-signing identity is available in
//! this repository, and this file never spawns a process, opens a network
//! socket, or executes a release artifact.
//!
//! It answers exactly one question, for each of two documents defined by
//! [`docs/RELEASE-SIGNING-POLICY-V1.md`](../../docs/RELEASE-SIGNING-POLICY-V1.md):
//! given a `semaprax.release-manifest.v1` (#167,
//! `scripts/release-manifest.py`), a `semaprax.release-provenance.v1`
//! (`scripts/release-provenance.py`), and a claimed
//! `semaprax.release-signature-claim.v1`, do their declared subjects bind to
//! the exact same bytes and to the pinned trusted-identity policy? A single
//! byte changed anywhere -- an archive, the manifest, the provenance
//! statement, or the claim -- changes a recomputed digest and is rejected;
//! an internally well-formed claim naming the wrong repository, workflow, or
//! tag is rejected by the identity-policy check; and a well-formed claim
//! whose `subject_digest` was computed over a *different* version's
//! provenance bytes (a replay) is rejected because that digest cannot equal
//! the one recomputed over the provenance under test.
//!
//! ## What this module deliberately does **not** do
//!
//! It never decodes, parses, or cryptographically verifies `signature` or
//! `certificate` bytes in a [`ParsedSignatureClaim`]. Doing that honestly
//! requires a signature-verification implementation (Sigstore/cosign
//! bundle verification, or raw Ed25519/ECDSA point arithmetic), which needs
//! a cryptography dependency this change is not permitted to add and which
//! nobody has requested SEMAPRAX carry today. Treating `signature` and
//! `certificate` as opaque, structurally-checked byte strings -- present,
//! non-empty, and bound to the right subject digest and identity -- is a
//! **binding** check, not an **authenticity** check. A forged claim that
//! never touched a real private key but happens to name an approved
//! identity and the right subject digest is *not* rejected by this module
//! alone; only pairing this binding check with a real external verifier
//! (e.g. `cosign verify-blob` against the recorded identity) closes that
//! gap. See `docs/RELEASE-SIGNING-POLICY-V1.md` for the full nonclaims list
//! and the human-owned checklist this module's callers still depend on.
//!
//! Integrity (do these bytes match what was recorded?), authenticity (were
//! they produced by the claimed identity?), provenance (what exactly was
//! bound?), and reproducibility (can a third party rebuild the same bytes?)
//! remain four separate claims here, exactly as
//! [issue #168](https://github.com/wavect/semaprax/issues/168) requires;
//! this module only ever advances the first and third.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::diagnostic::Diagnostic;

pub const PROVENANCE_SCHEMA: &str = "semaprax.release-provenance.v1";
pub const SIGNATURE_CLAIM_SCHEMA: &str = "semaprax.release-signature-claim.v1";
const MANIFEST_SCHEMA: &str = "semaprax.release-manifest.v1";

/// Trusted identity policy v1. These three constants are the enforced
/// counterpart of the "Trusted identity policy v1" table in
/// `docs/RELEASE-SIGNING-POLICY-V1.md`; a cross-check in
/// `tests/offline_package/release_provenance.rs` greps that document for
/// these exact literal strings so policy text and enforced code cannot
/// silently drift apart.
pub const TRUSTED_ISSUER: &str = "https://token.actions.githubusercontent.com";
pub const TRUSTED_REPOSITORY: &str = "wavect/semaprax";
pub const TRUSTED_WORKFLOW_PATH: &str = ".github/workflows/ci.yml";

/// The admitted release archive platforms, independent of
/// `scripts/release-reconcile.py`'s `ARCHIVE_TARGETS` (a Python tuple this
/// Rust module cannot import). `tests/offline_package/release_provenance.rs`
/// cross-checks the two lists agree, the same way
/// `tests/offline_package/ci_release_gate.rs` cross-checks its
/// `RELEASE_BLOCKERS` against `scripts/release-manifest.py`'s independent
/// parse of the same CI workflow.
pub const ARCHIVE_PLATFORMS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// Closed vocabulary of build-host classes a provenance statement may claim.
/// Matches the "Admitted release hosts" table in `docs/RELEASE-PROCESS.md`.
pub const KNOWN_HOST_CLASSES: &[&str] = &[
    "github-hosted-ubuntu-24.04",
    "github-hosted-macos-15",
    "github-hosted-windows-2025",
];

/// Closed vocabulary of signature-claim algorithm identifiers this module
/// recognizes as structurally admissible. Recognizing an identifier here is
/// not a cryptographic endorsement of it; see the module doc.
pub const KNOWN_CLAIM_ALGORITHMS: &[&str] = &["sigstore-cosign-bundle-v0.3"];

fn shape_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z701", message)
}

fn binding_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z702", message)
}

fn identity_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z703", message)
}

fn artifact_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z704", message)
}

fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn is_sha256_wire_form(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_lowercase_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, Diagnostic> {
    value
        .as_object()
        .ok_or_else(|| shape_error(format!("{context} must be a JSON object")))
}

fn require_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic> {
    value
        .as_str()
        .ok_or_else(|| shape_error(format!("`{field}` must be a string")))
}

fn require_bool(value: &Value, field: &str) -> Result<bool, Diagnostic> {
    value
        .as_bool()
        .ok_or_else(|| shape_error(format!("`{field}` must be a boolean")))
}

fn require_u64(value: &Value, field: &str) -> Result<u64, Diagnostic> {
    value
        .as_u64()
        .ok_or_else(|| shape_error(format!("`{field}` must be an unsigned integer")))
}

fn require_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| shape_error(format!("`{field}` must be an array")))
}

fn check_exact_keys(
    map: &serde_json::Map<String, Value>,
    expected: &[&str],
    context: &str,
) -> Result<(), Diagnostic> {
    let mut found: Vec<&str> = map.keys().map(String::as_str).collect();
    found.sort_unstable();
    let mut expected_sorted: Vec<&str> = expected.to_vec();
    expected_sorted.sort_unstable();
    if found != expected_sorted {
        return Err(shape_error(format!(
            "{context} keys must be exactly {expected_sorted:?}, found {found:?}"
        )));
    }
    Ok(())
}

fn parse_json(bytes: &[u8], context: &str) -> Result<Value, Diagnostic> {
    serde_json::from_slice(bytes)
        .map_err(|error| shape_error(format!("{context} is not valid JSON: {error}")))
}

/// One artifact entry, shared verbatim (name/platform/size/digest) between
/// `semaprax.release-manifest.v1` and the `artifacts` array a provenance
/// statement copies from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactEntry {
    pub name: String,
    pub platform: String,
    pub size: u64,
    pub digest: String,
}

fn parse_artifacts(value: &Value, context: &str) -> Result<Vec<ArtifactEntry>, Diagnostic> {
    let entries = require_array(value, context)?;
    let mut artifacts = Vec::with_capacity(entries.len());
    let mut seen_platforms = BTreeSet::new();
    for entry in entries {
        let map = object(entry, &format!("{context}[]"))?;
        check_exact_keys(
            map,
            &["name", "platform", "size", "digest"],
            &format!("{context}[]"),
        )?;
        let name = require_string(&entry["name"], "name")?.to_owned();
        let platform = require_string(&entry["platform"], "platform")?.to_owned();
        let size = require_u64(&entry["size"], "size")?;
        let digest = require_string(&entry["digest"], "digest")?.to_owned();
        if !is_sha256_wire_form(&digest) {
            return Err(shape_error(format!(
                "artifact `{name}` digest must be `sha256:<64 lowercase hex>`"
            )));
        }
        if !seen_platforms.insert(platform.clone()) {
            return Err(shape_error(format!(
                "artifact platform `{platform}` is repeated in {context}"
            )));
        }
        artifacts.push(ArtifactEntry {
            name,
            platform,
            size,
            digest,
        });
    }
    let known: BTreeSet<&str> = ARCHIVE_PLATFORMS.iter().copied().collect();
    let seen_str: BTreeSet<&str> = seen_platforms.iter().map(String::as_str).collect();
    if seen_str != known {
        let missing: Vec<&str> = known.difference(&seen_str).copied().collect();
        let extra: Vec<&str> = seen_str.difference(&known).copied().collect();
        return Err(artifact_error(format!(
            "{context} platform set disagrees with the admitted release targets \
             {ARCHIVE_PLATFORMS:?}: missing {missing:?}, extra {extra:?}"
        )));
    }
    artifacts.sort_by(|a, b| a.platform.cmp(&b.platform));
    Ok(artifacts)
}

/// A structurally validated `semaprax.release-manifest.v1` document,
/// independently re-derived from raw bytes rather than trusted from a
/// caller-supplied struct.
#[derive(Debug, Clone)]
pub struct ParsedManifest {
    pub version: String,
    pub tag: String,
    pub commit: String,
    pub prerelease: bool,
    pub required_checks: Vec<String>,
    pub artifacts: Vec<ArtifactEntry>,
}

/// Independently parse and structurally validate a `semaprax.release-manifest.v1`
/// document from its exact on-disk bytes. Rejects (fails closed) any schema
/// mismatch, wrong-shaped field, malformed digest, or artifact platform set
/// that disagrees with [`ARCHIVE_PLATFORMS`] (this is the "missing and extra
/// artifacts are rejected" case for the manifest itself).
pub fn parse_manifest(bytes: &[u8]) -> Result<ParsedManifest, Diagnostic> {
    let value = parse_json(bytes, "release manifest")?;
    let map = object(&value, "release manifest")?;
    check_exact_keys(
        map,
        &[
            "schema",
            "version",
            "tag",
            "commit",
            "prerelease",
            "required_checks",
            "changelog_section_digest",
            "artifacts",
        ],
        "release manifest",
    )?;
    if require_string(&value["schema"], "schema")? != MANIFEST_SCHEMA {
        return Err(shape_error(format!(
            "manifest schema must be {MANIFEST_SCHEMA}"
        )));
    }
    let version = require_string(&value["version"], "version")?.to_owned();
    let tag = require_string(&value["tag"], "tag")?.to_owned();
    if tag != format!("v{version}") {
        return Err(shape_error(format!(
            "manifest tag {tag:?} does not equal v plus the version {version:?}"
        )));
    }
    let commit = require_string(&value["commit"], "commit")?.to_owned();
    if !is_lowercase_commit(&commit) {
        return Err(shape_error(
            "manifest commit must be exactly 40 lowercase hexadecimal characters".to_owned(),
        ));
    }
    let prerelease = require_bool(&value["prerelease"], "prerelease")?;
    let checks_array = require_array(&value["required_checks"], "required_checks")?;
    if checks_array.is_empty() {
        return Err(shape_error("required_checks must not be empty".to_owned()));
    }
    let mut required_checks = Vec::with_capacity(checks_array.len());
    for entry in checks_array {
        let check = require_string(entry, "required_checks[]")?.to_owned();
        if check.is_empty() {
            return Err(shape_error(
                "required_checks entries must not be empty".to_owned(),
            ));
        }
        required_checks.push(check);
    }
    let digest = require_string(
        &value["changelog_section_digest"],
        "changelog_section_digest",
    )?;
    if !is_sha256_wire_form(digest) {
        return Err(shape_error(
            "changelog_section_digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    let artifacts = parse_artifacts(&value["artifacts"], "artifacts")?;
    Ok(ParsedManifest {
        version,
        tag,
        commit,
        prerelease,
        required_checks,
        artifacts,
    })
}

/// A structurally validated `semaprax.release-provenance.v1` document.
#[derive(Debug, Clone)]
pub struct ParsedProvenance {
    pub version: String,
    pub tag: String,
    pub commit: String,
    pub prerelease: bool,
    pub required_checks: Vec<String>,
    pub artifacts: Vec<ArtifactEntry>,
    pub manifest_digest: String,
    pub source_repository: String,
    pub builder_workflow_identity: String,
    pub build_host_class: String,
}

/// Independently parse and structurally validate a
/// `semaprax.release-provenance.v1` document from its exact on-disk bytes.
pub fn parse_provenance(bytes: &[u8]) -> Result<ParsedProvenance, Diagnostic> {
    let value = parse_json(bytes, "release provenance")?;
    let map = object(&value, "release provenance")?;
    check_exact_keys(
        map,
        &[
            "schema",
            "version",
            "tag",
            "commit",
            "prerelease",
            "required_checks",
            "artifacts",
            "manifest_digest",
            "source",
            "builder",
            "toolchain",
            "build_host_class",
            "nonclaims",
        ],
        "release provenance",
    )?;
    if require_string(&value["schema"], "schema")? != PROVENANCE_SCHEMA {
        return Err(shape_error(format!(
            "provenance schema must be {PROVENANCE_SCHEMA}"
        )));
    }
    let version = require_string(&value["version"], "version")?.to_owned();
    let tag = require_string(&value["tag"], "tag")?.to_owned();
    if tag != format!("v{version}") {
        return Err(shape_error(format!(
            "provenance tag {tag:?} does not equal v plus the version {version:?}"
        )));
    }
    let commit = require_string(&value["commit"], "commit")?.to_owned();
    if !is_lowercase_commit(&commit) {
        return Err(shape_error(
            "provenance commit must be exactly 40 lowercase hexadecimal characters".to_owned(),
        ));
    }
    let prerelease = require_bool(&value["prerelease"], "prerelease")?;
    let checks_array = require_array(&value["required_checks"], "required_checks")?;
    let mut required_checks = Vec::with_capacity(checks_array.len());
    for entry in checks_array {
        required_checks.push(require_string(entry, "required_checks[]")?.to_owned());
    }
    let artifacts = parse_artifacts(&value["artifacts"], "artifacts")?;
    let manifest_digest = require_string(&value["manifest_digest"], "manifest_digest")?.to_owned();
    if !is_sha256_wire_form(&manifest_digest) {
        return Err(shape_error(
            "manifest_digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }

    let source = object(&value["source"], "source")?;
    check_exact_keys(source, &["repository", "commit", "tag"], "source")?;
    let source_repository =
        require_string(&value["source"]["repository"], "source.repository")?.to_owned();
    let source_commit = require_string(&value["source"]["commit"], "source.commit")?;
    let source_tag = require_string(&value["source"]["tag"], "source.tag")?;
    if source_commit != commit {
        return Err(binding_error(
            "provenance source.commit does not match the top-level commit".to_owned(),
        ));
    }
    if source_tag != tag {
        return Err(binding_error(
            "provenance source.tag does not match the top-level tag".to_owned(),
        ));
    }

    let builder = object(&value["builder"], "builder")?;
    check_exact_keys(
        builder,
        &["workflow_identity", "run_id", "run_attempt"],
        "builder",
    )?;
    let builder_workflow_identity = require_string(
        &value["builder"]["workflow_identity"],
        "builder.workflow_identity",
    )?
    .to_owned();
    if builder_workflow_identity.is_empty() {
        return Err(shape_error(
            "builder.workflow_identity must not be empty".to_owned(),
        ));
    }
    require_string(&value["builder"]["run_id"], "builder.run_id")?;
    require_string(&value["builder"]["run_attempt"], "builder.run_attempt")?;

    let toolchain = object(&value["toolchain"], "toolchain")?;
    check_exact_keys(toolchain, &["rustc_version", "cargo_locked"], "toolchain")?;
    let rustc_version = require_string(
        &value["toolchain"]["rustc_version"],
        "toolchain.rustc_version",
    )?;
    if rustc_version.is_empty() {
        return Err(shape_error(
            "toolchain.rustc_version must not be empty".to_owned(),
        ));
    }
    require_bool(
        &value["toolchain"]["cargo_locked"],
        "toolchain.cargo_locked",
    )?;

    let build_host_class =
        require_string(&value["build_host_class"], "build_host_class")?.to_owned();
    if !KNOWN_HOST_CLASSES.contains(&build_host_class.as_str()) {
        return Err(shape_error(format!(
            "build_host_class {build_host_class:?} is outside the admitted host classes {KNOWN_HOST_CLASSES:?}"
        )));
    }

    let nonclaims = require_array(&value["nonclaims"], "nonclaims")?;
    if nonclaims.is_empty() {
        return Err(shape_error("nonclaims must not be empty".to_owned()));
    }
    for entry in nonclaims {
        require_string(entry, "nonclaims[]")?;
    }

    Ok(ParsedProvenance {
        version,
        tag,
        commit,
        prerelease,
        required_checks,
        artifacts,
        manifest_digest,
        source_repository,
        builder_workflow_identity,
        build_host_class,
    })
}

/// A structurally validated `semaprax.release-signature-claim.v1` document.
/// `signature` and `certificate` are kept as opaque strings: see the module
/// doc for why this module never decodes or cryptographically verifies them.
#[derive(Debug, Clone)]
pub struct ParsedSignatureClaim {
    pub subject_digest: String,
    pub algorithm: String,
    pub identity_issuer: String,
    pub identity_subject: String,
    pub identity_workflow_ref: String,
    pub signature: String,
    pub certificate: String,
}

/// Independently parse and structurally validate a
/// `semaprax.release-signature-claim.v1` document.
pub fn parse_signature_claim(bytes: &[u8]) -> Result<ParsedSignatureClaim, Diagnostic> {
    let value = parse_json(bytes, "signature claim")?;
    let map = object(&value, "signature claim")?;
    check_exact_keys(
        map,
        &[
            "schema",
            "subject_digest",
            "subject_name",
            "identity",
            "algorithm",
            "signature",
            "certificate",
        ],
        "signature claim",
    )?;
    if require_string(&value["schema"], "schema")? != SIGNATURE_CLAIM_SCHEMA {
        return Err(shape_error(format!(
            "signature claim schema must be {SIGNATURE_CLAIM_SCHEMA}"
        )));
    }
    let subject_digest = require_string(&value["subject_digest"], "subject_digest")?.to_owned();
    if !is_sha256_wire_form(&subject_digest) {
        return Err(shape_error(
            "subject_digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    require_string(&value["subject_name"], "subject_name")?;
    let algorithm = require_string(&value["algorithm"], "algorithm")?.to_owned();
    if !KNOWN_CLAIM_ALGORITHMS.contains(&algorithm.as_str()) {
        return Err(shape_error(format!(
            "algorithm {algorithm:?} is outside the recognized claim algorithms {KNOWN_CLAIM_ALGORITHMS:?}"
        )));
    }
    let identity = object(&value["identity"], "identity")?;
    check_exact_keys(identity, &["issuer", "subject", "workflow_ref"], "identity")?;
    let identity_issuer =
        require_string(&value["identity"]["issuer"], "identity.issuer")?.to_owned();
    let identity_subject =
        require_string(&value["identity"]["subject"], "identity.subject")?.to_owned();
    let identity_workflow_ref =
        require_string(&value["identity"]["workflow_ref"], "identity.workflow_ref")?.to_owned();
    let signature = require_string(&value["signature"], "signature")?.to_owned();
    let certificate = require_string(&value["certificate"], "certificate")?.to_owned();
    if signature.is_empty() {
        return Err(shape_error("signature must not be empty".to_owned()));
    }
    if certificate.is_empty() {
        return Err(shape_error("certificate must not be empty".to_owned()));
    }
    Ok(ParsedSignatureClaim {
        subject_digest,
        algorithm,
        identity_issuer,
        identity_subject,
        identity_workflow_ref,
        signature,
        certificate,
    })
}

/// Verify that a `semaprax.release-provenance.v1` document's own recorded
/// fields agree, field for field, with the `semaprax.release-manifest.v1`
/// it claims to describe -- including a byte-exact `manifest_digest`
/// recomputed from `manifest_bytes`, so any single-byte edit to the manifest
/// (even one that reparses to the same JSON value, such as added
/// whitespace) is rejected.
pub fn verify_provenance_binds_manifest(
    provenance_bytes: &[u8],
    manifest_bytes: &[u8],
) -> Result<(), Diagnostic> {
    let provenance = parse_provenance(provenance_bytes)?;
    let manifest = parse_manifest(manifest_bytes)?;

    let recomputed_manifest_digest = sha256_digest(manifest_bytes);
    if provenance.manifest_digest != recomputed_manifest_digest {
        return Err(binding_error(
            "provenance manifest_digest does not match the exact manifest bytes under test"
                .to_owned(),
        ));
    }
    if provenance.version != manifest.version {
        return Err(binding_error(format!(
            "provenance version {:?} disagrees with manifest version {:?}",
            provenance.version, manifest.version
        )));
    }
    if provenance.tag != manifest.tag {
        return Err(binding_error(format!(
            "provenance tag {:?} disagrees with manifest tag {:?}",
            provenance.tag, manifest.tag
        )));
    }
    if provenance.commit != manifest.commit {
        return Err(binding_error(format!(
            "provenance commit {:?} disagrees with manifest commit {:?}",
            provenance.commit, manifest.commit
        )));
    }
    if provenance.prerelease != manifest.prerelease {
        return Err(binding_error(
            "provenance prerelease flag disagrees with the manifest's".to_owned(),
        ));
    }
    if provenance.required_checks != manifest.required_checks {
        return Err(binding_error(
            "provenance required_checks disagrees with the manifest's exact inventory".to_owned(),
        ));
    }
    if provenance.artifacts != manifest.artifacts {
        return Err(binding_error(
            "provenance artifacts disagrees with the manifest's exact artifact inventory"
                .to_owned(),
        ));
    }
    if provenance.source_repository != TRUSTED_REPOSITORY {
        return Err(identity_error(format!(
            "provenance source.repository {:?} is not the trusted repository {TRUSTED_REPOSITORY:?}",
            provenance.source_repository
        )));
    }
    Ok(())
}

/// Verify that a claimed `semaprax.release-signature-claim.v1` binds to the
/// exact provenance statement under test and to the pinned trusted identity
/// policy (issuer, repository, workflow, and the exact tag the provenance
/// itself declares). This never verifies `signature`/`certificate`
/// cryptographically -- see the module doc.
///
/// Because `subject_digest` must equal a byte-exact recomputed digest of
/// `provenance_bytes`, a claim produced for any other version's provenance
/// (a replay) cannot pass here: its recorded `subject_digest` was computed
/// over different bytes and will not match.
pub fn verify_signature_claim_binds_provenance(
    claim_bytes: &[u8],
    provenance_bytes: &[u8],
) -> Result<(), Diagnostic> {
    let claim = parse_signature_claim(claim_bytes)?;
    let provenance = parse_provenance(provenance_bytes)?;

    let recomputed_subject_digest = sha256_digest(provenance_bytes);
    if claim.subject_digest != recomputed_subject_digest {
        return Err(binding_error(
            "signature claim subject_digest does not match the exact provenance bytes under \
             test (this rejects both a tampered provenance document and a claim replayed from a \
             different release)"
                .to_owned(),
        ));
    }
    if claim.identity_issuer != TRUSTED_ISSUER {
        return Err(identity_error(format!(
            "signature claim issuer {:?} is not the trusted issuer {TRUSTED_ISSUER:?}",
            claim.identity_issuer
        )));
    }
    let expected_subject = format!("repo:{TRUSTED_REPOSITORY}:ref:refs/tags/{}", provenance.tag);
    if claim.identity_subject != expected_subject {
        return Err(identity_error(format!(
            "signature claim identity.subject {:?} does not match the expected subject {expected_subject:?} \
             for the exact tag this provenance declares",
            claim.identity_subject
        )));
    }
    let expected_workflow_ref = format!(
        "{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/{}",
        provenance.tag
    );
    if claim.identity_workflow_ref != expected_workflow_ref {
        return Err(identity_error(format!(
            "signature claim identity.workflow_ref {:?} does not match the expected workflow \
             reference {expected_workflow_ref:?}",
            claim.identity_workflow_ref
        )));
    }
    if claim.identity_workflow_ref != provenance.builder_workflow_identity {
        return Err(binding_error(
            "signature claim identity.workflow_ref disagrees with the provenance statement's own \
             builder.workflow_identity"
                .to_owned(),
        ));
    }
    Ok(())
}

/// Convenience entry point tying the two binding checks together: a
/// provenance statement that exactly binds to the manifest, and a signature
/// claim that exactly binds to that provenance statement and the trusted
/// identity policy. Stops at the first failure (fail closed).
pub fn verify_release_binding(
    manifest_bytes: &[u8],
    provenance_bytes: &[u8],
    claim_bytes: &[u8],
) -> Result<(), Diagnostic> {
    verify_provenance_binds_manifest(provenance_bytes, manifest_bytes)?;
    verify_signature_claim_binds_provenance(claim_bytes, provenance_bytes)?;
    Ok(())
}

/// Re-hash every artifact a manifest names, from a caller-supplied
/// directory's actual bytes, and fail closed on any digest/size mismatch or
/// missing file. This is an independent replay of what
/// `scripts/release-manifest.py` asserted at build time -- it trusts
/// nothing the manifest recorded about an artifact except its name, and
/// touches only the exact files `manifest_bytes` names (no directory
/// listing, no ambient traversal).
pub fn verify_manifest_artifacts_on_disk(
    manifest_bytes: &[u8],
    archives_dir: &Path,
) -> Result<(), Diagnostic> {
    let manifest = parse_manifest(manifest_bytes)?;
    for artifact in &manifest.artifacts {
        let path = archives_dir.join(&artifact.name);
        let data = std::fs::read(&path).map_err(|error| {
            artifact_error(format!(
                "cannot read manifest artifact {}: {error}",
                path.display()
            ))
        })?;
        if data.len() as u64 != artifact.size {
            return Err(artifact_error(format!(
                "artifact {} is {} bytes on disk but the manifest records {}",
                artifact.name,
                data.len(),
                artifact.size
            )));
        }
        let digest = sha256_digest(&data);
        if digest != artifact.digest {
            return Err(artifact_error(format!(
                "artifact {} digest {digest} disagrees with the manifest's recorded {}",
                artifact.name, artifact.digest
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
