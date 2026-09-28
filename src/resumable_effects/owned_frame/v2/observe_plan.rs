//! Inert checked read-only Observe proof; no Agent association or journal.
use super::CheckedOwnedFrameHelperV2;
use crate::cleanup_plan::ExitContinuation;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedStatement, ResolvedType,
};

#[derive(Clone)]
pub(crate) struct CheckedOwnedObserveV2 {
    helper: CheckedOwnedFrameHelperV2,
    function: DeclarationId,
}
impl CheckedOwnedObserveV2 {
    pub(crate) fn helper(&self) -> &CheckedOwnedFrameHelperV2 {
        &self.helper
    }
    pub(crate) fn function(&self) -> &ResolvedFunction {
        self.helper
            .program()
            .functions
            .iter()
            .find(|f| f.id == self.function)
            .expect("sealed Observe")
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io(
        "SPX-T303",
        "owned Observe outside checked read-only profile",
    )
}
pub(crate) fn compile_owned_observe_v2(
    helper: &CheckedOwnedFrameHelperV2,
    function: &DeclarationId,
) -> Result<CheckedOwnedObserveV2, Diagnostic> {
    let p = helper.program();
    hir::validate(p)?;
    let f = p
        .functions
        .iter()
        .find(|f| f.id == *function)
        .ok_or_else(refused)?;
    if !p
        .declarations
        .declaration(function)
        .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
        || f.params.len() != 1
        || f.params[0].ownership != OwnershipMode::Borrow
        || f.params[0].ty != helper.function().params[0].ty
        || f.return_type != helper.function().params[1].ty
        || f.yields.is_some()
        || !f.effects.is_empty()
        || !f
            .requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
            .all(|e| readonly(e, f, &p.declarations))
    {
        return Err(refused());
    }
    if f.loan_plan.loans.iter().any(|loan| {
        loan.origin.root != f.params[0].id
            || !bytes_projection(&loan.origin.projections, &f.params[0].ty, &p.declarations)
            || !matches!(
                loan.cause,
                crate::loan_plan::LoanCause::SliceView
                    | crate::loan_plan::LoanCause::BorrowedCall { argument: 0 }
            )
    }) {
        return Err(refused());
    }
    let mut commits = f
        .cleanup_plan
        .exits
        .iter()
        .filter(|e| matches!(e.continuation, ExitContinuation::CommitResult { .. }));
    if commits
        .next()
        .is_none_or(|e| !e.finalize_in_order.is_empty())
        || commits.next().is_some()
        || f.cleanup_plan
            .exits
            .iter()
            .any(|e| !e.finalize_in_order.is_empty())
    {
        return Err(refused());
    }
    Ok(CheckedOwnedObserveV2 {
        helper: helper.clone(),
        function: function.clone(),
    })
}
fn copy_or_view(ty: &ResolvedType, d: &hir::DeclarationIndex) -> bool {
    *ty == ResolvedType::SliceU8
        || hir::is_scalar_resolved_type(ty)
        || d.type_facts(ty)
            .is_some_and(|f| f.copy && f.sized && !f.contains_resource && !f.needs_drop)
}
fn bytes_projection(
    projections: &[hir::PlaceProjection],
    state: &ResolvedType,
    d: &hir::DeclarationIndex,
) -> bool {
    let [hir::PlaceProjection::Field(field)] = projections else {
        return false;
    };
    let ResolvedType::Nominal { declaration, .. } = state else {
        return false;
    };
    d.record_fields(declaration).is_some_and(|fields| {
        fields
            .iter()
            .any(|f| f.id == *field && f.ty == ResolvedType::Bytes)
    })
}
fn readonly(e: &ResolvedExpr, f: &ResolvedFunction, d: &hir::DeclarationIndex) -> bool {
    if !copy_or_view(&e.ty, d) {
        return false;
    }
    match &e.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => true,
        ResolvedExprKind::Place(p) => {
            p.root != f.params[0].id
                || (p.projections.len() == 1 && hir::is_scalar_resolved_type(&e.ty))
        }
        ResolvedExprKind::BorrowPlace { operation, place } => {
            operation.as_str() == crate::byte_ops::BYTES_AS_SLICE_ID
                && place.root == f.params[0].id
                && bytes_projection(&place.projections, &f.params[0].ty, d)
        }
        ResolvedExprKind::Unary { value, .. } => readonly(value, f, d),
        ResolvedExprKind::Binary { left, right, .. } => {
            readonly(left, f, d) && readonly(right, f, d)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            readonly(condition, f, d) && readonly(then_branch, f, d) && readonly(else_branch, f, d)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().all(|s| match s {
                ResolvedStatement::Let {
                    binding,
                    value,
                    mutable: false,
                    ..
                } => {
                    matches!(
                        binding.ownership,
                        OwnershipMode::Value | OwnershipMode::Borrow
                    ) && copy_or_view(&binding.ty, d)
                        && readonly(value, f, d)
                }
                _ => false,
            }) && readonly(tail, f, d)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().all(|v| readonly(&v.value, f, d))
        }
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            type_arguments.is_empty()
                && instance.is_none()
                && matches!(
                    callee.as_str(),
                    crate::byte_ops::LEN_ID | crate::byte_ops::GET_ID
                )
                && args.iter().all(|v| readonly(v, f, d))
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            readonly(scrutinee, f, d)
                && arms.iter().all(|arm| {
                    arm.guard.as_ref().is_none_or(|g| readonly(g, f, d))
                        && readonly(&arm.value, f, d)
                })
        }
        _ => false,
    }
}
