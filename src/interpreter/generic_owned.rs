//! Interpreter admission for bounded generic owned-record expressions.

use crate::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedMatchArm,
    ResolvedMatchPattern, ResolvedProgram, ResolvedType,
};

pub(super) fn match_result_is_admitted(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
) -> bool {
    if hir::bounded_owned_record_template_for_function(program, function).is_none() {
        return false;
    }
    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
        return false;
    };
    let [arm] = arms else { return false };
    let ResolvedMatchPattern::Record {
        record, instance, ..
    } = &arm.pattern
    else {
        return false;
    };
    *mode == hir::ResolvedMatchMode::Own
        && function.return_type == expression.ty
        && expression.ty == scrutinee.ty
        && expression.ty == arm.value.ty
        && expression.ownership == OwnershipMode::Own
        && scrutinee.ownership == OwnershipMode::Own
        && arm.value.ownership == OwnershipMode::Own
        && instance == &expression.ty
        && matches!(&expression.ty, ResolvedType::Nominal { declaration, .. }
            if declaration == record)
        && hir::is_flat_owned_byte_record(&program.declarations, &expression.ty)
}
