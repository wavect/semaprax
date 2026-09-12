//! The injected embedding provider boundary.
//!
//! Exactly one implementation is wired in per host/deployment. The
//! compiler and generated program code create no implementation of this
//! trait on their own — there is no default, ambient, or discoverable
//! provider. This module ships only the deterministic
//! [`super::fixture::FixtureEmbeddingProvider`] and the scriptable
//! [`super::fixture::ScriptedEmbeddingProvider`]; a real implementation
//! binding an actual model transport is downstream integration work
//! against this trait, exactly as `live_invocation::model_invoke::
//! ModelHandler` documents for `model.invoke`.

use super::capability::EmbeddingCapability;
use super::request::{EmbeddingOutcome, EmbeddingRequest};

/// The injected, provider-transport-shaped handler for a `semantic.embed`
/// call.
///
/// # Cancellation
///
/// [`super::kernel::embed`] checks cancellation immediately before calling
/// this method; it does not interrupt an in-flight call. A provider
/// observing cancellation mid-call should itself return
/// `EmbeddingOutcome::Failed { failure: EmbeddingFailure::Cancelled, .. }`,
/// and an acknowledged cancellation never proves the provider stopped
/// billing or processing — matching the same nonclaim
/// `live_invocation::model_invoke::ModelHandler` documents.
///
/// # Trust
///
/// A settled vector's length and finiteness are re-validated by
/// [`super::kernel::embed`] before being trusted; an implementation must
/// still only ever report a length matching `request.dimensions`, since a
/// caller reading this trait directly (bypassing the kernel) receives no
/// such re-validation for free.
pub trait EmbeddingProvider {
    fn embed(
        &mut self,
        capability: &EmbeddingCapability,
        request: &EmbeddingRequest,
    ) -> EmbeddingOutcome;
}
