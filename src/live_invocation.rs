//! Live Invocation Contract v1: the provider-independent `model.invoke`
//! effect boundary (issue #177) and its causal journal (issue #108), which
//! this module treats as **one** shared design rather than two.
//!
//! See `docs/LIVE-INVOCATION-CONTRACT-V1.md` for the full contract: how a
//! live invocation is named and why that identity is stable across retry,
//! resume and recovery ([`identity`]); the causal journal's record format
//! and ordering rules ([`journal`]); the trust model and determinism/replay
//! rules ([`kernel`]); the `model.invoke` effect's typed request,
//! capability requirement, failure taxonomy and cancellation point
//! ([`model_invoke`]); and persisting/recovering that same journal across a
//! process boundary through a caller-owned store ([`persistence`]).
//!
//! # What this module is, on purpose
//!
//! A small, self-contained reference kernel and journal, exercised end to
//! end only through the deterministic fixture implementations in
//! [`fixture`]. It does not touch the parser, HIR, or the existing
//! `agent_lifecycle`/`agent_runtime_v2` compiled pipeline — those are the
//! `AgentDefinition`/`AgentDeployment`/HIR wiring owned by the downstream
//! issues this contract exists to unblock (#109–#116, #178–#181). Binding
//! [`model_invoke::ModelHandler`], [`model_invoke::ProposalDecoder`],
//! [`model_invoke::AuthorizationGate`] and
//! [`model_invoke::InvocationBudgetHook`] to real compiled/provider
//! implementations is exactly the seam those issues implement against.
//!
//! # No ambient authority
//!
//! Nothing in this module opens a file, spawns a process, or contacts a
//! network. [`model_invoke::ModelInvokeCapability`] must be explicitly
//! constructed by a caller before [`kernel::run_live_invocation`] can be
//! called at all.

pub mod budget;
pub mod fixture;
pub mod identity;
pub mod journal;
pub mod kernel;
pub mod model_invoke;
pub mod persistence;

#[cfg(test)]
mod tests;

pub use budget::{CumulativeBudgetLedger, InvocationClock};
pub use identity::{LiveInvocationId, LiveInvocationSeed};
pub use journal::{
    receipt_projection, DecodeError, JournalEntry, JournalError, ReceiptProjection,
    ValidatedJournal,
};
pub use kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationOutcome,
    LiveKernelError, LiveKernelRun, TurnEffect, TurnObserver, TurnPolicy, TurnTransition,
};
pub use model_invoke::{
    AuthorizationContext, AuthorizationGate, AuthorizationGrant, AuthorizationRefusal,
    BudgetRefusal, InvocationBudgetHook, InvocationUsage, ModelFailure, ModelHandler,
    ModelInvocationOutcome, ModelInvocationRequest, ModelInvokeCapability, ProposalDecoder,
    ProposalOutcome, ReservedBudget,
};
pub use persistence::{
    encode_envelope, recover_journal, CheckpointJournalSink, JournalSink, RecoveredJournal,
    RecoveryError, PERSISTED_JOURNAL_SCHEMA,
};
