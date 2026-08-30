use crate::diagnostic::Diagnostic;

use super::{limit_error, MAX_SNAPSHOT_BYTES};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionSnapshot {
    pub input_json: String,
    pub resolution_evidence_json: String,
    pub lock_json: String,
}

pub(super) fn validate_cumulative(snapshot: &ResolutionSnapshot) -> Result<(), Diagnostic> {
    let bytes = snapshot
        .input_json
        .len()
        .checked_add(snapshot.resolution_evidence_json.len())
        .and_then(|value| value.checked_add(snapshot.lock_json.len()))
        .ok_or_else(|| limit_error("snapshot cumulative byte accounting overflowed"))?;
    if snapshot.input_json.len() > super::MAX_INPUT_BYTES
        || snapshot.resolution_evidence_json.len() > crate::package_resolver::MAX_OUTPUT_BYTES
        || snapshot.lock_json.len() > crate::package_lock_v2::MAX_OUTPUT_BYTES
        || bytes > MAX_SNAPSHOT_BYTES
    {
        return Err(limit_error("snapshot cumulative byte bound exceeded"));
    }
    Ok(())
}
