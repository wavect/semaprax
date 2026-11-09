//! Paid same-box edge transport; the complete by-value receiver remains in finish.
use super::*;
pub(super) fn state_for_edge(
    function: &ResolvedFunction,
    mut state: Box<PathState>,
    condition: &EdgeCondition,
    contract_sources: &BTreeMap<ExpressionId, StatusSourceId>,
    budget: &mut ReplayBudget,
) -> Result<Box<PathState>, Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(&ExpressionId, &StatusSourceId, StatusSourceId)>(
        function,
        1,
        "selected path state_for_edge body receiving",
    )?;
    budget.charge(function, 3, "selected path state_for_edge body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        Box<PathState>,
        &EdgeCondition,
        &BTreeMap<ExpressionId, StatusSourceId>,
        &mut ReplayBudget,
        Result<Box<PathState>, Error>,
    )>(function, 1, "selected path state_for_edge formal receiving")?;
    budget.charge(function, 6, "selected path state_for_edge formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<Box<PathState>>(function, budget)
    )?;
    if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::ConditionalEdgeFollowsPendingSelectedOrPublishedState,)
        ));
    }
    match condition {
        EdgeCondition::Always => {}
        EdgeCondition::BooleanResult(expression, false) => {
            if let Some(source) = dynamic_get!(budget,function,contract_sources,&BTreeMap<ExpressionId,StatusSourceId>,expression,&ExpressionId,Option<&StatusSourceId>,(&StatusSourceId),4)
            {
                state.pending_failure =
                    Some(clone_value!(budget, function, source, StatusSourceId));
            }
        }
        EdgeCondition::BooleanResult(_, true)
        | EdgeCondition::VariantCase { .. }
        | EdgeCondition::ArmSelected { .. }
        | EdgeCondition::StatusZero(_) => {}
        EdgeCondition::StatusNonzero(source) => {
            state.pending_failure = Some(clone_value!(budget, function, source, StatusSourceId));
        }
    }
    Ok(state)
}

#[allow(
    clippy::borrowed_box,
    reason = "the singleton lends its existing Box and never reconstructs it"
)]
pub(super) fn validate_singleton(
    function: &ResolvedFunction,
    state: &Box<PathState>,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    budget.reserve_array::<(
        &Box<PathState>,
        std::iter::Once<&Box<PathState>>,
        BTreeSet<Box<PathState>>,
        std::collections::btree_set::Range<'_, Box<PathState>>,
    )>(function, 1, "compact singleton group body receiving")?;
    budget.charge(
        function,
        3,
        "selected path validate_control_group_with_budget body receiving",
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &Box<PathState>,
        BlockId,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "compact singleton group formal receiving")?;
    budget.charge(
        function,
        5,
        "selected path validate_control_group_with_budget formal receiving",
    )?;
    let absent = BTreeSet::<Box<PathState>>::new();
    let mut entries = std::iter::once(state);
    while let Some(state) = dynamic_next!(
        budget,
        function,
        entries,
        std::iter::Once<&Box<PathState>>,
        &Box<PathState>,
        (&Box<PathState>),
        3
    ) {
        path_child!(
            budget,
            function,
            ValidateJoinCompatibilityCall,
            6,
            renewal::validate_join_compatibility(
                function,
                absent.range::<Box<PathState>, _>(..state),
                state.as_ref(),
                block,
                budget
            )
        )?;
    }
    Ok(())
}
