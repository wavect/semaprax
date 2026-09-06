//! Synthetic and authored call-parameter lookup for cleanup replay.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, FunctionInstanceId, ResolvedFunction, ResolvedParam, ResolvedProgram,
    ResolvedType,
};

use super::replay_error;

/// Resolve one call's parameters for replay: compiler-owned operations carry
/// their reserved identity instead of an authored declaration and use their
/// synthetic parameters.
pub(super) fn resolved_call_params(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    callee: &DeclarationId,
    instance: Option<&FunctionInstanceId>,
    type_arguments: &[ResolvedType],
) -> Result<Vec<ResolvedParam>, Diagnostic> {
    if instance.is_none() {
        if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
            return Ok(crate::string_ops::resolved_params(op));
        }
        if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
            return Ok(crate::str_ops::resolved_params(op));
        }
        if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
            return Ok(crate::byte_ops::resolved_params(op));
        }
        if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
            return Ok(crate::host_io_ops::resolved_params(op));
        }
        if let Some(op) = crate::command_io_ops::by_id(callee.as_str()) {
            return Ok(crate::command_io_ops::resolved_params(op));
        }
        if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
            let [element] = type_arguments else {
                return Err(replay_error(
                    function,
                    "cleanup bounded Vec call has incorrect type arity",
                ));
            };
            return Ok(crate::vec_ops::resolved_params(op, element));
        }
    }
    let target = program
        .resolve_call_target(callee, instance)
        .ok_or_else(|| {
            replay_error(
                function,
                format!("cleanup call has unknown callee `{callee}`"),
            )
        })?;
    Ok(target.params.clone())
}
