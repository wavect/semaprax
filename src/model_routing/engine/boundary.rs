//! Runtime-safe routing boundary (MR-07).
//!
//! A runtime host calls [`recommend`] with candidates it has already admitted
//! (its pre-bound deployment selections, projected as [`ModelPlan`]s), the
//! task features, its policy and, optionally, an attached decision provider.
//! The answer is a [`Recommendation`]: an untrusted advisory that names one of
//! the host's own candidate ids and the frozen fallback order over them.
//!
//! The boundary deliberately has no type for a deployment, a tool grant, a
//! credential or a provider transport, so nothing a provider says can be
//! turned into one. A provider's choice is accepted only when it is one of
//! the screened candidate ids (v2 selection ids are mapped back by the core);
//! anything else is a deterministic rules fallback or a refusal. The host
//! resolves the recommendation against its own admitted objects with
//! [`Recommendation::select`] and rechecks authority (for example a durable
//! policy binding) at dispatch: a recommendation never authorizes anything.
//!
//! With no provider attached the decision is the rules path (or the trivial
//! single-candidate path) with zero router calls: no invoker exists to call.

use super::cache::DecisionCache;
use super::diag::DecisionResult;
use super::plan::FrozenRoutePlan;
use super::provider::{ConfiguredProvider, DecisionInvoker};
use super::router::{decide, DecisionSource, RouteContext, RouteDecision, RouteInputs};

/// An advisory routing result. It has no public constructor: the only way to
/// obtain one is a decision over host-admitted inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct Recommendation {
    decision: RouteDecision,
}

impl Recommendation {
    /// The full decision record (digests, wire facts, frozen plan).
    pub fn decision(&self) -> &RouteDecision {
        &self.decision
    }

    /// The recommended candidate id: always one of the host's admitted ids.
    pub fn choice(&self) -> &str {
        &self.decision.choice
    }

    pub fn source(&self) -> DecisionSource {
        self.decision.source
    }

    /// Router calls made for this recommendation (0 on the rules path).
    pub fn router_calls(&self) -> u32 {
        self.decision.router_calls
    }

    /// Frozen fallback order over admitted candidate ids only.
    pub fn plan(&self) -> &FrozenRoutePlan {
        &self.decision.plan
    }

    /// Map the recommendation back onto the host's own admitted objects by id.
    /// Returns a reference to the host's object, never a new one; `None` when
    /// the host no longer holds that candidate (the host must then refuse or
    /// re-route, not trust the recommendation).
    pub fn select<'h, T>(&self, admitted: &'h [T], id_of: impl Fn(&T) -> &str) -> Option<&'h T> {
        admitted.iter().find(|t| id_of(t) == self.choice())
    }

    /// The host's own objects in frozen-plan order (chosen first); ids the host
    /// does not hold are skipped, never synthesized.
    pub fn ordered<'h, T>(&self, admitted: &'h [T], id_of: impl Fn(&T) -> &str) -> Vec<&'h T> {
        self.plan()
            .ordered
            .iter()
            .filter_map(|slot| admitted.iter().find(|t| id_of(t) == slot.model_id))
            .collect()
    }

    pub fn into_decision(self) -> RouteDecision {
        self.decision
    }
}

/// Route over host-admitted inputs. `inputs` is the host's frozen snapshot and
/// is also the post-inference revalidation state; a host whose state can
/// change during inference uses [`decide`] with its own `live` closure.
pub fn recommend<I: ?Sized + DecisionInvoker>(
    inputs: &RouteInputs,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_, I>>,
    cache: Option<&mut DecisionCache>,
) -> DecisionResult<Recommendation> {
    decide(inputs, ctx, provider, &|| inputs.clone(), cache)
        .map(|decision| Recommendation { decision })
}

/// [`recommend`] with no decision provider attached: static/rules behavior,
/// zero router calls, no invoker and therefore no network.
pub fn recommend_static(
    inputs: &RouteInputs,
    ctx: &RouteContext,
) -> DecisionResult<Recommendation> {
    recommend::<dyn DecisionInvoker>(inputs, ctx, None, None)
}

#[cfg(test)]
#[path = "boundary_tests.rs"]
mod tests;
