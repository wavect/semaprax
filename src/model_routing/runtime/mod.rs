//! Runtime model routing over host-approved deployment profiles (MR-09) and
//! safe lifecycle re-route boundaries (MR-10).
//!
//! A host approves a finite set of concrete deployment profiles for one
//! semantic agent definition ([`ApprovedProfileSet`]). Every profile is a
//! `bind_agent_deployment` of the same definition source with its own
//! deployment document, pre-admitted with the exact provider order and limits
//! `DurablePolicyBinding` will recheck. At a new invocation boundary the host
//! projects bounded [`RuntimeFeatures`], the profile catalog is screened and
//! the shared decision core (or its zero-call rules path) names one profile
//! id. Nothing a router says can name an endpoint, credential, provider list
//! or deployment: the router sees profile ids and logical metadata only.
//!
//! The choice is frozen as a versioned [`RouteRecord`]; execution, instance
//! and invocation roots are derived from the selected deployment before any
//! adapter exists ([`bind_routed_invocation`]), and the record is attached to
//! the durable policy journal through a versioned checkpoint envelope, so
//! [`resume_routed_invocation`] reuses the retained choice with zero router
//! calls even after catalog or alias drift.
//!
//! [`RoutedSession`] (MR-10) runs a multi-turn agent as a sequence of such
//! invocations. A route may change only at a [`RerouteBoundary`]: after the
//! previous turn settled durably, the host accepted it and the session
//! journaled its `continue` transition. Each new turn is a new, properly bound
//! invocation over a bounded typed [`Handoff`] from committed state; the old
//! binding and frozen plan are never reordered. Child-agent work reserves
//! from the same parent allowance before dispatch, under a bounded delegation
//! depth with caller/callee identity.
//!
//! Transport failover stays inside one invocation (the deployment's ordered
//! provider policy); reasoning escalation is a configured bounded rule at a
//! re-route boundary; delegation is an explicit reserved child grant. An
//! uncertain dispatch or external effect halts the session for the existing
//! reconciliation path and is never replayed with another model.
//!
//! Contract: `docs/RUNTIME-MODEL-ROUTING-V1.md`.

mod envelope;
mod error;
mod features;
mod invoke;
mod profiles;
mod record;
mod select;
mod session;

pub use envelope::ENVELOPE_SCHEMA;
pub use error::RuntimeRoutingError;
pub use features::{RuntimeFeatures, MAX_REQUIRED_MODALITIES};
pub use invoke::{
    bind_routed_invocation, resume_routed_invocation, run_routed_invocation, start_routed_task,
    BoundRoutedInvocation, InvocationTarget, RoutedRun, RoutedRunHandlers,
};
pub use profiles::{ApprovedProfile, ApprovedProfileSet, ProfileModel, ProfileSpec};
pub use record::{RouteRecord, RouteSource, ROUTE_RECORD_SCHEMA};
pub use select::{route_new_invocation, RoutedProfile};
pub use session::{
    ChildGrant, DelegationRequest, EscalationRule, Handoff, LastOutcome, ProgressCounters,
    RerouteBoundary, RouteReason, RoutedSession, SessionPolicy, SpecialistGrant, TurnFeatures,
    TurnOutcome, TurnRequest, TurnStatus, TurnVerdict, HANDOFF_SCHEMA, SESSION_SCHEMA,
};
