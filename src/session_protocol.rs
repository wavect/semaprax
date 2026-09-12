//! Session/protocol types (issue #206): a bounded, general model of named
//! protocol states, message/effect transitions, branching, terminal
//! states, ownership transfer, cancellation/timeout, and local
//! duality/compatibility -- generalizing the ad hoc versioned state
//! machines this repository already ships one instance of each for (Agent
//! lifecycle, transaction resources, handles, checkpoints, network
//! streams, publication workflows) into one reusable model, the way
//! [`crate::resumable_effects`] already generalized suspend/resume itself.
//!
//! # What already exists on `main` and what this module adds
//!
//! Three real, tested mechanisms already ship, at the audit baseline named
//! by issue #206, that this module's design deliberately does not
//! duplicate or edit (each is outside this module's lease):
//!
//! - [`crate::resumable_effects::core`] already proves a generic
//!   suspend/resume driver: a typed journal, `Intent`/`Observed`/
//!   `Transition` entries, replay that dispatches zero new host calls for
//!   an already-completed run, and sticky terminal-status cleanup. Its
//!   `EffectScope`/`policy_epoch` is the direct model this module's
//!   `SessionTable` generation counter follows for "a stale credential
//!   cannot be replayed as a bearer token."
//! - [`crate::live_invocation`] generalizes one closed effect boundary
//!   (`model.invoke`) with its own causal journal and budget/migration
//!   machinery.
//! - [`crate::project::candidate::multi_agent_coordination`] (#207) proves
//!   a *different* shape: a bounded, one-shot coordination-session
//!   evidence document (participants, granted scopes, proposal
//!   evaluation) that is explicitly "proof data, not a scheduler" and never
//!   itself enforces a live message-order protocol on a resource.
//!
//! None of the three checks legal message *order* for an arbitrary
//! long-running interaction as a general, reusable model with named
//! states/transitions a caller declares once and an engine then checks
//! against every time -- that is exactly what #206 asks for and what
//! [`spec::ProtocolSpec`], [`engine::SessionTable`], and [`duality`] add.
//!
//! # Status: reference validator, not source syntax
//!
//! This is a Rust-level reference kernel exercised only through the
//! fixture protocols in [`protocols`] and [`tests`] -- the same posture
//! `resumable_effects` and `live_invocation` already document for their own
//! boundaries. It does **not** add `.spx` `protocol`/session syntax (that
//! is a different, unrelated existing use of the word "protocol" --
//! [`crate::protocol_check`] projects `.spx` `protocol` *interface*
//! declarations, a body-less method-signature construct with no relation
//! to message order), a parser diagnostic, an HIR node, a verifier rule, a
//! semantic-graph/architecture/Assurance-Manifest projection, or
//! native/Wasm lowering. Wiring a declared `ProtocolSpec` to real checked
//! HIR locals and projecting it into the graph/architecture/assurance
//! outputs is exactly the parser/HIR/verifier/graph generalization this
//! reference module intentionally leaves open --
//! [`docs/SESSION-PROTOCOL-TYPES-V1.md`](../../docs/SESSION-PROTOCOL-TYPES-V1.md)
//! records the full design and this exact scope boundary.
//!
//! # A protocol state is not authority
//!
//! Reaching a state, in the right order, proves only that a message
//! sequence was legal so far -- it never by itself authorizes the effect a
//! transition guards. [`capability::Grant`] is generic over a marker
//! capability type, so presenting a grant for the wrong capability at a
//! typed call site does not type-check -- the same shape
//! [`crate::model_call_receipt`] already ships for "a receipt is not
//! authorization":
//!
//! ```compile_fail
//! use semaprax::session_protocol::capability::*;
//!
//! fn commit(_grant: &Grant<CommitCapability>) {}
//!
//! let read_grant: Grant<ReadCapability> = Grant::issue();
//! // Holding *some* grant is not holding authority for THIS capability.
//! commit(&read_grant);
//! ```
//!
//! # Affine endpoints: double use is a compile error, not a runtime check
//!
//! [`engine::SessionTable::advance`] consumes its `Endpoint` argument by
//! value. Presenting the same live binding to a second operation is
//! therefore not this module checking anything at runtime -- it is `rustc`
//! rejecting a use of a moved value:
//!
//! ```compile_fail
//! use semaprax::session_protocol::engine::*;
//! use semaprax::session_protocol::protocols::model_stream_protocol;
//!
//! struct NoopCleanup;
//! impl CleanupHandler for NoopCleanup {
//!     fn run(&mut self, _s: &str, _t: &'static str, _op: &'static str) -> Result<(), String> {
//!         Ok(())
//!     }
//! }
//!
//! let spec = model_stream_protocol();
//! let mut table = SessionTable::new(&spec);
//! let endpoint = table.open("s1").unwrap();
//! let mut cleanup = NoopCleanup;
//! let _first = table.advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup);
//! // `endpoint` was moved into the first `advance` call: presenting it a
//! // second time does not type-check.
//! let _second = table.advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup);
//! ```

pub mod capability;
pub mod duality;
pub mod engine;
pub mod protocols;
pub mod spec;
#[cfg(test)]
mod tests;

pub use duality::{check_duality, check_duality_one_way, DualityError};
pub use engine::{
    AdvanceOutcome, Checkpoint, CheckpointError, CleanupHandler, Endpoint, ProtocolError,
    ResourceToken, SessionTable, TerminalOutcome,
};
pub use spec::{Kind, Label, Next, OwnershipMove, ProtocolSpec, SpecError, StateId, Transition};
