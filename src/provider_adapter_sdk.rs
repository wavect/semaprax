//! Provider Adapter SDK v1 (issue #181): one narrow, provider-neutral
//! interface external model-provider adapters implement, plus a shared
//! deterministic conformance and hostility suite an adapter must pass
//! before being described as supported.
//!
//! See [`docs/PROVIDER-ADAPTER-SDK-V1.md`](../docs/PROVIDER-ADAPTER-SDK-V1.md)
//! for the full contract this module implements.
//!
//! # Relationship to existing, read-only modules
//!
//! This module sits *behind* [`crate::live_invocation::model_invoke::ModelHandler`],
//! never replaces it: a real deployment still binds exactly one
//! `ModelHandler` per deployment, and a real integration adapts one
//! [`adapter::ProviderAdapter`] into that seam (that adaptation is
//! downstream integration work, out of this module's scope, exactly as
//! `model_invoke`'s own doc comment describes the handler seam itself as
//! future wiring for a real transport). This module adds the one thing
//! that seam does not: a richer, capability-declaring, event-streaming
//! adapter shape a third-party provider integration can implement once and
//! have conformance-checked, independent of `model_invoke`'s single
//! blocking `invoke` call.
//!
//! `src/live_invocation/`, `src/model_budget_policy/`,
//! `src/model_call_receipt/`, `src/streaming_proposal_decode*` and
//! `src/agent_interaction_schema/` are read-only from this module's
//! perspective. It reuses their already-public surface rather than
//! duplicating it:
//! [`crate::live_invocation::model_invoke::ModelFailure`] is this module's
//! closed error-normalization vocabulary too (one taxonomy, not two);
//! [`crate::model_budget_policy::classification::AttemptOutcomeClass`] and
//! `retry_is_permitted` are the retry-safety vocabulary
//! [`capability::AdapterCapabilities::retryable_failure_classes`] declares
//! into and [`capability::negotiate`] validates against, so an adapter can
//! never declare a class the budget/failover policy would refuse to trust.
//!
//! # No live network, no real provider, no key
//!
//! Every adapter this crate ships ([`fixture_adapters`], [`hostile`]) is
//! pure, offline, deterministic data. None of them open a socket, read an
//! environment variable, or hold a credential capable of reaching a real
//! provider. A real provider integration is downstream work maintained
//! outside this crate's core semantics, exactly as the owning issue's
//! "Explicitly out of scope" section requires.
//!
//! # Capabilities are explicit
//!
//! [`adapter::AdapterInvocationCapability`] mirrors
//! [`crate::live_invocation::model_invoke::ModelInvokeCapability`]: there is
//! no [`Default`] and no constructor that does not name why the grant
//! exists, and nothing in an adapter's declared capabilities, a request, or
//! a provider event can synthesize one. [`capability::negotiate`]
//! additionally refuses, categorically and before any dispatch, any adapter
//! that declares its own ambient endpoint
//! ([`capability::EndpointPolicy::AdapterDeclaredAmbient`]) or that
//! declares a retry-unsafe failure class as retryable — an adapter cannot
//! widen its own authority merely by claiming to.

pub mod adapter;
pub mod capability;
pub mod conformance;
pub mod fixture_adapters;
pub mod hostile;
pub mod report;

pub use adapter::{
    AdapterEvent, AdapterInvocationCapability, AdapterPoll, AdapterRefusal, AdapterRequest,
    AdapterSettlement, AdapterUsage, ProviderAdapter,
};
pub use capability::{
    negotiate, AdapterCapabilities, CancellationSemantics, EndpointPolicy, NegotiationRefusal,
    RequiredCapabilities, StructuredOutputMode, TokenAccountingSource,
};
pub use conformance::{drive_to_settlement, run_conformance_suite, DriveOutcome, ExpectedOutcome};
pub use report::{ConformanceReport, ReportCaseResult};

#[cfg(test)]
mod tests;
