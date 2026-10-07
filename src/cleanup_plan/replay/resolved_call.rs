//! Synthetic and authored call-parameter lookup for cleanup replay.

use std::collections::BTreeSet;

use crate::cleanup_plan::{CleanupPlace, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, FunctionInstanceId, IdentityOrigin, ResolvedExprKind,
    ResolvedFunction, ResolvedParam, ResolvedProgram, ResolvedType,
};

use super::{
    find_resolved_expression, replay_error, validate_place, Leaves, PathState,
    ReplayConditionalVariant,
};

pub(super) fn exact_owned_try(source: &[ResolvedType], target: &[ResolvedType]) -> bool {
    let admitted_source = matches!(source, [ResolvedType::Bytes, error]
        if *error == ResolvedType::Bytes || crate::hir::is_scalar_resolved_type(error))
        || matches!(source, [success, ResolvedType::Bytes]
            if crate::hir::is_scalar_resolved_type(success));
    let admitted_target = matches!(target, [ResolvedType::Bytes, _])
        || matches!(target, [success, ResolvedType::Bytes]
            if crate::hir::is_scalar_resolved_type(success));
    admitted_source
        && source.get(1) == target.get(1)
        && (source == target || (source.get(1) == Some(&ResolvedType::Bytes) && admitted_target))
}

pub(super) fn owned_try_residual_places(
    function: &ResolvedFunction,
    expression: &crate::hir::ResolvedExpr,
    mut source: CleanupPlace,
    work: &mut super::SkeletonWork<'_, '_>,
) -> Result<(CleanupPlace, CleanupPlace), Diagnostic> {
    let ResolvedExprKind::Try {
        operand,
        err_case,
        err_field,
        residual_type,
        ..
    } = &expression.kind
    else {
        return Err(replay_error(
            function,
            "owned Result residual projection does not name a Try expression",
        ));
    };
    let mut destination = CleanupPlace::whole(StorageId::ProvisionalResult);
    if operand.ty != *residual_type {
        source
            .projections
            .push(work.clone_owned(err_case, "changed-success residual source case projection")?);
        source.projections.push(work.clone_owned(
            err_field,
            "changed-success residual source field projection",
        )?);
        destination.projections.push(work.clone_owned(
            err_case,
            "changed-success residual destination case projection",
        )?);
        destination.projections.push(work.clone_owned(
            err_field,
            "changed-success residual destination field projection",
        )?);
    }
    Ok((source, destination))
}

pub(super) fn seal_changed_success_try_residual(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    at: &ExpressionId,
    destination: &CleanupPlace,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    let Some(expression) = find_resolved_expression(function, at) else {
        return Ok(());
    };
    let ResolvedExprKind::Try {
        operand,
        result,
        ok_case,
        ok_field,
        err_case,
        err_field,
        residual_type,
        ..
    } = &expression.kind
    else {
        return Ok(());
    };
    if operand.ty == *residual_type
        || destination.storage != StorageId::ProvisionalResult
        || destination.projections.as_slice() != [err_case.clone(), err_field.clone()]
    {
        return Ok(());
    }
    let (
        ResolvedType::Nominal {
            declaration: source,
            arguments: source_arguments,
        },
        ResolvedType::Nominal {
            declaration: target,
            arguments: target_arguments,
        },
    ) = (&operand.ty, residual_type)
    else {
        return Err(replay_error(
            function,
            "changed-success Try residual has non-nominal carrier metadata",
        ));
    };
    if result.as_str() != crate::prelude::RESULT_ID
        || ok_case.as_str() != crate::prelude::RESULT_OK_ID
        || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
        || err_case.as_str() != crate::prelude::RESULT_ERR_ID
        || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
        || [result, ok_case, ok_field, err_case, err_field]
            .iter()
            .any(|id| {
                program
                    .declarations
                    .declaration(id)
                    .is_none_or(|item| item.identity_origin != IdentityOrigin::CompilerOwned)
            })
        || program
            .declarations
            .variant_cases(result)
            .is_none_or(|cases| {
                cases.len() != 2
                    || !cases
                        .iter()
                        .any(|case| case.id == *ok_case && case.id != *err_case)
                    || !cases.iter().any(|case| case.id == *err_case)
            })
        || program
            .declarations
            .case_fields(ok_case)
            .is_none_or(|fields| !matches!(fields, [field] if field.id == *ok_field))
        || program
            .declarations
            .case_fields(err_case)
            .is_none_or(|fields| !matches!(fields, [field] if field.id == *err_field))
        || source != result
        || target != result
        || !exact_owned_try(source_arguments, target_arguments)
    {
        return Err(replay_error(
            function,
            "changed-success Try residual has unauthenticated Result types",
        ));
    }
    let root = CleanupPlace::whole(StorageId::ProvisionalResult);
    let flags = validate_place(function, &root, storage, leaves)?;
    let selected = flags
        .iter()
        .filter(|flag| {
            leaves[flag]
                .place
                .projections
                .starts_with(&destination.projections[..1])
        })
        .copied()
        .collect::<Vec<_>>();
    if selected.is_empty()
        || selected.iter().any(|flag| !state.live_order.contains(flag))
        || flags
            .iter()
            .any(|flag| state.live_order.contains(flag) && !selected.contains(flag))
        || state
            .conditional_variants
            .iter()
            .any(|variant| variant.root == root)
    {
        return Err(replay_error(
            function,
            "changed-success Try residual has inconsistent provisional liveness",
        ));
    }
    let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
    state.live_order.retain(|flag| !selected_set.contains(flag));
    state.conditional_variants.push(ReplayConditionalVariant {
        root,
        variant: result.clone(),
        cases: program
            .declarations
            .variant_cases(result)
            .ok_or_else(|| replay_error(function, "changed-success Result has no case domain"))?
            .iter()
            .map(|case| {
                (
                    case.id.clone(),
                    flags
                        .iter()
                        .filter(|flag| {
                            leaves[flag]
                                .place
                                .projections
                                .starts_with(&[case.id.clone()])
                        })
                        .copied()
                        .collect(),
                )
            })
            .collect(),
    });
    Ok(())
}

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
        if let Some(params) = crate::hir::closure::once::params(callee) {
            if !type_arguments.is_empty() {
                return Err(replay_error(function, "affine call has type arguments"));
            }
            return Ok(params);
        }
        if callee == &*crate::hir::function_value::INVOKE_ID {
            let (signature, parameters): (&ResolvedType, &[ResolvedType]) = match type_arguments {
                [signature @ ResolvedType::Function { parameters, .. }] => (signature, parameters),
                [signature @ ResolvedType::MutFunctionI64] => (signature, &[ResolvedType::I64]),
                _ => {
                    return Err(replay_error(
                        function,
                        "indirect call lacks exact callable signature",
                    ))
                }
            };
            if !signature.is_mut_function() && !crate::hir::function_value::is_signature(signature)
            {
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
        if callee.as_str() == crate::stdin_stream_ops::EOF_ID {
            if !type_arguments.is_empty() {
                return Err(replay_error(
                    function,
                    "streaming EOF call has type arguments",
                ));
            }
            return Ok(crate::stdin_stream_ops::PureOp::Eof.resolved_params());
        }
        if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
            return Ok(crate::host_io_ops::resolved_params(op));
        }
        if let Some(op) = crate::command_io_ops::by_id(callee.as_str()) {
            return Ok(crate::command_io_ops::resolved_params(op));
        }
        if let Some(op) = crate::list_ops::by_id(callee.as_str()) {
            if !type_arguments.is_empty() {
                return Err(replay_error(
                    function,
                    "immutable list call has incorrect type arity",
                ));
            }
            return Ok(op.resolved_params());
        }
        if let Some(op) = crate::iterator_ops::by_id(callee.as_str()) {
            let [element] = type_arguments else {
                return Err(replay_error(
                    function,
                    "iterator call has incorrect type arity",
                ));
            };
            if !crate::iterator_ops::resolved_element_is_admitted_in(&program.declarations, element)
            {
                return Err(replay_error(
                    function,
                    "iterator call has unsupported element",
                ));
            }
            return Ok(crate::iterator_ops::resolved_params(op, element));
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
    if instance.is_none()
        && program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .any(|import| import.native_rust && &import.id == callee)
    {
        return super::super::native_rust::params(program, callee);
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
    if matches!(&expression.kind, crate::hir::ResolvedExprKind::HostCommandCall(call)
        if call.operation == crate::hir::ResolvedHostCommandOperation::StdinStreamNext)
    {
        return true;
    }
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
            || callee.as_str() == crate::iterator_ops::NEXT_ID
            || (callee.as_str() == crate::iterator_ops::INTO_ITER_ID
                && matches!(type_arguments.as_slice(), [crate::hir::ResolvedType::Bytes]))
            || crate::byte_ops::by_id(callee.as_str())
                .is_some_and(crate::byte_ops::ByteOp::is_fallible)
            || (callee.as_str() == crate::box_ops::NEW_ID
                && matches!(type_arguments.as_slice(), [crate::hir::ResolvedType::Bytes]))
            // String Collections v1 map reopens fail before taking the map.
            || crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::reopens_map)
    )
}
