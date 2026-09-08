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
        if callee == &*crate::hir::function_value::INVOKE_ID {
            let [signature @ ResolvedType::Function { parameters, .. }] = type_arguments else {
                return Err(replay_error(
                    function,
                    "indirect call lacks exact callable signature",
                ));
            };
            if !crate::hir::function_value::is_signature(signature) {
                return Err(replay_error(
                    function,
                    "indirect call signature is not admitted",
                ));
            }
            return Ok(parameters
                .iter()
                .enumerate()
                .map(|(index, ty)| ResolvedParam {
                    id: crate::hir::ValueId::intrinsic_parameter(callee.as_str(), index),
                    name: format!("arg{index}"),
                    ty: ty.clone(),
                    ownership: crate::hir::OwnershipMode::Value,
                    span: crate::ast::Span::default(),
                })
                .collect());
        }
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
        if let Some(op) = crate::box_ops::by_id(callee.as_str()) {
            let [element] = type_arguments else {
                return Err(replay_error(
                    function,
                    "cleanup bounded Box call has incorrect type arity",
                ));
            };
            return Ok(crate::box_ops::resolved_params(op, element));
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

// Independently replay the exact failure-before-transfer operation profile.
pub(super) fn defers_owner_commit(expression: &crate::hir::ResolvedExpr) -> bool {
    matches!(
        &expression.kind,
        crate::hir::ResolvedExprKind::Call {
            callee,
            instance: None,
            type_arguments,
            ..
        } if matches!(
            crate::vec_ops::by_id(callee.as_str()),
            Some(
                crate::vec_ops::VecOp::Push
                    | crate::vec_ops::VecOp::ReserveExact
                    | crate::vec_ops::VecOp::Set
            )
        )
            || crate::byte_ops::by_id(callee.as_str())
                .is_some_and(crate::byte_ops::ByteOp::is_fallible)
            || (callee.as_str() == crate::box_ops::NEW_ID
                && matches!(type_arguments.as_slice(), [crate::hir::ResolvedType::Bytes]))
    )
}
