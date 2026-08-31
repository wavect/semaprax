//! Explicit persistence of historical typed drafts, never unfinished source.
use semaprax::candidate_archive_store;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidateDraft, ProjectCandidateDraftArchive,
    MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES,
};
use serde_json::json;
use std::path::Path;

pub(crate) fn persist(
    manifest: &Path,
    capsule: &Path,
    root: &Path,
) -> Result<String, Vec<Diagnostic>> {
    let bytes =
        super::project_image::read_bounded(capsule, MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES)
            .map_err(|error| vec![error])?;
    // Complete both live-source checks before the explicitly selected store
    // receives an immutable historical subject. No source lock is retained.
    let archive = with_authenticated_project(manifest, |snapshot| {
        let draft = ProjectCandidateDraft::restore(
            snapshot.retain_revision(),
            snapshot.project_revision(),
            &bytes,
        )?;
        ProjectCandidateDraftArchive::prepare(&draft, draft.draft_digest())
    })?;
    // Prepare the receipt before publication; post-pivot uncertainty remains
    // the store's uncertainty diagnostic, never an ordinary success receipt.
    let mut value = json!({
        "schema":"semaprax.candidate-draft-archive-store-receipt.v1",
        "archive_digest":archive.archive_digest(),
        "draft_digest":archive.draft_digest(),
        "base_revision":archive.base_revision(),
        "historical_source_snapshot":true,
        "current_source_admission":false,
        "source_authority":false,
        "commit_approval":false,
    });
    value.sort_all_objects();
    let output = format!("{value}\n");
    candidate_archive_store::persist_draft(root, &archive)?;
    Ok(output)
}

pub(crate) fn load(
    root: &Path,
    expected_archive: &str,
    expected_draft: &str,
) -> Result<String, Vec<Diagnostic>> {
    let draft = candidate_archive_store::load_draft(root, expected_archive, expected_draft)?;
    Ok(draft.summary(expected_draft)?.to_owned())
}
