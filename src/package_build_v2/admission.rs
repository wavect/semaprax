use crate::diagnostic::Diagnostic;

use super::model::{
    LinkedOfflinePackageBuildOptions, MAX_ARTIFACT_BYTES, MAX_EVIDENCE_BYTES, MAX_EXPORTS,
    MAX_STABLE_ID_BYTES, MIN_LIMIT_BYTES,
};

pub(crate) fn validate_options(
    options: &LinkedOfflinePackageBuildOptions,
) -> Result<(), Diagnostic> {
    if !(MIN_LIMIT_BYTES..=MAX_ARTIFACT_BYTES).contains(&options.max_artifact_bytes) {
        return Err(super::option_error(
            "linked package-build max_artifact_bytes is outside the frozen range",
        ));
    }
    if !(MIN_LIMIT_BYTES..=MAX_EVIDENCE_BYTES).contains(&options.max_evidence_bytes) {
        return Err(super::option_error(
            "linked package-build max_evidence_bytes is outside the frozen range",
        ));
    }
    if options.root_package.len() > 255
        || crate::workspace_graph::validate_entry_module(&options.root_package).is_err()
    {
        return Err(super::option_error(
            "linked package-build root is outside the canonical module-name grammar",
        ));
    }
    if !(1..=MAX_EXPORTS).contains(&options.exports.len()) {
        return Err(super::option_error(
            "linked package-build exports must contain 1..=32 stable IDs",
        ));
    }
    let mut previous: Option<&String> = None;
    for id in &options.exports {
        if id.is_empty()
            || id.len() > MAX_STABLE_ID_BYTES
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(super::option_error(
                "linked package-build export stable ID is outside the scalar profile grammar",
            ));
        }
        if previous.is_some_and(|value| value.as_bytes() >= id.as_bytes()) {
            return Err(super::option_error(
                "linked package-build exports must be strictly byte-sorted and unique",
            ));
        }
        previous = Some(id);
    }
    Ok(())
}

pub(crate) fn validate_root_exports(
    options: &LinkedOfflinePackageBuildOptions,
    authenticated_root_exports: &[String],
) -> Result<(), Diagnostic> {
    if options.exports.iter().all(|id| {
        authenticated_root_exports
            .binary_search_by(|candidate| candidate.as_bytes().cmp(id.as_bytes()))
            .is_ok()
    }) {
        Ok(())
    } else {
        Err(super::profile_error(
            "linked package-build export is not a root-owned authenticated capsule export",
        ))
    }
}
