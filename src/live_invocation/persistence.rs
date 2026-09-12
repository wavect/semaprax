//! Persisting and recovering a live invocation's causal journal across a
//! process boundary.
//!
//! # What this module adds and what it reuses
//!
//! `kernel::run_live_invocation` already contains every *in-memory*
//! property issue #108/#177 asked for: a resumed journal redispatches
//! nothing already recorded, a terminal journal replays with zero
//! dispatches, and an uncertain (`RequestIntent` with no response) or
//! stuck-mid-turn journal is refused rather than guessed. None of that is
//! rebuilt here. What a process boundary adds is that the journal the
//! kernel builds in memory must actually reach durable storage — and reach
//! it at the right points, specifically *before* the kernel calls
//! [`super::model_invoke::ModelHandler::invoke`] or
//! [`super::kernel::TurnEffect::call`], never only after the fact — or a
//! crash mid-turn loses exactly the fact the in-memory contract already
//! knows how to react to correctly (an uncertain intent). [`JournalSink`]
//! is the hook `kernel::run_live_invocation` calls immediately after every
//! journal append, in the same order the entries are produced; a caller
//! that leaves [`super::kernel::LiveInvocationHandlers::sink`] as `None`
//! gets exactly today's in-memory-only behavior, unchanged.
//!
//! This module also does **not** invent a second store contract.
//! [`CheckpointJournalSink`] adapts the causal journal onto the exact same
//! caller-owned [`CheckpointStore`] trait `agent_lifecycle::durable`
//! already defines and `agent_runtime_v2::checkpoint` already uses for its
//! per-operation journal: "a `commit` either replaces the whole stored
//! generation or leaves the previous one intact." That is reuse, not a
//! competing owner of the same effect journal — this module adds a live
//! invocation's *envelope* (schema tag, bound invocation identity, a
//! generation counter and a whole-journal chain digest) around
//! [`journal::render`]'s already-existing canonical wire format, nothing
//! more.
//!
//! # The one write that cannot be undone if it fails
//!
//! [`CheckpointJournalSink::persist`] is called *before* every dispatch
//! (`ModelHandler::invoke`, `TurnEffect::call`) and *after* every
//! settlement. A store failure on the *before* call is ordinary and safe:
//! nothing external has happened yet, so the kernel fails closed
//! (`LiveKernelError::PersistenceFailed`) and no call was ever attempted —
//! provably, since `dispatched` on that error is the same count as before
//! this turn. A store failure on the *after* call is the harder case: the
//! model or effect call already happened and cannot be undone, and now its
//! outcome additionally failed to become durable. This module treats that
//! exactly like a real process crash at the same point — which is honest,
//! because from a fresh process's perspective the two are indistinguishable
//! — rather than inventing a sentinel "commit failed but I already tracked
//! it anyway" state. It does **not** attempt to give a checked SEMAPRAX
//! *program* a way to observe that distinction, because no such channel
//! exists yet: every fallible host operation in this repository aborts its
//! enclosing invocation rather than returning an inspectable value
//! (issue #228). This module is Rust-host-side persistence around a
//! Rust-host-side kernel, so `Result<(), CheckpointStoreError>` is exactly
//! as expressive as it needs to be today; wiring a *checked* Agent's own
//! persistence failure handling through this same seam, later, still needs
//! #228's value-typed host operation, not a change here.
//!
//! # Recovery does not re-validate causal ordering twice
//!
//! [`recover_journal`] checks only what the *envelope* adds: the schema
//! tag, that the document is bound to the exact invocation recovering it
//! (independent of, and in addition to, every `TurnOpened` entry's own
//! `invocation` field, which `journal::validate` already checks), and that
//! [`journal::chain`] recomputed from the decoded entries matches the
//! chain link the document claims — catching accidental storage corruption
//! a per-field digest *format* check would not (see `journal.rs`'s own
//! documented nonclaim: entry digests are never recomputed from content on
//! decode, because without a signature a rewritten document could always
//! recompute them too; the chain link exists precisely so a shorter,
//! reordered or substituted document is independently detectable without
//! needing a signature to do it). Causal ordering (turn sequencing, phase
//! ordering, cross-invocation `TurnOpened` entries, post-terminal entries)
//! stays exactly where it already lives: `journal::validate`, called once,
//! from `kernel::run_live_invocation`. Two independent validators of the
//! same journal is exactly the "two competing owners" this issue's scope
//! warns against; this module hands `kernel::run_live_invocation` the
//! decoded entries and lets it be the sole causal authority.

use serde_json::Value;

use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::diagnostic::quote_json;

use super::identity::LiveInvocationId;
use super::journal::{self, JournalEntry};

#[cfg(test)]
mod tests;

/// Schema tag of the persisted-journal envelope this module writes and
/// reads. Distinct from `journal::render`'s bare array (which carries no
/// schema of its own) and from `agent_runtime_v2::checkpoint`'s
/// `CHECKPOINT_SCHEMA` (a different journal, a different owner).
pub const PERSISTED_JOURNAL_SCHEMA: &str = "semaprax.live-invocation.persisted-journal.v1";

/// The write-side seam [`super::kernel::run_live_invocation`] calls
/// immediately after every journal append, in the same order the entries
/// are produced — before the next dispatch, and once more at every
/// terminal or turn-boundary return. A `None` sink (the default for every
/// existing caller) makes every call a no-op; nothing about in-memory
/// behavior changes when persistence is not wired in.
pub trait JournalSink {
    /// Makes `journal` (the *whole* journal so far, not a delta) durable.
    /// An implementation is expected to satisfy the same "replace the whole
    /// generation, or leave the previous one intact" contract
    /// [`CheckpointStore::commit`] documents — this trait does not itself
    /// add incremental/delta semantics on top of that.
    fn persist(&mut self, journal: &[JournalEntry]) -> Result<(), CheckpointStoreError>;
}

/// Adapts a caller-owned [`CheckpointStore`] into a [`JournalSink`] by
/// re-rendering the whole journal and committing it as one generation each
/// time. This is the only [`JournalSink`] this module ships; a caller
/// wanting different storage implements the trait directly instead of
/// wrapping a `CheckpointStore` that was never meant to model it.
pub struct CheckpointJournalSink<'a> {
    store: &'a mut dyn CheckpointStore,
    invocation: String,
    generation: u64,
}

impl<'a> CheckpointJournalSink<'a> {
    /// Starts a fresh sink at generation `0`.
    #[must_use]
    pub fn new(store: &'a mut dyn CheckpointStore, invocation: impl Into<String>) -> Self {
        Self {
            store,
            invocation: invocation.into(),
            generation: 0,
        }
    }

    /// Continues an existing sink from `generation` (typically the
    /// generation [`recover_journal`] reported for a document this process
    /// is resuming). The next successful `persist` commits `generation + 1`.
    #[must_use]
    pub fn resume(
        store: &'a mut dyn CheckpointStore,
        invocation: impl Into<String>,
        generation: u64,
    ) -> Self {
        Self {
            store,
            invocation: invocation.into(),
            generation,
        }
    }

    /// The last generation this sink successfully committed (or the
    /// resumed starting generation, if `persist` has not yet been called).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl JournalSink for CheckpointJournalSink<'_> {
    fn persist(&mut self, journal: &[JournalEntry]) -> Result<(), CheckpointStoreError> {
        let next = self.generation.saturating_add(1);
        let document = encode_envelope(&self.invocation, next, journal);
        self.store.commit(next, &document)?;
        self.generation = next;
        Ok(())
    }
}

/// Renders the persisted-journal envelope: the schema tag, the exact bound
/// invocation identity digest, `generation`, [`journal::chain`]'s
/// whole-journal link, and `journal::render`'s canonical entry array. Pure
/// and independent of any store — a test can call this directly to inspect
/// or corrupt a document without going through [`CheckpointJournalSink`].
#[must_use]
pub fn encode_envelope(invocation: &str, generation: u64, journal: &[JournalEntry]) -> String {
    let rendered = journal::render(journal);
    let entries = rendered.trim_end();
    format!(
        "{{\"schema\":{},\"invocation\":{},\"generation\":{},\"chain\":{},\"entries\":{}}}\n",
        quote_json(PERSISTED_JOURNAL_SCHEMA),
        quote_json(invocation),
        generation,
        quote_json(&journal::chain(journal)),
        entries,
    )
}

/// A persisted-journal envelope rejection. Every variant names one thing
/// this module independently checked before handing decoded entries to
/// [`super::kernel::run_live_invocation`]; none of these overlaps with a
/// [`journal::JournalError`] causal-ordering rejection, which stays the
/// kernel's alone to raise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    /// Not the closed five-key envelope object this module writes.
    Malformed,
    /// The `schema` field is not [`PERSISTED_JOURNAL_SCHEMA`].
    SchemaMismatch,
    /// The `invocation` field does not name the identity recovery was
    /// asked to bind to.
    InvocationMismatch,
    /// `journal::decode`'s shape check on the `entries` array failed.
    Journal(journal::DecodeError),
    /// The recomputed [`journal::chain`] over the decoded entries does not
    /// match the document's own `chain` field: the document was corrupted,
    /// truncated, reordered or substituted after it was written.
    ChainMismatch,
}

/// One recovered document: the decoded entries (not yet causally
/// validated — that is [`super::kernel::run_live_invocation`]'s job, the
/// moment it is handed `entries` as a starting journal) and the generation
/// they were stored at, so a caller can resume a [`CheckpointJournalSink`]
/// from exactly where the last successful write left off rather than
/// restarting the generation counter and risking a store that treats a
/// lower generation as stale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredJournal {
    pub entries: Vec<JournalEntry>,
    pub generation: u64,
}

/// Recovers a persisted document, bound to `identity`. Rejects a document
/// aimed at a different invocation, a different schema, or one whose
/// recomputed chain link does not match what it claims — independent of,
/// and before, the causal validation `run_live_invocation` performs on the
/// returned entries.
pub fn recover_journal(
    document: &str,
    identity: &LiveInvocationId,
) -> Result<RecoveredJournal, RecoveryError> {
    let value: Value = serde_json::from_str(document).map_err(|_| RecoveryError::Malformed)?;
    let object = value.as_object().ok_or(RecoveryError::Malformed)?;
    const KEYS: [&str; 5] = ["schema", "invocation", "generation", "chain", "entries"];
    if object.len() != KEYS.len() || !KEYS.iter().all(|key| object.contains_key(*key)) {
        return Err(RecoveryError::Malformed);
    }
    if object["schema"].as_str() != Some(PERSISTED_JOURNAL_SCHEMA) {
        return Err(RecoveryError::SchemaMismatch);
    }
    if object["invocation"].as_str() != Some(identity.digest()) {
        return Err(RecoveryError::InvocationMismatch);
    }
    let generation = object["generation"]
        .as_u64()
        .ok_or(RecoveryError::Malformed)?;
    let claimed_chain = object["chain"].as_str().ok_or(RecoveryError::Malformed)?;
    let entries = journal::decode(&value["entries"]).map_err(RecoveryError::Journal)?;
    let recomputed_chain = journal::chain(&entries);
    if claimed_chain != recomputed_chain {
        return Err(RecoveryError::ChainMismatch);
    }
    Ok(RecoveredJournal {
        entries,
        generation,
    })
}
