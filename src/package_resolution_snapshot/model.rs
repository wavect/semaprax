use crate::diagnostic::Diagnostic;

use super::{limit_error, MAX_SNAPSHOT_BYTES};

pub(super) fn preflight_requirements(
    requirements: &[crate::package_resolver::Requirement],
) -> Result<(), Diagnostic> {
    validate_requirement_count(requirements.len())?;
    for requirement in requirements {
        validate_range_length(requirement.range.len())?;
    }
    Ok(())
}

pub(super) fn validate_requirement_count(count: usize) -> Result<(), Diagnostic> {
    if count == 0 || count > crate::package_resolver::MAX_REQUIREMENTS {
        return Err(super::input_error(
            "snapshot requirement count is outside bounds",
        ));
    }
    Ok(())
}

pub(super) fn validate_range_length(bytes: usize) -> Result<(), Diagnostic> {
    // Only borrowing length admission belongs here. The unchanged Resolver-v1
    // remains responsible for the complete canonical range grammar.
    if bytes > 33 {
        return Err(super::input_error(
            "snapshot requirement range exceeds its byte bound",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionSnapshot {
    pub input_json: String,
    pub resolution_evidence_json: String,
    pub lock_json: String,
}

pub(super) fn validate_cumulative(snapshot: &ResolutionSnapshot) -> Result<(), Diagnostic> {
    validate_lengths(
        snapshot.input_json.len(),
        snapshot.resolution_evidence_json.len(),
        snapshot.lock_json.len(),
    )
}

pub(super) fn validate_lengths(
    input_bytes: usize,
    evidence_bytes: usize,
    lock_bytes: usize,
) -> Result<(), Diagnostic> {
    let bytes = input_bytes
        .checked_add(evidence_bytes)
        .and_then(|value| value.checked_add(lock_bytes))
        .ok_or_else(|| limit_error("snapshot cumulative byte accounting overflowed"))?;
    if input_bytes > super::MAX_INPUT_BYTES
        || evidence_bytes > crate::package_resolver::MAX_OUTPUT_BYTES
        || lock_bytes > crate::package_lock_v2::MAX_OUTPUT_BYTES
        || bytes > MAX_SNAPSHOT_BYTES
    {
        return Err(limit_error("snapshot cumulative byte bound exceeded"));
    }
    Ok(())
}

pub(super) fn admit_subject_slot(existing: usize) -> Result<(), Diagnostic> {
    if existing >= crate::package_resolver::MAX_SUBJECTS {
        return Err(limit_error(
            "snapshot subject count exceeds Resolver-v1 bound",
        ));
    }
    Ok(())
}

pub(super) fn add_subject_bytes(total: usize, next: usize) -> Result<usize, Diagnostic> {
    if next > crate::package_resolver::MAX_SUBJECT_BYTES {
        return Err(limit_error(
            "snapshot raw subject exceeds Resolver-v1 bound",
        ));
    }
    let total = total
        .checked_add(next)
        .ok_or_else(|| limit_error("snapshot raw subject bytes overflowed"))?;
    if total > crate::package_resolver::MAX_TOTAL_SUBJECT_BYTES {
        return Err(limit_error(
            "snapshot raw subject total exceeds Resolver-v1 bound",
        ));
    }
    Ok(total)
}
