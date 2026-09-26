//! Resumable effects (issue #204): the non-Agent reference mechanism and the
//! bounded compiler-owned lowering for typed suspend/resume computation.
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
//! module's lease). [`core::ResumableEffectProgram`], [`core::Journal`] and
//! [`core::run`]/[`core::resume`] are a Rust-level generalization of its
//! vocabulary, parameterized over caller-chosen carrier types. They remain a
//! synchronous handler/journal reference protocol, not the runtime for source
//! `yield` and not an external-await continuation.
//!
//! # Status: bounded source lowering, not a general runtime
//!
//! The compiler admits one bounded `.spx` slice: an explicitly identified free
//! function with one to eight direct sequential top-level `yield` sites,
//! Copy-scalar state and no ordinary effects. [`lowering`] isolates its closed
//! reachable projection and derives deterministic entry/per-site-suspended/
//! complete identities plus independently validated yield-free start and
//! per-site resume HIR projections. `interpreter::resumable` consumes the
//! plan's state, invocation binding and opaque in-memory replay history. The
//! [`target`] prepares a bounded, deterministic in-memory inventory of those
//! projections as native C11 source or Core Wasm bytes without acquiring host
//! authority. [`source_checkpoint`] HMAC-authenticates the scalar continuation
//! and exact caller-supplied ProgramRoot, invocation, and policy-epoch facts
//! with a caller-owned key; decode remains inert and replay still owns
//! resumption. The crate-private,
//! `cfg(test)`-only `backend` module executes the
//! projections through real native `-O0`/`-O2` and Core Wasm target paths; its
//! temporary storage and local tool processes are test-harness authority only.
//! Ordinary native/Wasm emission still refuses a `yields` function; there is
//! no public continuation ABI, external-await scheduler, durable source
//! checkpoint, Agent migration, owned live-frame lowering or control-dependent
//! yield.
//!
//! [`continuation`] (Unix) is the public library route that drives the
//! two-to-eight-site sequential lane through a durable, authenticated,
//! append-only journal with non-bearer answers and crash recovery; see
//! [`docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md`](../../docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md).
//!
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

#[cfg(test)]
pub(crate) mod backend;
pub mod capability;
pub mod codec;
#[cfg(unix)]
pub mod continuation;
pub mod core;
pub(crate) mod lowering;
pub mod migration;
pub mod signature;
pub mod source_checkpoint;
pub mod source_driver;
pub mod source_signature;
pub mod target;
#[cfg(test)]
mod tests;

pub use capability::{
    CapabilityDenial, CapabilityGatedHandler, CapabilityPolicy, CapabilityPolicyError,
};
pub use codec::{
    decode_checkpoint, encode_checkpoint, CodecError, EffectCodec,
    RESUMABLE_EFFECTS_CHECKPOINT_SCHEMA,
};
pub use core::{
    resume, run, CleanupHandler, DriverError, EffectHandler, EffectScope, Journal, JournalEntry,
    JournalError, Outcome, ResumableEffectProgram, Step,
};
pub use lowering::{ResumableStateId, ResumableSuspensionBinding};
pub use migration::{migrate_suspended, MigratedState, MigrationError, StateMigration};
pub use signature::{
    validate_journal_signatures, EffectSignature, EffectSignatureTable, EffectTag,
    JournalSignatureError, SignatureCheckedHandler, SignatureMismatch, SignatureTableError,
};
pub use source_signature::{
    derive_source_effect_signature, SourceEffectSignature, SOURCE_TYPE_SHAPE_PREFIX,
};
