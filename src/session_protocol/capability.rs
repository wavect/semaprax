//! Phantom-typed authority tokens, kept deliberately separate from protocol
//! *state*.
//!
//! This repository's invariant is explicit: "a settlement or concurrency
//! model is proof data, not permission to perform a physical finalizer,
//! spawn runtime work, or publish an artifact." Applied to session
//! protocols: reaching the right state, in the right order, proves only
//! that a message sequence was legal -- it never by itself authorizes the
//! effect a transition guards. [`Grant`] is the separate, independently
//! minted token a caller must present; nothing in [`super::engine`] or
//! [`super::spec`] ever manufactures one from an [`super::engine::Endpoint`]'s
//! state.
//!
//! `Grant<C>` is generic over a marker capability type, so presenting the
//! wrong grant at a typed call site is a compile-time type error -- the
//! same shape [`crate::model_call_receipt`] already ships for "a receipt is
//! not authorization":
//!
//! ```compile_fail
//! use semaprax::session_protocol::capability::*;
//!
//! fn commit(_grant: &Grant<CommitCapability>) {}
//!
//! let read_grant: Grant<ReadCapability> = Grant::issue();
//! // Holding *some* grant is not holding authority for THIS capability:
//! // a Grant minted for a different capability does not type-check here.
//! commit(&read_grant);
//! ```
//!
//! The general session-protocol engine in [`super::engine`] is data-driven
//! (a [`super::spec::ProtocolSpec`]'s `required_capability` is a plain
//! string tag, not a distinct Rust type per protocol), so its own
//! `MissingAuthority`/capability check is necessarily a runtime one; this
//! module's generic `Grant<C>` is the independent, smaller compile-time
//! proof of the same principle for the one concrete capability shape that
//! *can* be pinned to a Rust type ahead of time. Wiring the data-driven
//! engine's capability tag to a real per-protocol Rust type (so the
//! compiler itself rejects the wrong grant for an arbitrary declared
//! protocol) is exactly the parser/HIR generalization this reference module
//! leaves open -- see the crate doc `Status` section.

use std::marker::PhantomData;

/// An authority token for one specific capability `C`. Never `Clone`: a
/// grant is not meant to be duplicated and replayed the way a serialized
/// protocol handle can be (see the crate doc's "stale handle" discussion);
/// each grant is minted once, by [`Grant::issue`], for one presentation.
#[derive(Debug)]
pub struct Grant<C> {
    _capability: PhantomData<C>,
}

impl<C> Grant<C> {
    /// Mint a grant for capability `C`. This is the caller's own authority
    /// boundary: nothing here checks *why* the caller may mint one --
    /// exactly as `ModelCallReceipt` construction is "a real integration's
    /// job", wiring this to a real capability/authorization source is a
    /// separate concern this reference module does not decide.
    pub fn issue() -> Self {
        Grant {
            _capability: PhantomData,
        }
    }
}

/// Marker capability types used by [`super::protocols`]'s two applied
/// example protocols. Each is a distinct, uninhabited Rust type: a
/// `Grant<ReadCapability>` and a `Grant<CommitCapability>` are different
/// types, so no runtime check is needed to tell them apart at a typed call
/// site -- only the data-driven engine's string tags need one.
pub struct ReadCapability;
pub struct WriteCapability;
pub struct CommitCapability;
pub struct StreamOpenCapability;
