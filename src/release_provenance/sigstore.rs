//! Network-free Sigstore verification against caller-supplied trust material.
//!
//! This module deliberately has no root-discovery or network path. The caller
//! supplies both the bundle and an authenticated trusted-root snapshot. A
//! successful result proves that the bundle was valid under that snapshot; it
//! does not establish that the snapshot is current or that a later revocation
//! has not occurred.

use sigstore_verify::trust_root::TrustedRoot;
use sigstore_verify::types::bundle::VerificationMaterialContent;
use sigstore_verify::types::Bundle;
use sigstore_verify::{VerificationPolicy, Verifier};
use x509_cert::der::asn1::Utf8StringRef;
use x509_cert::der::Decode;
use x509_cert::Certificate;

use super::{Diagnostic, ExpectedReleaseIdentity, OfflineBundleVerificationCapability};

const SIGSTORE_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
const MAX_SIGSTORE_BUNDLE_BYTES: usize = 2 * 1024 * 1024;
const MAX_TRUSTED_ROOT_BYTES: usize = 4 * 1024 * 1024;
const MAX_TRUSTED_ROOT_RECORDS: usize = 128;
const VERIFICATION_REFUSAL: &str = "offline Sigstore cryptographic verification refused";

/// Pure, network-free verifier for Sigstore v0.3 bundles.
///
/// Verification uses the dependency's strict default policy: transparency-log
/// inclusion, certificate-chain validation, and SCT validation all remain
/// enabled. The certificate identity and OIDC issuer must exactly match the
/// release identity derived from SEMAPRAX's already-bound provenance.
#[derive(Debug, Clone, Copy, Default)]
pub struct SigstoreOfflineVerifier;

impl OfflineBundleVerificationCapability for SigstoreOfflineVerifier {
    fn verify_offline_bundle(
        &self,
        expected_identity: &ExpectedReleaseIdentity,
        subject_bytes: &[u8],
        bundle_bytes: &[u8],
        trusted_root_bytes: &[u8],
    ) -> Result<(), Diagnostic> {
        verify_bound_bundle(
            expected_identity,
            subject_bytes,
            bundle_bytes,
            trusted_root_bytes,
        )
    }
}

fn verify_bound_bundle(
    expected_identity: &ExpectedReleaseIdentity,
    subject_bytes: &[u8],
    bundle_bytes: &[u8],
    trusted_root_bytes: &[u8],
) -> Result<(), Diagnostic> {
    if bundle_bytes.is_empty()
        || bundle_bytes.len() > MAX_SIGSTORE_BUNDLE_BYTES
        || trusted_root_bytes.is_empty()
        || trusted_root_bytes.len() > MAX_TRUSTED_ROOT_BYTES
    {
        return Err(verification_refusal());
    }

    let bundle_text = std::str::from_utf8(bundle_bytes).map_err(|_| verification_refusal())?;
    let bundle = Bundle::from_json(bundle_text).map_err(|_| verification_refusal())?;
    if bundle.media_type != SIGSTORE_BUNDLE_MEDIA_TYPE {
        return Err(verification_refusal());
    }

    let trusted_root_text =
        std::str::from_utf8(trusted_root_bytes).map_err(|_| verification_refusal())?;
    let certificate_identity = workflow_certificate_identity(expected_identity);
    verify_bundle_with_certificate_identity(
        expected_identity.issuer.as_str(),
        &certificate_identity,
        subject_bytes,
        &bundle,
        trusted_root_text,
    )?;
    verify_immutable_repository_claims(&bundle, expected_identity)
}

/// The workflow URL SAN alone is name-based. Pin the original GitHub OIDC
/// subject and immutable repository/owner IDs from the *verified* Fulcio leaf
/// certificate so a synthetic structural claim cannot stand in for them.
fn verify_immutable_repository_claims(
    bundle: &Bundle,
    expected_identity: &ExpectedReleaseIdentity,
) -> Result<(), Diagnostic> {
    let VerificationMaterialContent::Certificate(content) = &bundle.verification_material.content
    else {
        return Err(verification_refusal());
    };
    let certificate =
        Certificate::from_der(content.raw_bytes.as_bytes()).map_err(|_| verification_refusal())?;
    verify_certificate_repository_claims(
        certificate.tbs_certificate().extensions(),
        expected_identity,
    )
}

fn verify_certificate_repository_claims(
    extensions: Option<&x509_cert::ext::Extensions>,
    expected_identity: &ExpectedReleaseIdentity,
) -> Result<(), Diagnostic> {
    for (oid, expected) in [
        ("1.3.6.1.4.1.57264.1.24", expected_identity.subject.as_str()),
        ("1.3.6.1.4.1.57264.1.15", "1326961553"),
        ("1.3.6.1.4.1.57264.1.17", "47505194"),
    ] {
        let mut matches = extensions
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter(|extension| extension.extn_id.to_string() == oid);
        let extension = matches.next().ok_or_else(verification_refusal)?;
        if matches.next().is_some() {
            return Err(verification_refusal());
        }
        let value = Utf8StringRef::from_der(extension.extn_value.as_bytes())
            .map_err(|_| verification_refusal())?;
        if value.as_str() != expected {
            return Err(verification_refusal());
        }
    }
    Ok(())
}

fn verify_bundle_with_certificate_identity(
    issuer: &str,
    certificate_identity: &str,
    subject_bytes: &[u8],
    bundle: &Bundle,
    trusted_root_text: &str,
) -> Result<(), Diagnostic> {
    let policy = VerificationPolicy::default()
        .require_identity(certificate_identity)
        .require_issuer(issuer);

    let mut record_count = 0usize;
    let mut successful_roots = 0usize;
    for record in trusted_root_text.lines() {
        if record.trim().is_empty() {
            continue;
        }
        record_count = record_count
            .checked_add(1)
            .ok_or_else(verification_refusal)?;
        if record_count > MAX_TRUSTED_ROOT_RECORDS {
            return Err(verification_refusal());
        }

        let Ok(trusted_root) = TrustedRoot::from_json(record) else {
            continue;
        };
        if Verifier::new(&trusted_root)
            .verify(subject_bytes, bundle, &policy)
            .is_ok()
        {
            successful_roots += 1;
            if successful_roots > 1 {
                return Err(verification_refusal());
            }
        }
    }

    if record_count == 0 || successful_roots != 1 {
        return Err(verification_refusal());
    }
    Ok(())
}

fn workflow_certificate_identity(expected_identity: &ExpectedReleaseIdentity) -> String {
    format!("https://github.com/{}", expected_identity.workflow_ref)
}

fn verification_refusal() -> Diagnostic {
    Diagnostic::io("SPX-Z707", VERIFICATION_REFUSAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_BUNDLE: &str =
        include_str!("../../tests/fixtures/release_sigstore/cosign-v3-blob.sigstore.json");
    const FIXTURE_SUBJECT: &[u8] =
        include_bytes!("../../tests/fixtures/release_sigstore/cosign-v3-blob.txt");
    const FIXTURE_ROOT: &str =
        include_str!("../../tests/fixtures/release_sigstore/public-good.json");
    const FIXTURE_IDENTITY: &str = "w.vollprecht@gmail.com";
    const FIXTURE_ISSUER: &str = "https://github.com/login/oauth";

    fn identity() -> ExpectedReleaseIdentity {
        ExpectedReleaseIdentity {
            issuer: "https://token.actions.githubusercontent.com".to_owned(),
            repository: "wavect/semaprax".to_owned(),
            workflow_path: ".github/workflows/ci.yml".to_owned(),
            tag: "v1.2.3".to_owned(),
            subject: "repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/v1.2.3".to_owned(),
            workflow_ref: "wavect/semaprax/.github/workflows/ci.yml@refs/tags/v1.2.3".to_owned(),
        }
    }

    fn assert_stable_refusal(error: Diagnostic) {
        assert_eq!(error.code, "SPX-Z707");
        assert_eq!(error.message, VERIFICATION_REFUSAL);
        assert!(error.help.is_none());
    }

    fn fixture_bundle() -> Bundle {
        Bundle::from_json(FIXTURE_BUNDLE).expect("fixture bundle must decode")
    }

    fn fixture_root_jsonl() -> String {
        let value: serde_json::Value =
            serde_json::from_str(FIXTURE_ROOT).expect("fixture trusted root must decode");
        format!(
            "{}\n",
            serde_json::to_string(&value).expect("fixture trusted root must encode")
        )
    }

    fn extension(oid: &str, value: &str) -> x509_cert::ext::Extension {
        use x509_cert::der::asn1::OctetString;
        use x509_cert::der::Encode;
        x509_cert::ext::Extension {
            extn_id: oid.parse().expect("test OID"),
            critical: false,
            extn_value: OctetString::new(
                Utf8StringRef::new(value)
                    .expect("test UTF8")
                    .to_der()
                    .expect("test DER"),
            )
            .expect("test octet string"),
        }
    }

    #[test]
    fn immutable_github_certificate_claims_are_required_exactly_once() {
        let bundle = fixture_bundle();
        let VerificationMaterialContent::Certificate(content) =
            &bundle.verification_material.content
        else {
            panic!("fixture must contain a certificate");
        };
        let cert = Certificate::from_der(content.raw_bytes.as_bytes()).expect("fixture DER");
        let mut extensions: x509_cert::ext::Extensions = cert
            .tbs_certificate()
            .extensions()
            .expect("fixture extensions")
            .clone();
        extensions.retain(|extension| {
            !matches!(
                extension.extn_id.to_string().as_str(),
                "1.3.6.1.4.1.57264.1.24" | "1.3.6.1.4.1.57264.1.15" | "1.3.6.1.4.1.57264.1.17"
            )
        });
        assert_stable_refusal(
            verify_certificate_repository_claims(Some(&extensions), &identity()).unwrap_err(),
        );
        extensions.push(extension("1.3.6.1.4.1.57264.1.24", &identity().subject));
        extensions.push(extension("1.3.6.1.4.1.57264.1.15", "1326961553"));
        extensions.push(extension("1.3.6.1.4.1.57264.1.17", "47505194"));
        verify_certificate_repository_claims(Some(&extensions), &identity())
            .expect("exact immutable claims");
        extensions.push(extension("1.3.6.1.4.1.57264.1.17", "47505194"));
        assert_stable_refusal(
            verify_certificate_repository_claims(Some(&extensions), &identity()).unwrap_err(),
        );
        extensions.pop();
        let last = extensions.len() - 1;
        extensions[last] = extension("1.3.6.1.4.1.57264.1.17", "other-owner");
        assert_stable_refusal(
            verify_certificate_repository_claims(Some(&extensions), &identity()).unwrap_err(),
        );
    }

    #[test]
    fn workflow_identity_is_the_exact_github_actions_san() {
        assert_eq!(
            workflow_certificate_identity(&identity()),
            "https://github.com/wavect/semaprax/.github/workflows/ci.yml@refs/tags/v1.2.3"
        );
    }

    #[test]
    fn valid_message_signature_bundle_verifies_without_network_access() {
        verify_bundle_with_certificate_identity(
            FIXTURE_ISSUER,
            FIXTURE_IDENTITY,
            FIXTURE_SUBJECT,
            &fixture_bundle(),
            &fixture_root_jsonl(),
        )
        .expect("valid fixture must verify against its supplied root snapshot");
    }

    #[test]
    fn artifact_identity_and_issuer_are_each_cryptographically_bound() {
        let bundle = fixture_bundle();
        let root = fixture_root_jsonl();

        let subject_error = verify_bundle_with_certificate_identity(
            FIXTURE_ISSUER,
            FIXTURE_IDENTITY,
            b"mutated artifact",
            &bundle,
            &root,
        )
        .expect_err("mutated artifact must be refused");
        assert_stable_refusal(subject_error);

        let identity_error = verify_bundle_with_certificate_identity(
            FIXTURE_ISSUER,
            "different@example.com",
            FIXTURE_SUBJECT,
            &bundle,
            &root,
        )
        .expect_err("wrong certificate identity must be refused");
        assert_stable_refusal(identity_error);

        let issuer_error = verify_bundle_with_certificate_identity(
            "https://issuer.invalid",
            FIXTURE_IDENTITY,
            FIXTURE_SUBJECT,
            &bundle,
            &root,
        )
        .expect_err("wrong certificate issuer must be refused");
        assert_stable_refusal(issuer_error);
    }

    #[test]
    fn multiple_accepting_root_records_are_refused_as_ambiguous() {
        let root = fixture_root_jsonl();
        let duplicated_root = format!("{root}{root}");
        let error = verify_bundle_with_certificate_identity(
            FIXTURE_ISSUER,
            FIXTURE_IDENTITY,
            FIXTURE_SUBJECT,
            &fixture_bundle(),
            &duplicated_root,
        )
        .expect_err("more than one accepting root record must be refused");
        assert_stable_refusal(error);
    }

    #[test]
    fn malformed_bundle_is_a_stable_cryptographic_refusal() {
        let error = SigstoreOfflineVerifier
            .verify_offline_bundle(
                &identity(),
                b"subject",
                br#"{"attacker":"detail"}"#,
                b"{}\n",
            )
            .expect_err("malformed bundle must be refused");
        assert_stable_refusal(error);
    }

    #[test]
    fn non_utf8_material_is_a_stable_cryptographic_refusal() {
        let bundle_error = SigstoreOfflineVerifier
            .verify_offline_bundle(&identity(), b"subject", &[0xff], b"{}\n")
            .expect_err("non-UTF-8 bundle must be refused");
        assert_stable_refusal(bundle_error);

        let root_error = SigstoreOfflineVerifier
            .verify_offline_bundle(
                &identity(),
                b"subject",
                br#"{"mediaType":"application/vnd.dev.sigstore.bundle.v0.3+json"}"#,
                &[0xff],
            )
            .expect_err("non-UTF-8 trusted root must be refused");
        assert_stable_refusal(root_error);
    }

    #[test]
    fn oversized_or_empty_material_is_refused_before_decoding() {
        let oversized_bundle = vec![b' '; MAX_SIGSTORE_BUNDLE_BYTES + 1];
        let error = SigstoreOfflineVerifier
            .verify_offline_bundle(&identity(), b"subject", &oversized_bundle, b"{}\n")
            .expect_err("oversized bundle must be refused");
        assert_stable_refusal(error);

        let error = SigstoreOfflineVerifier
            .verify_offline_bundle(&identity(), b"subject", b"", b"")
            .expect_err("empty verification material must be refused");
        assert_stable_refusal(error);
    }
}
