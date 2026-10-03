//! Ownership-bearing and scoped borrowed Rust imports use the ordinary call protocol.
//! Scalar imports retain their existing cleanup projection.
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, FunctionInstanceId, OwnershipMode, ResolvedExpr, ResolvedExprKind,
    ResolvedParam, ResolvedProgram, ResolvedType, ValueId,
};

// A borrowed native target can refuse its dynamic loan guard or panic. Its
// status must select the canonical failure exit before any owner finalizer;
// the older scalar-only branch has no such operation-failure edge.
pub(super) fn owns(expression: &ResolvedExpr) -> bool {
    matches!(&expression.kind, ResolvedExprKind::NativeRustImportCall(call)
        if expression.ownership == OwnershipMode::Own
            || call.args.iter().any(|arg| matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)))
}

pub(super) fn params(
    program: &ResolvedProgram,
    callee: &DeclarationId,
) -> Result<Vec<ResolvedParam>, Diagnostic> {
    let import = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| import.native_rust && &import.id == callee)
        .ok_or_else(|| Diagnostic::io("SPX-H006", "unknown owned native Rust import"))?;
    Ok(import
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| ResolvedParam {
            id: ValueId::intrinsic_parameter(callee.as_str(), index),
            name: parameter.name.clone(),
            ownership: parameter.ownership,
            ty: parameter.ty.clone(),
            span: import.span,
        })
        .collect())
}

pub(super) struct CallParts<'a> {
    pub callee: &'a DeclarationId,
    pub instance: Option<&'a FunctionInstanceId>,
    pub args: &'a [ResolvedExpr],
    pub type_arguments: &'a [ResolvedType],
}

pub(super) fn parts(expression: &ResolvedExpr) -> Option<CallParts<'_>> {
    match &expression.kind {
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            type_arguments,
        } => Some(CallParts {
            callee,
            instance: instance.as_ref(),
            args,
            type_arguments,
        }),
        ResolvedExprKind::NativeRustImportCall(call) if owns(expression) => Some(CallParts {
            callee: &call.import,
            instance: None,
            args: &call.args,
            type_arguments: &[],
        }),
        _ => None,
    }
}

/// A declared native borrow observes a named String in place. Evaluating it as
/// an ordinary owned String expression would allocate an implicit clone.
pub(super) fn lends_string_place(
    call: &ResolvedExpr,
    argument: &ResolvedExpr,
    mode: OwnershipMode,
) -> bool {
    matches!(call.kind, ResolvedExprKind::NativeRustImportCall(_))
        && mode == OwnershipMode::Borrow
        && argument.ty == ResolvedType::String
        && argument.ownership == OwnershipMode::Own
        && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
}
