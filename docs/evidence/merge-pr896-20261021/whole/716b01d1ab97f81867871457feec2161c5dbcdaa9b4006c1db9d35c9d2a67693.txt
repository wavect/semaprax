//! Exact selected lexical state obligations; ordinary owner remains intact.
use super::*;
use crate::hir::{ControlTarget, LoopLatchKind};
macro_rules! loop_index {
    ($auth:ident,$target:ident,$function:ident,$budget:ident) => {{
        $budget.reserve_array::<(
            &lexical_cert::LexicalAuth,
            &ControlTarget,
            &ExpressionId,
            usize,
            Option<usize>,
        )>($function, 1, "selected path loop target query")?;
        $budget.charge($function, 5, "selected path loop target query")?;
        match $target {
            ControlTarget::Loop { site, .. } => dynamic_get!(
                $budget,$function,$auth.loops,&BTreeMap<ExpressionId,usize>,site,
                &ExpressionId,Option<&usize>,(&usize),4).copied(),
            ControlTarget::Function(_) => None,
        }
    }};
}
macro_rules! restores {
    ($entry:ident,$state:ident,$function:ident,$budget:ident) => {{
        $budget.reserve_array::<(&PathState, &PathState, bool)>(
            $function,
            1,
            "selected path loop restore query",
        )?;
        $budget.charge($function, 3, "selected path loop restore query")?;
        $entry.live_order == $state.live_order
            && $entry.conditional_variants == $state.conditional_variants
            && $entry.renewals == $state.renewals
    }};
}
pub(super) fn capture_loop_entry(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    condition: &EdgeCondition,
    state: &PathState,
    entries: &mut BTreeMap<usize, PathState>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &ExpressionId,
        usize,
        &crate::cleanup_plan::LoopCertificate,
        &PathState,
        PathState,
        &crate::cleanup_plan::LexicalControlPlan,
    )>(
        function,
        1,
        "selected path capture_loop_entry body receiving",
    )?;
    budget.charge(
        function,
        6,
        "selected path capture_loop_entry body receiving",
    )?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &LexicalAuth,
        &EdgeCondition,
        &PathState,
        &mut BTreeMap<usize, PathState>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path capture_loop_entry formal receiving",
    )?;
    budget.charge(
        function,
        7,
        "selected path capture_loop_entry formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let EdgeCondition::BooleanResult(expression, true) = condition else {
        return Ok(());
    };
    let Some(index) = receive!(
        budget,
        function,
        (Option<usize>, usize),
        2,
        "selected path copied loop index receiving",
        dynamic_get!(budget,function,auth.loops,&BTreeMap<ExpressionId,usize>,expression,
            &ExpressionId,Option<&usize>,(&usize),4)
        .copied()
    ) else {
        return Ok(());
    };
    let certificate = receive!(
        budget,
        function,
        (
            &Box<crate::cleanup_plan::LexicalControlPlan>,
            &crate::cleanup_plan::LexicalControlPlan,
            Option<&crate::cleanup_plan::LoopCertificate>
        ),
        3,
        "selected path loop certificate result receiving",
        match &function.cleanup_plan.lexical_control {
            Some(control) => dynamic_get!(
                budget,
                function,
                control.loops,
                &Vec<crate::cleanup_plan::LoopCertificate>,
                index,
                usize,
                Option<&crate::cleanup_plan::LoopCertificate>,
                (&crate::cleanup_plan::LoopCertificate),
                4
            ),
            None => None,
        }
    )
    .ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        &crate::cleanup_plan::LoopCertificate,
        Kind::LoopEntryHasNoCertificate
    ))?;
    if certificate.entry_order != state.live_order {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::LoopCertificateEntryOrderDiffersFromTheReplayedEntryOwnership,)
        ));
    }
    if let Some(previous) = dynamic_get!(budget,function,entries,&BTreeMap<usize,PathState>,&index,&usize,Option<&PathState>,(&PathState),4)
    {
        return if previous == state {
            Ok(())
        } else {
            Err(path_error!(
                permit,
                Reason::Finite(Kind::LoopIsEnteredWithTwoDifferentOwnershipStates,)
            ))
        };
    }
    budget.reserve_btree_node::<usize, PathState>(function, "lexical loop snapshot node")?;
    retained_child!(
        budget,
        function,
        StateClone,
        5,
        storage::reserve_state_clone(function, state, budget, "lexical loop snapshot")
    )?;
    map_insert!(
        budget,
        function,
        entries,
        index,
        clone_value!(budget, function, state, PathState),
        usize,
        PathState
    );
    Ok(())
}

/// A loop latch ends one iteration: it must restore the exact entry state.
pub(super) fn check_latch(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    exit: &ExitTarget,
    state: &PathState,
    entries: &BTreeMap<usize, PathState>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(&crate::hir::ControlTarget, &PathState, usize)>(
        function,
        1,
        "selected path check_latch body receiving",
    )?;
    budget.charge(function, 3, "selected path check_latch body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &LexicalAuth,
        &ExitTarget,
        &PathState,
        &BTreeMap<usize, PathState>,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path check_latch formal receiving")?;
    budget.charge(function, 7, "selected path check_latch formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let ExitContinuation::LoopLatch { target } = &exit.continuation else {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::LatchCheckOnANonLatchExit)
        ));
    };
    let entry = receive!(budget,function,(usize,Option<&PathState>),2,
        "selected path latch entry result receiving",match loop_index!(auth,target,function,budget) {
            Some(index)=>dynamic_get!(budget,function,entries,&BTreeMap<usize,PathState>,&index,&usize,Option<&PathState>,(&PathState),4),
            None=>None,
        })
        .ok_or_else(path_refusal!(
            budget,
            function,
            permit,
            &PathState,
            Kind::LoopLatchPrecedesItsLoopEntryState
        ))?;
    if restores!(entry, state, function, budget) {
        Ok(())
    } else {
        Err(path_error!(
            permit,
            Reason::Finite(Kind::LoopIterationDoesNotRestoreItsExactEntryOwnershipState,)
        ))
    }
}

/// A `break` of a counter or Copy loop joins the loop continuation, so its state
/// must equal the entry state exactly. An owning traversal enters its wrapper
/// fragment instead: the consumed hidden Step must be dead here, and the exact
/// parent join is checked where the fragment rejoins ordinary flow.
pub(super) fn check_break(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    exit: &ExitTarget,
    state: &PathState,
    entries: &BTreeMap<usize, PathState>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual persistent body roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &crate::hir::ControlTarget,
        usize,
        &crate::cleanup_plan::LoopCertificate,
        &crate::cleanup_plan::LexicalControlPlan,
        &crate::hir::ValueId,
        StorageId,
        bool,
        &StorageId,
        &PathState,
    )>(function, 1, "selected path check_break body receiving")?;
    budget.charge(function, 9, "selected path check_break body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        &LexicalAuth,
        &ExitTarget,
        &PathState,
        &BTreeMap<usize, PathState>,
        &Leaves,
        &mut ReplayBudget,
        Result<(), Error>,
    )>(function, 1, "selected path check_break formal receiving")?;
    budget.charge(function, 8, "selected path check_break formal receiving")?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let ExitContinuation::LoopBreak { target, .. } = &exit.continuation else {
        return Ok(());
    };
    let index = loop_index!(auth, target, function, budget).ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        usize,
        Kind::LoopBreakNamesAnUnknownLoop
    ))?;
    let certificate = receive!(
        budget,
        function,
        (
            &Box<crate::cleanup_plan::LexicalControlPlan>,
            &crate::cleanup_plan::LexicalControlPlan,
            Option<&crate::cleanup_plan::LoopCertificate>
        ),
        3,
        "selected path loop certificate result receiving",
        match &function.cleanup_plan.lexical_control {
            Some(control) => dynamic_get!(
                budget,
                function,
                control.loops,
                &Vec<crate::cleanup_plan::LoopCertificate>,
                index,
                usize,
                Option<&crate::cleanup_plan::LoopCertificate>,
                (&crate::cleanup_plan::LoopCertificate),
                4
            ),
            None => None,
        }
    )
    .ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        &crate::cleanup_plan::LoopCertificate,
        Kind::LoopBreakHasNoCertificate
    ))?;
    if let LoopLatchKind::OwnedIteratorStep { step, .. } = &certificate.latch {
        // The hidden Step was consumed entering Yield; it cannot reappear on a
        // break path merely to agree with the loop's false-condition branch.
        let step = StorageId::Value(clone_value!(budget, function, step, crate::hir::ValueId));
        receive!(
            budget,
            function,
            (bool, std::slice::Iter<'_, LivenessFlagId>),
            2,
            "selected path hidden step live scan receiving",
            ()
        );
        let mut live = false;
        let mut live_flags = state.live_order.iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            live_flags,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if receive!(
                budget,
                function,
                (&Leaves, &LivenessFlagId, &Leaf, bool),
                4,
                "selected path hidden step leaf receiving",
                leaf_at!(budget, function, leaves, flag).place.storage == step
            ) {
                live = true;
                break;
            }
        }
        if !live {
            receive!(
                budget,
                function,
                std::slice::Iter<'_, ReplayConditionalVariant>,
                1,
                "selected path hidden step variant iterator receiving",
                ()
            );
            let mut variants = state.conditional_variants.iter();
            while let Some(variant) = dynamic_next!(
                budget,
                function,
                variants,
                std::slice::Iter<'_, ReplayConditionalVariant>,
                &ReplayConditionalVariant,
                (&ReplayConditionalVariant),
                3
            ) {
                if receive!(
                    budget,
                    function,
                    bool,
                    1,
                    "selected path hidden step comparison receiving",
                    variant.root.storage == step
                ) {
                    live = true;
                    break;
                }
            }
        }
        if live {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::OwningBreakRetainsALiveHiddenStep)
            ));
        }
        return Ok(());
    }
    let entry = dynamic_get!(budget,function,entries,&BTreeMap<usize,PathState>,&index,&usize,Option<&PathState>,(&PathState),4).ok_or_else(path_refusal!(
        budget,
        function,
        permit,
        &PathState,
        Kind::LoopBreakPrecedesItsLoopEntryState
    ))?;
    if restores!(entry, state, function, budget) {
        Ok(())
    } else {
        Err(path_error!(
            permit,
            Reason::Finite(Kind::LoopBreakDoesNotRestoreItsExactSurroundingOwnershipState,)
        ))
    }
}
