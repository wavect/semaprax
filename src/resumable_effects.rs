//! Resumable effects (issue #204): a general, non-Agent reference mechanism
//! for typed suspend/resume computation, generalizing the vocabulary the
//! closed six-role Agent shape already ships one instance of.
//!
//! # What already exists and what this module adds
//!
//! `agent_lifecycle::iterative` already lowers one closed Agent shape's
//! `initialize`/`observe`/`propose`/`authorize`/`execute`/`reduce` operations
//! into a `Continue`/`Complete`/`Suspend`/`Fail` state machine, and
//! `agent_runtime_v2::checkpoint` already durably journals each such turn as
//! `Intent`/`Observed`/`Transition` entries with replay that dispatches zero
//! host calls for an already-completed run. That machinery is real,
//! HOSTED GREEN, and stays untouched by this module (it is outside this
//! module's lease). What it does **not** yet do — and what issue #204 asks
//! for — is let an ordinary, non-Agent function declare typed resumable
//! effects with the same shape. [`core::ResumableEffectProgram`],
//! [`core::Journal`] and [`core::run`]/[`core::resume`] are that
//! generalization: the same `Step`/journal-entry vocabulary, parameterized
//! over a caller-chosen `State`/`Request`/`Observation`/`Result`/`CleanupOp`
//! instead of the six fixed Agent roles.
//!
//! # Status: reference validator, not source syntax
//!
//! This is a Rust-level reference kernel exercised only through the fixture
//! programs in [`tests`] — the same posture `live_invocation` already
//! documents for the `model.invoke` boundary ("a small, self-contained
//! reference kernel... it does not touch the parser, HIR, or the existing
//! compiled pipeline"). It does **not** add `.spx` `yield`/effect syntax, a
//! parser diagnostic, an HIR node, a verifier rule, a semantic-graph
//! projection, or native/Wasm lowering, and it does not migrate the
//! existing Agent lifecycle onto itself. Those are exactly the parser,
//! HIR, verifier, semantic-graph, native-backend and Wasm-backend changes
//! the repository's change protocol requires to move together once syntax
//! carries runtime meaning — a change too large for one bounded slice.
//! [`docs/RESUMABLE-EFFECTS-V1.md`](../../docs/RESUMABLE-EFFECTS-V1.md)
//! records the full design and exactly this scope boundary.
//!
//! "Typed" here means Rust's own type system: `ResumableEffectProgram`
//! fixes one `Request`/`Observation` pair per program, so presenting the
//! wrong observation type for a resume is rejected by `rustc`, not at
//! runtime:
//!
//! ```compile_fail
//! use semaprax::resumable_effects::core::*;
//!
//! #[derive(Clone, Debug, Eq, PartialEq)]
//! struct Program;
//! impl ResumableEffectProgram for Program {
//!     type State = i64;
//!     type Result = i64;
//!     type Request = i64;
//!     type Observation = i64; // this program's effect channel is i64
//!     type CleanupOp = ();
//!     fn request(&self, _s: &i64) -> Option<i64> { None }
//!     fn transition(&self, s: &i64, _o: Option<&i64>) -> Step<i64, i64> {
//!         Step::Complete(*s)
//!     }
//!     fn cleanup_plan(&self, _s: &i64) -> Vec<()> { Vec::new() }
//! }
//!
//! // A resumed observation of the wrong type does not type-check: this
//! // program's Observation is i64, never a String.
//! let _bad: JournalEntry<Program> = JournalEntry::Observed {
//!     turn: 0,
//!     scope: EffectScope { program_root: String::new(), invocation_id: String::new(), policy_epoch: 0 },
//!     request: 0,
//!     observation: "wrong type".to_string(),
//! };
//! ```
//!
//! Ownership is enforced the same way: every carrier type must be
//! `'static`, so a borrowed local cannot be named as a program's `State`:
//!
//! ```compile_fail
//! use semaprax::resumable_effects::core::*;
//!
//! struct BorrowingProgram;
//! impl<'a> ResumableEffectProgram for BorrowingProgram {
//!     type State = &'a str; // not 'static: rejected at compile time
//!     type Result = ();
//!     type Request = ();
//!     type Observation = ();
//!     type CleanupOp = ();
//!     fn request(&self, _s: &Self::State) -> Option<()> { None }
//!     fn transition(&self, _s: &Self::State, _o: Option<&()>) -> Step<Self::State, ()> {
//!         Step::Complete(())
//!     }
//!     fn cleanup_plan(&self, _s: &Self::State) -> Vec<()> { Vec::new() }
//! }
//! ```

pub mod core;
#[cfg(test)]
mod tests;

pub use core::{
    resume, run, CleanupHandler, DriverError, EffectHandler, EffectScope, Journal, JournalEntry,
    JournalError, Outcome, ResumableEffectProgram, Step,
};
