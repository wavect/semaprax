//! Exact selected supplemental walk; original walk remains independently compiled.
use super::*;
fn parameter<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    call: &'a ResolvedExpr,
    index: usize,
    budget: &mut ReplayBudget,
) -> Result<Option<(OwnershipMode, &'a ResolvedType)>, EngineError> {
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<Option<(OwnershipMode, &ResolvedType)>>(function, budget)
    )?;
    budget.reserve_array::<(
        &dyn ReplayContext,
        &ResolvedFunction,
        &ResolvedExpr,
        usize,
        &mut ReplayBudget,
        Option<(OwnershipMode, &ResolvedType)>,
        &ResolvedFunction,
        Result<Option<(OwnershipMode, &ResolvedType)>, EngineError>,
    )>(function, 1, "selected parameter receiving")?;
    budget.charge(function, 8, "selected parameter receiving")?;
    let ResolvedExprKind::Call {
        callee,
        args,
        type_arguments,
        ..
    } = &call.kind
    else {
        return Err(permit.family(Reason::SelectedParameterReceiverHasNoCall));
    };
    if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
        let [element] = type_arguments.as_slice() else {
            return Err(permit.family(Reason::SelectedVecParameterLacksExactElement));
        };
        let owned = *element == ResolvedType::Bytes
            || *element == ResolvedType::String
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
        return Ok(args.get(index).map(|arg| {
            (
                if owned {
                    op.param_ownership_for(index, element)
                } else {
                    op.param_ownership(index)
                },
                &arg.ty,
            )
        }));
    }
    if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
        return Ok(op
            .param_types()
            .get(index)
            .map(|ty| (op.param_ownership(index), ty)));
    }
    if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
        return Ok(op
            .param_types()
            .get(index)
            .map(|ty| (op.param_ownership(index), ty)));
    }
    if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
        return Ok(op.param_types().get(index).map(|ty| {
            (
                if *ty == ResolvedType::Str {
                    OwnershipMode::Borrow
                } else {
                    OwnershipMode::Value
                },
                ty,
            )
        }));
    }
    let target = match program.resolve_call_target(callee, None) {
        Some(target) => target,
        None => return Err(permit.family(Reason::SelectedSupplementalCallHasNoAuthenticatedBody)),
    };
    Ok(target.params.get(index).map(|p| (p.ownership, &p.ty)))
}

pub(super) fn collect_supplemental(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    next_flag: &mut u32,
    slots: &mut Vec<ExpectedSupplementalSlot>,
    budget: &mut ReplayBudget,
) -> Result<(), EngineError> {
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected family call receiving",
        Permit::family_receiver::<()>(function, budget)
    )?;
    budget.reserve_array::<(
        &dyn ReplayContext,
        &ResolvedFunction,
        &ResolvedExpr,
        &mut u32,
        &mut Vec<ExpectedSupplementalSlot>,
        &mut ReplayBudget,
        OwnershipMode,
        &ResolvedType,
        &ResolvedExpr,
        usize,
        Result<(), EngineError>,
    )>(function, 1, "selected supplemental argument receiving")?;
    budget.charge(function, 11, "selected supplemental argument receiving")?;
    enum Frame<'a> {
        Expr(expression_children::ReplayChildCursor<'a>, usize),
        Argument(&'a ResolvedExpr, usize),
    }
    budget.reserve_array::<(Vec<Frame<'_>>, Option<Frame<'_>>, Frame<'_>, usize)>(
        function,
        1,
        "selected supplemental receiving lifetime",
    )?;
    budget.charge(function, 4, "selected supplemental receiving lifetime")?;
    budget.reserve_array::<Frame<'_>>(function, 1028, "supplemental expression cursor")?;
    let mut frames = Vec::with_capacity(1028);
    if frames.capacity() != 1028 {
        return Err(
            permit.family(Reason::SupplementalExpressionCursorCapacityDisagreesWithReservation)
        );
    }
    frames.push(Frame::Expr(
        expression_children::ReplayChildCursor::new(expression),
        0,
    ));
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(mut cursor, next) => {
                let expression = cursor.expression();
                if let ResolvedExprKind::Call {
                    callee,
                    instance,
                    type_arguments,
                    args,
                } = &expression.kind
                {
                    if instance.is_some()
                        || (!type_arguments.is_empty()
                            && crate::vec_ops::by_id(callee.as_str()).is_none())
                    {
                        return Err(permit.family(
                            Reason::SelectedSupplementalCallDoesNotSupportGenericInstances,
                        ));
                    }
                    if crate::cleanup_plan::replay::inventory_coverage::paid::receive!(budget, function, (&dyn crate::cleanup::ReplayContext, &crate::hir::ResolvedFunction, &crate::hir::ResolvedExpr, usize, &mut crate::cleanup_plan::replay::ReplayBudget, Result<Option<(crate::hir::OwnershipMode, &crate::hir::ResolvedType)>, crate::cleanup_plan::replay::expression_facts::errors::EngineError>), 6, "selected parameter call receiving", parameter(program, function, expression, args.len(), budget))?.is_some()
                        || (!args.is_empty()
                            && crate::cleanup_plan::replay::inventory_coverage::paid::receive!(budget, function, (&dyn crate::cleanup::ReplayContext, &crate::hir::ResolvedFunction, &crate::hir::ResolvedExpr, usize, &mut crate::cleanup_plan::replay::ReplayBudget, Result<Option<(crate::hir::OwnershipMode, &crate::hir::ResolvedType)>, crate::cleanup_plan::replay::expression_facts::errors::EngineError>), 6, "selected parameter call receiving", parameter(program, function, expression, args.len() - 1, budget))?
                                .is_none())
                    {
                        return Err(
                            permit.family(Reason::SelectedSupplementalCallHasInconsistentArity)
                        );
                    }
                    if let Some(argument) = args.get(next) {
                        if frames.len() + 3 > frames.capacity() {
                            return Err(permit
                                .family(Reason::SupplementalSlotTraversalExceedsTheAdmittedDepth));
                        }
                        frames.push(Frame::Expr(cursor, next + 1));
                        frames.push(Frame::Argument(expression, next));
                        frames.push(Frame::Expr(
                            expression_children::ReplayChildCursor::new(argument),
                            0,
                        ));
                    }
                } else {
                    if matches!(
                        expression.kind,
                        ResolvedExprKind::Invoke { .. }
                            | ResolvedExprKind::Closure { .. }
                            | ResolvedExprKind::NativeRustImportCall(_)
                            | ResolvedExprKind::HostCommandCall(_)
                            | ResolvedExprKind::LiteralFormat { .. }
                    ) {
                        return Err(permit.family(
                            Reason::SelectedSupplementalInventoryDoesNotSupportDelegatedCalls,
                        ));
                    }
                    if let Some(child) = cursor.next() {
                        if frames.len() + 2 > frames.capacity() {
                            return Err(permit
                                .family(Reason::SupplementalSlotTraversalExceedsTheAdmittedDepth));
                        }
                        frames.push(Frame::Expr(cursor, next + 1));
                        frames.push(Frame::Expr(
                            expression_children::ReplayChildCursor::new(child),
                            0,
                        ));
                    }
                }
            }
            Frame::Argument(expression, index) => {
                let ResolvedExprKind::Call { args, .. } = &expression.kind else {
                    return Err(permit.family(Reason::SupplementalContinuationLacksACall));
                };
                let Some((ownership, ty)) = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &dyn crate::cleanup::ReplayContext,
                        &crate::hir::ResolvedFunction,
                        &crate::hir::ResolvedExpr,
                        usize,
                        &mut crate::cleanup_plan::replay::ReplayBudget,
                        Result<
                            Option<(crate::hir::OwnershipMode, &crate::hir::ResolvedType)>,
                            crate::cleanup_plan::replay::expression_facts::errors::EngineError,
                        >
                    ),
                    6,
                    "selected parameter call receiving",
                    parameter(program, function, expression, index, budget)
                )?
                else {
                    return Err(permit.family(Reason::SupplementalCallParameterIsMissing));
                };
                let argument = &args[index];
                if ownership == OwnershipMode::Own && matches!(ty, ResolvedType::Nominal { .. }) {
                    selected_profile::paid_children::prepay(
                        program,
                        function,
                        selected_profile::paid_children::type_key_bytes(ty, function, budget)?,
                        budget,
                    )?;
                }
                if ownership == OwnershipMode::Own
                    && super::queries::needs_drop_in!(permit, function, budget, program, ty)
                {
                    if ty != &argument.ty {
                        return Err(permit.family(
                            Reason::SupplementalOwnedArgumentTypeDisagreesWithItsParameter,
                        ));
                    }
                    let shape = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &dyn crate::cleanup::ReplayContext,
                            &crate::hir::ResolvedFunction,
                            &crate::hir::ResolvedType,
                            &mut u32,
                            &mut crate::cleanup_plan::replay::ReplayBudget,
                            Result<
                                crate::cleanup::FieldLivenessShape,
                                crate::cleanup_plan::replay::expression_facts::errors::EngineError,
                            >
                        ),
                        6,
                        "selected shape call receiving",
                        super::shape::shape(program, function, ty, next_flag, budget)
                    )?;
                    budget.reserve_clone(function, ty, "supplemental type backing")?;
                    budget.reserve_vec(function, slots, 1, "supplemental slot vector")?;
                    slots.push(ExpectedSupplementalSlot {
                        storage: StorageId::CallArgument {
                            call: expression.id.clone(),
                            parameter_index: match u32::try_from(index) {
                                Ok(index) => index,
                                Err(_) => return Err(permit.family(Reason::TooManyCallParameters)),
                            },
                            value_expression: argument.id.clone(),
                        },
                        ty: ty.clone(),
                        shape,
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
