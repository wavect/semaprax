//! Declared adapter capabilities and the negotiation gate.
//!
//! Issue #181 step 2: "Require adapters to declare supported
//! structured-output modes, streaming, token accounting source,
//! cancellation semantics, retryable failure classes, endpoint policy, and
//! maximum limits." [`AdapterCapabilities`] is that declaration;
//! [`negotiate`] is "capability negotiation rejects unsupported
//! combinations before dispatch" (the acceptance criterion of the same
//! name) — the *only* function this module exposes for deciding whether a
//! declared adapter may be dispatched against a caller's requirement, and
//! it never inspects a request or a provider response: negotiation is
//! decided entirely from the two declarations, before either side has sent
//! or received a byte.

use crate::model_budget_policy::classification::{retry_is_permitted, AttemptOutcomeClass};

/// A structured-output mode an adapter can request from (and admit back
/// from) a provider. Deliberately small and closed: a provider-specific
/// mode never becomes a mandatory core-language field (issue #181's
/// "Explicitly out of scope"), so a provider that supports something this
/// enum does not name simply cannot declare it, rather than this SDK
/// growing a new variant per provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StructuredOutputMode {
    /// Unstructured text; the caller's own streaming decoder
    /// ([`crate::streaming_proposal_decode`]) is responsible for admission.
    RawText,
    /// The provider's own "JSON mode"/"JSON object" response constraint.
    JsonMode,
    /// The provider's own native tool/function-call response shape.
    /// Declaring this capability is not authority to dispatch it: issue
    /// #181's "Provider-native tool calls can bypass Proposal decoding" is
    /// exactly the failure case this SDK never admits a path around —
    /// nothing in this crate turns a `ToolCallShaped` event into a
    /// dispatched effect. See the module doc's "Capabilities are
    /// explicit" section.
    ToolCallShaped,
}

impl StructuredOutputMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RawText => "raw_text",
            Self::JsonMode => "json_mode",
            Self::ToolCallShaped => "tool_call_shaped",
        }
    }
}

/// Where a settled attempt's usage/cost numbers came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenAccountingSource {
    /// The provider itself reported usage as part of the response or a
    /// mid-stream event.
    ProviderReported,
    /// The adapter estimates usage locally (e.g. by counting bytes); never
    /// to be presented as an authoritative provider figure.
    LocalEstimate,
    /// No usage figure is available at all for this adapter/model
    /// combination.
    Unavailable,
}

impl TokenAccountingSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProviderReported => "provider_reported",
            Self::LocalEstimate => "local_estimate",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The declared cancellation contract. There is no `Guaranteed` variant:
/// issue #181's own "Failure and security cases" names "cancellation APIs
/// vary and often do not prove server cancellation" as a standing risk, so
/// this SDK gives no adapter a vocabulary word that would let it claim
/// otherwise. `BestEffortRequestStop` is the strongest legal claim: this
/// process asked the adapter to stop, nothing more.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationSemantics {
    BestEffortRequestStop,
    Unsupported,
}

/// Where the endpoint an adapter reaches comes from. `HostInjected` is the
/// only legal value a real adapter may declare — see [`negotiate`], which
/// refuses `AdapterDeclaredAmbient` unconditionally, before any dispatch,
/// regardless of what a caller requires. `AdapterDeclaredAmbient` exists
/// only so the conformance suite (and its hostile fixtures, see
/// `super::hostile`) can name and refuse this exact failure case rather
/// than the SDK simply having no representation for it at all — issue
/// #181's "Adapters can hide ambient endpoint, proxy, environment, or
/// credential lookup" is the standing risk this variant lets a test name
/// directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EndpointPolicy {
    HostInjected,
    AdapterDeclaredAmbient(String),
}

/// One adapter's full, declared capability set. Every field is a
/// self-report the adapter's own implementation supplies from
/// [`super::adapter::ProviderAdapter::capabilities`]; nothing here is
/// inferred from a live call, and nothing here carries a credential,
/// header, proxy, or endpoint value beyond the categorical
/// [`EndpointPolicy`] tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterCapabilities {
    /// A caller-facing, stable identity for this exact adapter
    /// implementation and version (never a provider's own model or
    /// account identifier).
    pub adapter_identity: String,
    pub adapter_version: String,
    /// An opaque, host-assigned provider profile label. Never parsed;
    /// carries no authority of its own, exactly like
    /// [`crate::live_invocation::model_invoke::ModelInvokeCapability::reason`].
    pub provider_profile: String,
    pub structured_output_modes: Vec<StructuredOutputMode>,
    pub supports_streaming: bool,
    pub token_accounting_source: TokenAccountingSource,
    pub cancellation_semantics: CancellationSemantics,
    /// The subset of [`AttemptOutcomeClass`] this adapter promises it can
    /// produce as a *retryable* classification. [`negotiate`] refuses any
    /// adapter that lists a class [`retry_is_permitted`] would refuse.
    pub retryable_failure_classes: Vec<AttemptOutcomeClass>,
    pub endpoint_policy: EndpointPolicy,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_context_tokens: u64,
    pub max_output_tokens: u64,
}

/// What a caller (a deployment binding) requires of an adapter before it
/// may be dispatched against.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredCapabilities {
    pub require_streaming: bool,
    pub require_structured_output_mode: Option<StructuredOutputMode>,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
}

/// Why [`negotiate`] refused, naming the exact disagreeing dimension —
/// never a bare "unsupported".
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NegotiationRefusal {
    /// The adapter declared an ambient endpoint rather than
    /// [`EndpointPolicy::HostInjected`]. Refused unconditionally,
    /// independent of `required`.
    AmbientEndpointDeclared { declared: String },
    /// The adapter declared a failure class as retryable that
    /// [`retry_is_permitted`] does not admit. Refused unconditionally,
    /// independent of `required`.
    UnsafeRetryableClassDeclared { class: AttemptOutcomeClass },
    StreamingNotSupported,
    StructuredOutputModeNotSupported { mode: StructuredOutputMode },
    RequestBudgetExceedsAdapterMax { requested: usize, adapter_max: usize },
    ResponseBudgetExceedsAdapterMax { requested: usize, adapter_max: usize },
}

/// Admits (or refuses) dispatching against an adapter declaring `caps`
/// under a caller's `required` capabilities. Pure and read-only: this never
/// calls [`super::adapter::ProviderAdapter::start`] or any other adapter
/// method, and a caller that gets `Err` back must never call `start`
/// either — the whole point of "before dispatch" is that a refused
/// combination never reaches the adapter at all.
pub fn negotiate(
    caps: &AdapterCapabilities,
    required: &RequiredCapabilities,
) -> Result<(), NegotiationRefusal> {
    if let EndpointPolicy::AdapterDeclaredAmbient(declared) = &caps.endpoint_policy {
        return Err(NegotiationRefusal::AmbientEndpointDeclared {
            declared: declared.clone(),
        });
    }
    for class in &caps.retryable_failure_classes {
        if !retry_is_permitted(*class) {
            return Err(NegotiationRefusal::UnsafeRetryableClassDeclared { class: *class });
        }
    }
    if required.require_streaming && !caps.supports_streaming {
        return Err(NegotiationRefusal::StreamingNotSupported);
    }
    if let Some(mode) = required.require_structured_output_mode {
        if !caps.structured_output_modes.contains(&mode) {
            return Err(NegotiationRefusal::StructuredOutputModeNotSupported { mode });
        }
    }
    if required.max_request_bytes > caps.max_request_bytes {
        return Err(NegotiationRefusal::RequestBudgetExceedsAdapterMax {
            requested: required.max_request_bytes,
            adapter_max: caps.max_request_bytes,
        });
    }
    if required.max_response_bytes > caps.max_response_bytes {
        return Err(NegotiationRefusal::ResponseBudgetExceedsAdapterMax {
            requested: required.max_response_bytes,
            adapter_max: caps.max_response_bytes,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conforming_caps() -> AdapterCapabilities {
        AdapterCapabilities {
            adapter_identity: "test-adapter".into(),
            adapter_version: "1.0.0".into(),
            provider_profile: "fixture".into(),
            structured_output_modes: vec![StructuredOutputMode::JsonMode],
            supports_streaming: true,
            token_accounting_source: TokenAccountingSource::LocalEstimate,
            cancellation_semantics: CancellationSemantics::BestEffortRequestStop,
            retryable_failure_classes: vec![
                AttemptOutcomeClass::NotDispatched,
                AttemptOutcomeClass::RejectedBeforeProcessing,
            ],
            endpoint_policy: EndpointPolicy::HostInjected,
            max_request_bytes: 4096,
            max_response_bytes: 65_536,
            max_context_tokens: 8192,
            max_output_tokens: 2048,
        }
    }

    fn permissive_requirement() -> RequiredCapabilities {
        RequiredCapabilities {
            require_streaming: false,
            require_structured_output_mode: None,
            max_request_bytes: 0,
            max_response_bytes: 0,
        }
    }

    #[test]
    fn a_conforming_adapter_is_admitted() {
        assert_eq!(negotiate(&conforming_caps(), &permissive_requirement()), Ok(()));
    }

    #[test]
    fn an_ambient_endpoint_declaration_is_refused_before_any_streaming_check() {
        let mut caps = conforming_caps();
        caps.endpoint_policy = EndpointPolicy::AdapterDeclaredAmbient("http://169.254.169.254/".into());
        // Also fails the streaming requirement below, to prove the ambient
        // check is not merely one of several equally likely reasons: it is
        // reported first, unconditionally.
        let mut required = permissive_requirement();
        required.require_streaming = true;
        caps.supports_streaming = false;
        assert_eq!(
            negotiate(&caps, &required),
            Err(NegotiationRefusal::AmbientEndpointDeclared {
                declared: "http://169.254.169.254/".into()
            })
        );
    }

    #[test]
    fn a_retry_unsafe_declared_class_is_refused() {
        let mut caps = conforming_caps();
        caps.retryable_failure_classes.push(AttemptOutcomeClass::Uncertain);
        assert_eq!(
            negotiate(&caps, &permissive_requirement()),
            Err(NegotiationRefusal::UnsafeRetryableClassDeclared {
                class: AttemptOutcomeClass::Uncertain
            })
        );
    }

    #[test]
    fn missing_streaming_is_refused_only_when_required() {
        let mut caps = conforming_caps();
        caps.supports_streaming = false;
        assert_eq!(negotiate(&caps, &permissive_requirement()), Ok(()));
        let mut required = permissive_requirement();
        required.require_streaming = true;
        assert_eq!(
            negotiate(&caps, &required),
            Err(NegotiationRefusal::StreamingNotSupported)
        );
    }

    #[test]
    fn an_unsupported_structured_output_mode_is_refused_by_exact_mode() {
        let caps = conforming_caps();
        let mut required = permissive_requirement();
        required.require_structured_output_mode = Some(StructuredOutputMode::ToolCallShaped);
        assert_eq!(
            negotiate(&caps, &required),
            Err(NegotiationRefusal::StructuredOutputModeNotSupported {
                mode: StructuredOutputMode::ToolCallShaped
            })
        );
    }

    #[test]
    fn a_requested_budget_larger_than_the_adapter_max_is_refused_with_both_numbers() {
        let caps = conforming_caps();
        let mut required = permissive_requirement();
        required.max_response_bytes = caps.max_response_bytes + 1;
        assert_eq!(
            negotiate(&caps, &required),
            Err(NegotiationRefusal::ResponseBudgetExceedsAdapterMax {
                requested: caps.max_response_bytes + 1,
                adapter_max: caps.max_response_bytes,
            })
        );
    }
}
