//! Exact selected structural twin; complete original remains in the parent owner.
// Selected primitive block/edge replay. Legacy validation remains unchanged.
use super::super::call_receiving::Policy;
use super::*;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive;
use crate::cleanup_plan::ExitTargetId;

#[allow(clippy::too_many_arguments)]
pub(in crate::cleanup_plan::replay) fn validate_blocks_and_edges_paid(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expressions: &dyn expression_facts::Lookup,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    renewals: &crate::cleanup_plan::ReplayRenewalBindings<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a dyn expression_facts::Lookup,
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        &'a crate::cleanup_plan::ReplayRenewalBindings<'a>,
        &'a mut ReplayBudget,
        &'a crate::cleanup_plan::CleanupPlan,
        BTreeSet<StatusSourceId>,
        BTreeSet<EdgeId>,
        BTreeSet<ExitTargetId>,
        BTreeSet<&'a ExpressionId>,
        Option<&'a StatusSourceId>,
        &'a crate::cleanup_plan::CleanupEdge,
        &'a crate::cleanup_plan::CleanupBlock,
        &'a CleanupTransition,
        usize,
        Vec<LivenessFlagId>,
        Vec<LivenessFlagId>,
        usize,
        usize,
        &'a ExpressionId,
        &'a CallFact<'a>,
        std::iter::Enumerate<std::slice::Iter<'a, crate::cleanup_plan::CleanupEdge>>,
        std::iter::Enumerate<std::slice::Iter<'a, crate::cleanup_plan::CleanupBlock>>,
        std::slice::Iter<'a, CleanupTransition>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_blocks_and_edges_paid local receiving",
    )?;
    budget.charge(
        function,
        26,
        "selected validate_blocks_and_edges_paid local receiving",
    )?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let _receiving = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &mut ReplayBudget,
            Result<super::super::call_receiving::Permit<'_, Error>, Error>
        ),
        3,
        "selected control existing call permit receiving",
        Error::enter::<()>(function, budget)
    )?;
    #[cfg(test)]
    const STATUS_FIELDS: usize = 6;
    #[cfg(not(test))]
    const STATUS_FIELDS: usize = 5;
    let plan = &function.cleanup_plan;
    // Test-only optional callback scratch carriers are distinct from the
    // original Ids::new backing charge, which remains unchanged below.
    #[cfg(test)]
    {
        budget.reserve_array::<(Option<borrowed_ids::Ids<'_>>, Option<borrowed_ids::Ids<'_>>)>(
            function,
            1,
            "selected borrowed scratch optional receiving",
        )?;
        budget.charge(function, 2, "selected borrowed scratch optional receiving")?;
    }
    let mut statuses = BTreeSet::new();
    #[cfg(test)]
    let mut borrowed_statuses = if expressions.is_metered() {
        Some(borrowed_ids::Ids::new(function, budget)?)
    } else {
        None
    };
    let mut status_entries = plan.status_sources.iter();
    while let Some(source) = dynamic_next!(
        budget,
        function,
        status_entries,
        std::slice::Iter<'_, StatusSource>,
        &StatusSource,
        (&StatusSource),
        3
    ) {
        budget.charge(function, 1, "structural status references")?;
        #[cfg(test)]
        if let Some(ids) = &mut borrowed_statuses {
            ids.insert(function, borrowed_ids::Key::Status(&source.id), budget)?;
            continue;
        }
        if !statuses.contains(&source.id) {
            budget.reserve_btree_node::<StatusSourceId, ()>(
                function,
                "structural status references",
            )?;
            budget.reserve_clone(function, &source.id, "structural status references")?;
            statuses.insert(source.id.clone());
        }
    }
    let mut edge_entries = plan.edges.iter().enumerate();
    while let Some((index, edge)) = dynamic_next!(
        budget,
        function,
        edge_entries,
        std::iter::Enumerate<std::slice::Iter<'_, crate::cleanup_plan::CleanupEdge>>,
        (usize, &crate::cleanup_plan::CleanupEdge),
        (usize, &crate::cleanup_plan::CleanupEdge),
        4
    ) {
        budget.charge(function, 1, "structural edges")?;
        if edge.id.0
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
                helpers::index(function, index, Kind::TooManyCleanupEdges, budget)
            )?
            || !block_exists(function, edge.from)
            || !block_exists(function, edge.to)
        {
            return Err(permit.control(Reason::Finite(
                Kind::CleanupEdgeHasANonContiguousIdOrUnknownEndpoint,
            )));
        }
        match &edge.condition {
            EdgeCondition::Always => {}
            EdgeCondition::BooleanResult(expression, _)
            | EdgeCondition::VariantCase {
                scrutinee: expression,
                ..
            }
            | EdgeCondition::ArmSelected {
                scrutinee: expression,
                ..
            } => crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &dyn expression_facts::Lookup,
                    &ResolvedFunction,
                    &ExpressionId,
                    &mut ReplayBudget,
                    Result<(), Error>
                ),
                5,
                "selected control require_expression call receiving",
                helpers::require_expression(expressions, function, expression, budget)
            )?,
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    StatusReceiving<'_>,
                    STATUS_FIELDS,
                    "selected control status reference receiving",
                    require_status_received(
                        function,
                        &statuses,
                        source,
                        #[cfg(test)]
                        borrowed_statuses.as_ref(),
                        budget,
                    )
                )?
            }
        }
    }
    let mut referenced_edges = BTreeSet::new();
    let mut referenced_exits = BTreeSet::new();
    let mut committed_calls = BTreeSet::new();
    #[cfg(test)]
    let mut borrowed_commits = if expressions.is_metered() {
        Some(borrowed_ids::Ids::new(function, budget)?)
    } else {
        None
    };
    // No call cursor backing is acquired for a plan with no commits.
    budget.reserve_array::<Option<calls::CallCursor<'_>>>(
        function,
        1,
        "structural call cursor header",
    )?;
    let mut cursor = None;
    let mut block_entries = plan.blocks.iter().enumerate();
    while let Some((index, block)) = dynamic_next!(
        budget,
        function,
        block_entries,
        std::iter::Enumerate<std::slice::Iter<'_, crate::cleanup_plan::CleanupBlock>>,
        (usize, &crate::cleanup_plan::CleanupBlock),
        (usize, &crate::cleanup_plan::CleanupBlock),
        4
    ) {
        budget.charge(function, 1, "structural blocks")?;
        if block.id.0
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
                helpers::index(function, index, Kind::TooManyCleanupBlocks, budget)
            )?
            || receive!(
                budget,
                function,
                (
                    Result<usize, std::num::TryFromIntError>,
                    Option<usize>,
                    usize,
                    bool
                ),
                4,
                "selected block region query receiving",
                {
                    match usize::try_from(block.region.0).ok() {
                        Some(region) => region >= plan.regions.len(),
                        None => true,
                    }
                }
            )
        {
            return Err(permit.control(Reason::Finite(
                Kind::CleanupBlockHasANonContiguousIdOrUnknownRegion,
            )));
        }
        let mut selected = None;
        let mut transition_entries = block.transitions.iter();
        while let Some(transition) = dynamic_next!(
            budget,
            function,
            transition_entries,
            std::slice::Iter<'_, CleanupTransition>,
            &CleanupTransition,
            (&CleanupTransition),
            3
        ) {
            budget.charge(function, 1, "structural transitions")?;
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &dyn expression_facts::Lookup,
                            &ResolvedFunction,
                            &ExpressionId,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        5,
                        "selected control require_expression call receiving",
                        helpers::require_expression(expressions, function, at, budget)
                    )?;
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
                        helpers::validate_place(function, destination, storage, leaves, budget)
                    )?;
                }
                CleanupTransition::ReserveRenewal { at, binding } => {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &ResolvedFunction,
                            &ExpressionId,
                            &CleanupPlace,
                            &crate::cleanup_plan::ReplayRenewalBindings<'_>,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        6,
                        "selected control validate_binding_selected call receiving",
                        helpers::validate_binding_selected(function, at, binding, renewals, budget)
                    )?;
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &dyn expression_facts::Lookup,
                            &ResolvedFunction,
                            &ExpressionId,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        5,
                        "selected control require_expression call receiving",
                        helpers::require_expression(expressions, function, at, budget)
                    )?;
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
                        helpers::validate_place(function, binding, storage, leaves, budget)
                    )?;
                }
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &ResolvedFunction,
                            &ExpressionId,
                            &CleanupPlace,
                            &crate::cleanup_plan::ReplayRenewalBindings<'_>,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        6,
                        "selected control validate_binding_selected call receiving",
                        helpers::validate_binding_selected(
                            function,
                            at,
                            destination,
                            renewals,
                            budget,
                        )
                    )?;
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &dyn expression_facts::Lookup,
                            &ResolvedFunction,
                            &ExpressionId,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        5,
                        "selected control require_expression call receiving",
                        helpers::require_expression(expressions, function, at, budget)
                    )?;
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
                        helpers::validate_place(function, source, storage, leaves, budget)
                    )?;
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
                        helpers::validate_place(function, destination, storage, leaves, budget)
                    )?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &dyn expression_facts::Lookup,
                            &ResolvedFunction,
                            &ExpressionId,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        5,
                        "selected control require_expression call receiving",
                        helpers::require_expression(expressions, function, at, budget)
                    )?;
                    let source_flags = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
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
                        helpers::validate_place(function, source, storage, leaves, budget)
                    )?;
                    let destination_flags = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
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
                        helpers::validate_place(function, destination, storage, leaves, budget)
                    )?;
                    // validate_place derives each unique flag from the independently
                    // indexed inventory. Compare borrowed flags, with no second tree.
                    if source_flags.len() != destination_flags.len() {
                        return Err(permit.control(Reason::Finite(
                            Kind::CleanupTransferHasIncompatibleStructuralPlaces,
                        )));
                    }
                    let mut source_flag_entries = source_flags.iter();
                    while let Some(flag) = dynamic_next!(
                        budget,
                        function,
                        source_flag_entries,
                        std::slice::Iter<'_, LivenessFlagId>,
                        &LivenessFlagId,
                        (&LivenessFlagId),
                        3
                    ) {
                        budget.charge(
                            function,
                            destination_flags.len(),
                            "structural transfer overlap",
                        )?;
                        if destination_flags.contains(flag) {
                            return Err(permit.control(Reason::Finite(
                                Kind::CleanupTransferHasIncompatibleStructuralPlaces,
                            )));
                        }
                    }
                }
                CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::InitializeVariant { .. } => {
                    return Err(permit.control(Reason::Finite(
                        Kind::SelectedStructuralReplayDoesNotSupportConditionalVariantOwnership,
                    )));
                }
                CleanupTransition::CallCommit { call, arguments } => {
                    let Some(Some(fact)) = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
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
                        expressions.fact(function, call, budget)
                    )?
                    else {
                        return Err(permit
                            .control(Reason::Finite(Kind::CallCommitDoesNotNameACallExpression)));
                    };
                    #[cfg(test)]
                    let inserted = match &mut borrowed_commits {
                        Some(ids) => {
                            ids.insert(function, borrowed_ids::Key::Expression(call), budget)?
                        }
                        None => {
                            if committed_calls.contains(call) {
                                false
                            } else {
                                budget.reserve_btree_node::<&ExpressionId, ()>(
                                    function,
                                    "structural committed calls",
                                )?;
                                committed_calls.insert(call)
                            }
                        }
                    };
                    #[cfg(not(test))]
                    let inserted = if committed_calls.contains(call) {
                        false
                    } else {
                        budget.reserve_btree_node::<&ExpressionId, ()>(
                            function,
                            "structural committed calls",
                        )?;
                        committed_calls.insert(call)
                    };
                    if !inserted && plan.lexical_control.is_none() {
                        return Err(
                            permit.control(Reason::Finite(Kind::CallHasMoreThanOneAtomicCommit))
                        );
                    }
                    #[cfg(test)]
                    let needs_cursor = !expressions.is_metered();
                    #[cfg(not(test))]
                    let needs_cursor = true;
                    if needs_cursor && cursor.is_none() {
                        cursor = Some(
                            crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                                budget,
                                function,
                                (
                                    &ResolvedFunction,
                                    &mut ReplayBudget,
                                    Result<calls::CallCursor<'_>, Error>
                                ),
                                3,
                                "selected existing call cursor constructor receiving",
                                calls::CallCursor::new::<Error>(function, budget)
                            )?,
                        );
                    }
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &dyn crate::cleanup::ReplayContext,
                            &ResolvedFunction,
                            &ExpressionId,
                            &CallFact<'_>,
                            &[crate::cleanup_plan::CallArgumentTransfer],
                            &BTreeSet<StorageId>,
                            &Leaves,
                            Option<&mut super::super::calls::CallCursor<'_>>,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        10,
                        "selected control call validation receiving",
                        calls::validate(
                            program,
                            function,
                            call,
                            fact,
                            arguments,
                            storage,
                            leaves,
                            cursor.as_mut(),
                            budget,
                        )
                    )?;
                }
                CleanupTransition::SelectFailure { source } => {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        StatusReceiving<'_>,
                        STATUS_FIELDS,
                        "selected control status reference receiving",
                        require_status_received(
                            function,
                            &statuses,
                            source,
                            #[cfg(test)]
                            borrowed_statuses.as_ref(),
                            budget,
                        )
                    )?;
                    if selected.replace(source).is_some() {
                        return Err(permit.control(Reason::Finite(
                            Kind::CleanupBlockSelectsMoreThanOneFailureStatus,
                        )));
                    }
                }
                CleanupTransition::StageCopyResult { source } => {
                    // Try/residual validation may allocate concrete substitution
                    // carriers. Those shapes are closed before any delegation.
                    let StagedCopyResultSource::Body { expression, .. } = source else {
                        return Err(permit.control(Reason::Finite(
                            Kind::SelectedStructuralReplayDoesNotSupportResidualStaging,
                        )));
                    };
                    #[cfg(test)]
                    if expressions.is_metered() {
                        borrowed_branch::staged(function, source, budget)?;
                    } else {
                        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                            budget,
                            function,
                            (
                                &ResolvedFunction,
                                &StagedCopyResultSource,
                                &mut ReplayBudget,
                                Result<(), Error>
                            ),
                            4,
                            "selected control validate_staged_target call receiving",
                            helpers::validate_staged_target(function, source, budget)
                        )?;
                    }
                    #[cfg(not(test))]
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &ResolvedFunction,
                            &StagedCopyResultSource,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        4,
                        "selected control validate_staged_target call receiving",
                        helpers::validate_staged_target(function, source, budget)
                    )?;
                    #[cfg(test)]
                    let matches = if expressions.is_metered() {
                        schema::borrowed::same_schema(
                            function,
                            expression.as_str(),
                            function.body.id.as_str(),
                            budget,
                        )?
                    } else {
                        expression == &function.body.id
                    };
                    #[cfg(not(test))]
                    let matches = expression == &function.body.id;
                    if !matches {
                        return Err(permit.control(Reason::Finite(
                            Kind::BodyCopyResultStageNamesAnotherExpression,
                        )));
                    }
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
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
                    owned_edge(function, block.id, *edge, &mut referenced_edges, budget)
                )?;
                // Selected renewal schemas admit the same authenticated match
                // decision goto as the legacy validator, never arbitrary branches.
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always)
                    && !(matches!(
                        plan.schema,
                        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
                            | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
                    ) && matches!(
                        plan.edges[edge.0 as usize].condition,
                        EdgeCondition::VariantCase { matches: true, .. }
                    ))
                {
                    return Err(permit.control(Reason::Finite(Kind::GotoEdgeIsConditional)));
                }
            }
            CleanupTerminator::Branch(edges) => {
                if edges.len() != 2 || edges[0] == edges[1] {
                    return Err(permit.control(Reason::Finite(
                        Kind::CleanupBranchMustOwnExactlyTwoDistinctEdges,
                    )));
                }
                let mut branch_edge_entries = edges.iter();
                while let Some(edge) = dynamic_next!(
                    budget,
                    function,
                    branch_edge_entries,
                    std::slice::Iter<'_, EdgeId>,
                    &EdgeId,
                    (&EdgeId),
                    3
                ) {
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
                        owned_edge(function, block.id, *edge, &mut referenced_edges, budget)
                    )?;
                }
                #[cfg(test)]
                if expressions.is_metered() {
                    borrowed_branch::validate(
                        function,
                        &plan.edges[edges[0].0 as usize].condition,
                        &plan.edges[edges[1].0 as usize].condition,
                        budget,
                    )?;
                } else {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        (
                            &ResolvedFunction,
                            &EdgeCondition,
                            &EdgeCondition,
                            &mut ReplayBudget,
                            Result<(), Error>
                        ),
                        5,
                        "selected control validate_branch_pair call receiving",
                        helpers::validate_branch_pair(
                            function,
                            &plan.edges[edges[0].0 as usize].condition,
                            &plan.edges[edges[1].0 as usize].condition,
                            budget,
                        )
                    )?;
                }
                #[cfg(not(test))]
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        &EdgeCondition,
                        &EdgeCondition,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    5,
                    "selected control validate_branch_pair call receiving",
                    helpers::validate_branch_pair(
                        function,
                        &plan.edges[edges[0].0 as usize].condition,
                        &plan.edges[edges[1].0 as usize].condition,
                        budget,
                    )
                )?;
            }
            CleanupTerminator::Exit(exit) => {
                if receive!(
                    budget,
                    function,
                    (
                        Result<usize, std::num::TryFromIntError>,
                        Option<usize>,
                        usize,
                        bool
                    ),
                    4,
                    "selected block exit query receiving",
                    {
                        match usize::try_from(exit.0).ok() {
                            Some(value) => value >= plan.exits.len(),
                            None => true,
                        }
                    }
                ) || referenced_exits.contains(exit)
                {
                    return Err(permit.control(Reason::Finite(
                        Kind::CleanupBlockReferencesAnUnknownOrRepeatedExit,
                    )));
                }
                budget.reserve_btree_node::<crate::cleanup_plan::ExitTargetId, ()>(
                    function,
                    "structural referenced exits",
                )?;
                referenced_exits.insert(*exit);
                if plan.exits[exit.0 as usize].from != block.id {
                    return Err(
                        permit.control(Reason::Finite(Kind::CleanupExitIsOwnedByTheWrongBlock))
                    );
                }
            }
        }
    }
    if plan.lexical_control.is_none() {
        let mut expected_count = 0usize;
        let mut fact_entries = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn expression_facts::Lookup,
                &ResolvedFunction,
                &mut ReplayBudget,
                Result<expression_facts::Entries<'_>, Diagnostic>
            ),
            4,
            "selected control expressions entries_paid call receiving",
            expressions.entries_paid(function, budget)
        )?;
        while let Some((expression, fact)) = dynamic_next!(
            budget,
            function,
            fact_entries,
            (
                &mut expression_facts::Entries<'_>,
                Option<(&ExpressionId, &Option<CallFact<'_>>)>,
                &ExpressionId,
                &Option<CallFact<'_>>,
            ),
            4
        ) {
            budget.charge(function, 1, "structural call coverage")?;
            if fact.is_some() {
                expected_count = expected_count.checked_add(1).ok_or_else(control_refusal!(
                    budget,
                    function,
                    permit,
                    usize,
                    Kind::TooManyCallExpressions
                ))?;
                #[cfg(test)]
                let present = match &borrowed_commits {
                    Some(ids) => {
                        ids.contains(function, borrowed_ids::Key::Expression(expression), budget)?
                    }
                    None => committed_calls.contains(expression),
                };
                #[cfg(not(test))]
                let present = committed_calls.contains(expression);
                if !present {
                    return Err(permit.control(Reason::Finite(
                        Kind::CleanupPlanDoesNotContainExactlyOneAtomicCommitForEveryCall,
                    )));
                }
            }
        }
        #[cfg(test)]
        let actual_count = match &borrowed_commits {
            Some(ids) => ids.len(),
            None => committed_calls.len(),
        };
        #[cfg(not(test))]
        let actual_count = committed_calls.len();
        if actual_count != expected_count {
            return Err(permit.control(Reason::Finite(
                Kind::CleanupPlanDoesNotContainExactlyOneAtomicCommitForEveryCall,
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
    // Validate endpoint/owner before charging a new tree key, preserving refusal
    // precedence and never inserting a forged edge into the reference inventory.
    let Some(item) = dynamic_get!(
        budget,
        function,
        function.cleanup_plan.edges,
        &[crate::cleanup_plan::CleanupEdge],
        edge.0 as usize,
        usize,
        Option<&crate::cleanup_plan::CleanupEdge>,
        (&crate::cleanup_plan::CleanupEdge),
        4
    ) else {
        return Err(permit.control(Reason::Finite(Kind::CleanupTerminatorReferencesUnknownEdge)));
    };
    if item.id != edge || item.from != owner || referenced.contains(&edge) {
        return Err(permit.control(Reason::Finite(
            Kind::CleanupEdgeHasTheWrongOwnerOrIsReferencedRepeatedly,
        )));
    }
    budget.reserve_btree_node::<EdgeId, ()>(function, "structural referenced edges")?;
    referenced.insert(edge);
    Ok(())
}

fn require_status_received(
    function: &ResolvedFunction,
    ordinary: &BTreeSet<StatusSourceId>,
    source: &StatusSourceId,
    #[cfg(test)] borrowed: Option<&borrowed_ids::Ids<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    #[cfg(not(test))]
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a BTreeSet<StatusSourceId>,
        &'a StatusSourceId,
        &'a mut ReplayBudget,
    );
    #[cfg(test)]
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a BTreeSet<StatusSourceId>,
        &'a StatusSourceId,
        &'a mut ReplayBudget,
        Option<&'a borrowed_ids::Ids<'a>>,
    );
    #[cfg(test)]
    const LOCAL_FIELDS: usize = 5;
    #[cfg(not(test))]
    const LOCAL_FIELDS: usize = 4;
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected require_status_received local receiving",
    )?;
    budget.charge(
        function,
        LOCAL_FIELDS,
        "selected require_status_received local receiving",
    )?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    #[cfg(test)]
    if let Some(ids) = borrowed {
        if ids.contains(function, borrowed_ids::Key::Status(source), budget)? {
            return Ok(());
        }
        return Err(permit.control(Reason::UnknownStatus(source.expression.clone())));
    }
    let _ = budget;
    if ordinary.contains(source) {
        Ok(())
    } else {
        Err(permit.control(Reason::UnknownStatus(source.expression.clone())))
    }
}

#[cfg(not(test))]
type StatusReceiving<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<StatusSourceId>,
    &'a StatusSourceId,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
#[cfg(test)]
type StatusReceiving<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<StatusSourceId>,
    &'a StatusSourceId,
    Option<&'a borrowed_ids::Ids<'a>>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
