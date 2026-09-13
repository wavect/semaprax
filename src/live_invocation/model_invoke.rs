//! The provider-independent `model.invoke` effect boundary.
//!
//! `model.invoke` is an ordinary checked effect: a typed request crosses an
//! explicit capability gate, an injected handler answers it, and the raw
//! answer is decoded through a compiler-derived grammar before anything
//! downstream can call it a proposal. Nothing here names a provider. The
//! [`ModelHandler`], [`ProposalDecoder`] and [`AuthorizationGate`] traits are
//! the seams a deployment binds to an actual provider transport, a real
//! compiled proposal grammar, and the real lifecycle authorization stage
//! respectively; this crate ships only deterministic fixture
//! implementations (see [`super::fixture`]) so that ordinary compiler CI
//! stays fully offline. [`InvocationBudgetHook`] is the same kind of seam
//! for cumulative budget policy: this module reserves and records through
//! it but implements no cross-invocation accounting itself, so issues that
//! own budget policy attach behind this one hook rather than inventing a
//! second one.

use super::identity::digest;
use crate::diagnostic::quote_json;

const REQUEST_DOMAIN: &[u8] = b"semaprax.live-invocation.model-request.v1\0";

/// Explicit, non-ambient authority to reach a model provider.
///
/// There is no [`Default`] implementation and no constructor that does not
/// name why the grant exists. A caller wires this in explicitly (typically
/// once, at the same place `AgentDeployment` binds a provider/model policy);
/// nothing in source, in a proposal, or in a model response can synthesize
/// one. Compiled program text and generated code hold no path to this type
/// without an explicit host-supplied value, matching the "capabilities are
/// explicit" invariant: no ambient network authority exists merely because a
/// module declares a `propose` role.
#[derive(Clone, Debug)]
pub struct ModelInvokeCapability {
    reason: String,
}

impl ModelInvokeCapability {
    /// Grants the capability. `reason` is caller-facing diagnostic text
    /// (e.g. `"fixture test"`, `"deployment fixture.deploy.v1"`); it carries
    /// no authority of its own and is never parsed.
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

/// The typed `model.invoke` request for exactly one turn.
///
/// Every field here is compiler- or deployment-derived, never model output:
/// a request is built *before* any provider is contacted, from the task,
/// the current turn's deterministic observation/context projection, the
/// identity of the grammar the response must decode against, which exact
/// deployment binding is in force, and the effective per-call limits. A
/// `model.invoke` request never contains a raw filesystem path, environment
/// variable, credential or unchecked dynamic map: the host handler is
/// responsible for attaching provider credentials outside checked program
/// data, exactly as [`ModelHandler::invoke`] documents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInvocationRequest {
    /// The zero-based turn this request belongs to.
    pub turn: u32,
    /// The task bytes the whole invocation was bound to.
    pub task: Vec<u8>,
    /// This turn's deterministic observation/context projection.
    pub observation: Vec<u8>,
    /// Digest of the compiler-derived grammar the response must decode
    /// against. A handler that answers on behalf of a stale grammar cannot
    /// pass decode; a decoder is bound to exactly this digest before use
    /// (see [`ProposalDecoder::schema_digest`]).
    pub proposal_grammar_digest: String,
    /// Digest of the exact deployment/model policy binding in force. Model
    /// or provider substitution changes this digest; Agent source identity
    /// does not depend on it.
    pub deployment_binding: String,
    /// The maximum response size this call may return, in bytes.
    pub max_response_bytes: usize,
    /// The effective per-call budget reserved for this turn (opaque units;
    /// the deployment defines what one unit costs).
    pub effective_budget: i64,
}

impl ModelInvocationRequest {
    /// The canonical digest identifying this exact request. Two requests
    /// that would ask a provider the same question produce the same digest;
    /// any differing field (including the turn number) changes it.
    #[must_use]
    pub fn digest(&self) -> String {
        let body = format!(
            "{{\"schema\":\"semaprax.live-invocation.model-request.v1\",\"turn\":{},\"task\":{},\"observation\":{},\"proposal_grammar_digest\":{},\"deployment_binding\":{},\"max_response_bytes\":{},\"effective_budget\":{}}}",
            self.turn,
            quote_json(&super::identity::hex(&self.task)),
            quote_json(&super::identity::hex(&self.observation)),
            quote_json(&self.proposal_grammar_digest),
            quote_json(&self.deployment_binding),
            self.max_response_bytes,
            self.effective_budget,
        );
        digest(REQUEST_DOMAIN, body.as_bytes())
    }
}

/// The closed `model.invoke` failure domain.
///
/// Provider-specific errors never become language semantics: a handler
/// normalizes whatever a real transport reports into exactly one of these
/// cases before returning. Redaction-aware provider detail (if any) is a
/// diagnostic concern for the handler's own logging, never a journal field —
/// the causal journal records the closed tag and a bounded attempted-byte
/// count, nothing provider-shaped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelFailure {
    /// The call did not settle before its deadline.
    Timeout,
    /// Cancellation was observed before or during the call.
    Cancelled,
    /// The provider or deployment reported no capacity for this call.
    CapacityExceeded,
    /// The provider reported an error unrelated to the above.
    ProviderError,
    /// A response arrived but was too large, truncated, or not a byte
    /// stream the handler could produce at all.
    MalformedResponse,
    /// The handler declined to make the call (e.g. a policy refusal).
    Refused,
}

impl ModelFailure {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::ProviderError => "provider_error",
            Self::MalformedResponse => "malformed_response",
            Self::Refused => "refused",
        }
    }
}

/// The outcome of one dispatched `model.invoke` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelInvocationOutcome {
    /// The provider answered; these are the raw, still-untrusted response
    /// bytes. They are recorded before decode is attempted.
    Settled(Vec<u8>),
    /// The call did not settle successfully, in the closed failure domain.
    /// `attempted_bytes` is a bounded measurement of whatever partial or
    /// malformed payload was seen, charged the same way a failed typed
    /// effect result stays charged in [`crate::agent_lifecycle::iterative`].
    Failed {
        failure: ModelFailure,
        attempted_bytes: usize,
    },
}

/// The injected, provider-transport-shaped handler.
///
/// Exactly one implementation is wired in per deployment. The compiler and
/// generated program code create no implementation of this trait on their
/// own — there is no default, ambient, or discoverable provider. A real
/// implementation is responsible for attaching credentials from outside
/// checked program data (environment, secret store, host configuration);
/// [`ModelInvocationRequest`] structurally cannot carry one.
///
/// # Cancellation
///
/// The kernel checks cancellation immediately before calling `invoke` and
/// again before committing a durable request intent, matching the existing
/// iterative lifecycle's "checked at each deterministic stage and before
/// dispatch" rule. A handler observing cancellation mid-call should return
/// `Failed { failure: ModelFailure::Cancelled, .. }`; the kernel does not
/// interrupt an in-flight call, and an acknowledged cancellation never
/// proves the provider stopped billing or processing — that is a declared
/// nonclaim, matching Direct Runtime v2's cancellation contract.
pub trait ModelHandler {
    fn invoke(
        &mut self,
        capability: &ModelInvokeCapability,
        request: &ModelInvocationRequest,
    ) -> ModelInvocationOutcome;
}

/// The decode outcome of one settled response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalOutcome {
    /// The response decoded against the bound grammar. The carried bytes
    /// are the canonical decoded form; this is the *only* value the kernel
    /// ever passes to [`AuthorizationGate::authorize`] or to a reducer —
    /// raw model output never reaches effect dispatch.
    Admitted(Vec<u8>),
    /// Decode refused: malformed bytes, schema drift, or a grammar the
    /// decoder does not recognise.
    Refused(String),
}

/// The injected decode boundary between an untrusted response and a checked
/// proposal.
///
/// A real implementation binds `schema_digest` to the compiler-derived
/// grammar for the deployed Agent and decodes through it (see
/// `docs/AGENT-PROPOSAL-RUNTIME-V1-COMPATIBILITY-V1.md` for the existing
/// grammar/decode machinery this seam is meant to be bound to downstream).
/// This module ships only a fixture decoder.
pub trait ProposalDecoder {
    /// The grammar digest this decoder decodes against. The kernel compares
    /// this against every request's `proposal_grammar_digest` before
    /// dispatch and refuses schema drift without calling the handler.
    fn schema_digest(&self) -> &str;
    fn decode(&mut self, turn: u32, response: &[u8]) -> ProposalOutcome;
}

/// The context one authorization decision is made against.
pub struct AuthorizationContext<'a> {
    pub turn: u32,
    pub observation_digest: &'a str,
    pub proposal_digest: &'a str,
}

/// One opaque, consumed grant. It carries a digest for journal binding only;
/// it is not decodable back into the context it was granted against, so a
/// journal can record that a grant existed without a journal reader being
/// able to forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationGrant(String);

impl AuthorizationGrant {
    #[must_use]
    pub fn new(digest: String) -> Self {
        Self(digest)
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationRefusal(pub String);

/// The injected authorization boundary. A real implementation binds the
/// existing `agent_lifecycle::authorization` mint site; this module ships
/// only a fixture gate that always grants within budget.
pub trait AuthorizationGate {
    fn authorize(
        &mut self,
        context: &AuthorizationContext<'_>,
    ) -> Result<AuthorizationGrant, AuthorizationRefusal>;
}

/// A budget reservation handed back by [`InvocationBudgetHook::reserve`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservedBudget {
    pub amount: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetRefusal(pub String);

/// Usage reported to the hook after a turn settles, win or lose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvocationUsage {
    pub turn: u32,
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub failed: bool,
}

/// The accounting seam issues #113/#179 attach cumulative budget policy
/// behind. This module reserves before every dispatch and records usage
/// after every settlement, but implements no policy of its own: the fixture
/// hook in [`super::fixture`] is a trivial per-invocation counter, not a
/// cumulative-budget reference implementation.
pub trait InvocationBudgetHook {
    /// Check the invocation's existing absolute deadline without reserving or
    /// refunding work. Called at settlement and dispatch/publication boundaries.
    /// Policies without a deadline retain their existing behavior.
    fn check_deadline(&self) -> Result<(), BudgetRefusal> {
        Ok(())
    }

    fn reserve(
        &mut self,
        request: &ModelInvocationRequest,
    ) -> Result<ReservedBudget, BudgetRefusal>;
    fn record(&mut self, usage: &InvocationUsage);
}
