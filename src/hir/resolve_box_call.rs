//! Exact compiler-owned `Box<T>` call lowering.

use super::expr_nodes::{ResolvedExpr, ResolvedExprKind};
use super::ids::{DeclarationId, ExpressionId, FunctionExecutionId};
use super::nodes::{OwnershipMode, ResolvedType};
use super::resolve_expr_frame::Frame;
mod iterator;

#[derive(Clone, Copy)]
pub(super) enum OwnedGenericCallSite {
    Vec(crate::vec_ops::VecOp),
    Box(crate::box_ops::BoxOp),
    Iterator(crate::iterator_ops::IteratorOp),
}

#[cfg(test)]
pub(super) struct ReferenceCall<'a> {
    pub(super) id: ExpressionId,
    pub(super) type_arguments: &'a [Type],
    pub(super) args: &'a [Expr],
    pub(super) bindings: &'a BTreeMap<String, Binding>,
    pub(super) path: &'a str,
    pub(super) span: Span,
}

#[cfg(test)]
impl<'a> ReferenceCall<'a> {
    pub(super) fn new(
        id: ExpressionId,
        type_arguments: &'a [Type],
        args: &'a [Expr],
        bindings: &'a BTreeMap<String, Binding>,
        path: &'a str,
        span: Span,
    ) -> Self {
        Self {
            id,
            type_arguments,
            args,
            bindings,
            path,
            span,
        }
    }
}

impl OwnedGenericCallSite {
    pub(super) fn by_name(name: &str) -> Option<Self> {
        crate::vec_ops::by_name(name)
            .map(Self::Vec)
            .or_else(|| crate::box_ops::by_name(name).map(Self::Box))
            .or_else(|| crate::iterator_ops::by_name(name).map(Self::Iterator))
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn schedule<'expr>(
        self,
        resolver: &Resolver<'_>,
        function: &FunctionExecutionId,
        frames: &mut Vec<Frame<'expr>>,
        type_arguments: &[Type],
        args: &'expr [Expr],
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match self {
            Self::Iterator(op) => iterator::schedule(
                resolver,
                function,
                frames,
                type_arguments,
                args,
                bindings,
                path,
                span,
                op,
            ),
            Self::Vec(op) => super::resolve_vec_call::schedule(
                resolver,
                function,
                frames,
                type_arguments,
                args,
                bindings,
                super::resolve_vec_call::VecCallSite::new(path, span, op),
            ),
            Self::Box(op) => schedule(
                resolver,
                function,
                frames,
                type_arguments,
                args,
                bindings,
                path,
                span,
                op,
            ),
        }
    }
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_reference(
        self,
        resolver: &Resolver<'_>,
        function: &FunctionExecutionId,
        call: ReferenceCall<'_>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        match self {
            Self::Vec(op) => {
                super::resolve_vec_call::resolve_reference(resolver, function, call, op)
            }
            Self::Box(op) => resolve_reference(resolver, function, call, op),
            Self::Iterator(op) => iterator::reference(resolver, function, call, op),
        }
    }
}

pub(super) fn finish_owned(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    path: &str,
    span: Span,
    site: OwnedGenericCallSite,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    match site {
        OwnedGenericCallSite::Vec(op) => {
            super::resolve_vec_call::finish(resolver, function, path, span, op, element, args)
        }
        OwnedGenericCallSite::Box(op) => finish(function, path, span, op, element, args),
        OwnedGenericCallSite::Iterator(op) => {
            iterator::finish(function, path, span, op, element, args)
        }
    }
}
use super::{Binding, Resolver};
use crate::ast::{Expr, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;
use std::rc::Rc;

fn resolve_element(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    op: crate::box_ops::BoxOp,
    ty: &Type,
    span: Span,
) -> Result<ResolvedType, Diagnostic> {
    if let FunctionExecutionId::Monomorphic(owner) = function {
        if let Some(candidate) = resolver
            .program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == owner.as_str())
            .filter(|candidate| {
                crate::box_ops::source_wrapper(resolver.program, candidate) == Some(op)
                    || crate::source_verify::generic_collection_profile(candidate)
            })
        {
            return resolver.resolve_function_type(candidate, ty, span);
        }
    }
    resolver.resolve_type(ty, span)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'expr>(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>,
    type_arguments: &[Type],
    args: &'expr [Expr],
    bindings: Rc<BTreeMap<String, Binding>>,
    path: String,
    span: Span,
    op: crate::box_ops::BoxOp,
) -> Result<(), Diagnostic> {
    if type_arguments.len() != 1 || args.len() != 1 {
        return Err(resolver.error(
            "SPX-H006",
            format!("invalid box operation `{}` call shape", op.name()),
            span,
        ));
    }
    let element = resolve_element(resolver, function, op, &type_arguments[0], span)?;
    if !crate::box_ops::resolved_operation_element_is_admitted(op, &element)
        && !crate::box_ops::resolved_parameter_is_admitted(function, op, &element)
        && !matches!(function, FunctionExecutionId::Monomorphic(owner) if super::generic_collection::parameter(&element, owner))
    {
        return Err(resolver.error(
            "SPX-H006",
            format!(
                "box operation `{}` has an inadmissible element type",
                op.name()
            ),
            span,
        ));
    }
    frames.push(Frame::FinishOwnedGenericOp {
        span,
        path: path.clone(),
        op: OwnedGenericCallSite::Box(op),
        element,
        argument_count: 1,
    });
    frames.push(Frame::ChildNext {
        children: args,
        index: 0,
        bindings,
        path,
        segment: "arg",
    });
    Ok(())
}

pub(super) fn finish(
    function: &FunctionExecutionId,
    path: &str,
    span: Span,
    op: crate::box_ops::BoxOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    let Some(argument) = args.first() else {
        return Err(Diagnostic::io(
            "SPX-H006",
            "box operation argument is missing",
        ));
    };
    if argument.ty != op.resolved_param_type(&element) {
        return Err(Diagnostic::io(
            "SPX-H006",
            "box operation argument has the wrong type",
        ));
    }
    let ty = op.resolved_return_type(&element);
    Ok(ResolvedExpr {
        id: ExpressionId::new(function, path),
        ownership: if op == crate::box_ops::BoxOp::New || element == ResolvedType::Bytes {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        },
        ty,
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
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_reference(
    resolver: &Resolver<'_>,
    function: &FunctionExecutionId,
    call: ReferenceCall<'_>,
    op: crate::box_ops::BoxOp,
) -> Result<ResolvedExpr, Diagnostic> {
    let ReferenceCall {
        id,
        type_arguments,
        args,
        bindings,
        path,
        span,
    } = call;
    if type_arguments.len() != 1 || args.len() != 1 {
        return Err(resolver.error(
            "SPX-H006",
            format!("invalid box operation `{}` call shape", op.name()),
            span,
        ));
    }
    let element = resolve_element(resolver, function, op, &type_arguments[0], span)?;
    if !crate::box_ops::resolved_operation_element_is_admitted(op, &element)
        && !crate::box_ops::resolved_parameter_is_admitted(function, op, &element)
        && !matches!(function, FunctionExecutionId::Monomorphic(owner) if super::generic_collection::parameter(&element, owner))
    {
        return Err(resolver.error(
            "SPX-H006",
            "box operation has an inadmissible element type",
            span,
        ));
    }
    let args = vec![resolver.resolve_expr_recursive_reference(
        function,
        &args[0],
        bindings,
        &format!("{path}.arg.0"),
    )?];
    let mut result = finish(function, path, span, op, element, args)?;
    result.id = id;
    Ok(result)
}
