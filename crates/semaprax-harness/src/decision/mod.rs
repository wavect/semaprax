//! Decision-provider layer and policy-first model routing (HP-10).
//!
//! Hard host policy screens the catalog first; rules decide trivial cases with
//! no router call; an external `decision.evaluate/v1` provider is optional,
//! its answer is revalidated against live state, and every failure mode falls
//! back deterministically or refuses. Diagnostics: `SPX-HPJ001..` (see
//! `docs/HARNESS-DECISION-V1.md`). The frozen plan maps 1:1 onto the compiler
//! crate's `ProviderPolicy::new(Vec<ProviderSlot>)` via a later bridge.
//!
//! MR-07: the deterministic engine (screening, rules, policy, plans, advisory
//! validation, v2 rendering, cache and replay) lives once in the decision core
//! (`semaprax_decision_core`, the standalone package's `model_routing::engine`).
//! Its modules are re-exported here under their historical paths; the harness
//! keeps the CLI, evidence/qualification, governed routing, cost routing and
//! the host adapter invoker, and adapts its envelope invoker in `provider`.

pub use semaprax_decision_core::{
    cache, call, choice, choice_fixture, choice_select, model_profile, plan, policy, registry,
    render, replay, route, route_v2, rules,
};
pub mod cli;
pub mod cost_route;
pub mod evidence;
pub mod governed;
pub mod host_invoker;
pub mod provider;
pub mod qualify;
pub mod router;

pub use cache::{CacheKey, DecisionCache};
pub use call::{
    AbstentionReason, Billing, CallMetadata, IdentityKind, ResultV2, ScoreKind, Usage, UsageBasis,
};
// MR-11: the finite-choice runtime task over the same decision adapters. The
// harness `ConfiguredProvider` (over its envelope invoker) is accepted as is.
pub use choice::{
    ChoiceInputs, ChoiceOption, ChoicePolicy, ChoiceQuestion, DestinationKind, PreparedChoice,
    Rejection, SingleOption, CHOICE_NORMALIZATION, CHOICE_RENDERER, CHOICE_TASK,
    CHOICE_WIRE_VERSION,
};
pub use choice_fixture::FixtureChoiceInvoker;
pub use choice_select::{
    select_choice, ChoiceAbstain, ChoiceOutcome, ChoiceReport, ChoiceSelection, ChoiceSource,
};
pub use evidence::{
    Calibration, EvidenceKey, EvidenceRecord, EvidenceRegistry, MatchedBudget, Origin, Outcome,
    RetryOwner, NORMALIZATION_ID, NORMALIZATION_ID_V2,
};
pub use governed::{
    gate_attests_key, governed_decide, recheck_dispatch, DriftMonitor, GovernedRoute, Governor,
    ProfileStore, RoutingConfig, RoutingMode, SessionLock,
};
pub use host_invoker::HostDecisionInvoker;
pub use model_profile::{AdapterIdentity, InstanceConfig, ModelProfile};
pub use plan::{AttemptGrant, AttemptKind, AttemptLedger, FrozenRoutePlan, PlanSlot};
pub use policy::{FallbackMode, LineageBudgets, RoutePolicy};
pub use provider::{
    wire_version, ConfiguredProvider, DecisionCall, DecisionInvoker, EnablementGate, GateStatus,
    ProviderMode, ProviderProfile,
};
pub use qualify::{
    calibrate_min_confidence, evaluate, gate_for, GateBasis, GateDecision, GateSpec, RULES_ARM,
};
pub use registry::{resolve, resolve_route, DecisionTask, TASKS};
pub use render::{
    router_output_reserve, CandidateV2, Estimate, PreparedRouteV2, RenderedRequest, V2Digests,
    RENDERER_V2,
};
// `replay` (module and function) arrives with the module re-export above.
pub use replay::DecisionRecord;
pub use route::{
    screen, Budget, Confidentiality, Destination, LatencyClass, ModelPlan, RouteRequest, Screening,
    TaskFamily, TaskFeatures,
};
pub use route_v2::{
    Disclosure, EstimateBasis, ExecutionDomain, Modality, Phase, PlanDescriptor, PreviousFailure,
    QualityTier, RouteSignals, TaskFeaturesV2,
};
pub use router::{
    choice_digest, decide, DecisionSource, Digests, FallbackReason, RouteContext, RouteDecision,
    RouteInputs, WireInfo,
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
