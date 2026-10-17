//! Fixed String-slot specialization. No nominal field or generic callback grants
//! this origin; complete current HIR shape is checked before returning a view.
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};
pub(crate) const NAME: &str = "vec_str_at";
pub(crate) const ID: &str = "core.vec.str-at";
/// Compiler-only normalization of exactly str_as_bytes(vec_str_at(...)).
pub(crate) const BYTES_ID: &str = "core.vec.str-at-bytes";
pub(crate) const SHAPE: &str =
    "vec_str_at requires an available named Vec<string> owner or borrow and a value usize index";

pub(crate) struct Slot<'a> {
    pub(crate) source: &'a ResolvedExpr,
    pub(crate) index: &'a ResolvedExpr,
    pub(crate) bytes: bool,
}
pub(crate) fn arguments(args: &[ResolvedExpr]) -> bool {
    matches!(args, [source, index]
        if super::resolved_vec_matches(&source.ty, &ResolvedType::String)
        && matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        && matches!(&source.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
        && index.ty == ResolvedType::Usize && index.ownership == OwnershipMode::Value)
}
pub(crate) fn inspect(expression: &ResolvedExpr) -> Option<Slot<'_>> {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &expression.kind
    else {
        return None;
    };
    let bytes = match callee.as_str() {
        ID => false,
        BYTES_ID => true,
        _ => return None,
    };
    if type_arguments.as_slice() != [ResolvedType::String]
        || !arguments(args)
        || expression.ownership != OwnershipMode::Borrow
        || expression.ty
            != if bytes {
                ResolvedType::SliceU8
            } else {
                ResolvedType::Str
            }
    {
        return None;
    }
    Some(Slot {
        source: &args[0],
        index: &args[1],
        bytes,
    })
}
pub(crate) fn function_uses(function: &crate::hir::ResolvedFunction) -> bool {
    let mut found = false;
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
            found |= matches!(callee.as_str(), ID | BYTES_ID);
        });
    }
    found
}
pub(crate) fn resolved_uses(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(function_uses)
}

pub(crate) fn argument_path(expression: &ResolvedExpr, path: &str, index: usize) -> String {
    if matches!(&expression.kind, ResolvedExprKind::Call {callee,..} if callee.as_str() == BYTES_ID)
    {
        crate::bounded_output::budgeted_format(format_args!("{path}.arg.0.arg.{index}"))
    } else {
        crate::bounded_output::budgeted_format(format_args!(
            "{path}.arg.{}",
            index
                + usize::from(matches!(
                    expression.kind,
                    ResolvedExprKind::LiteralFormat { .. }
                ))
        ))
    }
}
pub(crate) fn fuse<'p>(
    function: &crate::hir::FunctionExecutionId,
    path: impl Into<crate::hir::ExprPath<'p>>,
    span: crate::ast::Span,
    op: crate::byte_ops::ByteOp,
    args: &mut Vec<ResolvedExpr>,
) -> Option<ResolvedExpr> {
    if op != crate::byte_ops::ByteOp::StrAsBytes
        || args.len() != 1
        || !args
            .first()
            .is_some_and(|argument| inspect(argument).is_some_and(|slot| !slot.bytes))
    {
        return None;
    }
    let argument = args.pop()?;
    let ResolvedExprKind::Call { args, .. } = argument.kind else {
        return None;
    };
    Some(ResolvedExpr {
        id: path.into().expression_id(function),
        ty: ResolvedType::SliceU8,
        ownership: OwnershipMode::Borrow,
        kind: ResolvedExprKind::Call {
            callee: crate::hir::DeclarationId::new(BYTES_ID),
            type_arguments: vec![ResolvedType::String],
            instance: None,
            args,
        },
        span,
    })
}

pub(crate) fn place(expression: &ResolvedExpr) -> Option<&crate::hir::Place> {
    let slot = inspect(expression)?;
    match &slot.source.kind {
        ResolvedExprKind::Place(place) => Some(place),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
