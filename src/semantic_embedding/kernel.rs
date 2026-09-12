//! The enforcement boundary between a caller's request and the injected
//! [`EmbeddingProvider`]: explicit capacity/cancellation checks that run
//! before a provider is ever reached, and result validation that runs on
//! whatever the provider claims to have produced.

use super::capability::EmbeddingCapability;
use super::provider::EmbeddingProvider;
use super::request::{EmbeddingFailure, EmbeddingOutcome, EmbeddingRequest};

/// Dispatches one embedding request through `provider`.
///
/// `cancelled` is checked exactly once, immediately before the provider is
/// called; this function never re-checks it mid-call (the provider itself
/// owns that, matching `live_invocation::model_invoke::ModelHandler`'s
/// documented contract). Two checks happen entirely without invoking
/// `provider`, so a caller can prove a request never reaches the injected
/// capability at all:
///
/// 1. `cancelled()` is `true` -> `Failed { Cancelled, attempted_bytes: 0 }`.
/// 2. `request.input.len() > request.max_input_bytes` ->
///    `Failed { CapacityExceeded, attempted_bytes: request.input.len() }`.
///
/// After a genuine dispatch, a settled vector is re-validated against the
/// request before it is trusted: a wrong length or any non-finite
/// component becomes `Failed { MalformedResponse, .. }` rather than being
/// passed through — a provider's self-reported shape is never trusted
/// blindly, matching the "raw model output never reaches [downstream]
/// dispatch" rule `live_invocation::model_invoke::ProposalOutcome`
/// documents for the analogous `model.invoke` boundary.
pub fn embed(
    provider: &mut dyn EmbeddingProvider,
    capability: &EmbeddingCapability,
    request: &EmbeddingRequest,
    cancelled: &dyn Fn() -> bool,
) -> EmbeddingOutcome {
    if cancelled() {
        return EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::Cancelled,
            attempted_bytes: 0,
        };
    }
    if request.input.len() > request.max_input_bytes {
        return EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::CapacityExceeded,
            attempted_bytes: request.input.len(),
        };
    }
    match provider.embed(capability, request) {
        EmbeddingOutcome::Settled(vector) => {
            if vector.len() != request.dimensions as usize {
                return EmbeddingOutcome::Failed {
                    failure: EmbeddingFailure::MalformedResponse,
                    attempted_bytes: vector.len().saturating_mul(4),
                };
            }
            if vector.iter().any(|component| !component.is_finite()) {
                return EmbeddingOutcome::Failed {
                    failure: EmbeddingFailure::MalformedResponse,
                    attempted_bytes: vector.len().saturating_mul(4),
                };
            }
            EmbeddingOutcome::Settled(vector)
        }
        failed @ EmbeddingOutcome::Failed { .. } => failed,
    }
}
