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
    if let ResolvedExprKind::Match { mode, .. } = &expression.kind {
        if hir::generic_variant::match_result(
            program,
            function,
            *mode,
            &expression.ty,
            expression.ownership,
        ) && super::variant_pattern_is_admitted(
            &program.declarations,
            *mode,
            &scrutinee.ty,
            arms,
        ) {
            return true;
        }
    }
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
        && expression.ty == arm.value.ty
        && expression.ownership == OwnershipMode::Own
        && scrutinee.ownership == OwnershipMode::Own
        && arm.value.ownership == OwnershipMode::Own
        && instance == &scrutinee.ty
        && matches!(&scrutinee.ty, ResolvedType::Nominal { declaration, .. }
            if declaration == record)
        && crate::hir::is_admitted_nested_owned_byte_record(&program.declarations, &expression.ty)
}
