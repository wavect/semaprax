//! Complete selected shape twin; retains all old payload debits.
use super::*;
use crate::cleanup::{FieldLiveness, FieldLivenessShape};
fn leaf(
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next: &mut u32,
    budget: &mut ReplayBudget,
) -> Result<FieldLivenessShape, EngineError> {
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<FieldLivenessShape>(function, budget)
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &ResolvedType,
        &mut u32,
        &mut ReplayBudget,
        crate::hir::CompilerDeclaration,
        Result<FieldLivenessShape, EngineError>,
    )>(function, 1, "selected leaf receiving")?;
    budget.charge(function, 6, "selected leaf receiving")?;
    if !super::queries::primitive_drop!(permit, function, budget, ty) {
        return Ok(FieldLivenessShape::NoDrop);
    }
    let lifecycle = match crate::cleanup::primitive_leaf_lifecycle_identity(ty) {
        Some(lifecycle) => lifecycle,
        None => return Err(permit.family(Reason::SelectedFieldHasNoPrimitiveLifecycle)),
    };
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &crate::hir::ResolvedFunction,
            crate::hir::CompilerDeclaration,
            &mut u32,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<
                crate::cleanup::FieldLivenessShape,
                crate::cleanup_plan::replay::expression_facts::errors::EngineError,
            >
        ),
        5,
        "selected lifecycle call receiving",
        lifecycle_leaf(function, lifecycle, next, budget)
    )
}
/// Lifecycles are fixed compiler identities and allocate no backing. The
/// original text term is still reserved through the replay budget's own
/// route, unchanged and whatever that budget's payer is, so caps and the
/// first-refusal order match the owned-text replay. It is conservative and
/// pays for no identity allocation.
fn lifecycle_leaf(
    function: &ResolvedFunction,
    lifecycle: crate::hir::CompilerDeclaration,
    next: &mut u32,
    budget: &mut ReplayBudget,
) -> Result<FieldLivenessShape, EngineError> {
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<FieldLivenessShape>(function, budget)
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        crate::hir::CompilerDeclaration,
        &mut u32,
        &mut ReplayBudget,
        LivenessFlagId,
        Result<FieldLivenessShape, EngineError>,
    )>(function, 1, "selected lifecycle receiving")?;
    budget.charge(function, 6, "selected lifecycle receiving")?;
    if *next as usize >= crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
        return Err(permit.family(Reason::SelectedFieldFlagsExceedTheOwnedLeafLimit));
    }
    let flag = LivenessFlagId(*next);
    *next = match next.checked_add(1) {
        Some(next) => next,
        None => return Err(permit.family(Reason::SelectedFieldFlagsOverflowed)),
    };
    budget.reserve_bytes(function, lifecycle.text().len(), "selected field lifecycle")?;
    Ok(FieldLivenessShape::Leaf {
        flag,
        lifecycle: DeclarationId::compiler(lifecycle),
    })
}
pub(super) fn shape(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next: &mut u32,
    budget: &mut ReplayBudget,
) -> Result<FieldLivenessShape, EngineError> {
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<FieldLivenessShape>(function, budget)
    )?;
    budget.reserve_array::<(
        &dyn ReplayContext,
        &ResolvedFunction,
        &ResolvedType,
        &mut u32,
        &mut ReplayBudget,
        Vec<FieldLiveness>,
        &DeclarationId,
        &[crate::hir::ResolvedFieldDeclaration],
        Result<FieldLivenessShape, EngineError>,
    )>(function, 1, "selected shape receiving")?;
    budget.charge(function, 9, "selected shape receiving")?;
    if matches!(ty, ResolvedType::Nominal { .. }) {
        selected_profile::paid_children::prepay(
            program,
            function,
            selected_profile::paid_children::type_key_bytes(ty, function, budget)?,
            budget,
        )?;
    }
    if super::queries::vector_element!(permit, function, budget, program, ty).is_some() {
        return crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &crate::hir::ResolvedFunction,
                crate::hir::CompilerDeclaration,
                &mut u32,
                &mut crate::cleanup_plan::replay::ReplayBudget,
                Result<
                    crate::cleanup::FieldLivenessShape,
                    crate::cleanup_plan::replay::expression_facts::errors::EngineError,
                >
            ),
            5,
            "selected lifecycle call receiving",
            lifecycle_leaf(
                function,
                crate::hir::CompilerDeclaration::VecDrop,
                next,
                budget,
            )
        );
    }
    if let Some((owner, fields)) =
        super::queries::flat_fields!(permit, function, budget, program, ty)
    {
        if !fields
            .iter()
            .any(|field| matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
        {
            return Ok(FieldLivenessShape::NoDrop);
        }
        let mut shapes = Vec::new();
        budget.reserve_vec(
            function,
            &mut shapes,
            fields.len(),
            "selected whole record fields",
        )?;
        for field in fields {
            budget.reserve_clone(function, &field.id, "selected record field identity")?;
            shapes.push(FieldLiveness {
                field: field.id.clone(),
                field_index: field.index,
                shape: crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &crate::hir::ResolvedFunction,
                        &crate::hir::ResolvedType,
                        &mut u32,
                        &mut crate::cleanup_plan::replay::ReplayBudget,
                        Result<
                            crate::cleanup::FieldLivenessShape,
                            crate::cleanup_plan::replay::expression_facts::errors::EngineError,
                        >
                    ),
                    5,
                    "selected leaf call receiving",
                    leaf(function, &field.ty, next, budget)
                )?,
            });
        }
        budget.reserve_clone(function, owner, "selected record declaration identity")?;
        return Ok(FieldLivenessShape::Record {
            declaration: owner.clone(),
            fields: shapes,
        });
    }
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &crate::hir::ResolvedFunction,
            &crate::hir::ResolvedType,
            &mut u32,
            &mut crate::cleanup_plan::replay::ReplayBudget,
            Result<
                crate::cleanup::FieldLivenessShape,
                crate::cleanup_plan::replay::expression_facts::errors::EngineError,
            >
        ),
        5,
        "selected leaf call receiving",
        leaf(function, ty, next, budget)
    )
}
