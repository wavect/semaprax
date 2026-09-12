//! Independent replay of a [`super::receipt::ModelCallReceipt`].
//!
//! # Replay is not re-contacting
//!
//! [`replay_receipt`] never accepts, imports, or references
//! [`crate::live_invocation::model_invoke::ModelHandler`] — the trait a real
//! provider transport implements. There is no parameter of that type
//! anywhere in this module's public surface, so a caller cannot wire a live
//! handler in even by mistake: replaying a receipt structurally cannot
//! dispatch a new provider call. What it *can* do is independently
//! recompute every commitment the receipt claims from
//! caller-supplied retained bytes ([`super::audit_view::ReceiptPrivateExtras`])
//! and, when a [`ProposalDecoder`] is supplied, re-run the same decode step
//! the original attempt used to prove the decoded proposal reproduces —
//! never a fresh model answer.
//!
//! # Digests are recomputed, never trusted
//!
//! Every comparison here recomputes a digest from retained bytes with
//! [`super::receipt::commit_task_bytes`] / `commit_observation_bytes` /
//! `commit_response_bytes` / `commit_proposal_bytes` and compares the
//! result against the receipt's embedded field — it never treats the
//! embedded field as self-certifying. A receipt whose `response_digest` was
//! copied from a different attempt, or whose retained response bytes were
//! tampered with after recording, is rejected here, not accepted because
//! the two fields happen to agree by construction.

use crate::live_invocation::model_invoke::{ProposalDecoder, ProposalOutcome};

use super::audit_view::ReceiptPrivateExtras;
use super::receipt::{
    commit_observation_bytes, commit_proposal_bytes, commit_response_bytes, commit_task_bytes,
    ModelCallReceipt,
};

/// Why [`replay_receipt`] refused a receipt. Every variant names the exact
/// disagreement; none of them is a provider-shaped error, matching
/// `crate::live_invocation::model_invoke::ModelFailure`'s own closed-domain
/// discipline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayError {
    /// Retained task bytes do not commit to `receipt.task_digest`.
    TaskDigestMismatch,
    /// Retained observation bytes do not commit to `receipt.observation_digest`.
    ObservationDigestMismatch,
    /// The receipt claims a settled response but no retained response bytes
    /// were supplied to replay against.
    MissingRetainedResponse,
    /// Retained response bytes do not commit to `receipt.response_digest`.
    ResponseDigestMismatch,
    /// The receipt claims a decoded or refused proposal, but no decoder was
    /// supplied to replay the decode step against.
    MissingDecoder,
    /// The supplied decoder is bound to a different grammar than the
    /// receipt's `proposal_grammar_digest` — schema drift, refused before
    /// decode is ever attempted, exactly like the live kernel's own
    /// pre-dispatch grammar check.
    SchemaDrift,
    /// The decoder admitted a proposal whose recomputed digest disagrees
    /// with `receipt.proposal_digest`.
    ProposalDigestMismatch,
    /// The receipt claims the proposal was admitted, but replay's decode
    /// refused it (or vice versa) — the recorded outcome does not reproduce.
    DecodeDisagreesWithReceipt,
}

/// Independently replays one receipt against retained bytes.
///
/// `decoder` is `None` for a receipt whose `terminal_stage` never reached
/// decode (e.g. a provider failure or cancellation before any response
/// settled) — no bytes exist to decode there, and this function does not
/// require one in that case. When the receipt does claim a decode outcome
/// (either `proposal_digest` or `proposal_refusal_reason` is set),
/// `decoder` must be supplied or replay fails closed with
/// [`ReplayError::MissingDecoder`] rather than silently skipping the check.
pub fn replay_receipt(
    receipt: &ModelCallReceipt,
    retained: &ReceiptPrivateExtras,
    decoder: Option<&mut dyn ProposalDecoder>,
) -> Result<(), ReplayError> {
    if commit_task_bytes(&retained.task) != receipt.task_digest {
        return Err(ReplayError::TaskDigestMismatch);
    }
    if commit_observation_bytes(&retained.observation) != receipt.observation_digest {
        return Err(ReplayError::ObservationDigestMismatch);
    }

    let Some(expected_response_digest) = &receipt.response_digest else {
        // No response was ever recorded (a pre-settlement failure or
        // cancellation) — nothing further to replay.
        return Ok(());
    };
    let Some(response_bytes) = &retained.response else {
        return Err(ReplayError::MissingRetainedResponse);
    };
    if &commit_response_bytes(response_bytes) != expected_response_digest {
        return Err(ReplayError::ResponseDigestMismatch);
    }

    if receipt.proposal_digest.is_none() && receipt.proposal_refusal_reason.is_none() {
        // Settled but never decoded (e.g. cancelled after the response
        // arrived, before decode ran).
        return Ok(());
    }

    let decoder = decoder.ok_or(ReplayError::MissingDecoder)?;
    if decoder.schema_digest() != receipt.proposal_grammar_digest {
        return Err(ReplayError::SchemaDrift);
    }
    match decoder.decode(receipt.turn, response_bytes) {
        ProposalOutcome::Admitted(proposal_bytes) => {
            let Some(expected_proposal_digest) = &receipt.proposal_digest else {
                return Err(ReplayError::DecodeDisagreesWithReceipt);
            };
            if &commit_proposal_bytes(&proposal_bytes) != expected_proposal_digest {
                return Err(ReplayError::ProposalDigestMismatch);
            }
        }
        ProposalOutcome::Refused(_) => {
            if receipt.proposal_refusal_reason.is_none() {
                return Err(ReplayError::DecodeDisagreesWithReceipt);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_invocation::fixture::{fixture_response, FixtureModelHandler, FixtureProposalDecoder};
    use crate::live_invocation::model_invoke::{
        ModelHandler, ModelInvocationOutcome, ModelInvokeCapability,
    };
    use crate::model_call_receipt::receipt::tests::sample_receipt;
    use crate::model_call_receipt::receipt::ReceiptStage;

    fn extras_for(task: &[u8], observation: &[u8], response: Option<Vec<u8>>) -> ReceiptPrivateExtras {
        ReceiptPrivateExtras {
            task: task.to_vec(),
            observation: observation.to_vec(),
            response,
            private_payload_reference_material: None,
            adapter_diagnostic_hint: None,
            provider_error_detail: None,
            authorization_header_echo: None,
        }
    }

    /// Builds a receipt end to end through the *real* fixture handler and
    /// decoder (a genuine dispatch, exactly once), and hands back the
    /// receipt plus the handler used, so the replay test below can prove
    /// replay adds no further dispatches to that same handler's counter.
    fn dispatch_once_and_build_receipt() -> (super::ModelCallReceipt, FixtureModelHandler, FixtureProposalDecoder, ReceiptPrivateExtras) {
        let response = fixture_response(0, "42");
        let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
            response.clone(),
        )]);
        let capability = ModelInvokeCapability::grant("replay fixture test");
        let request = crate::live_invocation::model_invoke::ModelInvocationRequest {
            turn: 0,
            task: b"do the thing".to_vec(),
            observation: b"turn 0 context".to_vec(),
            proposal_grammar_digest: "sha256:grammar".into(),
            deployment_binding: "sha256:policy".into(),
            max_response_bytes: 4096,
            effective_budget: 100,
        };
        let outcome = handler.invoke(&capability, &request);
        assert_eq!(handler.calls, 1, "the real dispatch must count exactly once");
        let ModelInvocationOutcome::Settled(response_bytes) = outcome else {
            panic!("scripted outcome must settle");
        };

        let mut decoder = FixtureProposalDecoder::new("sha256:grammar");
        let admitted = decoder.decode(0, &response_bytes);
        let ProposalOutcome::Admitted(proposal_bytes) = admitted else {
            panic!("fixture decoder must admit a well-formed fixture response");
        };

        let mut receipt = sample_receipt();
        receipt.task_digest = super::commit_task_bytes(&request.task);
        receipt.observation_digest = super::commit_observation_bytes(&request.observation);
        receipt.response_digest = Some(commit_response_bytes(&response_bytes));
        receipt.response_bytes_len = Some(response_bytes.len());
        receipt.proposal_digest = Some(commit_proposal_bytes(&proposal_bytes));
        receipt.proposal_refusal_reason = None;
        receipt.terminal_stage = ReceiptStage::Decoded;
        receipt.proposal_grammar_digest = "sha256:grammar".into();

        let extras = extras_for(&request.task, &request.observation, Some(response_bytes));
        (receipt, handler, decoder, extras)
    }

    #[test]
    fn replay_accepts_an_honest_receipt_and_reproduces_the_same_decode() {
        let (receipt, _handler, mut decoder, extras) = dispatch_once_and_build_receipt();
        assert_eq!(replay_receipt(&receipt, &extras, Some(&mut decoder)), Ok(()));
    }

    #[test]
    fn replay_makes_zero_additional_dispatches_against_the_shared_handler() {
        let (receipt, handler, mut decoder, extras) = dispatch_once_and_build_receipt();
        assert_eq!(handler.calls, 1, "exactly one real dispatch happened before replay");

        // Replay does not even accept a handler, so there is no way to pass
        // `handler` into `replay_receipt` at all; the strongest available
        // proof is that the shared counter is unchanged after replay runs,
        // and that replaying the same receipt many times never moves it.
        for _ in 0..5 {
            assert_eq!(replay_receipt(&receipt, &extras, Some(&mut decoder)), Ok(()));
        }
        assert_eq!(
            handler.calls, 1,
            "replay must never increase the dispatch counter beyond the original real call"
        );

        // A handler that panics if ever called at all, scoped to prove the
        // *type itself* structurally cannot be reached from replay: it is
        // never constructed as an argument anywhere in this test's replay
        // calls, and `replay_receipt`'s signature has no parameter it could
        // even be passed through.
        let never_called = FixtureModelHandler::must_not_be_called();
        assert_eq!(never_called.calls, 0);
    }

    #[test]
    fn replay_rejects_tampered_task_bytes() {
        let (receipt, _handler, mut decoder, extras) = dispatch_once_and_build_receipt();
        let mut tampered = extras.clone();
        tampered.task = b"a completely different task".to_vec();
        assert_eq!(
            replay_receipt(&receipt, &tampered, Some(&mut decoder)),
            Err(ReplayError::TaskDigestMismatch)
        );
    }

    #[test]
    fn replay_rejects_tampered_observation_bytes() {
        let (receipt, _handler, mut decoder, extras) = dispatch_once_and_build_receipt();
        let mut tampered = extras.clone();
        tampered.observation = b"a completely different observation".to_vec();
        assert_eq!(
            replay_receipt(&receipt, &tampered, Some(&mut decoder)),
            Err(ReplayError::ObservationDigestMismatch)
        );
    }

    #[test]
    fn replay_rejects_a_wrong_response_by_recomputing_its_digest_independently() {
        let (receipt, _handler, mut decoder, extras) = dispatch_once_and_build_receipt();
        // A different, still well-formed fixture response for the same
        // turn: the embedded `response_digest` field is untouched, but the
        // retained bytes it is checked against are not the ones that
        // produced it. This is exactly the "wrong response" rejection case
        // and the "recomputation is independent" property in one test: if
        // replay merely re-read `receipt.response_digest` it would trivially
        // "pass" against anything.
        let mut tampered = extras.clone();
        tampered.response = Some(fixture_response(0, "not-the-real-answer"));
        assert_eq!(
            replay_receipt(&receipt, &tampered, Some(&mut decoder)),
            Err(ReplayError::ResponseDigestMismatch)
        );
    }

    #[test]
    fn replay_rejects_schema_drift_before_attempting_decode() {
        let (receipt, _handler, _decoder, extras) = dispatch_once_and_build_receipt();
        let mut drifted_decoder = FixtureProposalDecoder::new("sha256:a-different-grammar");
        assert_eq!(
            replay_receipt(&receipt, &extras, Some(&mut drifted_decoder)),
            Err(ReplayError::SchemaDrift)
        );
    }

    #[test]
    fn replay_requires_a_decoder_when_the_receipt_claims_a_decode_outcome() {
        let (receipt, _handler, _decoder, extras) = dispatch_once_and_build_receipt();
        assert_eq!(
            replay_receipt(&receipt, &extras, None),
            Err(ReplayError::MissingDecoder)
        );
    }

    #[test]
    fn replay_rejects_a_receipt_claiming_admitted_when_decode_actually_refuses() {
        let (mut receipt, _handler, mut decoder, mut extras) = dispatch_once_and_build_receipt();
        // Corrupt the retained response so the *real* fixture decoder would
        // refuse it, while the receipt still claims (falsely) that it was
        // admitted with the original proposal digest.
        extras.response = Some(b"not json shaped at all".to_vec());
        receipt.response_digest = Some(commit_response_bytes(extras.response.as_ref().unwrap()));
        assert_eq!(
            replay_receipt(&receipt, &extras, Some(&mut decoder)),
            Err(ReplayError::DecodeDisagreesWithReceipt)
        );
    }

    #[test]
    fn replay_skips_decode_for_a_receipt_that_never_reached_a_response() {
        let mut receipt = sample_receipt();
        receipt.response_digest = None;
        receipt.response_bytes_len = None;
        receipt.proposal_digest = None;
        receipt.proposal_refusal_reason = None;
        receipt.terminal_stage = ReceiptStage::Uncertain;
        receipt.failure = Some("timeout".into());
        let extras = extras_for(b"do the thing", b"turn 0 context", None);
        assert_eq!(replay_receipt(&receipt, &extras, None), Ok(()));
    }
}
