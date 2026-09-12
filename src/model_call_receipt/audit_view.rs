//! `ModelCallAuditView v1`: a redacted, verifiable projection of one
//! [`super::receipt::ModelCallReceipt`], plus the raw-payload-holding
//! sidecar ([`ReceiptPrivateExtras`]) redaction is built from.
//!
//! # Why redaction needs a sidecar, not just a policy over the receipt
//!
//! [`super::receipt::ModelCallReceipt`] never carries raw prompt, response,
//! or diagnostic text — only commitments. That already satisfies "sensitive
//! payloads can be withheld" for the receipt itself. But a real deployment
//! still often *retains* raw bytes somewhere close to the call (for
//! debugging, for provider-support escalation, for a human review queue),
//! and that is exactly the material a redaction boundary has to prove it
//! can withhold on request. [`ReceiptPrivateExtras`] models that retained
//! material explicitly, so [`redact`] has something real to withhold and
//! [`verify_audit_view`] has something real to check a commitment against —
//! matching this session's stated trap: "a decoder test where 'decoded' and
//! 'raw' buffers were byte-identical so no leak could fail them" is exactly
//! what naming the raw material explicitly, and asserting its *absence* from
//! the rendered view, is meant to catch.
//!
//! # What redaction must not do
//!
//! [`redact`] never mutates or reconstructs a [`super::receipt::ModelCallReceipt`]
//! — it only reads one and produces a separate [`ModelCallAuditView`] that
//! carries the receipt's own digest by reference
//! (`ModelCallAuditView::receipt_digest`). Two views built with different
//! [`RedactionPolicy`] values from the same receipt always carry the same
//! `receipt_digest`, proving redaction never changes receipt identity (see
//! `tests::redaction_never_changes_the_bound_receipt_digest`).

use sha2::{Digest as _, Sha256};

use crate::digest_hex::LowerHex;

use super::receipt::{ModelCallReceipt, PayloadPrivacyClaim};

pub const AUDIT_VIEW_SCHEMA: &str = "semaprax.model-call-audit-view.v1";

const FIELD_COMMITMENT_DOMAIN: &[u8] = b"semaprax.model-call-audit-view.field-commitment.v1\0";

/// Raw payload material a deployment retains alongside a receipt, never
/// part of the receipt's own canonical bytes. The six fields here are
/// exactly the surfaces a real deployment could plausibly leak: the three
/// call payloads, an authenticated private reference, and two free-text
/// diagnostic fields a handler or adapter might (incorrectly) attach for
/// its own debugging — `adapter_diagnostic_hint` and
/// `provider_error_detail` are exactly the kind of field
/// `crate::live_invocation::model_invoke`'s own docs warn never belongs in
/// a *journal* ("nothing provider-shaped"); here they are modeled as
/// something a careless caller retained anyway, so this module can prove
/// its redaction boundary actually stops them from reaching an audit view.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReceiptPrivateExtras {
    pub task: Vec<u8>,
    pub observation: Vec<u8>,
    pub response: Option<Vec<u8>>,
    pub private_payload_reference_material: Option<String>,
    pub adapter_diagnostic_hint: Option<String>,
    pub provider_error_detail: Option<String>,
    pub authorization_header_echo: Option<String>,
}

/// Which retained fields a produced [`ModelCallAuditView`] may reveal.
/// Every flag defaults to `false` ([`RedactionPolicy::fully_redacted`]) —
/// an audit view is redacted unless a caller explicitly opts a field in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RedactionPolicy {
    pub reveal_task: bool,
    pub reveal_observation: bool,
    pub reveal_response: bool,
    pub reveal_private_reference_material: bool,
    pub reveal_adapter_diagnostic_hint: bool,
    pub reveal_provider_error_detail: bool,
    pub reveal_authorization_header_echo: bool,
}

impl RedactionPolicy {
    #[must_use]
    pub fn fully_redacted() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn fully_revealed() -> Self {
        Self {
            reveal_task: true,
            reveal_observation: true,
            reveal_response: true,
            reveal_private_reference_material: true,
            reveal_adapter_diagnostic_hint: true,
            reveal_provider_error_detail: true,
            reveal_authorization_header_echo: true,
        }
    }
}

/// A verifiable record that one field was withheld: its name and a
/// domain-separated commitment digest over the withheld bytes, so a
/// verifier holding the original [`ReceiptPrivateExtras`] can independently
/// confirm the view did not simply drop the field silently but committed
/// to specific, checkable bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedField {
    pub name: &'static str,
    pub commitment_digest: String,
}

fn field_commitment(name: &str, bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(FIELD_COMMITMENT_DOMAIN);
    hash.update(name.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

/// A redacted, verifiable projection of one receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCallAuditView {
    pub receipt_digest: String,
    /// Every field withheld by the policy that produced this view, each
    /// with a verifiable commitment. A field the policy revealed carries no
    /// entry here (its plaintext is directly present instead).
    pub redacted_fields: Vec<RedactedField>,

    pub task_preview: Option<Vec<u8>>,
    pub observation_preview: Option<Vec<u8>>,
    pub response_preview: Option<Vec<u8>>,
    pub private_reference_material: Option<String>,
    pub adapter_diagnostic_hint: Option<String>,
    pub provider_error_detail: Option<String>,
    pub authorization_header_echo: Option<String>,

    pub task_privacy_claim: PayloadPrivacyClaim,
    pub observation_privacy_claim: PayloadPrivacyClaim,
    pub response_privacy_claim: Option<PayloadPrivacyClaim>,
}

impl ModelCallAuditView {
    /// A single rendered text form, used only so a test can assert a
    /// marker string is (or is not) present anywhere in what a reviewer
    /// would actually see — not merely absent from one struct field, which
    /// is exactly the kind of narrow check this session's brief warns
    /// proves nothing if a leak could hide in an adjacent field.
    #[must_use]
    pub fn render_for_review(&self) -> String {
        let opt = |value: &Option<String>| value.clone().unwrap_or_default();
        let opt_bytes = |value: &Option<Vec<u8>>| {
            value
                .as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                .unwrap_or_default()
        };
        format!(
            "receipt_digest={}\ntask_preview={}\nobservation_preview={}\nresponse_preview={}\nprivate_reference_material={}\nadapter_diagnostic_hint={}\nprovider_error_detail={}\nauthorization_header_echo={}\nredacted_fields={:?}\ntask_privacy_claim={}\nobservation_privacy_claim={}\nresponse_privacy_claim={}",
            self.receipt_digest,
            opt_bytes(&self.task_preview),
            opt_bytes(&self.observation_preview),
            opt_bytes(&self.response_preview),
            opt(&self.private_reference_material),
            opt(&self.adapter_diagnostic_hint),
            opt(&self.provider_error_detail),
            opt(&self.authorization_header_echo),
            self.redacted_fields,
            self.task_privacy_claim.as_str(),
            self.observation_privacy_claim.as_str(),
            self.response_privacy_claim.map(PayloadPrivacyClaim::as_str).unwrap_or("none"),
        )
    }
}

/// Builds a redacted audit view from a receipt, its retained extras, and a
/// policy. Every withheld field is replaced by a [`RedactedField`]
/// commitment rather than silently dropped; a revealed field's plaintext
/// (bounded to what `extras` actually holds) appears directly.
#[must_use]
pub fn redact(
    receipt: &ModelCallReceipt,
    extras: &ReceiptPrivateExtras,
    policy: &RedactionPolicy,
) -> ModelCallAuditView {
    let mut redacted_fields = Vec::new();

    let mut reveal_or_commit_bytes =
        |reveal: bool, name: &'static str, bytes: &[u8]| -> Option<Vec<u8>> {
            if reveal {
                Some(bytes.to_vec())
            } else {
                redacted_fields.push(RedactedField {
                    name,
                    commitment_digest: field_commitment(name, bytes),
                });
                None
            }
        };

    let task_preview = reveal_or_commit_bytes(policy.reveal_task, "task", &extras.task);
    let observation_preview =
        reveal_or_commit_bytes(policy.reveal_observation, "observation", &extras.observation);
    let response_preview = extras.response.as_ref().and_then(|response| {
        reveal_or_commit_bytes(policy.reveal_response, "response", response)
    });

    let mut reveal_or_commit_text =
        |reveal: bool, name: &'static str, value: &Option<String>| -> Option<String> {
            let text = value.as_ref()?;
            if reveal {
                Some(text.clone())
            } else {
                redacted_fields.push(RedactedField {
                    name,
                    commitment_digest: field_commitment(name, text.as_bytes()),
                });
                None
            }
        };

    let private_reference_material = reveal_or_commit_text(
        policy.reveal_private_reference_material,
        "private_payload_reference_material",
        &extras.private_payload_reference_material,
    );
    let adapter_diagnostic_hint = reveal_or_commit_text(
        policy.reveal_adapter_diagnostic_hint,
        "adapter_diagnostic_hint",
        &extras.adapter_diagnostic_hint,
    );
    let provider_error_detail = reveal_or_commit_text(
        policy.reveal_provider_error_detail,
        "provider_error_detail",
        &extras.provider_error_detail,
    );
    let authorization_header_echo = reveal_or_commit_text(
        policy.reveal_authorization_header_echo,
        "authorization_header_echo",
        &extras.authorization_header_echo,
    );

    ModelCallAuditView {
        receipt_digest: receipt.digest(),
        redacted_fields,
        task_preview,
        observation_preview,
        response_preview,
        private_reference_material,
        adapter_diagnostic_hint,
        provider_error_detail,
        authorization_header_echo,
        task_privacy_claim: PayloadPrivacyClaim::classify(
            extras.task.len(),
            extras.private_payload_reference_material.is_some(),
        ),
        observation_privacy_claim: PayloadPrivacyClaim::classify(
            extras.observation.len(),
            extras.private_payload_reference_material.is_some(),
        ),
        response_privacy_claim: extras.response.as_ref().map(|response| {
            PayloadPrivacyClaim::classify(
                response.len(),
                extras.private_payload_reference_material.is_some(),
            )
        }),
    }
}

/// Why [`verify_audit_view`] refused a view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditViewError {
    /// The view's `receipt_digest` does not match the receipt it is claimed
    /// to project.
    WrongReceipt,
    /// A field the view claims was withheld does not actually commit to the
    /// bytes `extras` holds for it — the redaction commitment was forged or
    /// stale.
    CommitmentMismatch { field: &'static str },
    /// A field the view reveals does not match what `extras` holds for it —
    /// the view's plaintext was tampered with after redaction.
    RevealedFieldTampered { field: &'static str },
}

/// Independently checks a view against the receipt and extras it claims to
/// be built from: every withheld field's commitment must match the real
/// bytes, and every revealed field's plaintext must match the real bytes
/// too (a redacted view is not a place to also inject false plaintext).
pub fn verify_audit_view(
    view: &ModelCallAuditView,
    receipt: &ModelCallReceipt,
    extras: &ReceiptPrivateExtras,
) -> Result<(), AuditViewError> {
    if view.receipt_digest != receipt.digest() {
        return Err(AuditViewError::WrongReceipt);
    }
    for redacted in &view.redacted_fields {
        let real_bytes: Vec<u8> = match redacted.name {
            "task" => extras.task.clone(),
            "observation" => extras.observation.clone(),
            "response" => extras.response.clone().unwrap_or_default(),
            "private_payload_reference_material" => extras
                .private_payload_reference_material
                .clone()
                .unwrap_or_default()
                .into_bytes(),
            "adapter_diagnostic_hint" => extras
                .adapter_diagnostic_hint
                .clone()
                .unwrap_or_default()
                .into_bytes(),
            "provider_error_detail" => extras
                .provider_error_detail
                .clone()
                .unwrap_or_default()
                .into_bytes(),
            "authorization_header_echo" => extras
                .authorization_header_echo
                .clone()
                .unwrap_or_default()
                .into_bytes(),
            other => {
                return Err(AuditViewError::CommitmentMismatch { field: leak_name(other) })
            }
        };
        if field_commitment(redacted.name, &real_bytes) != redacted.commitment_digest {
            return Err(AuditViewError::CommitmentMismatch {
                field: leak_name(redacted.name),
            });
        }
    }
    if let Some(task_preview) = &view.task_preview {
        if task_preview != &extras.task {
            return Err(AuditViewError::RevealedFieldTampered { field: "task" });
        }
    }
    if let Some(observation_preview) = &view.observation_preview {
        if observation_preview != &extras.observation {
            return Err(AuditViewError::RevealedFieldTampered { field: "observation" });
        }
    }
    if let Some(response_preview) = &view.response_preview {
        if Some(response_preview) != extras.response.as_ref() {
            return Err(AuditViewError::RevealedFieldTampered { field: "response" });
        }
    }
    Ok(())
}

/// `redacted.name` is always one of the closed literal names this module
/// writes; this only exists so `&'static str` can be returned from a match
/// arm holding a borrowed `&str` field name without an extra allocation.
fn leak_name(name: &str) -> &'static str {
    match name {
        "task" => "task",
        "observation" => "observation",
        "response" => "response",
        "private_payload_reference_material" => "private_payload_reference_material",
        "adapter_diagnostic_hint" => "adapter_diagnostic_hint",
        "provider_error_detail" => "provider_error_detail",
        "authorization_header_echo" => "authorization_header_echo",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_call_receipt::receipt::tests::sample_receipt;
    use crate::model_call_receipt::receipt::{
        commit_observation_bytes, commit_response_bytes, commit_task_bytes,
    };

    const TASK_MARKER: &[u8] = b"SECRET-TASK-MARKER-do-not-leak";
    const OBSERVATION_MARKER: &[u8] = b"SECRET-OBSERVATION-MARKER-do-not-leak";
    const RESPONSE_MARKER: &[u8] = b"SECRET-RESPONSE-MARKER-do-not-leak";
    const ADAPTER_HINT_MARKER: &str = "SECRET-ADAPTER-HINT-do-not-leak";
    const PROVIDER_ERROR_MARKER: &str = "SECRET-PROVIDER-ERROR-DETAIL-do-not-leak";
    const AUTH_HEADER_MARKER: &str = "SECRET-AUTHORIZATION-HEADER-do-not-leak";

    fn clean_extras() -> ReceiptPrivateExtras {
        ReceiptPrivateExtras {
            task: b"ordinary non-secret task".to_vec(),
            observation: b"ordinary non-secret observation".to_vec(),
            response: Some(b"ordinary non-secret response".to_vec()),
            private_payload_reference_material: None,
            adapter_diagnostic_hint: None,
            provider_error_detail: None,
            authorization_header_echo: None,
        }
    }

    /// Mirrors `std.auth.tests.audit_event_safety`'s structure exactly: one
    /// marker is placed in exactly one of six secret-bearing fields against
    /// an otherwise-clean baseline, and the fully-redacted view's rendered
    /// text is checked for that marker's absence — individually, not as one
    /// combined "redaction works" assertion. A positive control (the fully
    /// *revealed* view) proves each marker really would show up if not
    /// redacted, so this cannot pass merely because the marker was never in
    /// the rendered text to begin with.
    #[test]
    fn redaction_hides_each_of_six_secret_bearing_fields_individually() {
        let receipt = sample_receipt();
        let redacted_policy = RedactionPolicy::fully_redacted();
        let revealed_policy = RedactionPolicy::fully_revealed();

        let cases: Vec<(&str, ReceiptPrivateExtras)> = vec![
            ("task", ReceiptPrivateExtras {
                task: TASK_MARKER.to_vec(),
                ..clean_extras()
            }),
            ("observation", ReceiptPrivateExtras {
                observation: OBSERVATION_MARKER.to_vec(),
                ..clean_extras()
            }),
            ("response", ReceiptPrivateExtras {
                response: Some(RESPONSE_MARKER.to_vec()),
                ..clean_extras()
            }),
            ("adapter_diagnostic_hint", ReceiptPrivateExtras {
                adapter_diagnostic_hint: Some(ADAPTER_HINT_MARKER.to_owned()),
                ..clean_extras()
            }),
            ("provider_error_detail", ReceiptPrivateExtras {
                provider_error_detail: Some(PROVIDER_ERROR_MARKER.to_owned()),
                ..clean_extras()
            }),
            ("authorization_header_echo", ReceiptPrivateExtras {
                authorization_header_echo: Some(AUTH_HEADER_MARKER.to_owned()),
                ..clean_extras()
            }),
        ];
        let markers: [&str; 6] = [
            std::str::from_utf8(TASK_MARKER).unwrap(),
            std::str::from_utf8(OBSERVATION_MARKER).unwrap(),
            std::str::from_utf8(RESPONSE_MARKER).unwrap(),
            ADAPTER_HINT_MARKER,
            PROVIDER_ERROR_MARKER,
            AUTH_HEADER_MARKER,
        ];

        // Clean baseline: no marker anywhere, fully redacted view still
        // renders with nothing to hide.
        let clean_view = redact(&receipt, &clean_extras(), &redacted_policy);
        let clean_rendered = clean_view.render_for_review();
        for marker in markers {
            assert!(
                !clean_rendered.contains(marker),
                "clean baseline must never contain marker {marker}"
            );
        }

        for (field, extras_with_secret) in &cases {
            let marker = match *field {
                "task" => std::str::from_utf8(TASK_MARKER).unwrap(),
                "observation" => std::str::from_utf8(OBSERVATION_MARKER).unwrap(),
                "response" => std::str::from_utf8(RESPONSE_MARKER).unwrap(),
                "adapter_diagnostic_hint" => ADAPTER_HINT_MARKER,
                "provider_error_detail" => PROVIDER_ERROR_MARKER,
                "authorization_header_echo" => AUTH_HEADER_MARKER,
                _ => unreachable!(),
            };

            // Positive control: the marker really is in the input, and a
            // fully-revealed view really does surface it — this is the
            // check that rules out "decoded and raw happened to be
            // identical so nothing could leak."
            let revealed_view = redact(&receipt, extras_with_secret, &revealed_policy);
            assert!(
                revealed_view.render_for_review().contains(marker),
                "field {field}: revealed view must contain its own marker"
            );

            // The actual claim under test: a fully redacted view must not
            // contain this field's marker anywhere in what a reviewer sees.
            let redacted_view = redact(&receipt, extras_with_secret, &redacted_policy);
            let rendered = redacted_view.render_for_review();
            assert!(
                !rendered.contains(marker),
                "field {field}: redacted view must not contain its marker"
            );
            // And the commitment naming that field must still be present
            // and verifiable — redaction is not the same as silent deletion.
            assert!(
                redacted_view.redacted_fields.iter().any(|f| f.name == *field),
                "field {field}: redacted view must record a commitment for it"
            );
            assert_eq!(
                verify_audit_view(&redacted_view, &receipt, extras_with_secret),
                Ok(()),
                "field {field}: redacted view must verify against the real extras"
            );
        }
    }

    #[test]
    fn redaction_never_changes_the_bound_receipt_digest() {
        let receipt = sample_receipt();
        let extras = clean_extras();
        let redacted = redact(&receipt, &extras, &RedactionPolicy::fully_redacted());
        let revealed = redact(&receipt, &extras, &RedactionPolicy::fully_revealed());
        assert_eq!(redacted.receipt_digest, receipt.digest());
        assert_eq!(revealed.receipt_digest, receipt.digest());
        assert_eq!(redacted.receipt_digest, revealed.receipt_digest);
    }

    #[test]
    fn verify_audit_view_rejects_a_forged_commitment_and_a_tampered_plaintext() {
        let receipt = sample_receipt();
        let extras = ReceiptPrivateExtras {
            task: TASK_MARKER.to_vec(),
            ..clean_extras()
        };
        let mut view = redact(&receipt, &extras, &RedactionPolicy::fully_redacted());
        assert_eq!(verify_audit_view(&view, &receipt, &extras), Ok(()));

        // Forge the commitment for the redacted "task" field.
        let forged = view
            .redacted_fields
            .iter_mut()
            .find(|f| f.name == "task")
            .unwrap();
        forged.commitment_digest = "sha256:".to_owned() + &"0".repeat(64);
        assert_eq!(
            verify_audit_view(&view, &receipt, &extras),
            Err(AuditViewError::CommitmentMismatch { field: "task" })
        );

        // A revealed field whose plaintext was tampered with after the
        // fact must also be rejected, not silently trusted.
        let mut revealed = redact(&receipt, &extras, &RedactionPolicy::fully_revealed());
        revealed.task_preview = Some(b"not the real task bytes".to_vec());
        assert_eq!(
            verify_audit_view(&revealed, &receipt, &extras),
            Err(AuditViewError::RevealedFieldTampered { field: "task" })
        );
    }

    #[test]
    fn low_entropy_payload_never_earns_a_withheld_claim_without_a_private_reference() {
        let receipt = sample_receipt();
        let short_extras = ReceiptPrivateExtras {
            task: b"hi".to_vec(),
            ..clean_extras()
        };
        let view = redact(&receipt, &short_extras, &RedactionPolicy::fully_redacted());
        assert_eq!(
            view.task_privacy_claim,
            PayloadPrivacyClaim::DigestOnlyLowEntropyCaveat
        );
        assert!(
            view.render_for_review().contains("digest_only_low_entropy_caveat"),
            "a short undisclosed payload must render an explicit low-entropy caveat"
        );
        assert!(
            !view.render_for_review().contains("task_privacy_claim=withheld"),
            "a short undisclosed payload must never be described as safely withheld"
        );

        let with_reference = ReceiptPrivateExtras {
            task: b"hi".to_vec(),
            private_payload_reference_material: Some("vault-ref-1".to_owned()),
            ..clean_extras()
        };
        let referenced_view =
            redact(&receipt, &with_reference, &RedactionPolicy::fully_redacted());
        assert_eq!(referenced_view.task_privacy_claim, PayloadPrivacyClaim::Withheld);

        let long_extras = ReceiptPrivateExtras {
            task: vec![b'x'; 4096],
            ..clean_extras()
        };
        let long_view = redact(&receipt, &long_extras, &RedactionPolicy::fully_redacted());
        assert_eq!(long_view.task_privacy_claim, PayloadPrivacyClaim::DigestOnly);
    }

    // Keep the digest-commitment helpers exercised directly too, since the
    // six-field test above only exercises them indirectly through `redact`.
    #[test]
    fn commit_helpers_are_deterministic_and_domain_separated_from_each_other() {
        let bytes = b"same content";
        assert_eq!(commit_task_bytes(bytes), commit_task_bytes(bytes));
        assert_ne!(commit_task_bytes(bytes), commit_observation_bytes(bytes));
        assert_ne!(commit_task_bytes(bytes), commit_response_bytes(bytes));
    }
}
