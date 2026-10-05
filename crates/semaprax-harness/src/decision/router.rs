//! Harness entry to the decision core's router (MR-07): the policy-first
//! decision itself is `semaprax_decision_core::router::decide`; this module
//! re-exports its types and fixes the invoker to the harness trait object.

use super::cache::DecisionCache;
use super::provider::ConfiguredProvider;
use crate::diag::HarnessResult;
pub use semaprax_decision_core::router::{
    choice_digest, DecisionSource, Digests, FallbackReason, RouteContext, RouteDecision,
    RouteInputs, WireInfo,
};

/// Decide a route. `live` returns the current inputs and is evaluated after
/// any router inference so a stale decision is rejected, not trusted.
pub fn decide(
    inputs: &RouteInputs,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_>>,
    live: &dyn Fn() -> RouteInputs,
    cache: Option<&mut DecisionCache>,
) -> HarnessResult<RouteDecision> {
    semaprax_decision_core::router::decide(inputs, ctx, provider, live, cache)
}
