//! Exact compiler-owned `Vec<T>` call lowering shared by the production
//! iterative resolver and its test-only recursive reference.
use crate::hir::resolver_scope::Scope;

use crate::ast::{Expr, Span, Type};
use crate::diagnostic::Diagnostic;

use super::expr_nodes::{ResolvedExpr, ResolvedExprKind};
use super::ids::{DeclarationId, FunctionExecutionId};
use super::nodes::{is_scalar_resolved_type, OwnershipMode, ResolvedBinding, ResolvedType};
use super::resolve_expr::path_arena::{PathId, Seg};
use super::resolve_expr_frame::Frame;
use super::{ExprPath, Resolver};

pub(super) struct VecCallSite {
    pub(super) path: PathId,
    pub(super) span: Span,
    pub(super) op: crate::vec_ops::VecOp,
}

impl VecCallSite {
    pub(super) fn new(path: PathId, span: Span, op: crate::vec_ops::VecOp) -> Self {
        Self { path, span, op }
    }
}

fn resolve_element(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    op: crate::vec_ops::VecOp,
    ty: &Type,
    span: Span,
) -> Result<ResolvedType, Diagnostic> {
    if let FunctionExecutionId::Monomorphic(owner) = function {
        if let Some(candidate) = resolver
            .source_functions()
            .find(|candidate| candidate.payload().stable_id == owner.as_str())
            .filter(|candidate| {
                crate::vec_ops::source_wrapper_ref(resolver.program, candidate) == Some(op)
                    || crate::source_verify::generic_collection_profile(candidate.payload())
            })
        {
            return resolver.resolve_function_type_ref(candidate, ty, span);
        }
    }
    resolver.resolve_type(ty, span)
}

pub(super) fn validate_whole_assignment(
    resolver: &Resolver<'_, '_>,
    execution: &FunctionExecutionId,
    target: &ResolvedBinding,
    value: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    if value.ty != target.ty {
        return Err(resolver.error(
            "SPX-U102",
            format!(
                "assigned value type `{}` does not exactly match binding type `{}`",
                value.ty.identity_key(),
                target.ty.identity_key()
            ),
            value.span,
        ));
    }
    if (value.ownership != OwnershipMode::Value || !is_scalar_resolved_type(&value.ty))
        && !crate::string_ops::replacement::admitted(target, value)
        && !scoped_collection_assignment(resolver, execution, target, value)
        && !crate::vec_ops::is_same_owner_reassignment_hir_source(
            resolver.program,
            value,
            &target.id,
        )
        && !crate::byte_ops::is_same_owner_set_hir(value, &target.id)
        && !crate::string_ops::is_same_owner_concat_hir(value, &target.id)
        && !byte_helper_renewal(resolver, target, value)
        && !record_owner_renewal(resolver, target, value)
        && !crate::stdin_stream_ops::hir_reopen(value, &target.id)
    {
        return Err(resolver.error(
            "SPX-U105",
            "explicit mutation v1 supports only scalar Copy values",
            value.span,
        ));
    }
    Ok(())
}

fn record_owner_renewal(
    resolver: &Resolver<'_, '_>,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    if binding.ownership != OwnershipMode::Own
        || value.ownership != OwnershipMode::Own
        || binding.ty != value.ty
        || !crate::hir::iterator_loop::is_owner_renewal_record(&resolver.declarations, &binding.ty)
    {
        return false;
    }
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = resolver
        .source_functions()
        .find(|function| function.payload().stable_id == callee.as_str())
    else {
        return false;
    };
    target.payload().effects.is_empty()
        && target.payload().type_parameters.is_empty()
        && type_arguments.is_empty()
        && resolver
            .resolve_type(&target.payload().return_type, value.span)
            .is_ok_and(|ty| ty == binding.ty)
        && target.payload().params.len() == args.len()
        && target
            .payload()
            .params
            .iter()
            .zip(args)
            .filter(|(parameter, argument)| {
                parameter.mode == crate::ast::ParamMode::Own
                    && resolver
                        .resolve_type(&parameter.ty, argument.span)
                        .is_ok_and(|ty| ty == binding.ty)
                    && matches!(&argument.kind, ResolvedExprKind::Place(place)
                        if place.root == binding.id && place.projections.is_empty())
            })
            .count()
            == 1
        && target
            .payload()
            .params
            .iter()
            .zip(args)
            .all(|(parameter, argument)| {
                resolver
                    .resolve_type(&parameter.ty, argument.span)
                    .is_ok_and(|ty| ty == argument.ty)
                    && match parameter.mode {
                        crate::ast::ParamMode::Own => {
                            argument.ownership == OwnershipMode::Own
                                && argument.ty == binding.ty
                                && matches!(&argument.kind, ResolvedExprKind::Place(place)
                                if place.root == binding.id && place.projections.is_empty())
                        }
                        crate::ast::ParamMode::Borrow => {
                            matches!(
                                argument.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            ) && matches!(&argument.kind, ResolvedExprKind::Place(place)
                                if place.projections.is_empty())
                                && (crate::hir::iterator_loop::is_owner_renewal_record(
                                    &resolver.declarations,
                                    &argument.ty,
                                ) || (argument.ownership == OwnershipMode::Borrow
                                    && crate::loop_calls::resolved_renewal_view(&argument.ty)))
                        }
                        crate::ast::ParamMode::Value => argument.ownership == OwnershipMode::Value,
                        crate::ast::ParamMode::Shared => false,
                    }
            })
}

pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    type_arguments: &[Type],
    args: &'expr [Expr],
    bindings: Scope<'expr>,
    site: VecCallSite,
) -> Result<(), Diagnostic> {
    let VecCallSite { path, span, op } = site;
    if (if op == crate::vec_ops::VecOp::StrAt {
        !type_arguments.is_empty() || function.instance().is_some()
    } else {
        type_arguments.len() != 1
    }) || args.len() != op.arity()
    {
        return Err(resolver.error(
            "SPX-H006",
            format!("invalid vector operation `{}` call shape", op.name()),
            span,
        ));
    }
    let element = if op == crate::vec_ops::VecOp::StrAt {
        ResolvedType::String
    } else {
        resolve_element(resolver, function, op, &type_arguments[0], span)?
    };
    if !super::owned_leaf_collection::vec_operation_admitted(&resolver.declarations, op, &element)
        && !crate::vec_ops::resolved_parameter_is_admitted(function, op, &element)
        && !(!op.owned_leaf_only()
            && super::generic_collection::source_parameter(resolver.program, function, &element))
    {
        return Err(resolver.error(
            "SPX-H006",
            format!(
                "vector operation `{}` has an inadmissible element type",
                op.name()
            ),
            span,
        ));
    }
    frames.push(Frame::FinishOwnedGenericOp {
        span,
        path,
        op: super::resolve_box_call::OwnedGenericCallSite::Vec(op),
        element,
        argument_count: args.len(),
    });
    frames.push(Frame::ChildNext {
        children: args,
        index: 0,
        bindings,
        path,
        segment: Seg::Arg,
    });
    Ok(())
}

pub(super) fn finish<'p>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: crate::vec_ops::VecOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    for (index, argument) in args.iter().enumerate() {
        if !op.accepts_resolved(index, &argument.ty, &element) {
            return Err(resolver.error(
                "SPX-H006",
                format!(
                    "vector operation `{}` argument {index} has the wrong type",
                    op.name()
                ),
                argument.span,
            ));
        }
    }
    let ty = op.resolved_return_type(&element);
    if op == crate::vec_ops::VecOp::StrAt && !crate::vec_ops::string_slot::arguments(&args) {
        return Err(resolver.error("SPX-H006", crate::vec_ops::string_slot::SHAPE, span));
    }
    if op == crate::vec_ops::VecOp::UniqueBy
        && !crate::vec_ops::field_unique::arguments(&args, &element)
    {
        return Err(resolver.error("SPX-H006", crate::vec_ops::field_unique::SHAPE, span));
    }
    let ownership = if op == crate::vec_ops::VecOp::StrAt {
        OwnershipMode::Borrow
    } else if op.returns_owner() {
        OwnershipMode::Own
    } else {
        OwnershipMode::Value
    };
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ty,
        ownership,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: vec![element],
            instance: None,
            args,
        },
        span,
    })
}

#[cfg(test)]
pub(super) fn resolve_reference(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    call: super::resolve_box_call::ReferenceCall<'_>,
    op: crate::vec_ops::VecOp,
) -> Result<ResolvedExpr, Diagnostic> {
    let super::resolve_box_call::ReferenceCall {
        id,
        type_arguments,
        args,
        bindings,
        path,
        span,
    } = call;
    let name = op.name();
    if (if op == crate::vec_ops::VecOp::StrAt {
        !type_arguments.is_empty() || function.instance().is_some()
    } else {
        type_arguments.len() != 1
    }) || args.len() != op.arity()
    {
        return Err(resolver.error(
            "SPX-H006",
            format!("invalid vector operation `{name}` call shape"),
            span,
        ));
    }
    let element = if op == crate::vec_ops::VecOp::StrAt {
        ResolvedType::String
    } else {
        resolve_element(resolver, function, op, &type_arguments[0], span)?
    };
    if !super::owned_leaf_collection::vec_operation_admitted(&resolver.declarations, op, &element)
        && !crate::vec_ops::resolved_parameter_is_admitted(function, op, &element)
        && !(!op.owned_leaf_only()
            && super::generic_collection::source_parameter(resolver.program, function, &element))
    {
        return Err(resolver.error(
            "SPX-H006",
            format!("vector operation `{name}` has an inadmissible element type"),
            span,
        ));
    }
    let args = args
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            resolver.resolve_expr_recursive_reference(
                function,
                argument,
                bindings,
                &format!("{path}.arg.{index}"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut resolved = finish(resolver, function, path, span, op, element, args)?;
    resolved.id = id;
    Ok(resolved)
}

fn scoped_collection_assignment(
    resolver: &Resolver<'_, '_>,
    execution: &FunctionExecutionId,
    target: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    if let ResolvedType::TypeParameter { owner, .. } = &value.ty {
        if value.ownership == OwnershipMode::Value
            && resolver.source_functions().any(|function| {
                function.payload().stable_id == owner.as_str()
                    && crate::source_verify::generic_collection_profile(function.payload())
                    && super::generic_collection::parameter(
                        &value.ty,
                        owner,
                        function.payload().type_parameters.len(),
                    )
            })
        {
            return true;
        }
    }
    let Some(owner) = execution.monomorphic_declaration() else {
        return false;
    };
    if !resolver.source_functions().any(|f| {
        f.payload().stable_id == owner.as_str()
            && crate::source_verify::generic_collection_profile(f.payload())
    }) {
        return false;
    }
    if value.ownership == OwnershipMode::Value
        && super::generic_collection::parameter(
            &value.ty,
            owner,
            super::generic_collection::source_count(resolver.program, owner),
        )
    {
        return true;
    }
    if !super::generic_collection::slot(
        &value.ty,
        owner,
        super::generic_collection::source_count(resolver.program, owner),
    ) || value.ownership != OwnershipMode::Own
    {
        return false;
    }
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &value.kind
    else {
        return false;
    };
    instance.is_none()
        && crate::vec_ops::by_id(callee.as_str())
            .is_some_and(|op| op.reopens_same_owner() && args.len() == op.arity())
        && matches!(type_arguments.as_slice(), [argument] if super::generic_collection::parameter(argument, owner, super::generic_collection::source_count(resolver.program, owner)))
        && matches!(&args[0].kind, ResolvedExprKind::Place(place) if place.root == target.id && place.projections.is_empty())
}

fn byte_helper_renewal(
    resolver: &Resolver<'_, '_>,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = resolver
        .source_functions()
        .find(|f| f.payload().stable_id == callee.as_str())
    else {
        return false;
    };
    binding.ty == super::ResolvedType::Bytes && value.ty == binding.ty
        && binding.ownership == OwnershipMode::Own && value.ownership == OwnershipMode::Own
        && type_arguments.is_empty() && args.len() == target.payload().params.len()
        && resolver.byte_helpers.target_ref(&target)
        && target.payload().params.iter().zip(args).all(|(p, arg)| {
            resolver.resolve_type(&p.ty, arg.span).is_ok_and(|ty| ty == arg.ty)
                && (p.mode != crate::ast::ParamMode::Own || matches!(&arg.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty()))
        })
}
