//! Semantic Embedding v1: an explicit-capability boundary for computing a
//! vector representation of caller-supplied bytes.
//!
//! This is a narrow, honestly-scoped slice of issue #203 ("publish a small
//! stable Semaprax embedding API with explicit host capabilities"), not the
//! whole issue. See `docs/SEMANTIC-EMBEDDING-V1.md` for exactly what this
//! module does and does not deliver relative to #203's full scope
//! (compiler/session handles, source/Project load, a C ABI, and effect
//! wiring into checked SEMAPRAX programs — none of which live here).
//!
//! # What this is
//!
//! [`EmbeddingCapability`] is a plain value a host constructs explicitly by
//! calling [`EmbeddingCapability::grant`]; nothing in compiled program
//! text, a request, or a provider's own response can synthesize one, and
//! [`kernel::embed`] cannot be called without holding one — that is
//! enforced by its signature, not by a runtime check. [`EmbeddingProvider`]
//! is the injected seam a real deployment binds to an actual embedding
//! model transport; this crate ships only the deterministic, offline
//! [`fixture::FixtureEmbeddingProvider`] so ordinary compiler CI spends no
//! model budget and reaches no network. [`kernel::embed`] is the
//! enforcement boundary between a caller's request and that provider: it
//! checks cancellation and the caller's declared input-size ceiling
//! *before* a provider is ever reached, and re-validates a settled
//! vector's length and finiteness before trusting it — a provider's
//! self-reported shape is never trusted blindly.
//!
//! # What this is not
//!
//! This module creates no compiler/session handle, loads no Source or
//! Project, offers no C ABI, and is wired into no checked SEMAPRAX effect
//! boundary, `interpreter`, `hir`, or `project` admission path — those
//! files are leased to other concurrent work on issue #203 and its
//! dependency #200 and are untouched here. It also carries no real model
//! access: [`fixture::FixtureEmbeddingProvider`]'s output is a pure,
//! bit-constructed function of the input digest and proves nothing about
//! what a real trained model would return. See "Determinism" in
//! `docs/SEMANTIC-EMBEDDING-V1.md` for exactly what byte-for-byte
//! reproducibility is and is not guaranteed.
//!
//! # No ambient authority
//!
//! Nothing in this module opens a file, spawns a process, or contacts a
//! network. [`EmbeddingCapability`] must be explicitly constructed by a
//! caller before [`kernel::embed`] can be called at all, and the only
//! implementation of [`EmbeddingProvider`] this crate ships
//! ([`fixture::FixtureEmbeddingProvider`]) never reads or writes anything
//! outside the request bytes it is handed.

pub mod capability;
pub mod fixture;
pub mod kernel;
pub mod provider;
pub mod request;

#[cfg(test)]
mod tests;

pub use capability::EmbeddingCapability;
pub use fixture::{FixtureEmbeddingProvider, ScriptedEmbeddingProvider};
pub use kernel::embed;
pub use provider::EmbeddingProvider;
pub use request::{EmbeddingFailure, EmbeddingOutcome, EmbeddingRequest};
