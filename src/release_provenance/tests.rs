//! Unit tests for release provenance/signature-claim binding (#168).
//!
//! Each hostile case here is one of the required tests issue #168 names:
//! single-byte mutation to the manifest, artifact, or claim; wrong
//! repository/workflow identity; a provenance statement for a different
//! commit/tag; a missing or extra artifact; and a signature claim replayed
//! from another version.

use std::cell::{Cell, RefCell};
use std::fs;

use super::*;

const FAKE_DIGEST_A: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FAKE_DIGEST_B: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FAKE_DIGEST_C: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const FAKE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const FIXTURE_CLAIM_SIGNATURE: &str = "RklYVFVSRS1OT1QtQS1SRUFMLVNJR05BVFVSRQ==";
const FIXTURE_CLAIM_CERTIFICATE: &str = "RklYVFVSRS1OT1QtQS1SRUFMLUNFUlRJRklDQVRF";
const FIXTURE_BUNDLE_SIGNATURE: &str = "RklYVFVSRS1TSUdTVE9SRS1TSUdOQVRVUkU=";
const FIXTURE_BUNDLE_CERTIFICATE: &str = "RklYVFVSRS1TSUdTVE9SRS1DRVJUSUZJQ0FURQ==";

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
    {{"name": "semaprax-{tag}-aarch64-unknown-linux-gnu.tar.gz", "platform": "aarch64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
    {{"name": "semaprax-{tag}-x86_64-apple-darwin.tar.gz", "platform": "x86_64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
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
    {{"name": "semaprax-{tag}-aarch64-unknown-linux-gnu.tar.gz", "platform": "aarch64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
    {{"name": "semaprax-{tag}-x86_64-apple-darwin.tar.gz", "platform": "x86_64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
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
    let subject = format!("{TRUSTED_OIDC_SUBJECT_PREFIX}:ref:refs/tags/{tag}");
    format!(
        r#"{{
  "schema": "semaprax.release-signature-claim.v1",
  "subject_digest": "{subject_digest}",
  "subject_name": "release-provenance.json",
  "identity": {{"issuer": "{TRUSTED_ISSUER}", "subject": "{subject}", "workflow_ref": "{workflow_ref}"}},
  "algorithm": "sigstore-cosign-bundle-v0.3",
  "signature": "{FIXTURE_CLAIM_SIGNATURE}",
  "certificate": "{FIXTURE_CLAIM_CERTIFICATE}"
}}"#
    )
}

fn standard_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[((first & 0x03) << 4 | second >> 4) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((second & 0x0f) << 2 | third >> 6) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(third & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn verification_material(kind: &str, certificate: &str) -> String {
    format!(
        r#"{{"certificate":{{"rawBytes":"{certificate}"}},"tlogEntries":[{{"logIndex":"1","logId":{{"keyId":"RklYVFVSRS1SRUtPUi1LRVk="}},"kindVersion":{{"kind":"{kind}","version":"0.0.1"}},"integratedTime":"1","inclusionPromise":{{"signedEntryTimestamp":"RklYVFVSRS1TRVQ="}},"inclusionProof":{{"logIndex":"1","rootHash":"RklYVFVSRS1ST09U","treeSize":"1","hashes":["RklYVFVSRS1IQVNI"],"checkpoint":{{"envelope":"fixture checkpoint"}}}},"canonicalizedBody":"RklYVFVSRS1SRUtPUi1CT0RZ"}}],"timestampVerificationData":{{"rfc3161Timestamps":[{{"signedTimestamp":"RklYVFVSRS1SRkMzMTYx"}}]}}}}"#
    )
}

fn message_signature_bundle(provenance_bytes: &[u8], signature: &str, certificate: &str) -> String {
    let digest = sha256_digest(provenance_bytes);
    let raw_digest = digest.strip_prefix("sha256:").unwrap();
    let digest_bytes: Vec<u8> = (0..raw_digest.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&raw_digest[offset..offset + 2], 16).unwrap())
        .collect();
    let encoded_digest = standard_base64(&digest_bytes);
    let material = verification_material("hashedrekord", certificate);
    format!(
        r#"{{"mediaType":"{SIGSTORE_BUNDLE_MEDIA_TYPE}","verificationMaterial":{material},"messageSignature":{{"messageDigest":{{"algorithm":"SHA2_256","digest":"{encoded_digest}"}},"signature":"{signature}"}}}}"#
    )
}

fn github_artifact_predicate() -> String {
    format!(
        r#"{{"buildDefinition":{{"buildType":"https://actions.github.io/buildtypes/workflow/v1","externalParameters":{{"workflow":{{"path":".github/workflows/ci.yml","ref":"refs/tags/v9.9.9","repository":"https://github.com/wavect/semaprax"}}}},"internalParameters":{{"github":{{"event_name":"push","repository_id":"1","repository_owner_id":"1","runner_environment":"github-hosted"}}}},"resolvedDependencies":[{{"digest":{{"gitCommit":"{FAKE_COMMIT}"}},"uri":"git+https://github.com/wavect/semaprax@refs/tags/v9.9.9"}}]}},"runDetails":{{"builder":{{"id":"https://github.com/actions/runner/github-hosted"}},"metadata":{{"invocationId":"https://github.com/wavect/semaprax/actions/runs/1/attempts/1"}}}}}}"#
    )
}

fn archive_attestation_bundle_with_predicate(
    archive_name: &str,
    archive_bytes: &[u8],
    predicate: &str,
) -> String {
    let digest = sha256_digest(archive_bytes);
    let raw_digest = digest.strip_prefix("sha256:").unwrap();
    let statement = format!(
        r#"{{"_type":"{IN_TOTO_STATEMENT_TYPE}","subject":[{{"name":"{archive_name}","digest":{{"sha256":"{raw_digest}"}}}}],"predicateType":"{SLSA_PROVENANCE_V1_PREDICATE_TYPE}","predicate":{predicate}}}"#
    );
    let payload = standard_base64(statement.as_bytes());
    let material = verification_material("dsse", "RklYVFVSRS1BVFRFU1RBVElPTi1DRVJUSUZJQ0FURQ==");
    format!(
        r#"{{"mediaType":"{SIGSTORE_BUNDLE_MEDIA_TYPE}","verificationMaterial":{material},"dsseEnvelope":{{"payload":"{payload}","payloadType":"{DSSE_IN_TOTO_PAYLOAD_TYPE}","signatures":[{{"sig":"RklYVFVSRS1EU1NFLVNJR05BVFVSRQ=="}}]}}}}"#
    )
}

fn archive_attestation_bundle(archive_name: &str, archive_bytes: &[u8]) -> String {
    archive_attestation_bundle_with_predicate(
        archive_name,
        archive_bytes,
        &github_artifact_predicate(),
    )
}

fn manifest_for_archive_bytes(tag: &str, bytes: &[u8]) -> String {
    let digest = sha256_digest(bytes);
    manifest_json(tag, FAKE_COMMIT)
        .replace(FAKE_DIGEST_A, &digest)
        .replace(FAKE_DIGEST_B, &digest)
        .replace(FAKE_DIGEST_C, &digest)
        .replace("\"size\": 20", &format!("\"size\": {}", bytes.len()))
        .replace("\"size\": 30", &format!("\"size\": {}", bytes.len()))
}

fn provenance_for_archive_bytes(tag: &str, bytes: &[u8], manifest_bytes: &[u8]) -> String {
    let digest = sha256_digest(bytes);
    provenance_json(tag, FAKE_COMMIT, manifest_bytes)
        .replace(FAKE_DIGEST_A, &digest)
        .replace(FAKE_DIGEST_B, &digest)
        .replace(FAKE_DIGEST_C, &digest)
        .replace("\"size\": 20", &format!("\"size\": {}", bytes.len()))
        .replace("\"size\": 30", &format!("\"size\": {}", bytes.len()))
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

fn fixture_expected_identity() -> ExpectedReleaseIdentity {
    ExpectedReleaseIdentity {
        issuer: TRUSTED_ISSUER.to_owned(),
        repository: TRUSTED_REPOSITORY.to_owned(),
        workflow_path: TRUSTED_WORKFLOW_PATH.to_owned(),
        tag: "v9.9.9".to_owned(),
        subject: "repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/v9.9.9".to_owned(),
        workflow_ref: format!("{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/v9.9.9"),
    }
}

fn assert_fixture_expected_identity(identity: &ExpectedReleaseIdentity) {
    assert_eq!(identity, &fixture_expected_identity());
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
    {{"name": "semaprax-v9.9.9-aarch64-unknown-linux-gnu.tar.gz", "platform": "aarch64-unknown-linux-gnu", "size": 10, "digest": "{real_digest}"}},
    {{"name": "semaprax-v9.9.9-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 10, "digest": "{real_digest}"}},
    {{"name": "semaprax-v9.9.9-x86_64-apple-darwin.tar.gz", "platform": "x86_64-apple-darwin", "size": 10, "digest": "{real_digest}"}},
    {{"name": "semaprax-v9.9.9-x86_64-pc-windows-msvc.zip", "platform": "x86_64-pc-windows-msvc", "size": 10, "digest": "{real_digest}"}}
  ]
}}"#
    );
    for extra in [
        "semaprax-v9.9.9-aarch64-apple-darwin.tar.gz",
        "semaprax-v9.9.9-aarch64-unknown-linux-gnu.tar.gz",
        "semaprax-v9.9.9-x86_64-apple-darwin.tar.gz",
    ] {
        fs::write(dir.join(extra), &real_bytes).unwrap();
    }
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
    {{"name": "semaprax-{tag}-aarch64-unknown-linux-gnu.tar.gz", "platform": "aarch64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
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
    {{"name": "semaprax-{tag}-aarch64-unknown-linux-gnu.tar.gz", "platform": "aarch64-unknown-linux-gnu", "size": 10, "digest": "{FAKE_DIGEST_A}"}},
    {{"name": "semaprax-{tag}-aarch64-apple-darwin.tar.gz", "platform": "aarch64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
    {{"name": "semaprax-{tag}-x86_64-apple-darwin.tar.gz", "platform": "x86_64-apple-darwin", "size": 20, "digest": "{FAKE_DIGEST_B}"}},
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
fn signature_claim_with_legacy_or_wrong_immutable_subject_is_rejected() {
    let fixture = valid_fixture();
    for subject in [
        "repo:wavect/semaprax:ref:refs/tags/v9.9.9",
        "repo:wavect@47505194/semaprax@1:ref:refs/tags/v9.9.9",
    ] {
        let claim = fixture.claim.replace(
            "repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/v9.9.9",
            subject,
        );
        let error = verify_signature_claim_binds_provenance(
            claim.as_bytes(),
            fixture.provenance.as_bytes(),
        )
        .expect_err("a legacy or wrong-repository-id subject must be rejected");
        assert!(error.message.contains("identity.subject"));
    }
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
    let claim = fixture.claim.replace(FIXTURE_CLAIM_SIGNATURE, "");
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

// -- `SignatureVerificationCapability` plumbing and a throwaway-key demo --
//
// No real signature-verification implementation exists anywhere in this
// repository (see the module doc): there is no cryptography dependency and
// no signing key or keyless identity. The tests below establish two
// separate things without pretending either is production signing:
//
// 1. `verify_release_binding_with_capability` actually calls the supplied
//    capability -- it is not a decoration that gets silently skipped -- and
//    still fails closed on the binding checks before ever reaching it.
// 2. A capability implementation can perform genuine cryptographic
//    verification (HMAC-SHA256, already a pinned dependency via
//    `semantic_cache_store`'s use of the same crate) against a key
//    generated fresh inside this test function and used nowhere else. This
//    is a throwaway test key demonstrating the interface, never a released
//    artifact's signature and never a claim about HMAC being the release
//    signing algorithm -- `docs/RELEASE-SIGNING-POLICY-V1.md` names
//    Sigstore/cosign as the real target.

struct AlwaysOkCapability;

impl SignatureVerificationCapability for AlwaysOkCapability {
    fn verify_signature(
        &self,
        expected_identity: &ExpectedReleaseIdentity,
        _subject_bytes: &[u8],
        _claim: &ParsedSignatureClaim,
    ) -> Result<(), Diagnostic> {
        assert_fixture_expected_identity(expected_identity);
        Ok(())
    }
}

/// The message a rejecting test capability returns. The assertions key off
/// this rather than off a diagnostic code, because a test must not invent one:
/// `build.rs` scans every `SPX-` token under `src/` into the public installed
/// diagnostic catalog, so a test-only code would ship as a code the compiler
/// can never emit. The message is unique to these stubs, so it distinguishes a
/// capability rejection from a binding rejection just as precisely.
const CAPABILITY_REJECTED: &str = "test capability unconditionally rejects";

struct AlwaysRejectCapability;

impl SignatureVerificationCapability for AlwaysRejectCapability {
    fn verify_signature(
        &self,
        expected_identity: &ExpectedReleaseIdentity,
        _subject_bytes: &[u8],
        _claim: &ParsedSignatureClaim,
    ) -> Result<(), Diagnostic> {
        assert_fixture_expected_identity(expected_identity);
        Err(Diagnostic::io(
            // A test stub must not invent a new diagnostic code: `build.rs`
            // scans every `SPX-` token under `src/` into the public installed
            // diagnostic catalog, so a test-only code would ship as a code the
            // compiler can never emit. Reuse the real binding code and let the
            // unique message carry the assertion instead.
            "SPX-Z702",
            CAPABILITY_REJECTED.to_owned(),
        ))
    }
}

#[test]
fn capability_variant_still_runs_binding_checks_before_the_capability() {
    let fixture = valid_fixture();
    let tampered_provenance = fixture.provenance.replacen('9', "8", 1);
    let error = verify_release_binding_with_capability(
        fixture.manifest.as_bytes(),
        tampered_provenance.as_bytes(),
        fixture.claim.as_bytes(),
        &AlwaysOkCapability,
    )
    .expect_err("a tampered provenance document must be rejected before any capability runs");
    // A capability that always accepts must never be reached: the binding
    // check's own diagnostic code, not the capability's, is what surfaces.
    assert_ne!(error.message, CAPABILITY_REJECTED);
}

#[test]
fn capability_variant_actually_invokes_the_supplied_capability() {
    let fixture = valid_fixture();
    let error = verify_release_binding_with_capability(
        fixture.manifest.as_bytes(),
        fixture.provenance.as_bytes(),
        fixture.claim.as_bytes(),
        &AlwaysRejectCapability,
    )
    .expect_err("a rejecting capability must fail the overall verification");
    assert_eq!(error.message, CAPABILITY_REJECTED);

    assert!(verify_release_binding_with_capability(
        fixture.manifest.as_bytes(),
        fixture.provenance.as_bytes(),
        fixture.claim.as_bytes(),
        &AlwaysOkCapability,
    )
    .is_ok());
}

/// A throwaway-key HMAC-SHA256 stand-in for a real signature verifier,
/// existing only in this test module to demonstrate that
/// `SignatureVerificationCapability` can be implemented with genuine
/// cryptographic verification -- not merely structural string checks --
/// once a real algorithm and key/identity material exist. HMAC is a
/// symmetric MAC, not the asymmetric/keyless scheme
/// `docs/RELEASE-SIGNING-POLICY-V1.md` specifies for real releases; this
/// struct must never be read as a recommendation to use it for that.
struct ThrowawayHmacCapability {
    key: [u8; 32],
}

fn decode_hex_32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = hex.as_bytes();
    for (index, slot) in out.iter_mut().enumerate() {
        let high = (bytes[index * 2] as char).to_digit(16)?;
        let low = (bytes[index * 2 + 1] as char).to_digit(16)?;
        *slot = ((high << 4) | low) as u8;
    }
    Some(out)
}

impl SignatureVerificationCapability for ThrowawayHmacCapability {
    fn verify_signature(
        &self,
        expected_identity: &ExpectedReleaseIdentity,
        subject_bytes: &[u8],
        claim: &ParsedSignatureClaim,
    ) -> Result<(), Diagnostic> {
        assert_fixture_expected_identity(expected_identity);
        use hmac::{Hmac, KeyInit, Mac};
        let tag = decode_hex_32(&claim.signature).ok_or_else(|| {
            Diagnostic::io(
                "SPX-Z702",
                "throwaway HMAC signature must be 64 lowercase hex characters".to_owned(),
            )
        })?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key)
            .expect("HMAC-SHA256 accepts a 32-byte key of any value");
        mac.update(subject_bytes);
        mac.verify_slice(&tag).map_err(|_| {
            Diagnostic::io(
                "SPX-Z702",
                "throwaway HMAC signature does not verify against the supplied key and subject \
                 bytes"
                    .to_owned(),
            )
        })
    }
}

fn hmac_tag_hex(key: &[u8; 32], subject_bytes: &[u8]) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("32-byte key is always accepted");
    mac.update(subject_bytes);
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(mac.finalize().into_bytes())
    )
}

#[test]
fn throwaway_hmac_capability_verifies_a_correct_signature_and_rejects_tampering() {
    let key: [u8; 32] = *b"throwaway-test-key-not-a-secret1";
    let subject_bytes = b"exact provenance bytes under test";
    let claim = ParsedSignatureClaim {
        subject_digest: sha256_digest(subject_bytes),
        algorithm: "test-only-hmac-sha256".to_owned(),
        identity_issuer: TRUSTED_ISSUER.to_owned(),
        identity_subject: "repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/v9.9.9"
            .to_owned(),
        identity_workflow_ref: format!(
            "{TRUSTED_REPOSITORY}/{TRUSTED_WORKFLOW_PATH}@refs/tags/v9.9.9"
        ),
        signature: hmac_tag_hex(&key, subject_bytes),
        certificate: "throwaway-test-only".to_owned(),
    };
    let capability = ThrowawayHmacCapability { key };
    let expected_identity = fixture_expected_identity();

    assert!(capability
        .verify_signature(&expected_identity, subject_bytes, &claim)
        .is_ok());

    let wrong_key: [u8; 32] = *b"a-different-throwaway-key-value1";
    let wrong_key_capability = ThrowawayHmacCapability { key: wrong_key };
    assert!(wrong_key_capability
        .verify_signature(&expected_identity, subject_bytes, &claim)
        .is_err());

    let tampered_subject: &[u8] = b"exact provenance bytes under tesT";
    assert!(capability
        .verify_signature(&expected_identity, tampered_subject, &claim)
        .is_err());

    let mut tampered_claim = claim.clone();
    tampered_claim.signature = hmac_tag_hex(&key, b"a completely different payload");
    assert!(capability
        .verify_signature(&expected_identity, subject_bytes, &tampered_claim)
        .is_err());
}

const FIXTURE_TRUSTED_ROOT: &[u8] = b"{\"trustedRoot\":\"fixture\"}\n";

struct ExactOfflineInputsCapability<'a> {
    expected_subject: &'a [u8],
    expected_bundle: &'a [u8],
    expected_root: &'a [u8],
    invoked: Cell<bool>,
}

struct AggregateOfflineCapability {
    subjects: RefCell<Vec<Vec<u8>>>,
}

impl OfflineBundleVerificationCapability for AggregateOfflineCapability {
    fn verify_offline_bundle(
        &self,
        expected_identity: &ExpectedReleaseIdentity,
        subject_bytes: &[u8],
        _bundle_bytes: &[u8],
        trusted_root_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        assert_eq!(expected_identity, &fixture_expected_identity());
        assert_eq!(trusted_root_bytes, FIXTURE_TRUSTED_ROOT);
        self.subjects.borrow_mut().push(subject_bytes.to_vec());
        Ok(())
    }
}

impl OfflineBundleVerificationCapability for ExactOfflineInputsCapability<'_> {
    fn verify_offline_bundle(
        &self,
        expected_identity: &ExpectedReleaseIdentity,
        subject_bytes: &[u8],
        bundle_bytes: &[u8],
        trusted_root_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        assert_fixture_expected_identity(expected_identity);
        assert_eq!(subject_bytes, self.expected_subject);
        assert_eq!(bundle_bytes, self.expected_bundle);
        assert_eq!(trusted_root_bytes, self.expected_root);
        self.invoked.set(true);
        Ok(())
    }
}

#[test]
fn archive_attestation_binds_exact_archive_bytes_and_manifest_digest() {
    let archive_name = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    let archive_bytes = b"0123456789";
    let manifest = manifest_for_archive_bytes("v9.9.9", archive_bytes);
    let bundle = archive_attestation_bundle(archive_name, archive_bytes);

    verify_archive_attestation_binds_manifest(
        manifest.as_bytes(),
        archive_name,
        archive_bytes,
        bundle.as_bytes(),
    )
    .expect("the archive subject and manifest must bind to the same exact bytes");

    let error = verify_archive_attestation_binds_manifest(
        manifest.as_bytes(),
        archive_name,
        b"9123456789",
        bundle.as_bytes(),
    )
    .expect_err("one changed archive byte must fail before any cryptographic verifier runs");
    assert!(error.message.contains("digest"));
}

#[test]
fn archive_attestation_for_a_different_archive_is_rejected() {
    let archive_name = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    let archive_bytes = b"0123456789";
    let manifest = manifest_for_archive_bytes("v9.9.9", archive_bytes);
    let bundle =
        archive_attestation_bundle("semaprax-v9.9.9-aarch64-apple-darwin.tar.gz", archive_bytes);
    let error = verify_archive_attestation_binds_manifest(
        manifest.as_bytes(),
        archive_name,
        archive_bytes,
        bundle.as_bytes(),
    )
    .expect_err("an attestation from another archive must not be reusable");
    assert!(error.message.contains("subject"));
}

#[test]
fn signature_claim_must_consume_the_exact_sigstore_bundle_material() {
    let fixture = valid_fixture();
    let signature = FIXTURE_BUNDLE_SIGNATURE;
    let certificate = FIXTURE_BUNDLE_CERTIFICATE;
    let claim = fixture
        .claim
        .replace(FIXTURE_CLAIM_SIGNATURE, signature)
        .replace(FIXTURE_CLAIM_CERTIFICATE, certificate);
    let bundle = message_signature_bundle(fixture.provenance.as_bytes(), signature, certificate);

    verify_signature_claim_consumes_sigstore_bundle(
        claim.as_bytes(),
        fixture.provenance.as_bytes(),
        bundle.as_bytes(),
    )
    .expect("a claim must consume the exact signature and certificate in its bundle");

    let changed_signature_bundle = message_signature_bundle(
        fixture.provenance.as_bytes(),
        "UkVQTEFZRUQtU0lHU1RPUkUtU0lHTkFUVVJF",
        certificate,
    );
    let error = verify_signature_claim_consumes_sigstore_bundle(
        claim.as_bytes(),
        fixture.provenance.as_bytes(),
        changed_signature_bundle.as_bytes(),
    )
    .expect_err("a claim must reject a bundle carrying different signature bytes");
    assert!(error.message.contains("signature"));
}

#[test]
fn offline_capability_receives_exact_root_bundle_and_subject_only_after_binding() {
    let fixture = valid_fixture();
    let signature = FIXTURE_BUNDLE_SIGNATURE;
    let certificate = FIXTURE_BUNDLE_CERTIFICATE;
    let claim = fixture
        .claim
        .replace(FIXTURE_CLAIM_SIGNATURE, signature)
        .replace(FIXTURE_CLAIM_CERTIFICATE, certificate);
    let bundle = message_signature_bundle(fixture.provenance.as_bytes(), signature, certificate);
    let capability = ExactOfflineInputsCapability {
        expected_subject: fixture.provenance.as_bytes(),
        expected_bundle: bundle.as_bytes(),
        expected_root: FIXTURE_TRUSTED_ROOT,
        invoked: Cell::new(false),
    };

    verify_signature_claim_with_offline_capability(
        fixture.manifest.as_bytes(),
        claim.as_bytes(),
        fixture.provenance.as_bytes(),
        bundle.as_bytes(),
        FIXTURE_TRUSTED_ROOT,
        &capability,
    )
    .expect("well-bound exact bytes must reach the caller-supplied verifier");
    assert!(capability.invoked.get());

    let malformed_root = b"{\"trustedRoot\":\"fixture\"}";
    let capability = ExactOfflineInputsCapability {
        expected_subject: fixture.provenance.as_bytes(),
        expected_bundle: bundle.as_bytes(),
        expected_root: malformed_root,
        invoked: Cell::new(false),
    };
    let error = verify_signature_claim_with_offline_capability(
        fixture.manifest.as_bytes(),
        claim.as_bytes(),
        fixture.provenance.as_bytes(),
        bundle.as_bytes(),
        malformed_root,
        &capability,
    )
    .expect_err("a non-JSONL trusted root package must fail before the verifier is invoked");
    assert!(error.message.contains("trusted-root"));
    assert!(!capability.invoked.get());
}

#[test]
fn unsupported_bundle_variant_and_noncanonical_base64_fail_closed() {
    let fixture = valid_fixture();
    let bundle = message_signature_bundle(
        fixture.provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let dsse_variant = bundle.replace("messageSignature", "dsseEnvelope");
    let error = parse_sigstore_message_signature_bundle(dsse_variant.as_bytes())
        .expect_err("a DSSE archive bundle must not be accepted as a blob-signature bundle");
    assert!(error.message.contains("keys"));

    let noncanonical = bundle.replacen("=\"},\"signature", "A=\"},\"signature", 1);
    let error = parse_sigstore_message_signature_bundle(noncanonical.as_bytes())
        .expect_err("noncanonical base64 digest material must reject");
    assert!(error.message.contains("base64"));
}

#[test]
fn aggregate_offline_release_binds_the_complete_inventory_before_capabilities_run() {
    let archive_bytes = b"0123456789";
    let manifest = manifest_for_archive_bytes("v9.9.9", archive_bytes);
    let provenance = provenance_for_archive_bytes("v9.9.9", archive_bytes, manifest.as_bytes());
    let claim = claim_json("v9.9.9", provenance.as_bytes())
        .replace(FIXTURE_CLAIM_SIGNATURE, FIXTURE_BUNDLE_SIGNATURE)
        .replace(FIXTURE_CLAIM_CERTIFICATE, FIXTURE_BUNDLE_CERTIFICATE);
    let message_bundle = message_signature_bundle(
        provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let linux = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    let macos = "semaprax-v9.9.9-aarch64-apple-darwin.tar.gz";
    let windows = "semaprax-v9.9.9-x86_64-pc-windows-msvc.zip";
    let linux_arm = "semaprax-v9.9.9-aarch64-unknown-linux-gnu.tar.gz";
    let macos_intel = "semaprax-v9.9.9-x86_64-apple-darwin.tar.gz";
    let linux_bundle = archive_attestation_bundle(linux, archive_bytes);
    let macos_bundle = archive_attestation_bundle(macos, archive_bytes);
    let windows_bundle = archive_attestation_bundle(windows, archive_bytes);
    let linux_arm_bundle = archive_attestation_bundle(linux_arm, archive_bytes);
    let macos_intel_bundle = archive_attestation_bundle(macos_intel, archive_bytes);
    let archives = [
        OfflineReleaseArchive {
            name: linux_arm,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_arm_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: macos_intel,
            bytes: archive_bytes,
            attestation_bundle_bytes: macos_intel_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: windows,
            bytes: archive_bytes,
            attestation_bundle_bytes: windows_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: linux,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: macos,
            bytes: archive_bytes,
            attestation_bundle_bytes: macos_bundle.as_bytes(),
        },
    ];
    let capability = AggregateOfflineCapability {
        subjects: RefCell::new(Vec::new()),
    };

    verify_offline_release_with_capability(
        manifest.as_bytes(),
        provenance.as_bytes(),
        claim.as_bytes(),
        message_bundle.as_bytes(),
        FIXTURE_TRUSTED_ROOT,
        &archives,
        &capability,
    )
    .expect("the complete release inventory must reach the verifier in canonical order");
    let subjects = capability.subjects.into_inner();
    assert_eq!(subjects.len(), 1 + ARCHIVE_PLATFORMS.len());
    assert_eq!(subjects[0], provenance.as_bytes());
    for subject in &subjects[1..] {
        assert_eq!(*subject, archive_bytes);
    }

    let malformed_linux_bundle =
        archive_attestation_bundle_with_predicate(linux, archive_bytes, "{}");
    let malformed_archives = [
        OfflineReleaseArchive {
            name: linux_arm,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_arm_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: macos_intel,
            bytes: archive_bytes,
            attestation_bundle_bytes: macos_intel_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: windows,
            bytes: archive_bytes,
            attestation_bundle_bytes: windows_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: linux,
            bytes: archive_bytes,
            attestation_bundle_bytes: malformed_linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: macos,
            bytes: archive_bytes,
            attestation_bundle_bytes: macos_bundle.as_bytes(),
        },
    ];
    let capability = AggregateOfflineCapability {
        subjects: RefCell::new(Vec::new()),
    };
    let error = verify_offline_release_with_capability(
        manifest.as_bytes(),
        provenance.as_bytes(),
        claim.as_bytes(),
        message_bundle.as_bytes(),
        FIXTURE_TRUSTED_ROOT,
        &malformed_archives,
        &capability,
    )
    .expect_err("a predicate placeholder must fail before any capability invocation");
    assert!(error.message.contains("predicate"));
    assert!(capability.subjects.borrow().is_empty());
}

#[test]
fn aggregate_release_rejects_archive_attestation_identity_or_commit_replays_before_capability() {
    let archive_bytes = b"0123456789";
    let manifest = manifest_for_archive_bytes("v9.9.9", archive_bytes);
    let provenance = provenance_for_archive_bytes("v9.9.9", archive_bytes, manifest.as_bytes());
    let claim = claim_json("v9.9.9", provenance.as_bytes())
        .replace(FIXTURE_CLAIM_SIGNATURE, FIXTURE_BUNDLE_SIGNATURE)
        .replace(FIXTURE_CLAIM_CERTIFICATE, FIXTURE_BUNDLE_CERTIFICATE);
    let message_bundle = message_signature_bundle(
        provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let linux = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    let macos = "semaprax-v9.9.9-aarch64-apple-darwin.tar.gz";
    let windows = "semaprax-v9.9.9-x86_64-pc-windows-msvc.zip";
    let linux_arm = "semaprax-v9.9.9-aarch64-unknown-linux-gnu.tar.gz";
    let macos_intel = "semaprax-v9.9.9-x86_64-apple-darwin.tar.gz";
    let macos_bundle = archive_attestation_bundle(macos, archive_bytes);
    let windows_bundle = archive_attestation_bundle(windows, archive_bytes);
    let linux_arm_bundle = archive_attestation_bundle(linux_arm, archive_bytes);
    let macos_intel_bundle = archive_attestation_bundle(macos_intel, archive_bytes);

    for (description, predicate, code) in [
        (
            "another repository",
            github_artifact_predicate().replace(
                "https://github.com/wavect/semaprax",
                "https://github.com/wavect/other",
            ),
            "SPX-Z703",
        ),
        (
            "another workflow path",
            github_artifact_predicate()
                .replace(".github/workflows/ci.yml", ".github/workflows/other.yml"),
            "SPX-Z703",
        ),
        (
            "another tag",
            github_artifact_predicate().replace("refs/tags/v9.9.9", "refs/tags/v9.9.8"),
            "SPX-Z703",
        ),
        (
            "another source commit",
            github_artifact_predicate()
                .replace(FAKE_COMMIT, "1111111111111111111111111111111111111111"),
            "SPX-Z702",
        ),
    ] {
        let linux_bundle =
            archive_attestation_bundle_with_predicate(linux, archive_bytes, &predicate);
        let archives = [
            OfflineReleaseArchive {
                name: linux_arm,
                bytes: archive_bytes,
                attestation_bundle_bytes: linux_arm_bundle.as_bytes(),
            },
            OfflineReleaseArchive {
                name: macos_intel,
                bytes: archive_bytes,
                attestation_bundle_bytes: macos_intel_bundle.as_bytes(),
            },
            OfflineReleaseArchive {
                name: windows,
                bytes: archive_bytes,
                attestation_bundle_bytes: windows_bundle.as_bytes(),
            },
            OfflineReleaseArchive {
                name: linux,
                bytes: archive_bytes,
                attestation_bundle_bytes: linux_bundle.as_bytes(),
            },
            OfflineReleaseArchive {
                name: macos,
                bytes: archive_bytes,
                attestation_bundle_bytes: macos_bundle.as_bytes(),
            },
        ];
        let capability = AggregateOfflineCapability {
            subjects: RefCell::new(Vec::new()),
        };
        let error = verify_offline_release_with_capability(
            manifest.as_bytes(),
            provenance.as_bytes(),
            claim.as_bytes(),
            message_bundle.as_bytes(),
            FIXTURE_TRUSTED_ROOT,
            &archives,
            &capability,
        )
        .expect_err(description);
        assert_eq!(error.code, code, "{description}");
        assert!(capability.subjects.borrow().is_empty(), "{description}");
    }
}

#[test]
fn aggregate_release_rejects_duplicate_or_missing_archives_before_any_capability() {
    let archive_bytes = b"0123456789";
    let manifest = manifest_for_archive_bytes("v9.9.9", archive_bytes);
    let provenance = provenance_for_archive_bytes("v9.9.9", archive_bytes, manifest.as_bytes());
    let claim = claim_json("v9.9.9", provenance.as_bytes())
        .replace(FIXTURE_CLAIM_SIGNATURE, FIXTURE_BUNDLE_SIGNATURE)
        .replace(FIXTURE_CLAIM_CERTIFICATE, FIXTURE_BUNDLE_CERTIFICATE);
    let message_bundle = message_signature_bundle(
        provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let linux = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    let linux_bundle = archive_attestation_bundle(linux, archive_bytes);
    let archives = [
        OfflineReleaseArchive {
            name: linux,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: linux,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: "unexpected-extra",
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: "unexpected-extra-2",
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: "unexpected-extra-3",
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
    ];
    let capability = AggregateOfflineCapability {
        subjects: RefCell::new(Vec::new()),
    };
    let error = verify_offline_release_with_capability(
        manifest.as_bytes(),
        provenance.as_bytes(),
        claim.as_bytes(),
        message_bundle.as_bytes(),
        FIXTURE_TRUSTED_ROOT,
        &archives,
        &capability,
    )
    .expect_err("duplicate or missing archive inventory must reject before capability use");
    assert_eq!(
        error.message,
        "offline archive inventory has an empty or repeated name"
    );
    assert!(capability.subjects.borrow().is_empty());
}

/// Differential proof that `SigstoreOfflineVerifier` is a real cryptographic
/// engine actually invoked at the aggregate release boundary, not dead code
/// the aggregate function never calls. This exact fixture -- manifest,
/// provenance, claim, message-signature bundle, and three archive
/// attestations -- is fully self-consistent: every digest, tag, commit, and
/// identity binds, which `AggregateOfflineCapability` proves by asserting on
/// its recorded identity/root and unconditionally accepting. Its signature,
/// certificate, and transparency-log bytes are fabricated, never produced by
/// any real signing operation (this repository has none, per
/// `docs/RELEASE-SIGNING-POLICY-V1.md`). Swapping in `SigstoreOfflineVerifier`
/// over the identical bytes must still fail, and specifically with the
/// cryptographic-layer code `SPX-Z707` rather than an earlier structural or
/// binding code, because the structural/binding gates already passed under
/// the caller-supplied capability above.
#[test]
fn aggregate_release_actually_invokes_the_built_in_sigstore_verifier() {
    let archive_bytes = b"0123456789";
    let manifest = manifest_for_archive_bytes("v9.9.9", archive_bytes);
    let provenance = provenance_for_archive_bytes("v9.9.9", archive_bytes, manifest.as_bytes());
    let claim = claim_json("v9.9.9", provenance.as_bytes())
        .replace(FIXTURE_CLAIM_SIGNATURE, FIXTURE_BUNDLE_SIGNATURE)
        .replace(FIXTURE_CLAIM_CERTIFICATE, FIXTURE_BUNDLE_CERTIFICATE);
    let message_bundle = message_signature_bundle(
        provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let linux = "semaprax-v9.9.9-x86_64-unknown-linux-gnu.tar.gz";
    let macos = "semaprax-v9.9.9-aarch64-apple-darwin.tar.gz";
    let windows = "semaprax-v9.9.9-x86_64-pc-windows-msvc.zip";
    let linux_arm = "semaprax-v9.9.9-aarch64-unknown-linux-gnu.tar.gz";
    let macos_intel = "semaprax-v9.9.9-x86_64-apple-darwin.tar.gz";
    let linux_bundle = archive_attestation_bundle(linux, archive_bytes);
    let macos_bundle = archive_attestation_bundle(macos, archive_bytes);
    let windows_bundle = archive_attestation_bundle(windows, archive_bytes);
    let linux_arm_bundle = archive_attestation_bundle(linux_arm, archive_bytes);
    let macos_intel_bundle = archive_attestation_bundle(macos_intel, archive_bytes);
    let archives = [
        OfflineReleaseArchive {
            name: linux_arm,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_arm_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: macos_intel,
            bytes: archive_bytes,
            attestation_bundle_bytes: macos_intel_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: linux,
            bytes: archive_bytes,
            attestation_bundle_bytes: linux_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: macos,
            bytes: archive_bytes,
            attestation_bundle_bytes: macos_bundle.as_bytes(),
        },
        OfflineReleaseArchive {
            name: windows,
            bytes: archive_bytes,
            attestation_bundle_bytes: windows_bundle.as_bytes(),
        },
    ];

    let accepting_capability = AggregateOfflineCapability {
        subjects: RefCell::new(Vec::new()),
    };
    verify_offline_release_with_capability(
        manifest.as_bytes(),
        provenance.as_bytes(),
        claim.as_bytes(),
        message_bundle.as_bytes(),
        FIXTURE_TRUSTED_ROOT,
        &archives,
        &accepting_capability,
    )
    .expect("the fixture must pass every structural and binding check on its own");
    assert_eq!(
        accepting_capability.subjects.borrow().len(),
        1 + ARCHIVE_PLATFORMS.len(),
        "the accepting capability must have been reached for the provenance subject and every archive"
    );

    let error = verify_offline_release_with_capability(
        manifest.as_bytes(),
        provenance.as_bytes(),
        claim.as_bytes(),
        message_bundle.as_bytes(),
        FIXTURE_TRUSTED_ROOT,
        &archives,
        &SigstoreOfflineVerifier,
    )
    .expect_err("fabricated signature/certificate material must not cryptographically verify");
    assert_eq!(error.code, "SPX-Z707");
}

#[test]
fn non_base64_bundle_certificate_and_signatures_are_rejected() {
    let fixture = valid_fixture();
    let bundle = message_signature_bundle(
        fixture.provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let bad_certificate = bundle.replace(FIXTURE_BUNDLE_CERTIFICATE, "NOT*BASE64");
    let error = parse_sigstore_message_signature_bundle(bad_certificate.as_bytes())
        .expect_err("certificate rawBytes must be canonical standard base64");
    assert!(error.message.contains("base64"));
    let bad_signature = bundle.replace(FIXTURE_BUNDLE_SIGNATURE, "NOT*BASE64");
    let error = parse_sigstore_message_signature_bundle(bad_signature.as_bytes())
        .expect_err("message signatures must be canonical standard base64");
    assert!(error.message.contains("base64"));

    let archive = b"0123456789";
    let attestation = archive_attestation_bundle("archive", archive)
        .replace("RklYVFVSRS1EU1NFLVNJR05BVFVSRQ==", "NOT*BASE64");
    let error = parse_sigstore_archive_attestation_bundle(attestation.as_bytes())
        .expect_err("DSSE signatures must be canonical standard base64");
    assert!(error.message.contains("base64"));
}

#[test]
fn sigstore_tlog_and_timestamp_records_are_closed_and_validate_every_byte_field() {
    let fixture = valid_fixture();
    let bundle = message_signature_bundle(
        fixture.provenance.as_bytes(),
        FIXTURE_BUNDLE_SIGNATURE,
        FIXTURE_BUNDLE_CERTIFICATE,
    );
    let placeholder_material = format!(
        r#"{{"certificate":{{"rawBytes":"{FIXTURE_BUNDLE_CERTIFICATE}"}},"tlogEntries":[{{}}],"timestampVerificationData":{{}}}}"#
    );
    let empty_tlog = bundle.replacen(
        &verification_material("hashedrekord", FIXTURE_BUNDLE_CERTIFICATE),
        &placeholder_material,
        1,
    );
    let error = parse_sigstore_message_signature_bundle(empty_tlog.as_bytes())
        .expect_err("an empty tlog entry must not stand in for published v0.3 structure");
    assert!(error.message.contains("keys"));

    let noncanonical_decimal =
        bundle.replace("\"integratedTime\":\"1\"", "\"integratedTime\":\"01\"");
    let error = parse_sigstore_message_signature_bundle(noncanonical_decimal.as_bytes())
        .expect_err("tlog numeric fields must use protobuf JSON's canonical decimal spelling");
    assert!(error.message.contains("canonical unsigned decimal"));

    let maximum_i64 = "9223372036854775807";
    let boundary = bundle
        .replacen(
            "\"logIndex\":\"1\",\"logId\"",
            &format!("\"logIndex\":\"{maximum_i64}\",\"logId\""),
            1,
        )
        .replacen(
            "\"integratedTime\":\"1\"",
            &format!("\"integratedTime\":\"{maximum_i64}\""),
            1,
        )
        .replacen(
            "\"inclusionProof\":{\"logIndex\":\"1\"",
            &format!("\"inclusionProof\":{{\"logIndex\":\"{maximum_i64}\""),
            1,
        )
        .replacen(
            "\"treeSize\":\"1\"",
            &format!("\"treeSize\":\"{maximum_i64}\""),
            1,
        );
    parse_sigstore_message_signature_bundle(boundary.as_bytes())
        .expect("the inclusive i64 maximum is an admitted protobuf JSON integer");
    for (needle, replacement) in [
        (
            "\"logIndex\":\"1\",\"logId\"",
            "\"logIndex\":\"9223372036854775808\",\"logId\"",
        ),
        (
            "\"integratedTime\":\"1\"",
            "\"integratedTime\":\"9223372036854775808\"",
        ),
        (
            "\"inclusionProof\":{\"logIndex\":\"1\"",
            "\"inclusionProof\":{\"logIndex\":\"9223372036854775808\"",
        ),
        ("\"treeSize\":\"1\"", "\"treeSize\":\"9223372036854775808\""),
    ] {
        let over = bundle.replacen(needle, replacement, 1);
        let error = parse_sigstore_message_signature_bundle(over.as_bytes())
            .expect_err("tlog integer fields must not exceed signed int64");
        assert!(error.message.contains("nonnegative i64"));
    }

    for (bad, field) in [
        ("RklYVFVSRS1SRUtPUi1LRVk=", "logId.keyId"),
        ("RklYVFVSRS1TRVQ=", "signedEntryTimestamp"),
        ("RklYVFVSRS1ST09U", "rootHash"),
        ("RklYVFVSRS1IQVNI", "hashes"),
        ("RklYVFVSRS1SRUtPUi1CT0RZ", "canonicalizedBody"),
        ("RklYVFVSRS1SRkMzMTYx", "rfc3161"),
    ] {
        let malformed = bundle.replacen(bad, "NOT*BASE64", 1);
        let error = parse_sigstore_message_signature_bundle(malformed.as_bytes())
            .expect_err("every admitted tlog/timestamp byte field must be standard base64");
        assert!(error.message.contains(field) || error.message.contains("base64"));
    }

    let no_timestamps = bundle.replace(
        r#""timestampVerificationData":{"rfc3161Timestamps":[{"signedTimestamp":"RklYVFVSRS1SRkMzMTYx"}]}"#,
        r#""timestampVerificationData":{}"#,
    );
    parse_sigstore_message_signature_bundle(no_timestamps.as_bytes())
        .expect("an exact empty protobuf timestamp message means zero timestamp records");
}
