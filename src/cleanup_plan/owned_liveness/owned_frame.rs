//! Exact flat owned-record foundation. Legacy whole-storage queries are unchanged.
use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{CleanupPlace, CleanupTransition, FinalizeAction, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, DeclarationIndex, ExpressionId, OwnershipMode, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedParam, ResolvedStatement, ResolvedType,
};

pub(crate) fn owned_frame_parameter(
    declarations: &DeclarationIndex,
    params: &[ResolvedParam],
) -> bool {
    params.len() == 1 && owned_frame_root_parameter(declarations, &params[0])
}
pub(super) fn owned_frame_root_parameter(
    declarations: &DeclarationIndex,
    param: &ResolvedParam,
) -> bool {
    if param.ownership != OwnershipMode::Own {
        return false;
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &param.ty
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
    let Some(fields) = declarations.record_fields(declaration) else {
        return false;
    };
    !fields.is_empty()
        && fields.len() <= 8
        && fields.iter().any(|f| f.ty == ResolvedType::Bytes)
        && fields.iter().all(|f| {
            (f.ty == ResolvedType::Bytes || hir::is_scalar_resolved_type(&f.ty))
                && declarations
                    .declaration(&f.id)
                    .is_some_and(|d| d.identity_origin == hir::IdentityOrigin::Explicit)
        })
}

pub(crate) fn owned_frame_copy_expression(expr: &ResolvedExpr) -> bool {
    if !hir::is_scalar_resolved_type(&expr.ty)
        || matches!(
            expr.ownership,
            OwnershipMode::Borrow | OwnershipMode::Shared
        )
    {
        return false;
    }
    match &expr.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => true,
        ResolvedExprKind::Place(p) => p.projections.len() <= 1,
        ResolvedExprKind::Unary { value, .. } => owned_frame_copy_expression(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            owned_frame_copy_expression(left) && owned_frame_copy_expression(right)
        }
        _ => false,
    }
}

pub(crate) fn owned_frame_body(
    params: &[ResolvedParam],
    return_type: &ResolvedType,
    body: &ResolvedExpr,
) -> bool {
    if params.len() != 1 || *return_type != params[0].ty {
        return false;
    }
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    if !matches!(&tail.kind, ResolvedExprKind::Place(p) if p.root == params[0].id && p.projections.is_empty())
    {
        return false;
    }
    let mut count = 0;
    for statement in statements {
        let ResolvedStatement::Let { binding, value, .. } = statement else {
            return false;
        };
        if !hir::is_scalar_resolved_type(&binding.ty) || binding.ownership != OwnershipMode::Value {
            return false;
        }
        if let ResolvedExprKind::Yield { request } = &value.kind {
            count += 1;
            if !owned_frame_copy_expression(request) {
                return false;
            }
        } else if !owned_frame_copy_expression(value) {
            return false;
        }
    }
    count == 1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedFrameLeaf {
    pub field: DeclarationId,
    pub flag: LivenessFlagId,
    pub lifecycle: DeclarationId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedFrameLiveness {
    pub storage: StorageId,
    pub site: ExpressionId,
    pub leaves: Vec<OwnedFrameLeaf>,
    pub suspension_cleanup: Vec<FinalizeAction>,
    pub failure_cleanup: Vec<FinalizeAction>,
    pub completion_cleanup: Vec<FinalizeAction>,
    pub result_disposal: Vec<FinalizeAction>,
}
fn refused(reason: &str) -> Diagnostic {
    Diagnostic::io("SPX-T303", format!("owned frame profile: {reason}"))
}

pub(crate) fn owned_frame_liveness(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<OwnedFrameLiveness, Diagnostic> {
    if !owned_frame_parameter(declarations, &function.params)
        || !owned_frame_body(&function.params, &function.return_type, &function.body)
        || !function.effects.is_empty()
        || !function.loan_plan.loans.is_empty()
        || !function
            .requires
            .iter()
            .chain(&function.ensures)
            .all(owned_frame_copy_expression)
    {
        return Err(refused(
            "signature/body/contracts/loans are outside the exact profile",
        ));
    }
    let yields = function
        .yields
        .as_ref()
        .ok_or_else(|| refused("missing yields clause"))?;
    if !hir::is_scalar_resolved_type(&yields.request_type)
        || !hir::is_scalar_resolved_type(&yields.response_type)
    {
        return Err(refused("channel is not Copy scalar"));
    }
    owned_frame_root_liveness(declarations, function)
}

/// Shared compiler proof over the ACTUAL function/cleanup inventory. Profile
/// wrappers validate their own complete signature/body before calling this.
pub(super) fn owned_frame_root_liveness(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<OwnedFrameLiveness, Diagnostic> {
    let sites = super::direct_yield_sites(function);
    if sites.len() != 1 {
        return Err(refused("expected one direct site"));
    }
    let site = sites[0].clone();
    let plan = &function.cleanup_plan;
    let storage = StorageId::Value(function.params[0].id.clone());
    let slot = plan
        .slots
        .iter()
        .find(|s| s.storage == storage)
        .ok_or_else(|| refused("missing root slot"))?;
    if slot.ty != function.params[0].ty || !plan.entry_state.conditional_owned_parameters.is_empty()
    {
        return Err(refused("root slot or conditional entry differs"));
    }
    let ResolvedType::Nominal { declaration, .. } = &slot.ty else {
        return Err(refused("not nominal"));
    };
    let FieldLivenessShape::Record {
        declaration: shape_id,
        fields,
    } = &slot.field_liveness_shape
    else {
        return Err(refused("root lacks actual record liveness"));
    };
    let declared = declarations
        .record_fields(declaration)
        .ok_or_else(|| refused("missing fields"))?;
    if shape_id != declaration || fields.len() != declared.len() {
        return Err(refused("nominal or field count differs"));
    }
    let mut leaves = Vec::new();
    for (index, (field, expected)) in fields.iter().zip(declared).enumerate() {
        if field.field != expected.id || field.field_index != index as u32 {
            return Err(refused("field identity/order differs"));
        }
        match (&field.shape, &expected.ty) {
            (FieldLivenessShape::Leaf { flag, lifecycle }, ResolvedType::Bytes)
                if lifecycle.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID =>
            {
                leaves.push(OwnedFrameLeaf {
                    field: field.field.clone(),
                    flag: *flag,
                    lifecycle: lifecycle.clone(),
                })
            }
            (FieldLivenessShape::NoDrop, ty) if hir::is_scalar_resolved_type(ty) => {}
            _ => return Err(refused("leaf shape/lifecycle differs")),
        }
    }
    if leaves
        .iter()
        .enumerate()
        .any(|(i, l)| leaves[..i].iter().any(|p| p.flag == l.flag))
    {
        return Err(refused("aliased live flags"));
    }
    if plan.entry_state.live_owned_parameters
        != [CleanupPlace {
            storage: storage.clone(),
            projections: Vec::new(),
        }]
    {
        return Err(refused("entry whole root differs"));
    }
    let failure_vector = cleanup_vector(function, slot)?;
    let result_slot = plan
        .slots
        .iter()
        .find(|s| s.storage == StorageId::ProvisionalResult)
        .ok_or_else(|| refused("missing provisional root"))?;
    let FieldLivenessShape::Record {
        declaration: result_id,
        fields: result_fields,
    } = &result_slot.field_liveness_shape
    else {
        return Err(refused("provisional root lacks record metadata"));
    };
    if result_slot.ty != slot.ty || result_id != declaration || result_fields.len() != fields.len()
    {
        return Err(refused("provisional shape differs"));
    }
    for (index, (field, expected)) in result_fields.iter().zip(declared).enumerate() {
        if field.field != expected.id || field.field_index != index as u32 {
            return Err(refused("provisional field order differs"));
        }
        match (&field.shape, &expected.ty) {
            (FieldLivenessShape::Leaf { lifecycle, .. }, ResolvedType::Bytes)
                if lifecycle.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID => {}
            (FieldLivenessShape::NoDrop, ty) if hir::is_scalar_resolved_type(ty) => {}
            _ => return Err(refused("provisional leaf differs")),
        }
    }
    let result_disposal = cleanup_vector(function, result_slot)?;
    let prefix = super::locate_predecessors(function, &site)?;
    for block in &plan.blocks {
        for transition in &block.transitions {
            if super::transition_trigger(transition).is_some_and(|at| prefix.contains(at))
                && !matches!(
                    transition,
                    CleanupTransition::SelectFailure { .. }
                        | CleanupTransition::StageCopyResult { .. }
                )
            {
                return Err(refused("owned transition precedes suspension"));
            }
        }
    }
    let failure_cleanup = failure_vector;
    let mut completion_cleanup = None;
    for exit in &plan.exits {
        match &exit.continuation {
            crate::cleanup_plan::ExitContinuation::ReturnFailure { .. } => {
                let actions = &exit.finalize_in_order;
                if actions.len() == leaves.len()
                    && actions.iter().all(|a| a.source.storage == storage)
                {
                    for action in actions {
                        let leaf = leaves
                            .iter()
                            .find(|l| action.source.projections == [l.field.clone()])
                            .ok_or_else(|| refused("cleanup path differs"))?;
                        if action.guard_flag != leaf.flag
                            || action.lifecycle_id != leaf.lifecycle
                            || action.active_case.is_some()
                        {
                            return Err(refused("cleanup flag/lifecycle differs"));
                        }
                    }
                    if &failure_cleanup != actions {
                        return Err(refused("inconsistent root failure cleanup"));
                    }
                }
            }
            crate::cleanup_plan::ExitContinuation::CommitResult { .. } => {
                if !exit.finalize_in_order.is_empty() {
                    return Err(refused("completion cleans returned root"));
                }
                completion_cleanup = Some(exit.finalize_in_order.clone());
            }
            _ => {}
        }
    }
    let completion_cleanup =
        completion_cleanup.ok_or_else(|| refused("no canonical result transfer exit"))?;
    Ok(OwnedFrameLiveness {
        storage,
        site,
        leaves,
        suspension_cleanup: failure_cleanup.clone(),
        failure_cleanup,
        completion_cleanup,
        result_disposal,
    })
}

fn cleanup_vector(
    function: &ResolvedFunction,
    slot: &crate::cleanup_plan::CleanupSlot,
) -> Result<Vec<FinalizeAction>, Diagnostic> {
    let inventory_slot = function
        .cleanup
        .slots
        .get(slot.storage_index as usize)
        .ok_or_else(|| refused("inventory slot missing"))?;
    if !crate::cleanup::field_liveness_shapes_equal(
        &inventory_slot.shape,
        &slot.field_liveness_shape,
    )? {
        return Err(refused("inventory/plan shape disagreement"));
    }
    let flags: Vec<_> = function
        .cleanup
        .flags
        .iter()
        .filter(|f| f.place.storage == inventory_slot.id)
        .collect();
    let order: Vec<_> = flags.iter().map(|f| f.id).collect();
    Ok(crate::cleanup_plan::build::canonical_finalizers_for(
        &order,
        |id| {
            let flag = flags
                .iter()
                .find(|f| f.id == id)
                .expect("ordered compiler flag");
            (
                CleanupPlace {
                    storage: slot.storage.clone(),
                    projections: flag.place.projections.clone(),
                },
                flag.lifecycle.clone(),
            )
        },
        |_| true,
    ))
}
