//! Runtime-safe model routing boundary (MR-07).
//!
//! [`engine`] is the deterministic decision core shared with the private
//! development harness: the same source files are also the workspace crate
//! `semaprax-decision-core`, so screening, rules, policy, frozen plans,
//! advisory validation, the v2 renderer and cache/replay have exactly one
//! implementation. This package carries no harness or model-runtime
//! dependency; the core uses only `serde_json` and `sha2`.
//!
//! The runtime entry is [`recommend`] (or [`recommend_static`] with no
//! decision provider): admitted candidates, features and policy in, an
//! untrusted [`Recommendation`] out. A host may attach a decision adapter by
//! implementing the MR-15 [`DecisionInvoker`] over its own approved transport;
//! the core never looks up credentials, endpoints or the environment.
//!
//! A recommendation is advisory. It names one of the host's candidate ids and
//! the frozen fallback order over them; it cannot construct a
//! `BoundAgentDeployment`, a tool grant or a provider transport, and the
//! host's existing authority (deployment binding, `DurablePolicyBinding`)
//! rechecks it at dispatch. Binding recommendations onto pre-bound deployment
//! profiles is the runtime routing integration's job, not this module's.
//! Stability and serialization versioning: `docs/DECISION-CORE-V1.md`.

pub mod engine;

pub use self::engine::boundary::{recommend, recommend_static, Recommendation};
pub use self::engine::provider::{
    ConfiguredProvider, DecisionCall, DecisionInvoker, EnablementGate, GateStatus, ProviderMode,
    ProviderProfile,
};
pub use self::engine::request::{DecisionRequest, ProjectBinding};
pub use self::engine::route::{
    Budget, Confidentiality, Destination, LatencyClass, ModelPlan, RouteRequest, TaskFamily,
    TaskFeatures,
};
pub use self::engine::router::{DecisionSource, FallbackReason, RouteContext, RouteInputs};
pub use self::engine::{Diagnostic, RoutePolicy};

pub mod runtime;
