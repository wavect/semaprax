//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive;
use crate::cleanup_plan::{CLEANUP_PLAN_SCHEMA_V18, CLEANUP_PLAN_SCHEMA_V19};
pub(in crate::cleanup_plan::replay) fn validate_place(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a CleanupPlace,
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a mut ReplayBudget,
        std::collections::btree_map::Values<'a, LivenessFlagId, super::super::Leaf>,
        Option<&'a super::super::Leaf>,
        &'a super::super::Leaf,
        usize,
        std::collections::btree_map::Iter<'a, LivenessFlagId, super::super::Leaf>,
        Option<(&'a LivenessFlagId, &'a super::super::Leaf)>,
        &'a LivenessFlagId,
        &'a super::super::Leaf,
        Vec<LivenessFlagId>,
        Result<Vec<LivenessFlagId>, Error>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_place local receiving",
    )?;
    budget.charge(function, 15, "selected validate_place local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<Vec<LivenessFlagId>>(function, budget)
    )?;
    if !known.contains(&place.storage) {
        return Err(permit.control(Reason::Finite(Kind::CleanupPlaceReferencesUnknownStorage)));
    }
    // Two full flag-order scans preserve the old count-then-publication order.
    // Their concrete iterator, next-result and binding roles are paid above.
    let mut count = 0;
    let mut leaf_values = leaves.values();
    while let Some(leaf) = dynamic_next!(
        budget,
        function,
        leaf_values,
        std::collections::btree_map::Values<'_, LivenessFlagId, super::super::Leaf>,
        &super::super::Leaf,
        (&super::super::Leaf),
        3
    ) {
        if leaf.place.storage == place.storage
            && leaf.place.projections.starts_with(&place.projections)
        {
            count += 1;
        }
    }
    if count == 0 {
        return Err(permit.control(Reason::Finite(Kind::CleanupPlaceHasNoDroppableLeaf)));
    }
    budget.reserve_array::<LivenessFlagId>(function, count, "entry place leaves")?;
    let mut result = Vec::with_capacity(count);
    let mut leaf_entries = leaves.iter();
    while let Some((flag, leaf)) = dynamic_next!(
        budget,
        function,
        leaf_entries,
        std::collections::btree_map::Iter<'_, LivenessFlagId, super::super::Leaf>,
        (&LivenessFlagId, &super::super::Leaf),
        (&LivenessFlagId, &super::super::Leaf),
        4
    ) {
        if leaf.place.storage == place.storage
            && leaf.place.projections.starts_with(&place.projections)
        {
            result.push(*flag);
        }
    }
    Ok(result)
}
pub(super) fn validate_binding_selected(
    function: &ResolvedFunction,
    at: &ExpressionId,
    place: &CleanupPlace,
    sites: &crate::cleanup_plan::ReplayRenewalBindings<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a ExpressionId,
        &'a CleanupPlace,
        &'a crate::cleanup_plan::ReplayRenewalBindings<'a>,
        &'a mut ReplayBudget,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_binding_selected local receiving",
    )?;
    budget.charge(
        function,
        5,
        "selected validate_binding_selected local receiving",
    )?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if !matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V18 | CLEANUP_PLAN_SCHEMA_V19
    ) || sites.is_helper(at) && function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V19
        || !receive!(
            budget,
            function,
            (
                Option<&crate::hir::ResolvedBinding>,
                &crate::hir::ResolvedBinding,
                bool
            ),
            3,
            "selected renewal binding query receiving",
            {
                match dynamic_get!(
                    budget,
                    function,
                    sites,
                    &crate::cleanup_plan::ReplayRenewalBindings<'_>,
                    at,
                    &ExpressionId,
                    Option<&crate::hir::ResolvedBinding>,
                    (&crate::hir::ResolvedBinding),
                    4
                ) {
                    Some(binding) => {
                        receive!(
                            budget,
                            function,
                            &crate::hir::ValueId,
                            1,
                            "selected renewal value binding receiving",
                            matches!(&place.storage, StorageId::Value(id) if id == &binding.id)
                        ) && place.projections.is_empty()
                    }
                    None => false,
                }
            }
        )
    {
        return Err(permit.control(Reason::Finite(
            Kind::RenewalIsOutsideAnExactAuthenticatedSameOwnerAssignment,
        )));
    }
    Ok(())
}

pub(super) fn validate_staged_target(
    function: &ResolvedFunction,
    source: &StagedCopyResultSource,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a StagedCopyResultSource,
        &'a mut ReplayBudget,
        &'a ResolvedType,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_staged_target local receiving",
    )?;
    budget.charge(
        function,
        4,
        "selected validate_staged_target local receiving",
    )?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let target = match source {
        StagedCopyResultSource::Body {
            expression,
            instance,
        } => {
            if expression != &function.body.id {
                return Err(permit.control(Reason::Finite(
                    Kind::BodyResultStagingReferencesAnotherExpression,
                )));
            }
            instance
        }
        StagedCopyResultSource::TryResidual {
            target_instance, ..
        }
        | StagedCopyResultSource::TryOptionNone {
            target_instance, ..
        } => target_instance,
    };
    if target != &function.return_type {
        return Err(permit.control(Reason::Finite(
            Kind::StagedCopyResultTargetsAnotherConcreteFunctionResult,
        )));
    }
    Ok(())
}
pub(super) fn validate_branch_pair(
    function: &ResolvedFunction,
    left: &EdgeCondition,
    right: &EdgeCondition,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a EdgeCondition,
        &'a EdgeCondition,
        &'a mut ReplayBudget,
        bool,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_branch_pair local receiving",
    )?;
    budget.charge(function, 5, "selected validate_branch_pair local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let valid = match (left, right) {
        (EdgeCondition::BooleanResult(a, av), EdgeCondition::BooleanResult(b, bv)) => {
            a == b && av != bv
        }
        (
            EdgeCondition::VariantCase {
                scrutinee: a_scrutinee,
                case: a_case,
                matches: a_matches,
            },
            EdgeCondition::VariantCase {
                scrutinee: b_scrutinee,
                case: b_case,
                matches: b_matches,
            },
        ) => a_scrutinee == b_scrutinee && a_case == b_case && a_matches != b_matches,
        (
            EdgeCondition::ArmSelected {
                scrutinee: a_scrutinee,
                arm: a_arm,
                selected: a_selected,
            },
            EdgeCondition::ArmSelected {
                scrutinee: b_scrutinee,
                arm: b_arm,
                selected: b_selected,
            },
        ) => a_scrutinee == b_scrutinee && a_arm == b_arm && a_selected != b_selected,
        (EdgeCondition::StatusZero(a), EdgeCondition::StatusNonzero(b))
        | (EdgeCondition::StatusNonzero(a), EdgeCondition::StatusZero(b)) => a == b,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(permit.control(Reason::Finite(
            Kind::CleanupBranchConditionsAreNotComplementary,
        )))
    }
}
pub(in crate::cleanup_plan::replay) fn finalizer_actions(
    function: &ResolvedFunction,
    actions: &[crate::cleanup_plan::FinalizeAction],
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a [crate::cleanup_plan::FinalizeAction],
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a mut ReplayBudget,
        BTreeSet<LivenessFlagId>,
        &'a crate::cleanup_plan::FinalizeAction,
        Option<(&'a LivenessFlagId, &'a super::super::Leaf)>,
        &'a LivenessFlagId,
        &'a super::super::Leaf,
        std::slice::Iter<'a, crate::cleanup_plan::FinalizeAction>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected finalizer_actions local receiving",
    )?;
    budget.charge(function, 11, "selected finalizer_actions local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    receive!(
        budget,
        function,
        Option<&crate::cleanup_plan::FinalizeAction>,
        1,
        "selected finalizer next action receiving",
        ()
    );
    let mut finalized = BTreeSet::new();
    let mut action_entries = actions.iter();
    while let Some(action) = dynamic_next!(
        budget,
        function,
        action_entries,
        std::slice::Iter<'_, crate::cleanup_plan::FinalizeAction>,
        &crate::cleanup_plan::FinalizeAction,
        (&crate::cleanup_plan::FinalizeAction),
        3
    ) {
        if !known.contains(&action.source.storage) {
            return Err(permit.control(Reason::Finite(Kind::CleanupPlaceReferencesUnknownStorage)));
        }
        receive!(
            budget,
            function,
            (
                std::collections::btree_map::Iter<'_, LivenessFlagId, super::super::Leaf>,
                Option<(&LivenessFlagId, &super::super::Leaf)>,
                &LivenessFlagId,
                &super::super::Leaf,
                Option<(&LivenessFlagId, &super::super::Leaf)>
            ),
            5,
            "selected finalizer leaf scan receiving",
            ()
        );
        let mut first = None;
        let mut second = None;
        let mut leaf_entries = leaves.iter();
        while let Some((candidate_flag, candidate_leaf)) = dynamic_next!(
            budget,
            function,
            leaf_entries,
            std::collections::btree_map::Iter<'_, LivenessFlagId, super::super::Leaf>,
            (&LivenessFlagId, &super::super::Leaf),
            (&LivenessFlagId, &super::super::Leaf),
            4
        ) {
            if candidate_leaf.place.storage == action.source.storage
                && candidate_leaf
                    .place
                    .projections
                    .starts_with(&action.source.projections)
            {
                if first.is_none() {
                    first = Some((candidate_flag, candidate_leaf));
                } else {
                    second = Some((candidate_flag, candidate_leaf));
                    break;
                }
            }
        }
        if first.is_none() {
            return Err(permit.control(Reason::Finite(Kind::CleanupPlaceHasNoDroppableLeaf)));
        }
        let (flag, leaf) = first.expect("nonempty borrowed leaf query");
        if second.is_some() || *flag != action.guard_flag || leaf.place != action.source {
            return Err(permit.control(Reason::Finite(
                Kind::FinalizerGuardDoesNotNameItsExactCleanupLeaf,
            )));
        }
        if leaf.lifecycle != action.lifecycle_id
            || !insert_copy(
                function,
                budget,
                &mut finalized,
                action.guard_flag,
                "lexical certificate set",
            )?
        {
            return Err(permit.control(Reason::Finite(
                Kind::FinalizerLifecycleOrGuardIsInvalidOrRepeated,
            )));
        }
        if let Some(condition) = &action.active_case {
            // Paying ordinary backing does not select the primitive profile.
            // Keep the original variant/path authentication without a cloned
            // leaf vector; insert_set above funds the only growing scratch.
            if budget.selected() {
                return Err(permit.control(Reason::Finite(
                    Kind::SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile,
                )));
            }
            if condition.storage != action.source.storage
                || action.source.projections.first() != Some(&condition.case)
                || !receive!(
                    budget,
                    function,
                    (
                        std::slice::Iter<'_, crate::cleanup_plan::CleanupSlot>,
                        Option<&crate::cleanup_plan::CleanupSlot>,
                        &crate::cleanup_plan::CleanupSlot,
                        bool,
                        bool,
                        &DeclarationId
                    ),
                    6,
                    "selected finalizer variant slot query receiving",
                    {
                        let mut agrees = false;
                        let mut slot_entries = function.cleanup_plan.slots.iter();
                        while let Some(slot) = dynamic_next!(
                            budget,
                            function,
                            slot_entries,
                            std::slice::Iter<'_, crate::cleanup_plan::CleanupSlot>,
                            &crate::cleanup_plan::CleanupSlot,
                            (&crate::cleanup_plan::CleanupSlot),
                            3
                        ) {
                            if slot.storage == condition.storage {
                                agrees = matches!(&slot.ty, ResolvedType::Nominal { declaration, .. }
                                if declaration == &condition.variant);
                                break;
                            }
                        }
                        agrees
                    }
                )
            {
                return Err(permit.control(Reason::Finite(
                    Kind::ConditionalFinalizerDoesNotBindItsExactVariantCasePath,
                )));
            }
        }
    }
    Ok(())
}
pub(super) fn require_expression(
    expressions: &dyn expression_facts::Lookup,
    function: &ResolvedFunction,
    key: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn expression_facts::Lookup,
        &'a ResolvedFunction,
        &'a ExpressionId,
        &'a mut ReplayBudget,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected require_expression local receiving",
    )?;
    budget.charge(function, 4, "selected require_expression local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn expression_facts::Lookup,
            &ResolvedFunction,
            &ExpressionId,
            &mut ReplayBudget,
            Result<Option<&Option<CallFact<'_>>>, Diagnostic>
        ),
        5,
        "selected control expressions fact call receiving",
        expressions.fact(function, key, budget)
    )?
    .is_some()
    {
        Ok(())
    } else {
        Err(permit.control(Reason::UnknownExpression(key.clone())))
    }
}

pub(super) fn index(
    function: &ResolvedFunction,
    index: usize,
    kind: Kind,
    budget: &mut ReplayBudget,
) -> Result<u32, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (&'a ResolvedFunction, usize, Kind, &'a mut ReplayBudget);
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected index local receiving")?;
    budget.charge(function, 4, "selected index local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<u32>(function, budget)
    )?;
    // Actual changed closure capture bank, distinct from the helper's Kind
    // argument and terminal receipt roles; acquired before closure formation.
    budget.reserve_array::<(
        &Permit<'_>,
        &Kind,
        std::num::TryFromIntError,
        Result<u32, Error>,
    )>(function, 1, "selected control index closure receiving")?;
    budget.charge(function, 4, "selected control index closure receiving")?;
    u32::try_from(index).map_err(|_| permit.control(Reason::Finite(kind)))
}
