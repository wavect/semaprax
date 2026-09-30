//! Consuming reducer proof from the real checked source Agent association.
//! Step mappings are borrowed from its existing checked lifecycle admission.
use super::{CheckedOwnedAgentWaitBindingV8, CheckedOwnedFrameHelperV2};
use crate::cleanup_plan::{owned_step_transfer_plan, OwnedStepTransferPlan};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedType,
};

#[derive(Clone)]
pub(crate) struct OwnedStepMappingV2 {
    pub case: DeclarationId,
    pub role: &'static str,
    pub target: DeclarationId,
    pub fields: Vec<(DeclarationId, DeclarationId)>,
}
#[derive(Clone)]
pub(crate) struct CheckedOwnedReduceV2 {
    helper: CheckedOwnedFrameHelperV2,
    function: DeclarationId,
    binding: String,
    mappings: Vec<OwnedStepMappingV2>,
    transfers: OwnedStepTransferPlan,
}
impl CheckedOwnedReduceV2 {
    pub(crate) fn helper(&self) -> &CheckedOwnedFrameHelperV2 {
        &self.helper
    }
    pub(crate) fn function(&self) -> &ResolvedFunction {
        self.helper
            .program()
            .functions
            .iter()
            .find(|f| f.id == self.function)
            .expect("checked reducer")
    }
    pub(crate) fn binding(&self) -> &str {
        &self.binding
    }
    pub(crate) fn mappings(&self) -> &[OwnedStepMappingV2] {
        &self.mappings
    }
    pub(crate) fn transfers(&self) -> &OwnedStepTransferPlan {
        &self.transfers
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io(
        "SPX-T303",
        "owned reducer outside checked source Agent transfer profile",
    )
}
pub(crate) fn compile_owned_reduce_v2(
    binding: &CheckedOwnedAgentWaitBindingV8,
) -> Result<CheckedOwnedReduceV2, Diagnostic> {
    let helper = binding.helper();
    let program = helper.program();
    hir::validate(program)?;
    let agent = program
        .agents
        .iter()
        .find(|a| a.stable_id == *binding.agent())
        .ok_or_else(refused)?;
    let role = agent
        .operations
        .iter()
        .find(|o| {
            o.role == hir::ResolvedAgentOperationRoleKind::Reduce
                && o.kind == hir::ResolvedAgentOperationKind::Deterministic
        })
        .ok_or_else(refused)?;
    let outcome = agent
        .types
        .iter()
        .find(|t| t.role == hir::ResolvedAgentTypeRoleKind::Outcome)
        .ok_or_else(refused)?;
    let f = program
        .functions
        .iter()
        .find(|f| f.id == role.stable_id)
        .ok_or_else(refused)?;
    let yields = helper.function().yields.as_ref().expect("checked helper");
    let proposal = program
        .declarations
        .record_fields(yields.response_type.nominal_id().ok_or_else(refused)?)
        .ok_or_else(refused)?;
    let step = binding.lifecycle().owned_wait_step_v8();
    if f.params.len() != proposal.len() + 2
        || f.params[0].ownership != OwnershipMode::Own
        || f.params[0].ty != helper.function().params[0].ty
        || f.params.last().is_none_or(|p| {
            p.ownership != OwnershipMode::Own || p.ty.nominal_id() != Some(&outcome.stable_id)
        })
        || f.return_type.nominal_id() != Some(step.id)
        || !f.effects.is_empty()
        || f.yields.is_some()
        || !f.loan_plan.loans.is_empty()
        || f.requires.iter().chain(&f.ensures).any(|e| !copy(e))
    {
        return Err(refused());
    }
    for (p, field) in f.params[1..f.params.len() - 1].iter().zip(proposal) {
        if p.ownership != OwnershipMode::Value
            || p.ty != field.ty
            || !hir::is_scalar_resolved_type(&p.ty)
        {
            return Err(refused());
        }
    }
    let Some(fields) = program.declarations.record_fields(&outcome.stable_id) else {
        return Err(refused());
    };
    if fields.is_empty()
        || fields.len() > 8
        || !fields.iter().any(|f| f.ty == ResolvedType::Bytes)
        || fields
            .iter()
            .any(|f| f.ty != ResolvedType::Bytes && !hir::is_scalar_resolved_type(&f.ty))
    {
        return Err(refused());
    }
    let mut constructors = Vec::new();
    body(&f.body, &mut constructors)?;
    let transfers = owned_step_transfer_plan(&program.declarations, f, &constructors)?;
    let mappings = step
        .cases()
        .map(|case| OwnedStepMappingV2 {
            case: case.id.clone(),
            role: case.role,
            target: if case.role == "Complete" {
                step.result.clone()
            } else {
                step.state.clone()
            },
            fields: case.fields.to_vec(),
        })
        .collect::<Vec<_>>();
    if constructors.iter().any(|e| match &e.kind {
        ResolvedExprKind::ConstructVariant { case, fields, .. } => {
            mappings.iter().find(|m| m.case == *case).is_none_or(|m| {
                fields.len() != m.fields.len()
                    || fields
                        .iter()
                        .any(|f| !m.fields.iter().any(|(id, _)| f.field == *id))
            })
        }
        _ => true,
    }) {
        return Err(refused());
    }
    Ok(CheckedOwnedReduceV2 {
        helper: helper.clone(),
        function: f.id.clone(),
        binding: binding.binding().to_owned(),
        mappings,
        transfers,
    })
}
fn copy(e: &ResolvedExpr) -> bool {
    crate::cleanup_plan::owned_frame_copy_expression(e)
}
fn body<'a>(
    e: &'a ResolvedExpr,
    constructors: &mut Vec<&'a ResolvedExpr>,
) -> Result<(), Diagnostic> {
    match &e.kind {
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
            body(tail, constructors)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } if copy(condition) => {
            body(then_branch, constructors)?;
            body(else_branch, constructors)
        }
        ResolvedExprKind::ConstructVariant { fields, .. }
            if fields
                .iter()
                .all(|f| f.value.ty == ResolvedType::Bytes || copy(&f.value)) =>
        {
            constructors.push(e);
            Ok(())
        }
        _ => Err(refused()),
    }
}

#[cfg(test)]
mod tests;
