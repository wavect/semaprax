//! Explicit historical candidate storage, distinct from canonical publication.
use semaprax::candidate_archive_store;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive,
    MAX_PROJECT_CANDIDATE_RECOVERY_BYTES,
};
use serde_json::json;
use std::path::Path;

pub(crate) fn persist(
    manifest: &Path,
    capsule: &Path,
    root: &Path,
) -> Result<String, Vec<Diagnostic>> {
    let bytes = super::project_image::read_bounded(capsule, MAX_PROJECT_CANDIDATE_RECOVERY_BYTES)
        .map_err(|error| vec![error])?;
    // Finish live-source authentication before any store effect. The resulting
    // archive is an immutable historical subject; persistence makes no claim
    // that the raw checkout remains at that revision afterward.
    let archive = with_authenticated_project(manifest, |snapshot| {
        let candidate = ProjectCandidate::restore(
            snapshot.retain_revision(),
            snapshot.project_revision(),
            &bytes,
        )?;
        ProjectCandidateArchive::prepare(&candidate, candidate.candidate_digest())
    })?;
    // Prepare stdout before publication; there is no fallible receipt encoding
    // after a successful store pivot, and store uncertainty codes propagate.
    let mut value = json!({
        "schema":"semaprax.candidate-archive-store-receipt.v1",
        "archive_digest":archive.archive_digest(),
        "candidate_digest":archive.candidate_digest(),
        "base_revision":archive.base_revision(),
        "historical_source_snapshot":true,
        "current_source_admission":false,
        "source_authority":false,
        "commit_approval":false,
    });
    value.sort_all_objects();
    let output = format!("{value}\n");
    candidate_archive_store::persist(root, &archive)?;
    Ok(output)
}

pub(crate) fn load(
    root: &Path,
    expected_archive: &str,
    expected_candidate: &str,
) -> Result<String, Vec<Diagnostic>> {
    let candidate = candidate_archive_store::load(root, expected_archive, expected_candidate)?;
    Ok(candidate.to_json().to_owned())
}
