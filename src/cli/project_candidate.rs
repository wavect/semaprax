//! Candidate preview has explicit input authority and returns no publication token.

use std::io::Read as _;
use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{self, ProjectCandidate, SemanticChange, MAX_SEMANTIC_CHANGE_BYTES};

pub(crate) fn preview(manifest: &Path, change_path: &Path) -> Result<String, Vec<Diagnostic>> {
    let bytes = read_change(change_path).map_err(|error| vec![error])?;
    let change = SemanticChange::from_json(&bytes)?;
    project::with_authenticated_project(manifest, |snapshot| {
        let candidate = ProjectCandidate::open(snapshot.retain_revision(), change.base_revision())?;
        let candidate = candidate.apply(candidate.candidate_digest(), &change)?;
        Ok(candidate.to_json().to_owned())
    })
}

fn read_change(path: &Path) -> Result<Vec<u8>, Diagnostic> {
    let file = super::project_image::open_image(path)?;
    let metadata = file
        .metadata()
        .map_err(|_| Diagnostic::io("SPX-G222", "cannot inspect semantic change input"))?;
    if !metadata.is_file() {
        return Err(Diagnostic::io(
            "SPX-G222",
            "semantic change input must be a regular file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(Diagnostic::io(
                "SPX-G222",
                "semantic change input must not be a reparse point",
            ));
        }
    }
    if metadata.len() > MAX_SEMANTIC_CHANGE_BYTES as u64 {
        return Err(Diagnostic::io(
            "SPX-G223",
            "semantic change input exceeds its limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SEMANTIC_CHANGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Diagnostic::io("SPX-G222", "cannot read semantic change input"))?;
    if bytes.len() > MAX_SEMANTIC_CHANGE_BYTES {
        return Err(Diagnostic::io(
            "SPX-G223",
            "semantic change input exceeds its limit",
        ));
    }
    Ok(bytes)
}

pub(crate) fn export(manifest: &Path, change_path: &Path) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(manifest, |snapshot| {
        let bytes = read_change(change_path).map_err(|error| vec![error])?;
        let change = SemanticChange::from_json(&bytes)?;
        let candidate = ProjectCandidate::open(snapshot.retain_revision(), change.base_revision())?;
        candidate
            .apply(candidate.candidate_digest(), &change)?
            .recovery_capsule()
    })
}

pub(crate) fn restore(manifest: &Path, capsule_path: &Path) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(manifest, |snapshot| {
        let bytes = read_capsule(capsule_path).map_err(|error| vec![error])?;
        let candidate = ProjectCandidate::restore(
            snapshot.retain_revision(),
            snapshot.project_revision(),
            &bytes,
        )?;
        Ok(candidate.to_json().to_owned())
    })
}

fn read_capsule(path: &Path) -> Result<Vec<u8>, Diagnostic> {
    let limit = project::MAX_PROJECT_CANDIDATE_RECOVERY_BYTES;
    let file = super::project_image::open_image(path)
        .map_err(|_| Diagnostic::io("SPX-G236", "cannot open recovery capsule input"))?;
    let metadata = file
        .metadata()
        .map_err(|_| Diagnostic::io("SPX-G236", "cannot inspect recovery capsule input"))?;
    if !metadata.is_file() {
        return Err(Diagnostic::io(
            "SPX-G236",
            "recovery capsule input must be a regular file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(Diagnostic::io(
                "SPX-G236",
                "recovery capsule input must not be a reparse point",
            ));
        }
    }
    if metadata.len() > limit as u64 {
        return Err(Diagnostic::io(
            "SPX-G237",
            "recovery capsule input exceeds its limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Diagnostic::io("SPX-G236", "cannot read recovery capsule input"))?;
    if bytes.len() > limit {
        return Err(Diagnostic::io(
            "SPX-G237",
            "recovery capsule input exceeds its limit",
        ));
    }
    Ok(bytes)
}
