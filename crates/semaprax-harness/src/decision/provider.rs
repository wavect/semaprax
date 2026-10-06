//! Harness side of the decision-provider boundary (MR-07). The calibration
//! profile, enablement gate and configured-provider shape are the decision
//! core's; the harness keeps its envelope-form invoker trait (adapters and
//! fixtures receive the full `RequestEnvelope`) and adapts it to the core's
//! MR-15 [`CoreInvoker`] at this one explicit boundary.

use crate::contract::{CapabilityKind, CapabilityRef, RequestEnvelope};
use semaprax_decision_core::provider::DecisionInvoker as CoreInvoker;
pub use semaprax_decision_core::provider::{
    DecisionCall, EnablementGate, GateStatus, ProviderMode, ProviderProfile,
};
use semaprax_decision_core::request::DecisionRequest;

/// Implemented by the host over adapters (and by fixtures in tests). Same
/// contract as the core's `DecisionInvoker`, carried in the harness envelope.
pub trait DecisionInvoker {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall;

    /// `decision.evaluate` contract versions this adapter negotiated. The
    /// router sends `model-route/v2` only when 2 is listed.
    fn decision_versions(&self) -> Vec<u32> {
        vec![1]
    }
}

/// The harness envelope for one core decision request (already validated by
/// the core against the same `decision.evaluate` bounds).
pub fn envelope(request: &DecisionRequest) -> RequestEnvelope {
    RequestEnvelope {
        invocation_id: request.invocation_id.clone(),
        project: request.project.clone(),
        lock_digest: request.lock_digest.clone(),
        capability: CapabilityRef {
            kind: CapabilityKind::DecisionEvaluate,
            version: request.version,
        },
        operation: "evaluate".into(),
        deadline_ms: request.deadline_ms,
        max_result_bytes: request.max_result_bytes,
        remaining_calls: request.remaining_calls,
        lineage: request.lineage.clone(),
        payload: request.payload.clone(),
    }
}

impl<'a> CoreInvoker for dyn DecisionInvoker + 'a {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall {
        DecisionInvoker::evaluate(self, &envelope(request))
    }

    fn decision_versions(&self) -> Vec<u32> {
        DecisionInvoker::decision_versions(self)
    }
}

/// A provider attached to one harness decision (the core shape over the
/// harness invoker trait object).
pub type ConfiguredProvider<'a> =
    semaprax_decision_core::provider::ConfiguredProvider<'a, dyn DecisionInvoker + 'a>;

/// The wire version a provider is consulted with: v2 only when the adapter
/// negotiated it and the model profile (if any) names the v2 renderer.
pub fn wire_version(profile: &ProviderProfile, invoker: &dyn DecisionInvoker) -> u32 {
    semaprax_decision_core::provider::wire_version(profile, invoker)
}
