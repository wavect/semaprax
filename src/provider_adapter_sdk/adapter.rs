//! The adapter ABI v1 surface: capability declaration lives in
//! [`super::capability`]; this module is request start, event
//! polling/streaming, cancellation, and settlement (issue #181 step 1),
//! plus the explicit, non-ambient capability token gating construction of
//! any adapter that could reach outside this process.
//!
//! # No ambient authority
//!
//! [`AdapterInvocationCapability`] mirrors
//! [`crate::live_invocation::model_invoke::ModelInvokeCapability`] exactly:
//! there is no [`Default`] and no constructor that does not name why the
//! grant exists, and nothing in [`AdapterRequest`], an [`AdapterEvent`], or
//! an [`AdapterSettlement`] can synthesize one. An adapter's `start` method
//! receives this token but has no method of its own that could mint a
//! *further* capability, an
//! [`crate::live_invocation::model_invoke::AuthorizationGrant`], or a wider
//! [`crate::model_budget_policy::EffectiveModelBudget`] — this module
//! imports neither of those two read-only types at all, so an adapter
//! implementation has no path to either regardless of what it returns.
//!
//! ```compile_fail
//! // An adapter cannot mint the authorization grant a real deployment's
//! // AuthorizationGate alone produces: there is no conversion from this
//! // SDK's event type into it, because this module never imports the type
//! // at all — `.into()` has no impl to reach for and this fails to typecheck.
//! fn from_adapter_event(event: semaprax::provider_adapter_sdk::AdapterEvent) -> semaprax::live_invocation::model_invoke::AuthorizationGrant {
//!     event.into()
//! }
//! ```

use crate::live_invocation::model_invoke::ModelFailure;

use super::capability::AdapterCapabilities;

/// Explicit, non-ambient authority to construct and drive one adapter
/// instance. A host wires this in once per deployment binding; nothing in
/// source, in a request, or in a provider response can synthesize one.
#[derive(Clone, Debug)]
pub struct AdapterInvocationCapability {
    reason: String,
}

impl AdapterInvocationCapability {
    /// Grants the capability. `reason` is caller-facing diagnostic text; it
    /// carries no authority of its own and is never parsed.
    #[must_use]
    pub fn grant(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// One outbound request. Every field is host/deployment-derived, never
/// model output; it carries no credential, header, proxy, or endpoint —
/// those are the concern of whatever concrete transport an adapter's own
/// constructor was given out of band, exactly as
/// [`crate::live_invocation::model_invoke::ModelInvocationRequest`]'s own
/// doc comment requires of the handler seam this SDK sits behind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRequest {
    pub request_bytes: Vec<u8>,
    pub max_response_bytes: usize,
}

/// One event an adapter reports while a request is in flight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterEvent {
    /// A chunk of raw, still-untrusted response bytes.
    Delta(Vec<u8>),
    /// A usage snapshot the provider reported mid-stream. Never trusted as
    /// final on its own: [`super::conformance::drive_to_settlement`]
    /// cross-checks every later snapshot against the one before it rather
    /// than trusting the latest.
    Usage {
        tokens_in: u64,
        tokens_out: u64,
        cost_micros: i64,
    },
    /// The provider signalled the response is fully sent. Exactly one
    /// `Completed` is legal per request.
    Completed,
}

/// The result of one [`ProviderAdapter::poll`] call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterPoll {
    /// One event is ready.
    Event(AdapterEvent),
    /// Nothing yet; poll again.
    Pending,
    /// The request settled. `response_bytes` is the adapter's own final
    /// assembly (a conforming adapter's `response_bytes` equals every
    /// `Delta` it emitted, concatenated in order, but this type does not
    /// itself enforce that — see
    /// [`super::conformance::drive_to_settlement`], which does).
    Settled(AdapterSettlement),
    /// The adapter reports failure, already normalized into the closed
    /// [`ModelFailure`] vocabulary — never a raw provider error string.
    Failed {
        failure: ModelFailure,
        attempted_bytes: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterSettlement {
    pub response_bytes: Vec<u8>,
    pub usage: AdapterUsage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterUsage {
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cost_micros: Option<i64>,
}

/// Why [`ProviderAdapter::start`] refused before any dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRefusal(pub String);

/// The injected, provider-transport-shaped adapter boundary. Exactly one
/// implementation is wired in per provider profile; this crate ships only
/// deterministic fixture implementations
/// ([`super::fixture_adapters`]) plus hostile ones
/// ([`super::hostile`]) built to fail the conformance suite on purpose. A
/// real provider transport is downstream integration work maintained
/// outside this crate's core semantics.
pub trait ProviderAdapter {
    /// The capabilities this adapter declares. Must be stable for the
    /// lifetime of one instance: [`super::capability::negotiate`] is called
    /// exactly once, before `start`, and a caller must be able to trust
    /// that a later call reports the same thing.
    fn capabilities(&self) -> &AdapterCapabilities;

    /// Begins one request. Called at most once per instance. Refusing here
    /// (e.g. malformed request shape) means the provider was never
    /// contacted at all — the same
    /// [`crate::model_budget_policy::classification::AttemptOutcomeClass::NotDispatched`]
    /// case a real deployment's budget policy already recognises.
    fn start(
        &mut self,
        capability: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal>;

    /// Polls for the next event, or a terminal settlement/failure. A
    /// conforming adapter never returns anything after its first terminal
    /// (`Settled`/`Failed`) result other than that same result again.
    fn poll(&mut self) -> AdapterPoll;

    /// Requests cancellation. This is a *request*, never proof: issue
    /// #181's own "Failure and security cases" names "cancellation APIs
    /// vary and often do not prove server cancellation" as a standing risk,
    /// so this method returns nothing and callers must keep polling to
    /// observe how (or whether) the adapter actually stops.
    fn cancel(&mut self, reason: &str);
}
