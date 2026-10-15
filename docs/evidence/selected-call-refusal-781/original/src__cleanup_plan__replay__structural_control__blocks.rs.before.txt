//! Selected primitive block/edge replay. Legacy validation remains unchanged.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::cleanup_plan::replay) fn validate_blocks_and_edges(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expressions: &dyn expression_facts::Lookup,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    renewals: &crate::cleanup_plan::ReplayRenewalBindings<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut statuses = BTreeSet::new();
    #[cfg(test)]
    let mut borrowed_statuses = if expressions.is_metered() {
        Some(borrowed_ids::Ids::new(function, budget)?)
    } else {
        None
    };
    for source in &plan.status_sources {
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
    for (index, edge) in plan.edges.iter().enumerate() {
        budget.charge(function, 1, "structural edges")?;
        if edge.id.0 != u32_index(function, index, "cleanup edge")?
            || !block_exists(function, edge.from)
            || !block_exists(function, edge.to)
        {
            return Err(replay_error(
                function,
                "cleanup edge has a non-contiguous ID or unknown endpoint",
            ));
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
            } => expressions.require(function, expression, budget)?,
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                require_status_received(
                    function,
                    &statuses,
                    source,
                    #[cfg(test)]
                    borrowed_statuses.as_ref(),
                    budget,
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
    for (index, block) in plan.blocks.iter().enumerate() {
        budget.charge(function, 1, "structural blocks")?;
        if block.id.0 != u32_index(function, index, "cleanup block")?
            || usize::try_from(block.region.0)
                .ok()
                .is_none_or(|region| region >= plan.regions.len())
        {
            return Err(replay_error(
                function,
                "cleanup block has a non-contiguous ID or unknown region",
            ));
        }
        let mut selected = None;
        for transition in &block.transitions {
            budget.charge(function, 1, "structural transitions")?;
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    expressions.require(function, at, budget)?;
                    path_join::storage::validate_place(
                        function,
                        destination,
                        storage,
                        leaves,
                        budget,
                    )?;
                }
                CleanupTransition::ReserveRenewal { at, binding } => {
                    renewal::validate_binding_selected(function, at, binding, renewals)?;
                    expressions.require(function, at, budget)?;
                    path_join::storage::validate_place(function, binding, storage, leaves, budget)?;
                }
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    renewal::validate_binding_selected(function, at, destination, renewals)?;
                    expressions.require(function, at, budget)?;
                    path_join::storage::validate_place(function, source, storage, leaves, budget)?;
                    path_join::storage::validate_place(
                        function,
                        destination,
                        storage,
                        leaves,
                        budget,
                    )?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    expressions.require(function, at, budget)?;
                    let source_flags = path_join::storage::validate_place(
                        function, source, storage, leaves, budget,
                    )?;
                    let destination_flags = path_join::storage::validate_place(
                        function,
                        destination,
                        storage,
                        leaves,
                        budget,
                    )?;
                    // validate_place derives each unique flag from the independently
                    // indexed inventory. Compare borrowed flags, with no second tree.
                    if source_flags.len() != destination_flags.len() {
                        return Err(replay_error(
                            function,
                            "cleanup transfer has incompatible structural places",
                        ));
                    }
                    for flag in &source_flags {
                        budget.charge(
                            function,
                            destination_flags.len(),
                            "structural transfer overlap",
                        )?;
                        if destination_flags.contains(flag) {
                            return Err(replay_error(
                                function,
                                "cleanup transfer has incompatible structural places",
                            ));
                        }
                    }
                }
                CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::InitializeVariant { .. } => {
                    return Err(replay_error(
                        function,
                        "selected structural replay does not support conditional variant ownership",
                    ));
                }
                CleanupTransition::CallCommit { call, arguments } => {
                    let Some(Some(fact)) = expressions.fact(function, call, budget)? else {
                        return Err(replay_error(
                            function,
                            "call commit does not name a call expression",
                        ));
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
                        return Err(replay_error(
                            function,
                            "call has more than one atomic commit",
                        ));
                    }
                    #[cfg(test)]
                    let needs_cursor = !expressions.is_metered();
                    #[cfg(not(test))]
                    let needs_cursor = true;
                    if needs_cursor && cursor.is_none() {
                        cursor = Some(calls::CallCursor::new(function, budget)?);
                    }
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
                    )?;
                }
                CleanupTransition::SelectFailure { source } => {
                    require_status_received(
                        function,
                        &statuses,
                        source,
                        #[cfg(test)]
                        borrowed_statuses.as_ref(),
                        budget,
                    )?;
                    if selected.replace(source).is_some() {
                        return Err(replay_error(
                            function,
                            "cleanup block selects more than one failure status",
                        ));
                    }
                }
                CleanupTransition::StageCopyResult { source } => {
                    // Try/residual validation may allocate concrete substitution
                    // carriers. Those shapes are closed before any delegation.
                    let StagedCopyResultSource::Body { expression, .. } = source else {
                        return Err(replay_error(
                            function,
                            "selected structural replay does not support residual staging",
                        ));
                    };
                    #[cfg(test)]
                    if expressions.is_metered() {
                        borrowed_branch::staged(function, source, budget)?;
                    } else {
                        validate_staged_target(function, source)?;
                    }
                    #[cfg(not(test))]
                    validate_staged_target(function, source)?;
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
                        return Err(replay_error(
                            function,
                            "body Copy-result stage names another expression",
                        ));
                    }
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                owned_edge(function, block.id, *edge, &mut referenced_edges, budget)?;
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
                    return Err(replay_error(function, "goto edge is conditional"));
                }
            }
            CleanupTerminator::Branch(edges) => {
                if edges.len() != 2 || edges[0] == edges[1] {
                    return Err(replay_error(
                        function,
                        "cleanup branch must own exactly two distinct edges",
                    ));
                }
                for edge in edges {
                    owned_edge(function, block.id, *edge, &mut referenced_edges, budget)?;
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
                    validate_branch_pair(
                        function,
                        &plan.edges[edges[0].0 as usize].condition,
                        &plan.edges[edges[1].0 as usize].condition,
                    )?;
                }
                #[cfg(not(test))]
                validate_branch_pair(
                    function,
                    &plan.edges[edges[0].0 as usize].condition,
                    &plan.edges[edges[1].0 as usize].condition,
                )?;
            }
            CleanupTerminator::Exit(exit) => {
                if usize::try_from(exit.0)
                    .ok()
                    .is_none_or(|value| value >= plan.exits.len())
                    || referenced_exits.contains(exit)
                {
                    return Err(replay_error(
                        function,
                        "cleanup block references an unknown or repeated exit",
                    ));
                }
                budget.reserve_btree_node::<crate::cleanup_plan::ExitTargetId, ()>(
                    function,
                    "structural referenced exits",
                )?;
                referenced_exits.insert(*exit);
                if plan.exits[exit.0 as usize].from != block.id {
                    return Err(replay_error(
                        function,
                        "cleanup exit is owned by the wrong block",
                    ));
                }
            }
        }
    }
    if plan.lexical_control.is_none() {
        let mut expected_count = 0usize;
        for (expression, fact) in expressions.entries_paid(function, budget)? {
            budget.charge(function, 1, "structural call coverage")?;
            if fact.is_some() {
                expected_count = expected_count
                    .checked_add(1)
                    .ok_or_else(|| replay_error(function, "too many call expressions"))?;
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
                    return Err(replay_error(
                        function,
                        "cleanup plan does not contain exactly one atomic commit for every call",
                    ));
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
            return Err(replay_error(
                function,
                "cleanup plan does not contain exactly one atomic commit for every call",
            ));
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
) -> Result<(), Diagnostic> {
    // Validate endpoint/owner before charging a new tree key, preserving refusal
    // precedence and never inserting a forged edge into the reference inventory.
    let Some(item) = function.cleanup_plan.edges.get(edge.0 as usize) else {
        return Err(replay_error(
            function,
            "cleanup terminator references unknown edge",
        ));
    };
    if item.id != edge || item.from != owner || referenced.contains(&edge) {
        return Err(replay_error(
            function,
            "cleanup edge has the wrong owner or is referenced repeatedly",
        ));
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
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if let Some(ids) = borrowed {
        if ids.contains(function, borrowed_ids::Key::Status(source), budget)? {
            return Ok(());
        }
        return Err(replay_error(
            function,
            format_args!(
                "cleanup plan references unknown status source `{}`",
                source.expression
            ),
        ));
    }
    let _ = budget;
    require_status(function, ordinary, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"module selected.structural_calls;
@id("keep") fn keep(data:own Bytes)->Bytes {data}
@id("main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize);let mut at=0;
 while at<2 {data=bytes_set(data,0usize,65u8);at=at+1;if at==1 {continue;};0}
 let finished=keep(data);let view=bytes_as_slice(finished);
 if byte_len(view)==1usize {42}else{0}
}
"#;
    fn program() -> ResolvedProgram {
        crate::hir::resolve(&crate::check(SOURCE, "selected-structural-calls.spx").unwrap())
            .unwrap()
    }
    fn run(program: &ResolvedProgram, limit: usize) -> Result<usize, Diagnostic> {
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "main")
            .unwrap();
        let mut budget = ReplayBudget::with_physical(function, limit)?;
        let physical = budget.physical.as_deref_mut().unwrap();
        let renewals = crate::cleanup_plan::renewal_profile::bindings_with_reservation(
            program, function, physical,
        )?;
        let expressions = expression_facts::expression_facts_with_reservation(function, physical)?;
        let storage = structural_maps::validate_slots(function, &mut budget)?;
        let leaves = structural_maps::collect_leaves(function, &mut budget)?;
        validate_blocks_and_edges(
            program,
            function,
            &expressions,
            &storage,
            &leaves,
            &renewals,
            &mut budget,
        )?;
        Ok(budget.physical.as_deref().unwrap().charged())
    }
    #[test]
    fn selected_blocks_and_calls_preserve_exact_and_one_short_physical_debits() {
        let program = program();
        let debit = run(&program, usize::MAX).unwrap();
        assert!(debit > 0);
        assert_eq!(run(&program, debit).unwrap(), debit);
        assert!(run(&program, debit - 1).is_err());
    }
    #[test]
    fn selected_call_commit_cannot_omit_or_retarget_its_staged_owned_parameter() {
        let program = program();
        for retarget in [false, true] {
            let mut forged = program.clone();
            let function = forged
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "main")
                .unwrap();
            let result_id = function.result_id.clone();
            let argument = function
                .cleanup_plan
                .blocks
                .iter_mut()
                .flat_map(|b| &mut b.transitions)
                .find_map(|t| {
                    let CleanupTransition::CallCommit { arguments, .. } = t else {
                        return None;
                    };
                    arguments.first_mut()
                })
                .expect("actual staged owned parameter");
            if retarget {
                argument.source.storage = StorageId::Value(result_id);
            } else {
                argument.parameter_index = 99;
            }
            assert!(run(&forged, usize::MAX).is_err());
        }
    }
    #[test]
    fn selected_structural_edges_preserve_unique_owner_and_complementary_branches() {
        let program = program();
        for wrong_owner in [false, true] {
            let mut forged = program.clone();
            let function = forged
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "main")
                .unwrap();
            let pair = function
                .cleanup_plan
                .blocks
                .iter()
                .find_map(|b| match &b.terminator {
                    CleanupTerminator::Branch(edges) => Some((b.id, edges[0], edges[1])),
                    _ => None,
                })
                .expect("actual branch");
            if wrong_owner {
                function.cleanup_plan.edges[pair.1 .0 as usize].from = BlockId(u32::MAX);
            } else {
                let condition = function.cleanup_plan.edges[pair.1 .0 as usize]
                    .condition
                    .clone();
                function.cleanup_plan.edges[pair.2 .0 as usize].condition = condition;
            }
            assert!(run(&forged, usize::MAX).is_err());
        }
    }
}
