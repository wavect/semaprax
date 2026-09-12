//! The canonical `ModelCallReceipt v1` schema: bounded, domain-separated,
//! and root-bound.
//!
//! A receipt carries **commitments**, never raw payload bytes: `task_digest`,
//! `observation_digest` and `response_digest` are domain-separated digests
//! over caller-held bytes, computed with [`commit_task_bytes`],
//! [`commit_observation_bytes`] and [`commit_response_bytes`] respectively.
//! The raw bytes themselves, if retained at all, live in a caller-owned
//! [`super::audit_view::RetainedPayloads`]/[`super::audit_view::ReceiptPrivateExtras`]
//! pair that is never part of this struct and never folds into its digest —
//! that separation is what makes a receipt safe to hand to a reviewer or an
//! audit capsule by default, and what [`super::audit_view`]'s redacted view
//! is built on top of.

use sha2::{Digest as _, Sha256};

use crate::diagnostic::quote_json;
use crate::digest_hex::LowerHex;

pub const RECEIPT_SCHEMA: &str = "semaprax.model-call-receipt.v1";

const RECEIPT_DOMAIN: &[u8] = b"semaprax.model-call-receipt.v1\0";
const TASK_DOMAIN: &[u8] = b"semaprax.model-call-receipt.task-bytes.v1\0";
const OBSERVATION_DOMAIN: &[u8] = b"semaprax.model-call-receipt.observation-bytes.v1\0";
const RESPONSE_DOMAIN: &[u8] = b"semaprax.model-call-receipt.response-bytes.v1\0";
const PROPOSAL_DOMAIN: &[u8] = b"semaprax.model-call-receipt.proposal-bytes.v1\0";

/// A payload short enough that a bare commitment digest over it can still be
/// recovered by brute force (dictionary/rainbow-table style) without ever
/// seeing the plaintext — a short prompt like `"hi"` has too little entropy
/// for SHA-256 alone to hide. Below this many bytes, [`payload_privacy_claim`]
/// refuses to call a digest-only commitment "withheld" and instead returns a
/// caveated claim; see `docs/MODEL-CALL-RECEIPT-V1.md`'s "low-entropy
/// payload" section.
pub const LOW_ENTROPY_BYTE_THRESHOLD: usize = 32;

/// A domain-separated SHA-256 digest, matching the `domain || bytes`
/// convention every other evidence module in this crate uses.
#[must_use]
fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

/// Commits to task bytes. Two calls with identical bytes produce the same
/// digest; any differing byte changes it.
#[must_use]
pub fn commit_task_bytes(bytes: &[u8]) -> String {
    digest(TASK_DOMAIN, bytes)
}

/// Commits to one turn's observation/context projection bytes.
#[must_use]
pub fn commit_observation_bytes(bytes: &[u8]) -> String {
    digest(OBSERVATION_DOMAIN, bytes)
}

/// Commits to raw, still-untrusted response bytes exactly as recorded by
/// `live_invocation`'s causal journal (`JournalEntry::ResponseRecorded`).
#[must_use]
pub fn commit_response_bytes(bytes: &[u8]) -> String {
    digest(RESPONSE_DOMAIN, bytes)
}

/// Commits to a decoded proposal's canonical bytes (the value
/// [`crate::live_invocation::model_invoke::ProposalOutcome::Admitted`]
/// carries).
#[must_use]
pub fn commit_proposal_bytes(bytes: &[u8]) -> String {
    digest(PROPOSAL_DOMAIN, bytes)
}

/// Whether a commitment digest over `bytes_len` bytes is safe to publish
/// alone (no private reference), a caveated digest-only claim, or backed by
/// an authenticated private reference and therefore safe to describe as
/// withheld. See [`LOW_ENTROPY_BYTE_THRESHOLD`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadPrivacyClaim {
    /// An authenticated private reference exists; the payload can honestly
    /// be described as withheld.
    Withheld,
    /// No private reference, and the payload is short enough that the bare
    /// digest could plausibly be reversed by brute force. Any audit surface
    /// must render an explicit low-entropy caveat here, never a bare
    /// "withheld" or "safe" claim.
    DigestOnlyLowEntropyCaveat,
    /// No private reference, but the payload is long enough that a bare
    /// digest is an ordinary, honest commitment.
    DigestOnly,
}

impl PayloadPrivacyClaim {
    #[must_use]
    pub fn classify(bytes_len: usize, has_private_reference: bool) -> Self {
        if has_private_reference {
            Self::Withheld
        } else if bytes_len < LOW_ENTROPY_BYTE_THRESHOLD {
            Self::DigestOnlyLowEntropyCaveat
        } else {
            Self::DigestOnly
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Withheld => "withheld",
            Self::DigestOnlyLowEntropyCaveat => "digest_only_low_entropy_caveat",
            Self::DigestOnly => "digest_only",
        }
    }
}

/// The closed attempt-lifecycle vocabulary #180 asks for: reserved,
/// intent-persisted, dispatched, first-byte, completed, decoded,
/// accepted/rejected, cancelled, uncertain, reconciled. A receipt records
/// the furthest stage this attempt reached; earlier stages are implied by
/// distinctness of the fields recorded (e.g. a receipt whose
/// `terminal_stage` is `Decoded` or later always carries a `response_digest`
/// and a `proposal_digest` or `proposal_refusal_reason`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptStage {
    Reserved,
    IntentPersisted,
    Dispatched,
    FirstByte,
    Completed,
    Decoded,
    Accepted,
    Rejected,
    Cancelled,
    Uncertain,
    Reconciled,
}

impl ReceiptStage {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::IntentPersisted => "intent_persisted",
            Self::Dispatched => "dispatched",
            Self::FirstByte => "first_byte",
            Self::Completed => "completed",
            Self::Decoded => "decoded",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Uncertain => "uncertain",
            Self::Reconciled => "reconciled",
        }
    }

    /// Whether this stage means the attempt has settled enough that billing
    /// reconciliation may run against it (`super::reconciliation`).
    #[must_use]
    pub fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Decoded | Self::Accepted | Self::Rejected
        )
    }
}

/// Usage a provider later reports for this call, distinct from and never
/// overwriting the receipt's own `local_request_bytes`/`local_response_bytes`
/// local counters. Imported as untrusted external evidence — see
/// [`super::reconciliation`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReportedUsage {
    pub provider_call_id: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub provider_cost_micros: i64,
}

/// One canonical, bounded receipt for exactly one `model.invoke` attempt.
///
/// Every field is either a compiler/deployment-derived identity, a
/// domain-separated commitment digest, a closed-vocabulary tag, or a plain
/// counter — never a raw prompt, raw response, credential, or
/// authorization header. Construction is a real integration's job (from an
/// already-validated `live_invocation` causal journal entry plus the
/// deployment's root/attempt metadata); this module only defines the shape,
/// its canonical rendering/digest, root-binding verification, replay, and
/// redaction.
///
/// A receipt is evidence, not authority: nothing in this crate lets one
/// stand in for a [`crate::live_invocation::AuthorizationGrant`]. Passing a
/// receipt where a grant is required is a compile-time type error, not a
/// runtime check any of this module's functions have to perform.
///
/// ```compile_fail
/// fn wants_grant(_: semaprax::live_invocation::AuthorizationGrant) {}
///
/// fn feed(receipt: semaprax::model_call_receipt::ModelCallReceipt) {
///     wants_grant(receipt); // does not type-check: wrong type entirely
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCallReceipt {
    // Root associations.
    pub agent_id: String,
    pub program_root: String,
    pub deployment_root: String,
    pub instance_root: String,
    pub invocation_id: String,
    pub turn: u32,
    /// 1-based ordinal of this attempt within `(invocation_id, turn)`. A
    /// retried turn's second attempt carries `attempt: 2`, never a reused
    /// `1`.
    pub attempt: u32,

    // Call shape.
    pub model_class: String,
    pub provider_class: String,
    pub adapter_identity: String,
    pub proposal_grammar_digest: String,
    pub deployment_policy_digest: String,

    // Request commitment.
    pub task_digest: String,
    pub observation_digest: String,
    pub request_digest: String,
    pub request_bytes_len: usize,
    pub reserved_budget: i64,

    // Response commitment (absent until the call settles).
    pub response_digest: Option<String>,
    pub response_bytes_len: Option<usize>,
    /// An authenticated reference into a separately retained, encrypted
    /// payload store — never the encrypted bytes themselves, never a key.
    pub private_payload_reference: Option<String>,

    // Outcome.
    pub terminal_stage: ReceiptStage,
    /// The closed `ModelFailure` tag
    /// (`crate::live_invocation::model_invoke::ModelFailure::as_str`), if
    /// this attempt failed before or during settlement.
    pub failure: Option<String>,
    pub proposal_digest: Option<String>,
    pub proposal_refusal_reason: Option<String>,

    // Timing, milliseconds since an invocation-scoped epoch a caller
    // defines; never wall-clock authority this module asserts on its own.
    pub reserved_at_ms: u64,
    pub dispatched_at_ms: Option<u64>,
    pub first_byte_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,

    // Local accounting, independent of whatever a provider later reports.
    pub local_request_bytes: usize,
    pub local_response_bytes: usize,
    pub cost_estimate_micros: i64,

    /// The call reference this attempt expects a provider invoice to cite
    /// back verbatim (an idempotency-key-shaped token the deployment
    /// generates locally before dispatch, never provider-assigned) —
    /// see [`super::reconciliation`].
    pub provider_call_reference: String,
    /// Usage a provider has already reported for this exact call, if any.
    /// Distinct from a later, separately-imported invoice row.
    pub provider_reported: Option<ProviderReportedUsage>,
}

impl ModelCallReceipt {
    /// The canonical, deterministic rendering of this receipt: a fixed
    /// field order, JSON-escaped strings, explicit `null` for every absent
    /// optional field. Two receipts with identical field values render
    /// identically; any differing field (including field order, which is
    /// fixed here rather than caller-controlled) renders differently.
    #[must_use]
    pub fn render(&self) -> String {
        let opt_str = |value: &Option<String>| match value {
            Some(text) => quote_json(text),
            None => "null".to_owned(),
        };
        let opt_num = |value: Option<u64>| match value {
            Some(number) => number.to_string(),
            None => "null".to_owned(),
        };
        let provider_reported = match &self.provider_reported {
            Some(usage) => format!(
                "{{\"provider_call_id\":{},\"tokens_in\":{},\"tokens_out\":{},\"provider_cost_micros\":{}}}",
                quote_json(&usage.provider_call_id),
                usage.tokens_in,
                usage.tokens_out,
                usage.provider_cost_micros
            ),
            None => "null".to_owned(),
        };
        format!(
            "{{\"schema\":{},\"agent_id\":{},\"program_root\":{},\"deployment_root\":{},\"instance_root\":{},\"invocation_id\":{},\"turn\":{},\"attempt\":{},\"model_class\":{},\"provider_class\":{},\"adapter_identity\":{},\"proposal_grammar_digest\":{},\"deployment_policy_digest\":{},\"task_digest\":{},\"observation_digest\":{},\"request_digest\":{},\"request_bytes_len\":{},\"reserved_budget\":{},\"response_digest\":{},\"response_bytes_len\":{},\"private_payload_reference\":{},\"terminal_stage\":{},\"failure\":{},\"proposal_digest\":{},\"proposal_refusal_reason\":{},\"reserved_at_ms\":{},\"dispatched_at_ms\":{},\"first_byte_at_ms\":{},\"completed_at_ms\":{},\"local_request_bytes\":{},\"local_response_bytes\":{},\"cost_estimate_micros\":{},\"provider_call_reference\":{},\"provider_reported\":{}}}",
            quote_json(RECEIPT_SCHEMA),
            quote_json(&self.agent_id),
            quote_json(&self.program_root),
            quote_json(&self.deployment_root),
            quote_json(&self.instance_root),
            quote_json(&self.invocation_id),
            self.turn,
            self.attempt,
            quote_json(&self.model_class),
            quote_json(&self.provider_class),
            quote_json(&self.adapter_identity),
            quote_json(&self.proposal_grammar_digest),
            quote_json(&self.deployment_policy_digest),
            quote_json(&self.task_digest),
            quote_json(&self.observation_digest),
            quote_json(&self.request_digest),
            self.request_bytes_len,
            self.reserved_budget,
            opt_str(&self.response_digest),
            opt_num(self.response_bytes_len.map(|n| n as u64)),
            opt_str(&self.private_payload_reference),
            quote_json(self.terminal_stage.as_str()),
            opt_str(&self.failure),
            opt_str(&self.proposal_digest),
            opt_str(&self.proposal_refusal_reason),
            self.reserved_at_ms,
            opt_num(self.dispatched_at_ms),
            opt_num(self.first_byte_at_ms),
            opt_num(self.completed_at_ms),
            self.local_request_bytes,
            self.local_response_bytes,
            self.cost_estimate_micros,
            quote_json(&self.provider_call_reference),
            provider_reported,
        )
    }

    /// The canonical digest of this exact receipt. Any single differing
    /// byte in [`Self::render`] changes this; see
    /// `tests::single_byte_mutation_of_the_rendered_receipt_changes_the_digest`.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(RECEIPT_DOMAIN, self.render().as_bytes())
    }

    /// Re-derives this receipt's own low-entropy privacy classification for
    /// its task payload, given whether a private reference is attached.
    #[must_use]
    pub fn task_privacy_claim(&self, task_bytes_len: usize) -> PayloadPrivacyClaim {
        PayloadPrivacyClaim::classify(task_bytes_len, self.private_payload_reference.is_some())
    }
}

/// The exact root/policy/attempt state a receipt must bind to. A caller
/// re-derives this from its own trusted state (the deployment record, the
/// live invocation identity, the current attempt counter) — never from the
/// receipt under test — before calling [`verify_root_binding`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootBindingContext<'a> {
    pub agent_id: &'a str,
    pub program_root: &'a str,
    pub deployment_root: &'a str,
    pub instance_root: &'a str,
    pub invocation_id: &'a str,
    pub proposal_grammar_digest: &'a str,
    pub deployment_policy_digest: &'a str,
    /// The highest attempt ordinal already accounted for in this
    /// `(invocation_id, turn)` before this receipt; `0` if none yet. A
    /// legal receipt's `attempt` must be exactly one more than this.
    pub previous_attempt: u32,
}

/// Alias kept for callers that prefer the more descriptive name used in
/// `docs/MODEL-CALL-RECEIPT-V1.md`.
pub type ReceiptRootBinding<'a> = RootBindingContext<'a>;

/// Why [`verify_root_binding`] refused a receipt. Each variant names the
/// exact field that disagreed; a caller building diagnostics never has to
/// diff every field itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingError {
    WrongAgent,
    WrongProgramRoot,
    WrongDeploymentRoot,
    WrongInstanceRoot,
    WrongInvocation,
    WrongGrammar,
    WrongPolicy,
    AttemptOutOfOrder,
}

/// Checks a receipt's root/policy/attempt associations against trusted,
/// independently-held state. This never inspects request/response digests —
/// that is [`super::replay::replay_receipt`]'s job, which additionally
/// recomputes those independently from retained bytes rather than trusting
/// the embedded digest fields.
pub fn verify_root_binding(
    receipt: &ModelCallReceipt,
    context: &RootBindingContext<'_>,
) -> Result<(), BindingError> {
    if receipt.agent_id != context.agent_id {
        return Err(BindingError::WrongAgent);
    }
    if receipt.program_root != context.program_root {
        return Err(BindingError::WrongProgramRoot);
    }
    if receipt.deployment_root != context.deployment_root {
        return Err(BindingError::WrongDeploymentRoot);
    }
    if receipt.instance_root != context.instance_root {
        return Err(BindingError::WrongInstanceRoot);
    }
    if receipt.invocation_id != context.invocation_id {
        return Err(BindingError::WrongInvocation);
    }
    if receipt.proposal_grammar_digest != context.proposal_grammar_digest {
        return Err(BindingError::WrongGrammar);
    }
    if receipt.deployment_policy_digest != context.deployment_policy_digest {
        return Err(BindingError::WrongPolicy);
    }
    if receipt.attempt != context.previous_attempt + 1 {
        return Err(BindingError::AttemptOutOfOrder);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_receipt() -> ModelCallReceipt {
        ModelCallReceipt {
            agent_id: "sha256:agent".into(),
            program_root: "sha256:program".into(),
            deployment_root: "sha256:deployment".into(),
            instance_root: "sha256:instance".into(),
            invocation_id: "sha256:invocation".into(),
            turn: 0,
            attempt: 1,
            model_class: "fixture-model-v1".into(),
            provider_class: "fixture".into(),
            adapter_identity: "sha256:adapter".into(),
            proposal_grammar_digest: "sha256:grammar".into(),
            deployment_policy_digest: "sha256:policy".into(),
            task_digest: commit_task_bytes(b"do the thing"),
            observation_digest: commit_observation_bytes(b"turn 0 context"),
            request_digest: "sha256:request".into(),
            request_bytes_len: 12,
            reserved_budget: 100,
            response_digest: Some(commit_response_bytes(b"{\"answer\":42}")),
            response_bytes_len: Some(13),
            private_payload_reference: None,
            terminal_stage: ReceiptStage::Decoded,
            failure: None,
            proposal_digest: Some(commit_proposal_bytes(b"proposal-bytes")),
            proposal_refusal_reason: None,
            reserved_at_ms: 0,
            dispatched_at_ms: Some(1),
            first_byte_at_ms: Some(2),
            completed_at_ms: Some(3),
            local_request_bytes: 12,
            local_response_bytes: 13,
            cost_estimate_micros: 500,
            provider_call_reference: "invocation-turn0-attempt1".into(),
            provider_reported: None,
        }
    }

    fn binding_context(receipt: &ModelCallReceipt) -> RootBindingContext<'_> {
        RootBindingContext {
            agent_id: &receipt.agent_id,
            program_root: &receipt.program_root,
            deployment_root: &receipt.deployment_root,
            instance_root: &receipt.instance_root,
            invocation_id: &receipt.invocation_id,
            proposal_grammar_digest: &receipt.proposal_grammar_digest,
            deployment_policy_digest: &receipt.deployment_policy_digest,
            previous_attempt: receipt.attempt - 1,
        }
    }

    #[test]
    fn identical_receipts_render_and_digest_identically() {
        let a = sample_receipt();
        let b = sample_receipt();
        assert_eq!(a.render(), b.render());
        assert_eq!(a.digest(), b.digest());
    }

    #[test]
    fn single_byte_mutation_of_the_rendered_receipt_changes_the_digest() {
        let receipt = sample_receipt();
        let original_digest = receipt.digest();
        let rendered = receipt.render();
        // Change exactly one ASCII digit byte of `request_bytes_len` (12 ->
        // 13), the smallest possible tamper a storage-level bit flip or a
        // tampered replay input could produce while staying valid UTF-8.
        assert!(rendered.contains("\"request_bytes_len\":12,"));
        let mutated = rendered.replacen("\"request_bytes_len\":12,", "\"request_bytes_len\":13,", 1);
        assert_ne!(rendered, mutated, "the replacement must actually change one byte");
        assert_eq!(
            rendered.len(),
            mutated.len(),
            "exactly one byte changed, not an insertion/deletion"
        );
        let mutated_digest = digest(RECEIPT_DOMAIN, mutated.as_bytes());
        assert_ne!(
            original_digest, mutated_digest,
            "a single mutated byte in the canonical rendering must change the digest"
        );
    }

    #[test]
    fn verify_root_binding_accepts_the_correct_context() {
        let receipt = sample_receipt();
        let context = binding_context(&receipt);
        assert_eq!(verify_root_binding(&receipt, &context), Ok(()));
    }

    #[test]
    fn verify_root_binding_rejects_every_wrong_root_and_policy_field_individually() {
        let receipt = sample_receipt();
        let base = binding_context(&receipt);

        let mut wrong_agent = base.clone();
        wrong_agent.agent_id = "sha256:someone-else";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_agent),
            Err(BindingError::WrongAgent)
        );

        let mut wrong_program = base.clone();
        wrong_program.program_root = "sha256:other-program";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_program),
            Err(BindingError::WrongProgramRoot)
        );

        let mut wrong_deployment = base.clone();
        wrong_deployment.deployment_root = "sha256:other-deployment";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_deployment),
            Err(BindingError::WrongDeploymentRoot)
        );

        let mut wrong_instance = base.clone();
        wrong_instance.instance_root = "sha256:other-instance";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_instance),
            Err(BindingError::WrongInstanceRoot)
        );

        let mut wrong_invocation = base.clone();
        wrong_invocation.invocation_id = "sha256:other-invocation";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_invocation),
            Err(BindingError::WrongInvocation)
        );

        let mut wrong_grammar = base.clone();
        wrong_grammar.proposal_grammar_digest = "sha256:other-grammar";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_grammar),
            Err(BindingError::WrongGrammar)
        );

        let mut wrong_policy = base.clone();
        wrong_policy.deployment_policy_digest = "sha256:other-policy";
        assert_eq!(
            verify_root_binding(&receipt, &wrong_policy),
            Err(BindingError::WrongPolicy)
        );

        let mut wrong_attempt_order = base;
        // The receipt claims attempt 1; asserting a `previous_attempt` of 1
        // (as if attempt 1 had already happened) makes the *next* legal
        // attempt 2, not this receipt's 1 — an out-of-order replay/retry.
        wrong_attempt_order.previous_attempt = 1;
        assert_eq!(
            verify_root_binding(&receipt, &wrong_attempt_order),
            Err(BindingError::AttemptOutOfOrder)
        );
    }

    #[test]
    fn payload_privacy_claim_never_calls_a_short_undisclosed_payload_withheld() {
        assert_eq!(
            PayloadPrivacyClaim::classify(2, false),
            PayloadPrivacyClaim::DigestOnlyLowEntropyCaveat
        );
        assert_eq!(
            PayloadPrivacyClaim::classify(2, true),
            PayloadPrivacyClaim::Withheld,
            "a private reference makes even a short payload honestly withheld"
        );
        assert_eq!(
            PayloadPrivacyClaim::classify(4096, false),
            PayloadPrivacyClaim::DigestOnly
        );
    }
}
