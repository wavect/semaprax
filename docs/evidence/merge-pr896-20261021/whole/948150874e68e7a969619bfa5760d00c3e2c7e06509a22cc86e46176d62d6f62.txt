//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive;
pub(in crate::cleanup_plan::replay) fn validate_exits(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a BTreeSet<ExpressionId>,
        &'a mut ReplayBudget,
        &'a crate::cleanup_plan::CleanupPlan,
        BTreeSet<EdgeId>,
        usize,
        &'a ExitTarget,
        Option<CleanupRegionId>,
        &'a CleanupRegionId,
        &'a crate::cleanup_plan::CleanupRegion,
        &'a StatusSourceId,
        Option<&'a StatusSourceId>,
        std::iter::Enumerate<std::slice::Iter<'a, ExitTarget>>,
        std::slice::Iter<'a, crate::cleanup_plan::CleanupRegion>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_exits local receiving",
    )?;
    budget.charge(function, 17, "selected validate_exits local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let plan = &function.cleanup_plan;

    let mut continuation_edges = BTreeSet::new();
    let mut exit_entries = plan.exits.iter().enumerate();
    while let Some((index, exit)) = dynamic_next!(
        budget,
        function,
        exit_entries,
        std::iter::Enumerate<std::slice::Iter<'_, ExitTarget>>,
        (usize, &ExitTarget),
        (usize, &ExitTarget),
        4
    ) {
        budget.charge(
            function,
            exit.leaves_regions.len().saturating_add(1),
            "structural exit regions",
        )?;
        if plan.lexical_control.is_none()
            && matches!(
                exit.continuation,
                ExitContinuation::LoopLatch { .. }
                    | ExitContinuation::LoopBreak { .. }
                    | ExitContinuation::FunctionReturn { .. }
            )
        {
            return Err(permit.control(Reason::Finite(
                Kind::LexicalExitContinuationHasNoLexicalControlCertificate,
            )));
        }
        if exit.id.0
            != crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    usize,
                    Kind,
                    &mut ReplayBudget,
                    Result<u32, Error>
                ),
                5,
                "selected control index call receiving",
                helpers::index(function, index, Kind::TooManyCleanupExits, budget)
            )?
            || !block_exists(function, exit.from)
        {
            return Err(permit.control(Reason::Finite(
                Kind::CleanupExitHasANonContiguousIdOrUnknownSourceBlock,
            )));
        }
        if !crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &ExitTarget,
                &mut ReplayBudget,
                Result<bool, Error>
            ),
            4,
            "selected control regions_match call receiving",
            regions_match(function, exit, budget)
        )? {
            return Err(permit.control(Reason::Finite(
                Kind::CleanupExitDoesNotLeaveTheExactSourceToTargetRegionChain,
            )));
        }
        let mut prior: Option<super::CleanupRegionId> = None;
        let mut left_region_entries = exit.leaves_regions.iter();
        while let Some(region) = dynamic_next!(
            budget,
            function,
            left_region_entries,
            std::slice::Iter<'_, CleanupRegionId>,
            &CleanupRegionId,
            (&CleanupRegionId),
            3
        ) {
            let Some(item) = dynamic_get!(
                budget,
                function,
                plan.regions,
                &[crate::cleanup_plan::CleanupRegion],
                region.0 as usize,
                usize,
                Option<&crate::cleanup_plan::CleanupRegion>,
                (&crate::cleanup_plan::CleanupRegion),
                4
            ) else {
                return Err(
                    permit.control(Reason::Finite(Kind::CleanupExitReferencesUnknownRegion))
                );
            };
            if let Some(child) = prior {
                if plan.regions[child.0 as usize].parent != Some(*region) {
                    return Err(
                        permit.control(Reason::Finite(Kind::CleanupExitRegionsAreNotAParentChain))
                    );
                }
            }
            prior = Some(item.id);
        }
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &[crate::cleanup_plan::FinalizeAction],
                &BTreeSet<StorageId>,
                &Leaves,
                &mut ReplayBudget,
                Result<(), Error>
            ),
            6,
            "selected control finalizer_actions call receiving",
            helpers::finalizer_actions(function, &exit.finalize_in_order, storage, leaves, budget)
        )?;
        match &exit.continuation {
            ExitContinuation::Continue(edge) => {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        BlockId,
                        EdgeId,
                        &mut BTreeSet<EdgeId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    6,
                    "selected control owned_edge call receiving",
                    owned_edge(function, exit.from, *edge, &mut continuation_edges, budget)
                )?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(
                        permit.control(Reason::Finite(Kind::CleanupContinuationEdgeIsConditional))
                    );
                }
            }
            ExitContinuation::CommitResult { source } => {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &dyn crate::cleanup::ReplayContext,
                        &ResolvedFunction,
                        &CleanupResultSource,
                        &BTreeSet<StorageId>,
                        &Leaves,
                        &BTreeSet<ExpressionId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    8,
                    "selected control result_source call receiving",
                    result_source(
                        program,
                        function,
                        source,
                        storage,
                        leaves,
                        return_operands,
                        budget,
                    )
                )?
            }
            ExitContinuation::FunctionReturn { edge, source, .. } => {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        BlockId,
                        EdgeId,
                        &mut BTreeSet<EdgeId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    6,
                    "selected control owned_edge call receiving",
                    owned_edge(function, exit.from, *edge, &mut continuation_edges, budget)
                )?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(permit.control(Reason::Finite(
                        Kind::FunctionReturnContinuationEdgeIsConditional,
                    )));
                }
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &dyn crate::cleanup::ReplayContext,
                        &ResolvedFunction,
                        &CleanupResultSource,
                        &BTreeSet<StorageId>,
                        &Leaves,
                        &BTreeSet<ExpressionId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    8,
                    "selected control result_source call receiving",
                    result_source(
                        program,
                        function,
                        source,
                        storage,
                        leaves,
                        return_operands,
                        budget,
                    )
                )?
            }
            ExitContinuation::LoopBreak { edge, .. } => {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        BlockId,
                        EdgeId,
                        &mut BTreeSet<EdgeId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    6,
                    "selected control owned_edge call receiving",
                    owned_edge(function, exit.from, *edge, &mut continuation_edges, budget)
                )?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(permit
                        .control(Reason::Finite(Kind::LoopBreakContinuationEdgeIsConditional)));
                }
            }
            ExitContinuation::LoopLatch { .. } => {}
            ExitContinuation::ReturnFailure { source } => {
                budget.charge(
                    function,
                    plan.status_sources.len(),
                    "structural failure status lookup",
                )?;
                if !receive!(
                    budget,
                    function,
                    (
                        std::slice::Iter<'_, crate::cleanup_plan::StatusSource>,
                        Option<&crate::cleanup_plan::StatusSource>,
                        &crate::cleanup_plan::StatusSource,
                        bool,
                        bool
                    ),
                    5,
                    "selected exit status source scan receiving",
                    {
                        let mut found = false;
                        let mut status_entries = plan.status_sources.iter();
                        while let Some(candidate) = dynamic_next!(
                            budget,
                            function,
                            status_entries,
                            std::slice::Iter<'_, crate::cleanup_plan::StatusSource>,
                            &crate::cleanup_plan::StatusSource,
                            (&crate::cleanup_plan::StatusSource),
                            3
                        ) {
                            if &candidate.id == source {
                                found = true;
                                break;
                            }
                        }
                        found
                    }
                ) {
                    return Err(permit.control(Reason::Finite(
                        Kind::CleanupReferenceNamesAnUnknownStatusSource,
                    )));
                }
                budget.charge(
                    function,
                    plan.blocks[exit.from.0 as usize].transitions.len(),
                    "structural failure selection",
                )?;
                receive!(
                    budget,
                    function,
                    (
                        std::slice::Iter<'_, CleanupTransition>,
                        Option<&CleanupTransition>,
                        &StatusSourceId,
                        Option<&StatusSourceId>,
                        Option<&CleanupTransition>,
                        &StatusSourceId,
                        Option<&StatusSourceId>,
                        bool
                    ),
                    8,
                    "selected failure selection scan receiving",
                    ()
                );
                let mut selected = plan.blocks[exit.from.0 as usize].transitions.iter();
                let first = loop {
                    match dynamic_next!(
                        budget,
                        function,
                        selected,
                        std::slice::Iter<'_, CleanupTransition>,
                        &CleanupTransition,
                        (&StatusSourceId),
                        3
                    ) {
                        Some(CleanupTransition::SelectFailure { source }) => break Some(source),
                        Some(_) => {}
                        None => break None,
                    }
                };
                if first != Some(source) || {
                    let second = loop {
                        match dynamic_next!(
                            budget,
                            function,
                            selected,
                            std::slice::Iter<'_, CleanupTransition>,
                            &CleanupTransition,
                            (&StatusSourceId),
                            3
                        ) {
                            Some(CleanupTransition::SelectFailure { source }) => {
                                break Some(source)
                            }
                            Some(_) => {}
                            None => break None,
                        }
                    };
                    second.is_some()
                } {
                    return Err(permit.control(Reason::Finite(
                        Kind::FailureExitDoesNotReturnItsUniquelySelectedStatus,
                    )));
                }
            }
            ExitContinuation::ReturnUnit => {
                return Err(permit.control(Reason::Finite(
                    Kind::ReturnunitIsInvalidForCurrentSourceFunctionReturnTypes,
                )));
            }
        }
    }
    let mut region_entries = plan.regions.iter();
    while let Some(region) = dynamic_next!(
        budget,
        function,
        region_entries,
        std::slice::Iter<'_, crate::cleanup_plan::CleanupRegion>,
        &crate::cleanup_plan::CleanupRegion,
        (&crate::cleanup_plan::CleanupRegion),
        3
    ) {
        if plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END {
            // Authenticated as abrupt-only by the lexical certificate replay.
            continue;
        }
        let exit = &plan.exits[region.normal_scope_end.0 as usize];
        if !exit.leaves_regions.contains(&region.id) {
            return Err(permit.control(Reason::Finite(
                Kind::RegionNormalScopeExitDoesNotLeaveThatRegion,
            )));
        }
    }
    Ok(())
}

fn owned_edge(
    function: &ResolvedFunction,
    owner: BlockId,
    edge: EdgeId,
    referenced: &mut BTreeSet<EdgeId>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        BlockId,
        EdgeId,
        &'a mut BTreeSet<EdgeId>,
        &'a mut ReplayBudget,
        &'a crate::cleanup_plan::CleanupEdge,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected owned_edge local receiving")?;
    budget.charge(function, 6, "selected owned_edge local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let item = dynamic_get!(
        budget,
        function,
        function.cleanup_plan.edges,
        &[crate::cleanup_plan::CleanupEdge],
        edge.0 as usize,
        usize,
        Option<&crate::cleanup_plan::CleanupEdge>,
        (&crate::cleanup_plan::CleanupEdge),
        4
    )
    .ok_or_else(control_refusal!(
        budget,
        function,
        permit,
        &crate::cleanup_plan::CleanupEdge,
        Kind::CleanupTerminatorReferencesUnknownEdge
    ))?;
    if item.id != edge || item.from != owner {
        return Err(permit.control(Reason::Finite(
            Kind::CleanupEdgeHasTheWrongOwnerOrIsReferencedRepeatedly,
        )));
    }
    if !insert_copy(
        function,
        budget,
        referenced,
        edge,
        "structural continuation edges",
    )? {
        return Err(permit.control(Reason::Finite(
            Kind::CleanupEdgeHasTheWrongOwnerOrIsReferencedRepeatedly,
        )));
    }
    Ok(())
}

fn ancestor_contains(
    function: &ResolvedFunction,
    start: CleanupRegionId,
    wanted: CleanupRegionId,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        CleanupRegionId,
        CleanupRegionId,
        &'a mut ReplayBudget,
        Option<CleanupRegionId>,
        usize,
        bool,
        CleanupRegionId,
        &'a crate::cleanup_plan::CleanupRegion,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected ancestor_contains local receiving",
    )?;
    budget.charge(function, 9, "selected ancestor_contains local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<bool>(function, budget)
    )?;
    let mut current = Some(start);
    let mut remaining = function.cleanup_plan.regions.len();
    let mut found = false;
    while let Some(region) = current {
        budget.charge(function, 1, "structural target ancestry")?;
        remaining = remaining.checked_sub(1).ok_or_else(control_refusal!(
            budget,
            function,
            permit,
            usize,
            Kind::CleanupRegionTreeContainsACycle
        ))?;
        let item = dynamic_get!(
            budget,
            function,
            function.cleanup_plan.regions,
            &[crate::cleanup_plan::CleanupRegion],
            region.0 as usize,
            usize,
            Option<&crate::cleanup_plan::CleanupRegion>,
            (&crate::cleanup_plan::CleanupRegion),
            4
        )
        .ok_or_else(control_refusal!(
            budget,
            function,
            permit,
            &crate::cleanup_plan::CleanupRegion,
            Kind::CleanupRegionChainReferencesAnUnknownRegion
        ))?;
        found |= region == wanted;
        current = item.parent;
    }
    Ok(found)
}

/// Compare the complete exact leave prefix directly. No expected region chain
/// or ancestor tree is constructed; every current parent link is still checked.
pub(super) fn regions_match(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a ExitTarget,
        &'a mut ReplayBudget,
        &'a crate::cleanup_plan::CleanupPlan,
        CleanupRegionId,
        Option<CleanupRegionId>,
        Option<CleanupRegionId>,
        Option<CleanupRegionId>,
        usize,
        usize,
        bool,
        &'a crate::cleanup_plan::CleanupRegion,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected regions_match local receiving")?;
    budget.charge(function, 12, "selected regions_match local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<bool>(function, budget)
    )?;
    let plan = &function.cleanup_plan;
    let source = dynamic_get!(
        budget,
        function,
        plan.blocks,
        &[crate::cleanup_plan::CleanupBlock],
        exit.from.0 as usize,
        usize,
        Option<&crate::cleanup_plan::CleanupBlock>,
        (&crate::cleanup_plan::CleanupBlock),
        4
    )
    .ok_or_else(control_refusal!(
        budget,
        function,
        permit,
        &crate::cleanup_plan::CleanupBlock,
        Kind::CleanupExitHasAnUnknownSourceBlock
    ))?
    .region;
    let mut latch = None;
    let mut target = None;
    match &exit.continuation {
        ExitContinuation::LoopLatch {
            target: control_target,
        } => {
            if receive!(
                budget,
                function,
                (
                    Option<&crate::cleanup_plan::LexicalControlPlan>,
                    &crate::cleanup_plan::LexicalControlPlan,
                    std::slice::Iter<'_, crate::cleanup_plan::LoopCertificate>,
                    Option<&crate::cleanup_plan::LoopCertificate>,
                    &crate::cleanup_plan::LoopCertificate,
                    bool,
                    bool
                ),
                7,
                "selected latch owned iterator scan receiving",
                {
                    let mut owned = false;
                    if let Some(control) = plan.lexical_control.as_deref() {
                        let mut loop_entries = control.loops.iter();
                        while let Some(lp) = dynamic_next!(
                            budget,
                            function,
                            loop_entries,
                            std::slice::Iter<'_, crate::cleanup_plan::LoopCertificate>,
                            &crate::cleanup_plan::LoopCertificate,
                            (&crate::cleanup_plan::LoopCertificate),
                            3
                        ) {
                            if lp.target == *control_target
                                && matches!(
                                    &lp.latch,
                                    crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
                                )
                            {
                                owned = true;
                                break;
                            }
                        }
                    }
                    owned
                }
            ) {
                return Err(permit.control(Reason::Finite(
                    Kind::SelectedPrimitiveExitsDoNotAdmitOwnedIteratorScopes,
                )));
            }
            let region = receive!(
                budget,
                function,
                (
                    &crate::cleanup_plan::CleanupPlan,
                    &crate::hir::ControlTarget,
                    Option<CleanupRegionId>,
                    CleanupRegionId
                ),
                4,
                "selected loop region receiving",
                lexical_cert::loop_region(plan, control_target)
            )
            .ok_or_else(control_refusal!(
                budget,
                function,
                permit,
                CleanupRegionId,
                Kind::LoopLatchNamesAnUnknownLexicalLoop
            ))?;
            if !crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    CleanupRegionId,
                    CleanupRegionId,
                    &mut ReplayBudget,
                    Result<bool, Error>
                ),
                5,
                "selected control ancestor_contains call receiving",
                ancestor_contains(function, source, region, budget)
            )? {
                return Err(permit.control(Reason::Finite(
                    Kind::LoopLatchExitDoesNotOriginateInsideItsLoopRegion,
                )));
            }
            latch = Some(region);
        }
        ExitContinuation::Continue(edge)
        | ExitContinuation::LoopBreak { edge, .. }
        | ExitContinuation::FunctionReturn { edge, .. } => {
            target = Some(
                receive!(
                    budget,
                    function,
                    (
                        Option<&crate::cleanup_plan::CleanupEdge>,
                        &crate::cleanup_plan::CleanupEdge,
                        Option<&crate::cleanup_plan::CleanupBlock>
                    ),
                    3,
                    "selected continuation target query receiving",
                    {
                        match dynamic_get!(
                            budget,
                            function,
                            plan.edges,
                            &[crate::cleanup_plan::CleanupEdge],
                            edge.0 as usize,
                            usize,
                            Option<&crate::cleanup_plan::CleanupEdge>,
                            (&crate::cleanup_plan::CleanupEdge),
                            4
                        ) {
                            Some(edge) => dynamic_get!(
                                budget,
                                function,
                                plan.blocks,
                                &[crate::cleanup_plan::CleanupBlock],
                                edge.to.0 as usize,
                                usize,
                                Option<&crate::cleanup_plan::CleanupBlock>,
                                (&crate::cleanup_plan::CleanupBlock),
                                4
                            ),
                            None => None,
                        }
                    }
                )
                .ok_or_else(control_refusal!(
                    budget,
                    function,
                    permit,
                    &crate::cleanup_plan::CleanupBlock,
                    Kind::CleanupContinuationHasAnUnknownTarget
                ))?
                .region,
            );
        }
        ExitContinuation::CommitResult { .. }
        | ExitContinuation::ReturnFailure { .. }
        | ExitContinuation::ReturnUnit => {}
    }
    let mut current = Some(source);
    let mut remaining = plan.regions.len();
    let mut index = 0usize;
    let mut stopped = false;
    while let Some(region) = current {
        budget.charge(function, 1, "structural source ancestry")?;
        remaining = remaining.checked_sub(1).ok_or_else(control_refusal!(
            budget,
            function,
            permit,
            usize,
            Kind::CleanupRegionTreeContainsACycle
        ))?;
        let item = dynamic_get!(
            budget,
            function,
            plan.regions,
            &[crate::cleanup_plan::CleanupRegion],
            region.0 as usize,
            usize,
            Option<&crate::cleanup_plan::CleanupRegion>,
            (&crate::cleanup_plan::CleanupRegion),
            4
        )
        .ok_or_else(control_refusal!(
            budget,
            function,
            permit,
            &crate::cleanup_plan::CleanupRegion,
            Kind::CleanupRegionChainReferencesAnUnknownRegion
        ))?;
        stopped |= latch == Some(region);
        if let Some(target) = target {
            stopped |= crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    CleanupRegionId,
                    CleanupRegionId,
                    &mut ReplayBudget,
                    Result<bool, Error>
                ),
                5,
                "selected control ancestor_contains call receiving",
                ancestor_contains(function, target, region, budget)
            )?;
        }
        if !stopped {
            if dynamic_get!(
                budget,
                function,
                exit.leaves_regions,
                &[CleanupRegionId],
                index,
                usize,
                Option<&CleanupRegionId>,
                (&CleanupRegionId),
                4
            ) != Some(&region)
            {
                return Ok(false);
            }
            index += 1;
        }
        current = item.parent;
    }
    Ok(index == exit.leaves_regions.len())
}

fn result_source(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    source: &CleanupResultSource,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a CleanupResultSource,
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a BTreeSet<ExpressionId>,
        &'a mut ReplayBudget,
        bool,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected result_source local receiving")?;
    budget.charge(function, 8, "selected result_source local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    if matches!(function.return_type, ResolvedType::Nominal { .. }) {
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &ResolvedFunction,
                usize,
                &mut ReplayBudget,
                Result<(), Error>
            ),
            5,
            "selected control prepay child receiving",
            selected_profile::paid_children::prepay(
                program,
                function,
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &ResolvedType,
                        &ResolvedFunction,
                        &mut ReplayBudget,
                        Result<usize, Error>
                    ),
                    4,
                    "selected control type_key_bytes child receiving",
                    selected_profile::paid_children::type_key_bytes(
                        &function.return_type,
                        function,
                        budget,
                    )
                )?,
                budget,
            )
        )?;
    }
    let owned = crate::cleanup_plan::replay::inventory_coverage::paid::queries::needs_drop_in!(
        permit,
        function,
        budget,
        program,
        &function.return_type
    );
    match source {
        CleanupResultSource::Scalar { expression } => {
            if owned {
                return Err(permit.control(Reason::Finite(
                    Kind::DroppableFunctionResultUsesAScalarResultCommit,
                )));
            }
            if expression != &function.body.id && !return_operands.contains(expression) {
                return Err(permit.control(Reason::Finite(
                    Kind::ScalarResultCommitDoesNotPublishTheFunctionBody,
                )));
            }
        }
        CleanupResultSource::Owned { storage: result } => {
            if !owned
                || result.storage != StorageId::ProvisionalResult
                || !result.projections.is_empty()
            {
                return Err(permit.control(Reason::Finite(
                    Kind::OwnedResultCommitMustPublishTheWholeDroppableProvisionalResult,
                )));
            }
            crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    &CleanupPlace,
                    &BTreeSet<StorageId>,
                    &Leaves,
                    &mut ReplayBudget,
                    Result<Vec<LivenessFlagId>, Error>
                ),
                6,
                "selected control validate_place call receiving",
                helpers::validate_place(function, result, storage, leaves, budget)
            )?;
            if !receive!(
                budget,
                function,
                (
                    std::collections::btree_map::Values<'_, LivenessFlagId, super::super::Leaf>,
                    Option<&super::super::Leaf>,
                    &super::super::Leaf,
                    bool,
                    bool
                ),
                5,
                "selected provisional result leaf scan receiving",
                {
                    let mut found = false;
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
                        if leaf.place.storage == StorageId::ProvisionalResult {
                            found = true;
                            break;
                        }
                    }
                    found
                }
            ) {
                return Err(permit.control(Reason::Finite(
                    Kind::OwnedResultCommitHasNoProvisionalResultLeaves,
                )));
            }
        }
    }
    Ok(())
}
