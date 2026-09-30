//! Compiler-backed consuming initializer proof. No Agent binding or journal.
use super::CheckedOwnedFrameHelperV2;
use crate::cleanup_plan::{owned_record_transfer_plan, OwnedRecordTransferPlan};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedType,
};

#[derive(Clone)]
pub(crate) struct CheckedOwnedInitializeV2 {
    helper: CheckedOwnedFrameHelperV2,
    function: DeclarationId,
    transfers: OwnedRecordTransferPlan,
}
impl CheckedOwnedInitializeV2 {
    pub(crate) fn helper(&self) -> &CheckedOwnedFrameHelperV2 {
        &self.helper
    }
    pub(crate) fn function(&self) -> &ResolvedFunction {
        self.helper
            .program()
            .functions
            .iter()
            .find(|f| f.id == self.function)
            .expect("checked initializer")
    }
    pub(crate) fn transfers(&self) -> &OwnedRecordTransferPlan {
        &self.transfers
    }
    pub(crate) fn constructor(&self) -> &ResolvedExpr {
        let ResolvedExprKind::Block { tail, .. } = &self.function().body.kind else {
            unreachable!()
        };
        tail
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io(
        "SPX-T303",
        "owned initializer outside checked transfer profile",
    )
}
pub(crate) fn compile_owned_initialize_v2(
    helper: &CheckedOwnedFrameHelperV2,
    id: &DeclarationId,
) -> Result<CheckedOwnedInitializeV2, Diagnostic> {
    let program = helper.program();
    hir::validate(program)?;
    let f = program
        .functions
        .iter()
        .find(|f| f.id == *id)
        .ok_or_else(refused)?;
    if !program
        .declarations
        .declaration(id)
        .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
        || f.params.len() != 1
        || f.params[0].ownership != OwnershipMode::Own
        || f.return_type != helper.function().params[0].ty
        || f.yields.is_some()
        || !f.effects.is_empty()
        || !f.loan_plan.loans.is_empty()
    {
        return Err(refused());
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &f.params[0].ty
    else {
        return Err(refused());
    };
    let fields = program
        .declarations
        .record_fields(declaration)
        .ok_or_else(refused)?;
    if !arguments.is_empty()
        || !(1..=8).contains(&fields.len())
        || !fields.iter().any(|v| v.ty == ResolvedType::Bytes)
        || fields
            .iter()
            .any(|v| v.ty != ResolvedType::Bytes && !hir::is_scalar_resolved_type(&v.ty))
    {
        return Err(refused());
    }
    let ResolvedExprKind::Block { statements, tail } = &f.body.kind else {
        return Err(refused());
    };
    if !statements.is_empty() {
        return Err(refused());
    }
    let ResolvedExprKind::ConstructRecord { fields, .. } = &tail.kind else {
        return Err(refused());
    };
    if fields
        .iter()
        .any(|v| v.value.ty != ResolvedType::Bytes && !copy(&v.value))
        || !f.requires.iter().chain(&f.ensures).all(copy)
    {
        return Err(refused());
    }
    let transfers = owned_record_transfer_plan(&program.declarations, f, tail)?;
    Ok(CheckedOwnedInitializeV2 {
        helper: helper.clone(),
        function: id.clone(),
        transfers,
    })
}
fn copy(expression: &ResolvedExpr) -> bool {
    crate::cleanup_plan::owned_frame_copy_expression(expression)
}
