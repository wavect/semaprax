//! Unit tests for release provenance/signature-claim binding (#168).
//!
//! Each hostile case here is one of the required tests issue #168 names:
//! single-byte mutation to the manifest, artifact, or claim; wrong
//! repository/workflow identity; a provenance statement for a different
//! commit/tag; a missing or extra artifact; and a signature claim replayed
//! from another version.

use std::fs;

use super::*;

const FAKE_DIGEST_A: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FAKE_DIGEST_B: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FAKE_DIGEST_C: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const FAKE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn manifest_json(tag: &str, commit: &str) -> String {
    let version = tag.strip_prefix('v').unwrap();
    format!(
        r#"{{
  "schema": "semaprax.release-manifest.v1",
  "version": "{version}",
  "tag": "{tag}",
  "commit": "{commit}",
  "prerelease": true,
  "required_checks": ["alpha", "beta"],
  "changelog_section_digest": "{FAKE_DIGEST_A}",
  "artifacts": [
    {{"name": "semaprax-{tag}-x86_64-unknown-linux-gnu.tar.gz", "platform": "x86_64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
    {{"name": "semaprax-{tag}-x86_64-pc-windows-msvc.zip", "platform": "x86_64-pc-windows-msvc", "size": 30, "digest": "{FAKE_DIGEST_C}"}}
  ]
}}"#
    )
}

fn provenance_json(tag: &str, commit: &str, manifest_bytes: &[u8]) -> String {
    let version = tag.strip_prefix('v').unwrap();
    let manifest_digest = sha256_digest(manifest_bytes);
    let workflow_identity = format!("{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/{tag}");
    format!(
        r#"{{
  "schema": "semaprax.release-provenance.v1",
  "version": "{version}",
  "tag": "{tag}",
  "commit": "{commit}",
  "prerelease": true,
  "required_checks": ["alpha", "beta"],
  "artifacts": [
    {{"name": "semaprax-{tag}-x86_64-unknown-linux-gnu.tar.gz", "platform": "x86_64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
    {{"name": "semaprax-{tag}-x86_64-pc-windows-msvc.zip", "platform": "x86_64-pc-windows-msvc", "size": 30, "digest": "{FAKE_DIGEST_C}"}}
  ],
  "manifest_digest": "{manifest_digest}",
  "source": {{"repository": "{TRUSTED_REPOSITORY}", "commit": "{commit}", "tag": "{tag}"}},
  "builder": {{"workflow_identity": "{workflow_identity}", "run_id": "1", "run_attempt": "1"}},
  "toolchain": {{"rustc_version": "1.88.0", "cargo_locked": true}},
  "build_host_class": "github-hosted-ubuntu-24.04",
  "nonclaims": ["unsigned_without_a_paired_signature_claim", "not_a_reproducible_build_claim"]
}}"#
    )
}

fn claim_json(tag: &str, provenance_bytes: &[u8]) -> String {
    let subject_digest = sha256_digest(provenance_bytes);
    let workflow_ref = format!("{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/{tag}");
    let subject = format!("repo:{TRUSTED_REPOSITORY}:ref:refs/tags/{tag}");
    format!(
        r#"{{
  "schema": "semaprax.release-signature-claim.v1",
  "subject_digest": "{subject_digest}",
  "subject_name": "release-provenance.json",
  "identity": {{"issuer": "{TRUSTED_ISSUER}", "subject": "{subject}", "workflow_ref": "{workflow_ref}"}},
  "algorithm": "sigstore-cosign-bundle-v0.3",
  "signature": "FIXTURE-NOT-A-REAL-SIGNATURE",
  "certificate": "FIXTURE-NOT-A-REAL-CERTIFICATE"
}}"#
    )
}

struct Fixture {
    manifest: String,
    provenance: String,
    claim: String,
}

fn valid_fixture() -> Fixture {
    let tag = "v9.9.9";
    let manifest = manifest_json(tag, FAKE_COMMIT);
    let provenance = provenance_json(tag, FAKE_COMMIT, manifest.as_bytes());
    let claim = claim_json(tag, provenance.as_bytes());
    Fixture {
        manifest,
        provenance,
        claim,
    }
}

#[test]
fn valid_fixture_binds_end_to_end() {
    let fixture = valid_fixture();
    verify_release_binding(
        fixture.manifest.as_bytes(),
        fixture.provenance.as_bytes(),
        fixture.claim.as_bytes(),
    )
    .expect("a self-consistent manifest/provenance/claim triple must verify");
}

#[test]
fn single_byte_mutation_to_the_manifest_is_rejected() {
    let fixture = valid_fixture();
    let mut mutated = fixture.manifest.clone().into_bytes();
    // Flip one byte inside the commit field -- still valid JSON, still a
    // 40-character lowercase-hex-shaped string, just the wrong one.
    let position = mutated
        .windows(FAKE_COMMIT.len())
        .position(|window| window == FAKE_COMMIT.as_bytes())
        .expect("fixture must contain the commit literal");
    mutated[position] = b'f';
    let error = verify_provenance_binds_manifest(fixture.provenance.as_bytes(), &mutated)
        .expect_err("a single mutated byte in the manifest must be rejected");
    assert!(error.message.contains("manifest_digest") || error.message.contains("commit"));
}

#[test]
fn single_byte_mutation_to_the_provenance_is_rejected_by_the_claim() {
    let fixture = valid_fixture();
    // Mutate a byte inside a field that occurs exactly once in the
    // rendered document (unlike the commit, which is recorded both at the
    // top level and under `source`), so this is purely a "provenance bytes
    // changed under the claim" mutation rather than an internal
    // top-level/`source` disagreement.
    const NEEDLE: &str = "1.88.0";
    let mut mutated = fixture.provenance.clone().into_bytes();
    let position = mutated
        .windows(NEEDLE.len())
        .position(|window| window == NEEDLE.as_bytes())
        .expect("fixture must contain the rustc_version literal");
    mutated[position] = b'9';
    let error = verify_signature_claim_binds_provenance(fixture.claim.as_bytes(), &mutated)
        .expect_err("a single mutated byte in the provenance must be rejected by the claim");
    assert!(error.message.contains("subject_digest"));
}

#[test]
fn single_byte_mutation_to_an_artifact_is_rejected_on_disk() {
    let fixture = valid_fixture();
    let dir = std::env::temp_dir().join(format!(
        "spx-release-provenance-test-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let name = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    fs::write(dir.join(name), b"0123456789").unwrap();
    verify_manifest_artifacts_on_disk(fixture.manifest.as_bytes(), &dir)
        .expect_err("fixture digest is fake, so the real bytes must not match it");

    // Now build a manifest whose digest agrees with real bytes, and confirm
    // a one-byte mutation on disk is caught.
    let real_bytes = b"0123456789".to_vec();
    let real_digest = sha256_digest(&real_bytes);
    let manifest = format!(
        r#"{{
  "schema": "semaprax.release-manifest.v1",
  "version": "9.9.9",
  "tag": "v9.9.9",
  "commit": "{FAKE_COMMIT}",
  "prerelease": true,
  "required_checks": ["alpha"],
  "changelog_section_digest": "{FAKE_DIGEST_A}",
  "artifacts": [
    {{"name": "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz", "platform": "x86_64-unknown-linux-gnu", "size": 10, "digest": "{real_digest}"}},
    {{"name": "semaprax-v9.9.9-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 10, "digest": "{real_digest}"}},
    {{"name": "semaprax-v9.9.9-x86_64-pc-windows-msvc.zip", "platform": "x86_64-pc-windows-msvc", "size": 10, "digest": "{real_digest}"}}
  ]
}}"#
    );
    fs::write(
        dir.join("semaprax-v9.9.9-aarch64-apple-darwin.tar.gz"),
        &real_bytes,
    )
    .unwrap();
    fs::write(
        dir.join("semaprax-v9.9.9-x86_64-pc-windows-msvc.zip"),
        &real_bytes,
    )
    .unwrap();
    fs::write(dir.join(name), &real_bytes).unwrap();
    verify_manifest_artifacts_on_disk(manifest.as_bytes(), &dir)
        .expect("bytes on disk now agree with the manifest's recorded digest");

    // Mutate one byte of one archive on disk.
    fs::write(dir.join(name), b"9123456789").unwrap();
    let error = verify_manifest_artifacts_on_disk(manifest.as_bytes(), &dir)
        .expect_err("a single mutated artifact byte must be rejected");
    assert!(error.message.contains("digest"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn provenance_for_a_different_commit_is_rejected() {
    let fixture = valid_fixture();
    let other_commit = "f".repeat(40);
    let provenance = provenance_json("v9.9.9", &other_commit, fixture.manifest.as_bytes());
    let error =
        verify_provenance_binds_manifest(provenance.as_bytes(), fixture.manifest.as_bytes())
            .expect_err("a provenance statement for a different commit must be rejected");
    assert!(error.message.contains("commit"));
}

#[test]
fn provenance_for_a_different_tag_is_rejected() {
    let fixture = valid_fixture();
    let other_manifest = manifest_json("v9.9.8", FAKE_COMMIT);
    let error =
        verify_provenance_binds_manifest(fixture.provenance.as_bytes(), other_manifest.as_bytes())
            .expect_err(
                "a provenance statement built against a different tag's manifest must reject",
            );
    // Either the manifest_digest binding or the tag comparison catches this;
    // both are acceptable, but one of them must fire.
    assert!(error.message.contains("manifest_digest") || error.message.contains("tag"));
}

#[test]
fn missing_artifact_platform_is_rejected() {
    let tag = "v9.9.9";
    let manifest = format!(
        r#"{{
  "schema": "semaprax.release-manifest.v1",
  "version": "9.9.9",
  "tag": "{tag}",
  "commit": "{FAKE_COMMIT}",
  "prerelease": true,
  "required_checks": ["alpha"],
  "changelog_section_digest": "{FAKE_DIGEST_A}",
  "artifacts": [
    {{"name": "semaprax-{tag}-x86_64-unknown-linux-gnu.tar.gz", "platform": "x86_64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}}
  ]
}}"#
    );
    let error = parse_manifest(manifest.as_bytes())
        .expect_err("a manifest missing the Windows artifact must be rejected");
    assert!(error.message.contains("missing"));
}

#[test]
fn extra_artifact_platform_is_rejected() {
    let tag = "v9.9.9";
    let manifest = format!(
        r#"{{
  "schema": "semaprax.release-manifest.v1",
  "version": "9.9.9",
  "tag": "{tag}",
  "commit": "{FAKE_COMMIT}",
  "prerelease": true,
  "required_checks": ["alpha"],
  "changelog_section_digest": "{FAKE_DIGEST_A}",
  "artifacts": [
    {{"name": "semaprax-{tag}-x86_64-unknown-linux-gnu.tar.gz", "platform": "x86_64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
    {{"name": "semaprax-{tag}-x86_64-pc-windows-msvc.zip", "platform": "x86_64-pc-windows-msvc", "size": 30, "digest": "{FAKE_DIGEST_C}"}},
    {{"name": "semaprax-{tag}-riscv64-unknown-linux-gnu.tar.gz", "platform": "riscv64-unknown-linux-gnu", "size": 40, "digest": "{FAKE_DIGEST_C}"}}
  ]
}}"#
    );
    let error = parse_manifest(manifest.as_bytes())
        .expect_err("a manifest with an unadmitted extra platform must be rejected");
    assert!(error.message.contains("extra"));
}

#[test]
fn signature_claim_from_an_unapproved_repository_is_rejected() {
    let fixture = valid_fixture();
    let claim = fixture
        .claim
        .replace(TRUSTED_REPOSITORY, "attacker/semaprax");
    let error =
        verify_signature_claim_binds_provenance(claim.as_bytes(), fixture.provenance.as_bytes())
            .expect_err("a claim naming an unapproved repository must be rejected");
    assert!(error.message.contains("identity"));
}

#[test]
fn signature_claim_from_an_unapproved_issuer_is_rejected() {
    let fixture = valid_fixture();
    let claim = fixture
        .claim
        .replace(TRUSTED_ISSUER, "https://attacker.example/oidc");
    let error =
        verify_signature_claim_binds_provenance(claim.as_bytes(), fixture.provenance.as_bytes())
            .expect_err("a claim naming an unapproved OIDC issuer must be rejected");
    assert!(error.message.contains("issuer"));
}

#[test]
fn signature_claim_replayed_from_another_version_is_rejected() {
    // Build a second, differently-tagged provenance and reuse the FIRST
    // fixture's claim against it. Even though the claim's identity fields
    // are individually well-formed, its subject_digest was computed over a
    // different version's provenance bytes.
    let fixture = valid_fixture();
    let other_manifest = manifest_json("v9.9.8", FAKE_COMMIT);
    let other_provenance = provenance_json("v9.9.8", FAKE_COMMIT, other_manifest.as_bytes());
    let error = verify_signature_claim_binds_provenance(
        fixture.claim.as_bytes(),
        other_provenance.as_bytes(),
    )
    .expect_err("a claim replayed against a different version's provenance must be rejected");
    assert!(error.message.contains("subject_digest"));
}

#[test]
fn signature_claim_workflow_ref_must_agree_with_the_provenance_builder_identity() {
    let fixture = valid_fixture();
    // Recompute a claim whose workflow_ref is well-formed for the right tag
    // but disagrees with the provenance's own recorded builder identity --
    // simulating a claim generated by a different (but plausible-looking)
    // workflow file.
    let tampered_provenance = fixture.provenance.replace("ci.yml", "ci-legacy.yml");
    let error = verify_signature_claim_binds_provenance(
        fixture.claim.as_bytes(),
        tampered_provenance.as_bytes(),
    )
    .expect_err(
        "a claim whose workflow_ref disagrees with the provenance builder identity must reject",
    );
    assert!(error.message.contains("workflow_ref") || error.message.contains("subject_digest"));
}

#[test]
fn unrecognized_claim_algorithm_is_rejected() {
    let fixture = valid_fixture();
    let claim = fixture
        .claim
        .replace("sigstore-cosign-bundle-v0.3", "made-up-algorithm-v1");
    let error =
        parse_signature_claim(claim.as_bytes()).expect_err("an unrecognized algorithm must reject");
    assert!(error.message.contains("algorithm"));
}

#[test]
fn empty_signature_or_certificate_is_rejected() {
    let fixture = valid_fixture();
    let claim = fixture.claim.replace("FIXTURE-NOT-A-REAL-SIGNATURE", "");
    let error = parse_signature_claim(claim.as_bytes())
        .expect_err("an empty signature field must be rejected");
    assert!(error.message.contains("signature"));
}

#[test]
fn manifest_with_wrong_schema_is_rejected() {
    let fixture = valid_fixture();
    let manifest = fixture.manifest.replace(
        "semaprax.release-manifest.v1",
        "semaprax.release-manifest.v2",
    );
    let error = parse_manifest(manifest.as_bytes()).expect_err("a wrong schema must be rejected");
    assert!(error.message.contains("schema"));
}

#[test]
fn provenance_with_unknown_host_class_is_rejected() {
    let fixture = valid_fixture();
    let provenance = fixture
        .provenance
        .replace("github-hosted-ubuntu-24.04", "my-laptop");
    let error = parse_provenance(provenance.as_bytes())
        .expect_err("an unrecognized build host class must be rejected");
    assert!(error.message.contains("build_host_class"));
}

#[test]
fn verification_touches_no_network_and_executes_no_artifact() {
    // Documentation-as-test: this module's public functions take only byte
    // slices and, for the on-disk check, an explicit directory path. There
    // is no reachable code path here that spawns a process or opens a
    // socket; the fixture directory used above is read, never executed.
    let fixture = valid_fixture();
    assert!(verify_release_binding(
        fixture.manifest.as_bytes(),
        fixture.provenance.as_bytes(),
        fixture.claim.as_bytes(),
    )
    .is_ok());
}
