//! Carries [Agent Interaction Schema v1](../docs/AGENT-INTERACTION-SCHEMA-V1.md)
//! rich typed values through the checked Agent lifecycle's deterministic
//! stages and typed effect operations, so a value that entered as a
//! schema-checked type stays typed at every boundary it crosses rather than
//! degrading to opaque bytes, an untyped blob, or a caller-authored
//! selector trick.
//!
//! See [`docs/AGENT-LIFECYCLE-TYPED-CARRIER-V1.md`](../docs/AGENT-LIFECYCLE-TYPED-CARRIER-V1.md)
//! for the full specification this module implements.
//!
//! This is an additive module. It reads, but does not modify,
//! `agent_interaction_schema` (the derived schema and strict decoder,
//! issue #109) and the real checked interpreter's retained-call seam
//! (`interpreter::retained_call`); the existing flat scalar `EffectScalar`/
//! `EffectArgument`/`EffectResult` typed-effect boundary in
//! `agent_lifecycle::iterative::effects` is untouched and remains fully
//! available. Wiring this carrier directly into `agent_lifecycle::stages`'s
//! compiled stage bindings is downstream integration against the traits
//! and functions this module fixes, not a change made here.
//!
//! # Why projection, not a new value representation
//!
//! The checked interpreter's real owned-call seam
//! ([`crate::interpreter::retained_call`]) already carries recursive,
//! nominally-identified `Record`/`Variant` values with real ownership and
//! cleanup semantics — a strictly richer vocabulary than the flat
//! single-level scalar projection `agent_lifecycle::iterative::effects`
//! uses today. This module projects one admitted
//! [`agent_interaction_schema::DecodedInteractionValue`] into that exact
//! `RetainedValue` vocabulary and drives it through the real retained-call
//! evaluator, so "the checked stage" a rich value crosses is the actual
//! checked interpreter, not a second home-grown execution model.
//!
//! One leaf kind is structurally excluded: `retained_call` deliberately has
//! no owned-`String` carrier (see that module's own documentation), so a
//! decoded value containing a `string` field is refused explicitly by
//! [`projection::to_retained`] rather than silently dropped, truncated, or
//! re-encoded as bytes.
//!
//! # Modules
//!
//! - `binding`: [`StageBinding`](binding::StageBinding) — the exact
//!   nominal-type/schema-revision/(optional exact case) admission a checked
//!   stage or effect operation slot requires, and its `admit` refusal.
//! - `projection`: recursive `DecodedInteractionValue` → `RetainedValue`
//!   projection.
//! - `registry`: [`TypedCarrierRegistry`](registry::TypedCarrierRegistry)
//!   — an ordered, selector-addressed rich effect operation registry, and
//!   the orchestration that validates argument/result shapes and exact
//!   deployed-operation identity before/after a handler call.
//! - `ownership`: [`OwnershipLedger`](ownership::OwnershipLedger) and the
//!   staged retained-call orchestration that settles it exactly once on
//!   every path, including every failure edge.
//! - `checkpoint`: an independent, versioned rich-value checkpoint codec,
//!   additive to and never replacing the existing flat checkpoint codecs.
//!
//! Focused gate:
//!
//! ```sh
//! cargo test --locked -p semaprax --lib agent_lifecycle_typed_carrier
//! ```

use crate::diagnostic::Diagnostic;

pub mod binding;
pub mod checkpoint;
pub mod ownership;
pub mod projection;
pub mod registry;

#[cfg(test)]
mod tests;

pub use binding::{LifecycleStageRole, StageBinding};
pub use ownership::{stage_and_evaluate, OwnedToken, OwnershipLedger};
pub use projection::{to_retained, InteractionTypeGraph};
pub use registry::{
    call_typed_operation, TypedCarrierHandler, TypedCarrierOperation, TypedCarrierRegistry,
};

/// One stable, closed refusal: a value that cannot be carried while
/// preserving its type/ownership identity is refused with this diagnostic,
/// never silently degraded. `field` names exactly which admission rule
/// failed, matching the `SPX-Z202`-family convention
/// `agent_interaction_schema` already uses for its own derivation/decode
/// admission failures.
pub(crate) fn refusal(code: &'static str, field: &str) -> Diagnostic {
    Diagnostic::io(code, format!("AgentLifecycleTypedCarrier refused: {field}"))
}
