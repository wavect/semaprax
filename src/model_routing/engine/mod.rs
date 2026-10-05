//! SEMAPRAX decision core (MR-07): the one deterministic implementation of
//! policy-first model routing, shared by the private development harness and
//! the public runtime.
//!
//! This directory is compiled twice from the same files: as the module
//! `semaprax::model_routing::engine` of the standalone compiler package, and as
//! the crate root of the workspace crate `semaprax-decision-core`
//! (`crates/semaprax-decision-core`, `[lib] path` points here), which the
//! private harness and toolchain use. Nothing here may name `crate::`; modules
//! refer to each other through `super::` so both mountings resolve alike.
//!
//! Contents: route request/feature types (v1 and v2), candidate descriptors,
//! the hard host screen, rules selection, the routing policy, frozen route
//! plans, advisory-result validation (v1/v2 results and call metadata), the v2
//! renderer and digests, choice digests, the decision cache and replay record,
//! and [`router::decide`] over the host-supplied MR-15
//! [`provider::DecisionInvoker`]. [`boundary`] is the runtime-safe entry.
//!
//! Dependencies are `serde_json` and `sha2` only. The core performs no
//! filesystem, process, network, environment or clock access: elapsed time
//! and every answer arrive through the invoker as untrusted data, and a
//! recommendation carries selection ids only, never an authorization.
//! Stability and serialization versioning: `docs/DECISION-CORE-V1.md`.

pub mod boundary;
pub mod cache;
pub mod call;
mod consult;
pub mod diag;
pub mod json;
pub mod model_profile;
pub mod plan;
pub mod policy;
pub mod provider;
pub mod registry;
pub mod render;
pub mod replay;
pub mod request;
pub mod route;
pub mod route_v2;
pub mod router;
pub mod rules;
pub mod shape;
pub mod text;
pub mod wire;

pub use boundary::{recommend, recommend_static, Recommendation};
pub use cache::{CacheKey, DecisionCache};
pub use call::{
    AbstentionReason, Billing, CallMetadata, IdentityKind, ResultV2, ScoreKind, Usage, UsageBasis,
};
pub use diag::{DecisionResult, Diagnostic};
pub use model_profile::{AdapterIdentity, InstanceConfig, ModelProfile};
pub use plan::{AttemptGrant, AttemptKind, AttemptLedger, FrozenRoutePlan, PlanSlot};
pub use policy::{FallbackMode, LineageBudgets, RoutePolicy};
pub use provider::{
    wire_version, ConfiguredProvider, DecisionCall, DecisionInvoker, EnablementGate, GateStatus,
    ProviderMode, ProviderProfile,
};
pub use registry::{resolve, DecisionTask, TASKS};
pub use render::{
    router_output_reserve, CandidateV2, Estimate, PreparedRouteV2, RenderedRequest, V2Digests,
    RENDERER_V2,
};
pub use replay::{replay, DecisionRecord};
pub use request::{DecisionRequest, ProjectBinding};
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
