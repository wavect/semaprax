//! Decision-provider layer and policy-first model routing (HP-10).
//!
//! Hard host policy screens the catalog first; rules decide trivial cases with
//! no router call; an external `decision.evaluate/v1` provider is optional,
//! its answer is revalidated against live state, and every failure mode falls
//! back deterministically or refuses. Diagnostics: `SPX-HPJ001..` (see
//! `docs/HARNESS-DECISION-V1.md`). The frozen plan maps 1:1 onto the compiler
//! crate's `ProviderPolicy::new(Vec<ProviderSlot>)` via a later bridge.

pub mod cache;
pub mod cli;
pub mod cost_route;
pub mod evidence;
pub mod governed;
pub mod host_invoker;
pub mod plan;
pub mod policy;
pub mod provider;
pub mod qualify;
pub mod registry;
pub mod replay;
pub mod route;
pub mod router;
pub mod rules;

pub use cache::{CacheKey, DecisionCache};
pub use evidence::{
    EvidenceKey, EvidenceRecord, EvidenceRegistry, MatchedBudget, Origin, Outcome, RetryOwner,
    NORMALIZATION_ID,
};
pub use governed::{
    gate_attests_key, governed_decide, recheck_dispatch, DriftMonitor, GovernedRoute, Governor,
    ProfileStore, RoutingConfig, RoutingMode, SessionLock,
};
pub use host_invoker::HostDecisionInvoker;
pub use plan::{AttemptGrant, AttemptKind, AttemptLedger, FrozenRoutePlan, PlanSlot};
pub use policy::{FallbackMode, LineageBudgets, RoutePolicy};
pub use provider::{
    ConfiguredProvider, DecisionCall, DecisionInvoker, EnablementGate, GateStatus, ProviderMode,
    ProviderProfile,
};
pub use qualify::{
    calibrate_min_confidence, evaluate, gate_for, GateDecision, GateSpec, RULES_ARM,
};
pub use registry::{resolve, DecisionTask, TASKS};
pub use replay::{replay, DecisionRecord};
pub use route::{
    screen, Budget, Confidentiality, Destination, LatencyClass, ModelPlan, RouteRequest, Screening,
    TaskFamily, TaskFeatures,
};
pub use router::{
    choice_digest, decide, DecisionSource, Digests, FallbackReason, RouteContext, RouteDecision,
    RouteInputs,
};
pub use rules::{rules_choice, RULES_CHECKPOINT, RULES_PROVIDER_ID};

pub fn cli_decide(args: &[String], env: &crate::cli::Environment) -> crate::cli::Outcome {
    match cli::run(args, env) {
        Ok(Some(out)) => crate::cli::Outcome::ok(out),
        Ok(None) => crate::cli::Outcome::usage(
            "decide: usage: decide <task.json> [--catalog <catalog.json>] [--json]",
        ),
        Err(d) => crate::cli::Outcome::refused(&d),
    }
}
