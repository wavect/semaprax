//! Direct single-file Patch admission before the shared A0 commit core.

use super::*;

pub(super) fn apply_with_commit_hook(
    source_path: &Path,
    patch_path: &Path,
    hook: impl FnMut(CommitPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let guard = acquire_a0_commit_guard(source_path)?;
    let patch_source = std::fs::read_to_string(patch_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("cannot read {}: {error}", patch_path.display()),
        )]
    })?;
    let parsed_patch = parse_patch(&patch_source)?;
    let bounded_v3 = parsed_patch.schema == PatchSchema::V3;
    let authenticated = if bounded_v3 {
        authenticate_a0_source(&guard, Some((crate::repair::MAX_SOURCE_BYTES, "SPX-R101")))?
    } else {
        authenticate_a0_source(&guard, None)?
    };
    reject_read_only_source(&authenticated.snapshot, source_path)?;
    let source = authenticated.source().to_owned();
    let preflight = preflight_parsed_owned(
        source,
        patch_source,
        source_path.to_path_buf(),
        parsed_patch,
        None,
        None,
        CandidateValidation::Standalone,
    )?;
    if preflight.base_revision() == preflight.candidate_revision() {
        return Err(patch_conflict(
            "semantic patch produces no semantic revision change",
        ));
    }
    let prepared = prepare_a0_commit(&authenticated, &preflight)?;
    commit_prepared_a0(prepared, hook)
}

fn reject_read_only_source(
    snapshot: &SourceSnapshot,
    diagnostic_path: &Path,
) -> Result<(), Vec<Diagnostic>> {
    if snapshot.permissions.readonly() {
        return Err(vec![Diagnostic::io(
            "SPX-I205",
            format!(
                "semantic patch source {} is read-only",
                diagnostic_path.display()
            ),
        )]);
    }
    Ok(())
}
