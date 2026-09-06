//! Compiler-owned propagated-status admission for cleanup replay.
//!
//! A compiler-owned operation that can fail publishes exactly one normalized
//! status. Re-deriving that table here keeps a replayed trace from claiming a
//! domain, code, class, or retryability the operation cannot produce.

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::hir::DeclarationId;

use super::{invariant, CleanupExecutionError};

pub(super) fn validate_propagated_status(
    callee: &DeclarationId,
    status: &NormalizedStatus,
) -> Result<(), CleanupExecutionError> {
    if callee.as_str() == crate::byte_ops::SET_ID {
        if status.domain_id() != crate::byte_ops::SET_STATUS_DOMAIN
            || status.code() != crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE
            || status.class() != StatusClass::Adapter
            || status.retryability() != Retryability::Known(false)
        {
            return Err(invariant(
                "owned byte buffer store supplied a status outside its exact normalized failure domain",
            ));
        }
        return Ok(());
    }
    if callee.as_str() == crate::byte_ops::RANGE_ID {
        if status.domain_id() != crate::byte_ops::RANGE_STATUS_DOMAIN
            || ![
                crate::byte_ops::RANGE_START_AFTER_END_CODE,
                crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
            ]
            .contains(&status.code())
            || status.class() != StatusClass::Adapter
            || status.retryability() != Retryability::Known(false)
        {
            return Err(invariant(
                "byte range supplied a status outside its exact normalized failure domain",
            ));
        }
        return Ok(());
    }
    let Some(operation) = crate::command_io_ops::by_id(callee.as_str()) else {
        // Authored and other target-neutral calls retain their existing
        // normalized-status contract.
        return Ok(());
    };
    let metadata = crate::command_io_ops::status_metadata(operation).ok_or_else(|| {
        invariant(format!(
            "infallible command operation `{callee}` supplied a propagated status"
        ))
    })?;
    if status.domain_id() != metadata.domain
        || !metadata.codes.contains(&status.code())
        || status.class() != StatusClass::Adapter
        || status.retryability() != Retryability::Known(false)
    {
        return Err(invariant(format!(
            "command operation `{callee}` supplied a status outside its exact normalized failure domain"
        )));
    }
    Ok(())
}
