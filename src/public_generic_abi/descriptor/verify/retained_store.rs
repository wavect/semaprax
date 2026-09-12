//! A genuine, filesystem-backed retained-program store bound to
//! [`super::verify_public_generic_descriptor`]'s own trust boundary (issue
//! #215, follow-up from #152).
//!
//! [`super`]'s own module documentation, and
//! [`docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md`](../../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md)'s
//! "Recovery and currentness" section, previously stated plainly that "this
//! layer has no retained-store access of its own" and that currentness
//! policy was "entirely the caller's own retained-store responsibility."
//! `VerificationOptions::historical_mode` was recorded on
//! [`VerifiedPublicGenericDescriptor`] for audit but changed no check.
//!
//! This module closes that specific gap with the narrowest change that
//! makes those three things real rather than documented:
//!
//! - **missing retained subject fails closed**: [`RetainedProgramStore::resolve`]
//!   returns [`RETAINED_SUBJECT_UNAVAILABLE`] for an unknown digest, checked
//!   by [`verify_public_generic_descriptor_against_store`] before any byte
//!   of `candidate_bytes` is inspected;
//! - **currentness is enforced, not merely asserted**: the store itself
//!   records which one entry is current; requesting a non-current
//!   (historical) entry without [`VerificationOptions::historical_mode`]
//!   set is refused with [`HISTORICAL_REVISION_REQUIRES_HISTORICAL_MODE`]
//!   — `historical_mode` now gates a real check, not zero checks;
//! - **process restart is testable against a store that exists**: entries
//!   are persisted as canonical source text plus a revision label under a
//!   caller-given directory, content-addressed by
//!   [`super`]'s own `program_root_digest` scheme; a fresh
//!   [`RetainedProgramStore::open`] over that same directory, in a
//!   completely new value with no shared in-process state, independently
//!   reparses and re-resolves the persisted source
//!   (`crate::parse` + `crate::hir::resolve`, deterministic) and reproduces
//!   the identical trusted programme.
//!
//! ## What this deliberately does not do
//!
//! This is *not* `project::program_root::ProgramRoot` threaded through this
//! layer, and this module does not claim to be. `ProgramRoot` is a
//! content-addressed projection over a whole `SemanticWorkspaceRevision` —
//! three `ResolvedProgram`s (entry/public-API/test) plus manifest, source
//! projection, and dependency-closure segments, hashed under its own
//! `semaprax.program-root.digest.v1` domain
//! (`src/project/program_root.rs`) — not a wrapper around the one bare
//! `&ResolvedProgram` this verifier takes. `project` already depends on
//! `public_generic_abi` (`src/project/candidate/public_generic_delta.rs`
//! imports the classifier and descriptor producer); adding the reverse
//! import here to reach `project::ProgramRoot` would create a real
//! `project` ↔ `public_generic_abi` module cycle, not a mechanical
//! one-line addition, and is out of this bounded change's scope. Nor does
//! this module change [`super::recompute_program_root_digest`]'s
//! declaration-identity-inventory algorithm into a full structural digest
//! of a versioned `ProgramRoot` — that is a distinct, separately-scoped
//! acceptance item this change does not attempt; see the module docs for
//! [`super`] and the specification's "Evidence for source drift" section
//! for the existing, unchanged evidence that gap does not let a stale
//! descriptor verify.
//!
//! This store also performs no locking, no atomic rename-on-write, and no
//! `ACTIVE`-pivot publication semantics — unlike the managed-workspace
//! patch-apply pathway (`src/workspace.rs`'s
//! `acquire_semantic_change_lock`/`acquire_semantic_change_apply_lock`) or
//! `src/project_revision_store.rs`'s host-specific durable persistence.
//! Content-addressing means an entry's *content* can never be silently
//! corrupted into a different valid entry (the digest would no longer
//! match, and [`RetainedProgramStore::resolve`] checks that), but a
//! concurrent writer racing a reader on the small `CURRENT` pointer file is
//! not defended against; this is bounded, test-oriented infrastructure for
//! this one trust boundary, not a new production persistence layer.

use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{refusal, VerificationOptions, VerifiedPublicGenericDescriptor};

/// No entry in this store carries the requested programme-root digest.
/// Returned before any byte of the candidate descriptor is parsed.
pub const RETAINED_SUBJECT_UNAVAILABLE: &str = "SPX-PG714";
/// The requested entry exists but is not this store's current head, and the
/// caller did not set [`VerificationOptions::historical_mode`]. A caller
/// that genuinely intends to verify against a deliberately selected
/// historical revision must say so explicitly via
/// [`VerificationOptions::historical`].
pub const HISTORICAL_REVISION_REQUIRES_HISTORICAL_MODE: &str = "SPX-PG715";

const ENTRIES_DIR: &str = "entries";
const CURRENT_POINTER_FILE: &str = "CURRENT";

fn io_error(subject: &str, error: impl std::fmt::Display) -> Diagnostic {
    refusal(RETAINED_SUBJECT_UNAVAILABLE, &format!("{subject}: {error}"))
}

/// One trusted subject resolved from a [`RetainedProgramStore`]: a real,
/// independently re-derived `ResolvedProgram` plus the caller-chosen
/// revision label it was published or retained under, and whether the
/// store considers it the current head.
pub struct RetainedProgramSubject {
    program: ResolvedProgram,
    source_revision: String,
    is_current: bool,
}

impl RetainedProgramSubject {
    /// The independently re-resolved trusted programme.
    pub fn program(&self) -> &ResolvedProgram {
        &self.program
    }

    /// The revision label this subject was published or retained under.
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// `true` when this store considers the subject its current head at the
    /// moment [`RetainedProgramStore::resolve`] ran.
    pub fn is_current(&self) -> bool {
        self.is_current
    }
}

/// A real, filesystem-backed store of trusted programme subjects, keyed by
/// [`super`]'s own `program_root_digest`. See the module documentation for
/// exactly what this does and does not claim to be.
pub struct RetainedProgramStore {
    root: PathBuf,
}

impl RetainedProgramStore {
    /// Open (creating if necessary) a store rooted at `root`. Two
    /// `RetainedProgramStore` values opened over the same `root` — even in
    /// separate calls with no shared in-process state, the way a real
    /// process restart would look — observe the same entries: this is the
    /// property [Recovery and
    /// currentness](../../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#recovery-and-currentness)
    /// now enforces rather than merely documents.
    pub fn open(root: &Path) -> Result<Self, Diagnostic> {
        std::fs::create_dir_all(root.join(ENTRIES_DIR))
            .map_err(|error| io_error("retained-store root is not usable", error))?;
        Ok(Self {
            root: root.to_owned(),
        })
    }

    fn entry_path(&self, program_root_digest: &str) -> PathBuf {
        self.root.join(ENTRIES_DIR).join(program_root_digest)
    }

    fn current_pointer_path(&self) -> PathBuf {
        self.root.join(CURRENT_POINTER_FILE)
    }

    /// Parse and resolve `source`, and independently recompute its
    /// `program_root_digest` using [`super`]'s own unchanged algorithm —
    /// the exact same one [`super::verify_public_generic_descriptor`]
    /// itself recomputes against whatever subject this store hands back,
    /// so a published entry's digest is always the one the verifier will
    /// independently agree with.
    fn resolve_source(&self, source: &str) -> Result<(ResolvedProgram, String), Diagnostic> {
        let parsed = crate::parse(source, Path::new("retained-program-store.spx"))
            .map_err(|error| io_error("retained source does not parse", error))?;
        let program = crate::hir::resolve(&parsed).map_err(|mut errors| {
            io_error(
                "retained source does not resolve",
                errors.remove(0).message,
            )
        })?;
        let digest = super::recompute_program_root_digest(&program);
        Ok((program, digest))
    }

    /// Publish `source`/`source_revision` as this store's new current head.
    /// Returns its `program_root_digest`. The previously current entry, if
    /// any, is not deleted and is not lost: it remains resolvable, now as a
    /// historical entry, exactly like [`retain_historical`](Self::retain_historical).
    pub fn publish_current(
        &self,
        source: &str,
        source_revision: &str,
    ) -> Result<String, Diagnostic> {
        let digest = self.write_entry(source, source_revision)?;
        std::fs::write(self.current_pointer_path(), &digest)
            .map_err(|error| io_error("cannot record the new current-head pointer", error))?;
        Ok(digest)
    }

    /// Retain `source`/`source_revision` without making it current. Only
    /// resolvable under [`VerificationOptions::historical_mode`]. Returns
    /// its `program_root_digest`.
    pub fn retain_historical(
        &self,
        source: &str,
        source_revision: &str,
    ) -> Result<String, Diagnostic> {
        self.write_entry(source, source_revision)
    }

    fn write_entry(&self, source: &str, source_revision: &str) -> Result<String, Diagnostic> {
        let (_, digest) = self.resolve_source(source)?;
        let payload = serde_json::json!({
            "source": source,
            "source_revision": source_revision,
        });
        std::fs::write(
            self.entry_path(&digest),
            serde_json::to_vec(&payload)
                .expect("a JSON object of two strings always serializes"),
        )
        .map_err(|error| io_error("cannot persist the retained entry", error))?;
        Ok(digest)
    }

    /// Resolve `program_root_digest` to its trusted subject. Fails closed
    /// with [`RETAINED_SUBJECT_UNAVAILABLE`] if this store has never
    /// published or retained an entry under that exact digest — before any
    /// byte of a candidate descriptor is ever inspected by
    /// [`verify_public_generic_descriptor_against_store`].
    pub fn resolve(&self, program_root_digest: &str) -> Result<RetainedProgramSubject, Diagnostic> {
        let bytes = std::fs::read(self.entry_path(program_root_digest)).map_err(|_| {
            refusal(
                RETAINED_SUBJECT_UNAVAILABLE,
                "no retained subject for the requested programme-root digest",
            )
        })?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
            refusal(
                RETAINED_SUBJECT_UNAVAILABLE,
                "retained entry is not valid JSON",
            )
        })?;
        let source = value["source"].as_str().ok_or_else(|| {
            refusal(RETAINED_SUBJECT_UNAVAILABLE, "retained entry has no source field")
        })?;
        let source_revision = value["source_revision"]
            .as_str()
            .ok_or_else(|| {
                refusal(
                    RETAINED_SUBJECT_UNAVAILABLE,
                    "retained entry has no source_revision field",
                )
            })?
            .to_owned();
        let (program, recomputed_digest) = self.resolve_source(source)?;
        if recomputed_digest != program_root_digest {
            // The entry's own persisted bytes no longer match the
            // content-addressed digest they are filed under: corruption,
            // never adopted as a match.
            return Err(refusal(
                RETAINED_SUBJECT_UNAVAILABLE,
                "retained entry no longer matches its own content-addressed digest",
            ));
        }
        let current = std::fs::read_to_string(self.current_pointer_path()).unwrap_or_default();
        Ok(RetainedProgramSubject {
            program,
            source_revision,
            is_current: current.trim() == program_root_digest,
        })
    }
}

/// Verify `candidate_bytes` against the trusted subject `store` resolves
/// for `expected_program_root_digest`, converting this store's currentness
/// into an enforced check rather than a caller-declared, unchecked
/// assertion: a non-current (historical) subject is refused unless
/// `options.historical_mode` is set. Every other phase is
/// [`super::verify_public_generic_descriptor`]'s own unchanged algorithm,
/// called with the subject this store resolves.
pub fn verify_public_generic_descriptor_against_store(
    store: &RetainedProgramStore,
    expected_program_root_digest: &str,
    expected_export_id: &str,
    candidate_bytes: &[u8],
    options: &VerificationOptions,
) -> Result<VerifiedPublicGenericDescriptor, Diagnostic> {
    let subject = store.resolve(expected_program_root_digest)?;
    if !subject.is_current && !options.historical_mode {
        return Err(refusal(
            HISTORICAL_REVISION_REQUIRES_HISTORICAL_MODE,
            "the requested programme-root digest names a historical, non-current retained \
             subject; the caller must set VerificationOptions::historical_mode to accept it",
        ));
    }
    super::verify_public_generic_descriptor(
        subject.program(),
        subject.source_revision(),
        expected_export_id,
        expected_program_root_digest,
        candidate_bytes,
        options,
    )
}

#[cfg(test)]
mod tests;
