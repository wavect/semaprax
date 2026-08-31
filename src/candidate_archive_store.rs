//! Explicit host-selected persistence for source-backed candidates and drafts.
//! A stored archive and its receipt confer no source or publication authority.

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectCandidate, ProjectCandidateArchive, ProjectCandidateDraft, ProjectCandidateDraftArchive,
    MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES,
};

#[cfg(all(
    test,
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod tests;
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod unix;

pub const MAX_CANDIDATE_ARCHIVE_STORE_ENTRIES: usize = 32;
pub const MAX_CANDIDATE_ARCHIVE_STORE_PATH_BYTES: usize = 4096;
pub const MAX_CANDIDATE_ARCHIVE_STORE_PATH_DEPTH: usize = 64;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Subject identity only: no path, handle, approval, or reusable store authority.
#[derive(Debug)]
pub struct CandidateArchiveStoreReceipt {
    archive_digest: String,
    candidate_digest: String,
    base_revision: String,
}
impl CandidateArchiveStoreReceipt {
    pub fn archive_digest(&self) -> &str {
        &self.archive_digest
    }
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }
}

/// Draft identity only. This receipt carries no completed candidate, store path,
/// source authority, approval, or reusable filesystem handle.
#[derive(Debug)]
pub struct CandidateDraftArchiveStoreReceipt {
    archive_digest: String,
    draft_digest: String,
    base_revision: String,
}
impl CandidateDraftArchiveStoreReceipt {
    pub fn archive_digest(&self) -> &str {
        &self.archive_digest
    }
    pub fn draft_digest(&self) -> &str {
        &self.draft_digest
    }
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }
}

/// Replay source, valid history and pending selectors before opening the root,
/// then publish the exact immutable draft archive. Candidate and draft archives
/// share the same bounded inventory; only the selected typed loader admits
/// content. Existing entries and failed stages are never adopted or removed.
pub fn persist_draft(
    root: &Path,
    archive: &ProjectCandidateDraftArchive,
) -> Result<CandidateDraftArchiveStoreReceipt> {
    digest_hex(archive.archive_digest())?;
    digest_hex(archive.draft_digest())?;
    if archive.to_json().len() > MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES
        || archive.to_json().len() > MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES
    {
        return Err(capacity("draft archive exceeds the fixed store byte limit"));
    }
    // Restore binds all metadata by exact archive rederivation, including the
    // original base. No last-valid candidate is exposed or materialized here.
    let replay = ProjectCandidateDraftArchive::restore(
        archive.to_json().as_bytes(),
        archive.archive_digest(),
        archive.draft_digest(),
    )?;
    drop(replay);
    let receipt = CandidateDraftArchiveStoreReceipt {
        archive_digest: archive.archive_digest().to_owned(),
        draft_digest: archive.draft_digest().to_owned(),
        base_revision: archive.base_revision().to_owned(),
    };
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::persist_draft(root, archive)?;
        Ok(receipt)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = (root, receipt);
        Err(io(
            "draft archive store requires supported Unix no-replace publication",
        ))
    }
}

/// Rebuild only the selected draft archive while its file and root remain held,
/// then authenticate exact bytes again before returning the unresolved draft.
/// No original source paths are read and completion remains separate.
pub fn load_draft(
    root: &Path,
    expected_archive: &str,
    expected_draft: &str,
) -> Result<ProjectCandidateDraft> {
    digest_hex(expected_archive)?;
    digest_hex(expected_draft)?;
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::load_draft(root, expected_archive, expected_draft)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = root;
        Err(io(
            "draft archive store requires supported Unix held-directory input",
        ))
    }
}

/// Replay a complete archive before opening the root, then publish one immutable
/// file. Existing entries and failed stages are never adopted or removed.
pub fn persist(
    root: &Path,
    archive: &ProjectCandidateArchive,
) -> Result<CandidateArchiveStoreReceipt> {
    digest_hex(archive.archive_digest())?;
    digest_hex(archive.candidate_digest())?;
    if archive.to_json().len() > MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES {
        return Err(capacity("candidate archive exceeds the fixed byte limit"));
    }
    // Typed preparation is not a filesystem authority or a reason to skip replay.
    let replay = ProjectCandidateArchive::restore(
        archive.to_json().as_bytes(),
        archive.archive_digest(),
        archive.candidate_digest(),
    )?;
    if replay.base_revision().project_revision() != archive.base_revision() {
        return Err(binding("candidate archive original-base binding disagrees"));
    }
    drop(replay);
    // Prepare receipt allocations before the filesystem pivot.
    let receipt = CandidateArchiveStoreReceipt {
        archive_digest: archive.archive_digest().to_owned(),
        candidate_digest: archive.candidate_digest().to_owned(),
        base_revision: archive.base_revision().to_owned(),
    };
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::persist(root, archive)?;
        Ok(receipt)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = (root, receipt);
        Err(io(
            "candidate archive store requires supported Unix no-replace publication",
        ))
    }
}

/// Read only the selected file's bytes, independently rebuild its stored source
/// base, replay every intention, and authenticate the held input again afterward.
pub fn load(
    root: &Path,
    expected_archive: &str,
    expected_candidate: &str,
) -> Result<ProjectCandidate> {
    digest_hex(expected_archive)?;
    digest_hex(expected_candidate)?;
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::load(root, expected_archive, expected_candidate)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = root;
        Err(io(
            "candidate archive store requires supported Unix held-directory input",
        ))
    }
}

fn digest_hex(value: &str) -> Result<&str> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| invalid("archive store identity requires sha256 digest syntax"))?;
    if !canonical_hex(hex) {
        return Err(invalid(
            "archive store identity is not canonical lowercase SHA256",
        ));
    }
    Ok(hex)
}
fn canonical_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G300", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G301", message)]
}
fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G302", message)]
}
fn io(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I360", message)]
}
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn post_pivot(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I361", message)]
}
