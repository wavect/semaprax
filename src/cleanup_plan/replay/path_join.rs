//! Topological cleanup-CFG replay and ownership-only join normalization.

use super::*;

pub(super) fn validate_path_states(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let storage_regions = storage_regions(function)?;
    let contract_sources = plan
        .status_sources
        .iter()
        .filter(|source| source.id.lane == StatusLane::ContractFalse)
        .map(|source| (source.id.expression.clone(), source.id.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut initial = PathState {
        renewals: BTreeMap::new(),
        live_order: Vec::new(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
    };
    for place in &plan.entry_state.live_owned_parameters {
        let flags = validate_place(function, place, storage, leaves)?;
        append_dead_flags(function, &mut initial, flags, "entry state")?;
    }
    for entry in &plan.entry_state.conditional_owned_parameters {
        let mut cases = Vec::with_capacity(entry.cases.len());
        for case in &entry.cases {
            let mut flags = Vec::with_capacity(case.live_places.len());
            for place in &case.live_places {
                let under = validate_place(function, place, storage, leaves)?;
                if under.len() != 1 || place.projections.first() != Some(&case.case) {
                    return Err(replay_error(
                        function,
                        "conditional entry case does not name exact case-qualified leaves",
                    ));
                }
                flags.push(under[0]);
            }
            cases.push((case.case.clone(), flags));
        }
        initial.conditional_variants.push(ReplayConditionalVariant {
            root: CleanupPlace {
                storage: entry.storage.clone(),
                projections: Vec::new(),
            },
            variant: entry.variant.clone(),
            cases,
        });
    }

    let mut incoming = vec![BTreeSet::<PathState>::new(); plan.blocks.len()];
    incoming[plan.entry.0 as usize].insert(initial);
    let mut remaining_predecessors = vec![0_usize; plan.blocks.len()];
    for block in &plan.blocks {
        for successor in block_successors(function, block.id) {
            remaining_predecessors[successor.0 as usize] = remaining_predecessors
                [successor.0 as usize]
                .checked_add(1)
                .ok_or_else(|| replay_error(function, "too many cleanup predecessors"))?;
        }
    }
    let mut queue = VecDeque::from([plan.entry]);
    let mut terminal_paths = 0_usize;
    let mut successful_paths = 0_usize;

    while let Some(block_id) = queue.pop_front() {
        let states = std::mem::take(&mut incoming[block_id.0 as usize]);
        if states.is_empty() {
            return Err(replay_error(
                function,
                "reachable cleanup block has no incoming ownership state",
            ));
        }
        let join_units =
            states
                .len()
                .saturating_mul(states.iter().fold(1_usize, |total, state| {
                    total
                        .saturating_add(state.live_order.len())
                        .saturating_add(renewal::retained_units(state))
                }));
        budget.charge(function, join_units, "all-path ownership replay")?;
        let mut groups = BTreeMap::<
            (
                Option<StatusSourceId>,
                Option<StatusSourceId>,
                Option<StagedCopyResultSource>,
                bool,
            ),
            BTreeSet<PathState>,
        >::new();
        for state in states {
            groups
                .entry((
                    state.pending_failure.clone(),
                    state.selected_failure.clone(),
                    state.staged_copy_result.clone(),
                    state.published,
                ))
                .or_default()
                .insert(state);
        }
        let block = &plan.blocks[block_id.0 as usize];
        for states in groups.into_values() {
            let mut checked = BTreeSet::new();
            for state in &states {
                validate_join_compatibility(function, &checked, state, block_id)?;
                checked.insert(state.clone());
            }
            let (live_order, conditional_variants) =
                joined_ownership_state(program, function, &states)?;
            let mut state = states
                .into_iter()
                .next()
                .expect("replay control group is non-empty");
            state.live_order = live_order;
            state.conditional_variants = conditional_variants;
            for transition in &block.transitions {
                renewal::charge_transition(function, transition, &state, budget)?;
                execute_replay_transition(
                    program, function, transition, &mut state, storage, leaves,
                )?;
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    require_normal_flow_state(function, &state, block_id)?;
                    let edge = &plan.edges[edge.0 as usize];
                    let state =
                        state_for_edge(function, state, &edge.condition, &contract_sources)?;
                    incoming[edge.to.0 as usize].insert(state);
                }
                CleanupTerminator::Branch(edges) => {
                    require_normal_flow_state(function, &state, block_id)?;
                    for edge in edges {
                        let edge = &plan.edges[edge.0 as usize];
                        budget.charge(
                            function,
                            renewal::retained_units(&state),
                            "renewal branch history clone",
                        )?;
                        let next = state_for_edge(
                            function,
                            state.clone(),
                            &edge.condition,
                            &contract_sources,
                        )?;
                        incoming[edge.to.0 as usize].insert(next);
                    }
                }
                CleanupTerminator::Exit(exit_id) => {
                    let exit = &plan.exits[exit_id.0 as usize];
                    match replay_exit(function, exit, state, &storage_regions, storage, leaves)? {
                        Some((edge, continued)) => {
                            let target = plan.edges[edge.0 as usize].to;
                            incoming[target.0 as usize].insert(continued);
                        }
                        None => {
                            terminal_paths = terminal_paths.checked_add(1).ok_or_else(|| {
                                replay_error(function, "too many terminal cleanup paths")
                            })?;
                            if matches!(
                                exit.continuation,
                                ExitContinuation::CommitResult { .. }
                                    | ExitContinuation::ReturnUnit
                            ) {
                                successful_paths =
                                    successful_paths.checked_add(1).ok_or_else(|| {
                                        replay_error(function, "too many successful cleanup paths")
                                    })?;
                            }
                        }
                    }
                }
            }
        }
        for successor in block_successors(function, block_id) {
            let remaining = &mut remaining_predecessors[successor.0 as usize];
            *remaining = remaining
                .checked_sub(1)
                .ok_or_else(|| replay_error(function, "cleanup predecessor count underflowed"))?;
            if *remaining == 0 {
                queue.push_back(successor);
            }
        }
    }

    if terminal_paths == 0 || successful_paths == 0 {
        return Err(replay_error(
            function,
            "cleanup CFG has no replayable terminal success path",
        ));
    }
    Ok(())
}

fn joined_ownership_state(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    states: &BTreeSet<PathState>,
) -> Result<(Vec<LivenessFlagId>, Vec<ReplayConditionalVariant>), Diagnostic> {
    let mut states = states.iter();
    let first = states
        .next()
        .ok_or_else(|| replay_error(function, "cleanup join has no incoming states"))?;
    let mut live_order = first.live_order.clone();
    let mut conditional_variants = first.conditional_variants.clone();
    for state in states {
        live_order = join_live_orders(function, &live_order, &state.live_order)?;
        conditional_variants = join_conditional_variants(
            program,
            function,
            &conditional_variants,
            &state.conditional_variants,
        )?;
    }
    Ok((live_order, conditional_variants))
}

fn join_live_orders(
    function: &ResolvedFunction,
    left: &[LivenessFlagId],
    right: &[LivenessFlagId],
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    let flags = left.iter().chain(right).copied().collect::<BTreeSet<_>>();
    let mut successors = flags
        .iter()
        .map(|flag| (*flag, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = flags
        .iter()
        .map(|flag| (*flag, 0_u32))
        .collect::<BTreeMap<_, _>>();
    for history in [left, right] {
        for pair in history.windows(2) {
            if successors
                .get_mut(&pair[0])
                .expect("joined replay flag is indexed")
                .insert(pair[1])
            {
                *indegree
                    .get_mut(&pair[1])
                    .expect("joined replay flag is indexed") += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(flag, degree)| (*degree == 0).then_some(*flag))
        .collect::<BTreeSet<_>>();
    let mut joined = Vec::with_capacity(flags.len());
    while let Some(flag) = ready.pop_first() {
        joined.push(flag);
        for successor in successors[&flag].iter().copied() {
            let degree = indegree
                .get_mut(&successor)
                .expect("joined replay flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(successor);
            }
        }
    }
    if joined.len() != flags.len() {
        return Err(replay_error(
            function,
            "cleanup join has conflicting initialization histories",
        ));
    }
    Ok(joined)
}

fn join_conditional_variants(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    left: &[ReplayConditionalVariant],
    right: &[ReplayConditionalVariant],
) -> Result<Vec<ReplayConditionalVariant>, Diagnostic> {
    let left_len = left.len();
    let right_len = right.len();
    let left = left
        .iter()
        .map(|variant| (&variant.root, variant))
        .collect::<BTreeMap<_, _>>();
    let right = right
        .iter()
        .map(|variant| (&variant.root, variant))
        .collect::<BTreeMap<_, _>>();
    if left.len() != left_len || right.len() != right_len || left.keys().ne(right.keys()) {
        return Err(replay_error(
            function,
            "cleanup join disagrees on conditional variant roots",
        ));
    }
    let mut joined = Vec::with_capacity(left.len());
    for (root, left_variant) in left {
        let right_variant = right[root];
        if left_variant.variant != right_variant.variant {
            return Err(replay_error(
                function,
                "cleanup join disagrees on conditional variant identity",
            ));
        }
        let left_cases = left_variant
            .cases
            .iter()
            .map(|(case, flags)| (case, flags))
            .collect::<BTreeMap<_, _>>();
        let right_cases = right_variant
            .cases
            .iter()
            .map(|(case, flags)| (case, flags))
            .collect::<BTreeMap<_, _>>();
        let domain = program
            .declarations
            .variant_cases(&left_variant.variant)
            .ok_or_else(|| replay_error(function, "cleanup join variant has no closed domain"))?;
        let mut cases = Vec::new();
        for declared in domain {
            let flags = match (left_cases.get(&declared.id), right_cases.get(&declared.id)) {
                (Some(left), Some(right)) if left == right => (*left).clone(),
                (Some(_), Some(_)) => {
                    return Err(replay_error(
                        function,
                        "cleanup join disagrees on conditional case payload liveness",
                    ));
                }
                (Some(flags), None) | (None, Some(flags)) => (*flags).clone(),
                (None, None) => continue,
            };
            cases.push((declared.id.clone(), flags));
        }
        if cases.len()
            != left_cases
                .keys()
                .chain(right_cases.keys())
                .collect::<BTreeSet<_>>()
                .len()
        {
            return Err(replay_error(
                function,
                "cleanup join conditional state references a foreign case",
            ));
        }
        joined.push(ReplayConditionalVariant {
            root: root.clone(),
            variant: left_variant.variant.clone(),
            cases,
        });
    }
    Ok(joined)
}
