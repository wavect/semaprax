//! Separate two-parameter Copy-channel profile; no widening of v1 queries.
use super::owned_frame::{
    owned_frame_copy_expression, owned_frame_root_liveness, owned_frame_root_parameter,
    OwnedFrameLiveness,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationIndex, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedParam, ResolvedStatement, ResolvedType, ResolvedYieldsClause,
};

fn flat_copy_record(declarations: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty()
        || !declarations.declaration(declaration).is_some_and(|d| {
            d.kind == hir::DeclarationKind::Record
                && d.identity_origin == hir::IdentityOrigin::Explicit
        })
    {
        return false;
    }
    declarations
        .record_fields(declaration)
        .is_some_and(|fields| {
            !fields.is_empty()
                && fields.len() <= 8
                && fields.iter().all(|field| {
                    hir::is_scalar_resolved_type(&field.ty)
                        && declarations
                            .declaration(&field.id)
                            .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
                })
        })
}
pub(crate) fn owned_frame_v2_parameter(
    declarations: &DeclarationIndex,
    params: &[ResolvedParam],
) -> bool {
    params.len() == 2
        && owned_frame_root_parameter(declarations, &params[0])
        && params[1].ownership == OwnershipMode::Value
        && flat_copy_record(declarations, &params[1].ty)
}
pub(crate) fn owned_frame_v2_body(
    declarations: &DeclarationIndex,
    params: &[ResolvedParam],
    return_type: &ResolvedType,
    yields: &ResolvedYieldsClause,
    body: &ResolvedExpr,
) -> bool {
    if !owned_frame_v2_parameter(declarations, params)
        || *return_type != params[0].ty
        || yields.request_type != params[1].ty
        || !flat_copy_record(declarations, &yields.response_type)
    {
        return false;
    }
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    if statements.len() != 1
        || !matches!(&tail.kind,ResolvedExprKind::Place(p)
        if p.root==params[0].id && p.projections.is_empty())
    {
        return false;
    }
    let ResolvedStatement::Let { binding, value, .. } = &statements[0] else {
        return false;
    };
    if binding.ownership != OwnershipMode::Value || binding.ty != yields.response_type {
        return false;
    }
    let ResolvedExprKind::Yield { request } = &value.kind else {
        return false;
    };
    request.ty == yields.request_type
        && request.ownership == OwnershipMode::Value
        && matches!(&request.kind,ResolvedExprKind::Place(p)
            if p.root==params[1].id && p.projections.is_empty())
}
pub(crate) fn owned_frame_v2_liveness(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<OwnedFrameLiveness, Diagnostic> {
    let Some(yields) = &function.yields else {
        return Err(refused("missing yields"));
    };
    if !owned_frame_v2_body(
        declarations,
        &function.params,
        &function.return_type,
        yields,
        &function.body,
    ) || !function.effects.is_empty()
        || !function.loan_plan.loans.is_empty()
        || !function
            .requires
            .iter()
            .chain(&function.ensures)
            .all(owned_frame_copy_expression)
    {
        return Err(refused(
            "signature/body/contracts/loans outside exact profile",
        ));
    }
    // Signature-only resolution sees the yield's placeholder type. The final
    // query independently checks its rewritten response type on actual HIR.
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return Err(refused("body changed"));
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        return Err(refused("statement changed"));
    };
    if value.ty != yields.response_type
        || value.ownership != OwnershipMode::Value
        || tail.ty != function.return_type
        || tail.ownership != OwnershipMode::Own
    {
        return Err(refused("resolved yield/result type or ownership differs"));
    }
    owned_frame_root_liveness(declarations, function)
}
fn refused(reason: &str) -> Diagnostic {
    Diagnostic::io("SPX-T303", format!("owned frame v2 profile: {reason}"))
}
#[cfg(test)]
mod tests;
