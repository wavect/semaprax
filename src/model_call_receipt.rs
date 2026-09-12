//! Model-call receipts: canonical, replayable, redactable evidence for one
//! `model.invoke` attempt (issue #180).
//!
//! # Relationship to `src/live_invocation/`
//!
//! `src/live_invocation/` (issues #108/#177, read-only from this module's
//! perspective — leased elsewhere) already owns the *authoritative* record
//! of a `model.invoke` attempt: [`crate::live_invocation::journal`]'s causal
//! journal, plus [`crate::live_invocation::journal::receipt_projection`],
//! which that module's own docs already name as "the entire mechanism a
//! receipt (owned downstream by #180) uses" — a pure, coarse fold
//! (invocation id, turn/call/failure counts, terminal case) over an
//! already-[`crate::live_invocation::journal::validate`]d journal.
//!
//! This module does not duplicate that journal or invent a second causal
//! log. What it adds, as its own schema and its own module, is the
//! *per-call* receipt shape #180 asks for that the coarse aggregate
//! projection does not carry: Agent/ProgramRoot/DeploymentRoot/InstanceRoot
//! binding, attempt ordinal, model/provider/adapter identity, grammar and
//! policy digests, timing, local vs. provider-reported usage and cost, a
//! redacted audit view with verifiable per-field commitments, independent
//! replay (never redispatching), and untrusted provider invoice
//! reconciliation with a closed discrepancy vocabulary. A real integration
//! constructs one [`receipt::ModelCallReceipt`] per turn from the same
//! already-validated causal journal entries `receipt_projection` folds over
//! (see [`receipt::ModelCallReceipt`]'s docs) plus the root/attempt
//! metadata the deployment already carries; this module never re-implements
//! `journal::validate`'s ordering rules, and never accepts a raw
//! [`crate::live_invocation::model_invoke::ModelHandler`] anywhere, so it
//! structurally cannot redispatch a call.
//!
//! # A receipt is evidence, not authority
//!
//! Nothing here mints, decodes, or reconstructs a
//! [`crate::live_invocation::model_invoke::AuthorizationGrant`]: this module
//! does not import [`crate::live_invocation::model_invoke::AuthorizationGate`]
//! at all. A [`receipt::ModelCallReceipt`] cannot authorize a call, widen a
//! budget, select a provider, or approve itself — see
//! [`receipt::ModelCallReceipt`]'s own doc test for a caller who tries to use
//! one as a grant, which is rejected at compile time, not by a runtime
//! check.
//!
//! # No live network call, no real provider, no key
//!
//! Every test in this module is built from offline fixture bytes and the
//! same deterministic fixture seams `crate::live_invocation::fixture`
//! already ships (`FixtureModelHandler`, `FixtureProposalDecoder`). Wiring
//! a real provider adapter, a real compiled proposal grammar, or a real
//! invoice-import transport is downstream, human-gated integration work
//! against the traits this module and `live_invocation` already fix.

pub mod audit_view;
pub mod reconciliation;
pub mod receipt;
pub mod replay;

pub use audit_view::{
    redact, verify_audit_view, AuditViewError, ModelCallAuditView, RedactedField,
    RedactionPolicy, ReceiptPrivateExtras, AUDIT_VIEW_SCHEMA,
};
pub use reconciliation::{BillingReconciler, ProviderInvoiceRow, ReconciliationOutcome};
pub use receipt::{
    commit_observation_bytes, commit_proposal_bytes, commit_response_bytes, commit_task_bytes,
    verify_root_binding, BindingError, ModelCallReceipt, PayloadPrivacyClaim,
    ProviderReportedUsage, ReceiptRootBinding, ReceiptStage, RootBindingContext,
    LOW_ENTROPY_BYTE_THRESHOLD, RECEIPT_SCHEMA,
};
pub use replay::{replay_receipt, ReplayError};
