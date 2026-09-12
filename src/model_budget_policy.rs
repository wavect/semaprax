//! Model Budget Policy v1 (issue #179): the provider-neutral pre-dispatch
//! admission gate for model-call count, retry count, failover/provider
//! count, per-attempt and cumulative token ceilings, and cumulative
//! estimated cost — the #179 budget dimensions
//! [`crate::live_invocation`]'s #113 work (`live_invocation::budget::
//! CumulativeBudgetLedger`) does not cover.
//!
//! # Relationship to #113 and to `live_invocation`
//!
//! `live_invocation::model_invoke` documents its
//! [`crate::live_invocation::model_invoke::InvocationBudgetHook`] seam as
//! the one place "issues #113/#179 attach cumulative budget policy behind."
//! #113 (`live_invocation::budget::CumulativeBudgetLedger`) already, and
//! completely, delivers that seam's monetary-ceiling and absolute-deadline
//! dimensions, nonrefundably, with crash-safe resume, and five cancellation
//! checkpoints across the kernel's model/authorization/effect dispatch
//! points. None of that is duplicated here — this crate's
//! `src/live_invocation/**` is read-only for this module by design; the
//! monetary/deadline ledger stays there.
//!
//! What #113 does not add, and what this module is scoped to, is the rest
//! of #179's named dimensions: a maximum call count across every attempt of
//! an invocation, a maximum retry count *with a proven-safe retry
//! classification gating it*, a maximum failover/provider-switch count
//! *bound to the deployment's exact ordered, confidentiality-checked
//! provider list*, and per-attempt/cumulative context and output token
//! ceilings distinct from the opaque monetary unit #113's ceiling uses.
//! [`ledger::ModelPolicyLedger`] is the ledger that enforces all of these,
//! and [`limits::intersect`] is the "effective limit is the minimum of
//! three independently declared sources" computation #179 asks for by
//! name. A deployment wiring both #113's ledger and this one reserves
//! against each independently for the same attempt: this ledger decides
//! *whether an attempt of this shape is admissible at all*; #113's ledger
//! (or an equivalent [`crate::live_invocation::model_invoke::
//! InvocationBudgetHook`]) still separately charges the monetary/deadline
//! cost of the one attempt this ledger admits.
//!
//! # No live network, no real provider, no key
//!
//! Every type here is pure, offline data. [`ledger::ModelPolicyLedger`]
//! contacts no provider and holds no credential; a caller supplies its own
//! [`crate::live_invocation::budget::InvocationClock`] (this module reuses
//! that existing public trait rather than inventing a second clock seam)
//! and, for tests, the same deterministic
//! [`crate::live_invocation::fixture::StepClock`] `live_invocation`'s own
//! tests use.
//!
//! # Model data carries no authority
//!
//! Nothing in this module accepts raw provider response bytes as input.
//! [`classification::AttemptOutcomeClass`] is only ever constructed by
//! trusted adapter/host code that already knows, out of band, whether and
//! how a call settled; [`ledger::ModelPolicyLedger::record_outcome`] is
//! evidence-only and cannot reduce any committed ceiling regardless of what
//! the recorded outcome claims (`ledger::tests::
//! a_self_reported_zero_cost_outcome_never_reopens_spent_capacity` proves
//! this directly). Failover provider selection is likewise never taken
//! from response content: [`provider_policy::ProviderPolicy::
//! admit_failover`] only ever admits the exact next id in the deployment's
//! own declared order.
//!
//! See `docs/MODEL-BUDGET-POLICY-V1.md` for the full contract this module
//! implements.

pub mod classification;
pub mod ledger;
pub mod limits;
pub mod provider_policy;

pub use classification::{retry_is_permitted, AttemptOutcomeClass};
pub use ledger::{
    AttemptKind, AttemptRefusal, AttemptRequest, AttemptReservation, AttemptUsage,
    ModelPolicyLedger,
};
pub use limits::{intersect, EffectiveModelBudget, ModelBudgetLimits, PolicyRejection};
pub use provider_policy::{ProviderPolicy, ProviderRefusal, ProviderSlot};
