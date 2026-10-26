use super::*;

#[allow(clippy::too_many_arguments)] // Keep the independently held phase index explicit.
pub(super) fn validate_path_states<E: semantic_refusal::Error>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    lexical: Option<&lexical_cert::LexicalAuth>,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), E> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayPathStates,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayPathStates,
    );
    // Authenticate the absence once before path traversal. The source/HIR
    // wrapper certificate identifies a conditional Step transfer, which must
    // never acquire the one-leaf renewal authority of another profile.
    if let Some(auth) = lexical {
        for assignment in auth.wrapper_cancels.values() {
            if ordinary_renewals::lookup(program, function, assignment, helpers, budget)?.is_some()
            {
                return Err(replay_error(
                    function,
                    "owning Break conditional Step has unexpected renewal authority",
                )
                .into());
            }
        }
    }
    // The held edge helper uses only nonrecursive borrowed receiver frames.
    // Acquire their complete fixed capacity here; owned results retain their
    // ordinary per-call paths and insertion backing remains independently paid.
    budget.reserve_bytes(
        function,
        8 * std::mem::size_of::<&mut PathState>()
            + 4 * std::mem::size_of::<Result<(), Diagnostic>>()
            + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>(),
        "held-state borrowed receiver controls",
    )?;
    let plan = &function.cleanup_plan;
    let storage_regions = storage::storage_regions(function, budget)?;
    // Exact ownership state each lexical loop body must restore, captured once
    // from the state on its true-condition edge.
    let mut loop_entries = BTreeMap::<usize, PathState>::new();
    let no_extra_disposal = BTreeSet::<StorageId>::new();
    let mut contract_sources = BTreeMap::new();
    for source in plan
        .status_sources
        .iter()
        .filter(|source| source.id.lane == StatusLane::ContractFalse)
    {
        storage::insert_map(
            function,
            &mut contract_sources,
            source.id.expression.clone(),
            source.id.clone(),
            budget,
            "contract status lookup",
        )?;
    }

    let mut initial = PathState {
        renewals: BTreeMap::new(),
        live_order: Vec::new(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
        returned: None,
    };
    for place in &plan.entry_state.live_owned_parameters {
        let flags = storage::validate_place(function, place, storage, leaves, budget)?;
        budget.reserve_vec(
            function,
            &mut initial.live_order,
            flags.len(),
            "entry live flags",
        )?;
        append_dead_flags(function, &mut initial, flags, "entry state")?;
    }
    for entry in &plan.entry_state.conditional_owned_parameters {
        budget.reserve_array::<(DeclarationId, Vec<LivenessFlagId>)>(
            function,
            entry.cases.len(),
            "conditional entry cases",
        )?;
        let mut cases = Vec::with_capacity(entry.cases.len());
        for case in &entry.cases {
            budget.reserve_array::<LivenessFlagId>(
                function,
                case.live_places.len(),
                "conditional entry flags",
            )?;
            let mut flags = Vec::with_capacity(case.live_places.len());
            for place in &case.live_places {
                let under = storage::validate_place(function, place, storage, leaves, budget)?;
                if under.len() != 1 || place.projections.first() != Some(&case.case) {
                    return Err(replay_error(
                        function,
                        "conditional entry case does not name exact case-qualified leaves",
                    )
                    .into());
                }
                flags.push(under[0]);
            }
            budget.reserve_bytes(
                function,
                case.case.as_str().len(),
                "conditional entry case identity",
            )?;
            cases.push((case.case.clone(), flags));
        }
        budget.reserve_vec(
            function,
            &mut initial.conditional_variants,
            1,
            "conditional entry variants",
        )?;
        budget.reserve_bytes(
            function,
            entry.variant.as_str().len(),
            "conditional entry variant identity",
        )?;
        initial.conditional_variants.push(ReplayConditionalVariant {
            root: CleanupPlace {
                storage: entry.storage.clone(),
                projections: Vec::new(),
            },
            variant: entry.variant.clone(),
            cases,
        });
    }

    budget.reserve_array::<BTreeSet<Box<PathState>>>(
        function,
        plan.blocks.len(),
        "incoming state frontier",
    )?;
    let mut incoming = vec![BTreeSet::<Box<PathState>>::new(); plan.blocks.len()];
    let mut insertions = boxed_states::Insertions::<PathState>::acquire(function, budget)?;
    insertions.insert_owned(
        function,
        &mut incoming[plan.entry.0 as usize],
        initial,
        budget,
    );
    if insertions.failed {
        return Err(insertions.failure().into());
    }
    budget.reserve_array::<usize>(function, plan.blocks.len(), "cleanup predecessor counters")?;
    let mut remaining_predecessors = vec![0_usize; plan.blocks.len()];
    for block in &plan.blocks {
        for successor in block_successors(function, block.id) {
            remaining_predecessors[successor.0 as usize] = remaining_predecessors
                [successor.0 as usize]
                .checked_add(1)
                .ok_or_else(|| replay_error(function, "too many cleanup predecessors"))?;
        }
    }
    // A validated acyclic CFG schedules each block at most once. Hold its
    // complete queue capacity so push_back never grows an uncharged frontier.
    budget.reserve_array::<BlockId>(function, plan.blocks.len(), "cleanup ready queue")?;
    let mut queue = VecDeque::with_capacity(plan.blocks.len());
    queue.push_back(plan.entry);
    let mut terminal_paths = 0_usize;
    let mut successful_paths = 0_usize;
    let mut branch_joins = branch_join::BranchJoins::default();

    while let Some(block_id) = queue.pop_front() {
        let states = std::mem::take(&mut incoming[block_id.0 as usize]);
        if states.is_empty() {
            return Err(replay_error(
                function,
                "reachable cleanup block has no incoming ownership state",
            )
            .into());
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
        // A single incoming state is already its complete unique control group.
        // Keep every validation/join/transition/exit and scheduling step below;
        // no control-key clone, grouping map/set, or held insertion is needed.
        if states.len() == 1 {
            let block = &plan.blocks[block_id.0 as usize];
            // An owning break's fragment rejoins its wrapper's parent: every
            // incoming state must agree exactly, with no ownership or history
            // repair by the generic join below.
            let exact_join = lexical.is_some_and(|auth| auth.exact_joins.contains(&block_id));
            if exact_join && states.len() != 1 {
                return Err(replay_error(
                    function,
                    "owning break fragment does not rejoin its parent with exact ownership state",
                )
                .into());
            }
            validate_control_group_with_budget(function, &states, block_id, budget)?;
            // The validated singleton already owns the complete joined fields.
            // Consume it once; neither original vector nor any nested identity
            // is cloned, replaced from a clone, or temporarily reinitialized.
            let mut state = states
                .into_iter()
                .next()
                .expect("replay control group is non-empty");
            // Identical branch arms still require their original normalization.
            // Move the original vector into that same producer and receive its
            // result back into the same field. Partial failure drops the moved
            // input and every remaining state field through their original owners.
            state.conditional_variants = branch_joins.normalize(
                program,
                function,
                block_id,
                exact_join,
                state.conditional_variants,
                budget,
            )?;
            if let Some(auth) = lexical {
                <E as lexical_boundary::Policy>::check(
                    function,
                    auth,
                    block_id,
                    &state,
                    storage,
                    &storage_regions,
                    leaves,
                    budget,
                )?;
            }
            for transition in &block.transitions {
                renewal::charge_transition(function, transition, &state, budget)?;
                execute_replay_transition::<E>(
                    program, function, transition, &mut state, storage, leaves, helpers, budget,
                )?;
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    require_normal_flow_state(function, &state, block_id)?;
                    let edge = &plan.edges[edge.0 as usize];
                    super::transition::state_for_held_edge(
                        function,
                        &mut state,
                        &edge.condition,
                        &contract_sources,
                    )?;
                    // Preserve actual/formal owning Box transports at every
                    // reused insertion; only the new allocation disappears.
                    budget.reserve_bytes(
                        function,
                        2 * std::mem::size_of::<Box<PathState>>(),
                        "held-state owning box transports",
                    )?;
                    insertions.insert_held(
                        function,
                        &mut incoming[edge.to.0 as usize],
                        state,
                        budget,
                    );
                    if insertions.failed {
                        return Err(insertions.failure().into());
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    require_normal_flow_state(function, &state, block_id)?;
                    // Initial selection receivers are paid before len/index queries.
                    // This is a new metadata refusal frontier, not clone backing.
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<Result<(), Diagnostic>>()
                            + std::mem::size_of::<&mut ReplayBudget>()
                            + std::mem::size_of::<&ResolvedFunction>()
                            + std::mem::size_of::<&str>()
                            + 4 * std::mem::size_of::<usize>()
                            + std::mem::size_of::<bool>()
                            + 3 * std::mem::size_of::<&Vec<EdgeId>>(),
                        "branch final-state selection controls",
                    )?;
                    if edges.len() == 2 {
                        let edge = &plan.edges[edges[0].0 as usize];
                        budget.charge(
                            function,
                            renewal::retained_units(&state),
                            "renewal branch history clone",
                        )?;
                        let cloned = if storage::empty_renewals::selected(&state, budget) {
                            let payload = storage::empty_renewals::reserve(
                                function,
                                &state,
                                budget,
                                "ownership branch clone",
                            )?;
                            storage::empty_renewals::copy(payload)
                        } else {
                            storage::reserve_state_clone(
                                function,
                                &state,
                                budget,
                                "ownership branch clone",
                            )?;
                            state.as_ref().clone()
                        };
                        let next =
                            state_for_edge(function, cloned, &edge.condition, &contract_sources)?;
                        if let Some(auth) = lexical {
                            lexical_flow::capture_loop_entry(
                                function,
                                auth,
                                &edge.condition,
                                &next,
                                &mut loop_entries,
                                budget,
                            )?;
                        }
                        insertions.insert_owned(
                            function,
                            &mut incoming[edge.to.0 as usize],
                            next,
                            budget,
                        );
                        if insertions.failed {
                            return Err(insertions.failure().into());
                        }
                        // The earlier sibling owns its independent clone. The final
                        // sibling receives the original state, with identical work.

                        let edge = &plan.edges[edges[1].0 as usize];
                        budget.charge(
                            function,
                            renewal::retained_units(&state),
                            "renewal branch history clone",
                        )?;
                        super::transition::state_for_held_edge(
                            function,
                            &mut state,
                            &edge.condition,
                            &contract_sources,
                        )?;
                        let next = state;
                        if let Some(auth) = lexical {
                            lexical_flow::capture_loop_entry(
                                function,
                                auth,
                                &edge.condition,
                                &next,
                                &mut loop_entries,
                                budget,
                            )?;
                        }
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<Box<PathState>>(),
                            "held-state owning box transports",
                        )?;
                        insertions.insert_held(
                            function,
                            &mut incoming[edge.to.0 as usize],
                            next,
                            budget,
                        );
                        if insertions.failed {
                            return Err(insertions.failure().into());
                        }
                    } else {
                        // Complete old fallback for low-level malformed cardinality.
                        for edge in edges {
                            let edge = &plan.edges[edge.0 as usize];
                            budget.charge(
                                function,
                                renewal::retained_units(&state),
                                "renewal branch history clone",
                            )?;
                            let cloned = if storage::empty_renewals::selected(&state, budget) {
                                let payload = storage::empty_renewals::reserve(
                                    function,
                                    &state,
                                    budget,
                                    "ownership branch clone",
                                )?;
                                storage::empty_renewals::copy(payload)
                            } else {
                                storage::reserve_state_clone(
                                    function,
                                    &state,
                                    budget,
                                    "ownership branch clone",
                                )?;
                                state.as_ref().clone()
                            };
                            let next = state_for_edge(
                                function,
                                cloned,
                                &edge.condition,
                                &contract_sources,
                            )?;
                            if let Some(auth) = lexical {
                                lexical_flow::capture_loop_entry(
                                    function,
                                    auth,
                                    &edge.condition,
                                    &next,
                                    &mut loop_entries,
                                    budget,
                                )?;
                            }
                            insertions.insert_owned(
                                function,
                                &mut incoming[edge.to.0 as usize],
                                next,
                                budget,
                            );
                            if insertions.failed {
                                return Err(insertions.failure().into());
                            }
                        }
                    }
                }
                CleanupTerminator::Exit(exit_id) => {
                    let state = *state;
                    let exit = &plan.exits[exit_id.0 as usize];
                    let extra = lexical
                        .and_then(|auth| auth.item_disposals.get(exit_id))
                        .unwrap_or(&no_extra_disposal);
                    match replay_exit(
                        function,
                        exit,
                        state,
                        &storage_regions,
                        storage,
                        leaves,
                        extra,
                        budget,
                    )? {
                        ExitFlow::Edge(edge, continued) => {
                            let target = plan.edges[edge.0 as usize].to;
                            if let Some(assignment) =
                                lexical.and_then(|auth| auth.wrapper_cancels.get(exit_id))
                            {
                                // The authenticated hidden Step has no
                                // one-leaf reservation. Preserve every unrelated
                                // history; unexpected authority fails closed.
                                if continued.renewals.contains_key(assignment) {
                                    return Err(replay_error(function,
                                        "owning Break conditional Step has unexpected renewal history").into());
                                }
                            }
                            if let Some(auth) = lexical {
                                lexical_flow::check_break(
                                    function,
                                    auth,
                                    exit,
                                    &continued,
                                    &loop_entries,
                                    leaves,
                                )?;
                            }
                            insertions.insert_owned(
                                function,
                                &mut incoming[target.0 as usize],
                                continued,
                                budget,
                            );
                            if insertions.failed {
                                return Err(insertions.failure().into());
                            }
                        }
                        ExitFlow::Latch(latched) => {
                            let auth = lexical.ok_or_else(|| {
                                replay_error(function, "loop latch outside the lexical profile")
                            })?;
                            if let Some(entry) = lexical_cert::latch_resume(plan, exit.id) {
                                // A certified continue enters its finite generated
                                // advancement fragment, whose own latch checks entry.
                                insertions.insert_owned(
                                    function,
                                    &mut incoming[entry.0 as usize],
                                    latched,
                                    budget,
                                );
                                if insertions.failed {
                                    return Err(insertions.failure().into());
                                }
                            } else {
                                lexical_flow::check_latch(
                                    function,
                                    auth,
                                    exit,
                                    &latched,
                                    &loop_entries,
                                )?;
                                terminal_paths =
                                    terminal_paths.checked_add(1).ok_or_else(|| {
                                        replay_error(function, "too many terminal cleanup paths")
                                    })?;
                            }
                        }
                        ExitFlow::Terminal => {
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
            for successor in block_successors(function, block_id) {
                let remaining = &mut remaining_predecessors[successor.0 as usize];
                *remaining = remaining.checked_sub(1).ok_or_else(|| {
                    replay_error(function, "cleanup predecessor count underflowed")
                })?;
                if *remaining == 0 {
                    queue.push_back(successor);
                }
            }
            continue;
        }
        let mut groups = BTreeMap::<ControlKey, BTreeSet<Box<PathState>>>::new();
        for state in states {
            let key = if storage::atomic_controls::selected(&state, budget) {
                let payload = storage::atomic_controls::reserve(function, &state, budget)?;
                storage::atomic_controls::copy(payload)
            } else {
                storage::reserve_control_clone(function, &state, budget)?;
                (
                    state.pending_failure.clone(),
                    state.selected_failure.clone(),
                    state.staged_copy_result.clone(),
                    state.published,
                    state.returned.clone(),
                )
            };
            if !groups.contains_key(&key) {
                storage::reserve_btree_node::<ControlKey, BTreeSet<Box<PathState>>>(
                    function,
                    budget,
                    "ownership control groups",
                )?;
            }
            let group = groups.entry(key).or_default();
            insertions.insert_held(function, group, state, budget);
            if insertions.failed {
                return Err(insertions.failure().into());
            }
        }
        let block = &plan.blocks[block_id.0 as usize];
        for states in groups.into_values() {
            // An owning break's fragment rejoins its wrapper's parent: every
            // incoming state must agree exactly, with no ownership or history
            // repair by the generic join below.
            let exact_join = lexical.is_some_and(|auth| auth.exact_joins.contains(&block_id));
            if exact_join && states.len() != 1 {
                return Err(replay_error(
                    function,
                    "owning break fragment does not rejoin its parent with exact ownership state",
                )
                .into());
            }
            validate_control_group_with_budget(function, &states, block_id, budget)?;
            let (live_order, mut conditional_variants) =
                joined_ownership_state(program, function, &states, budget)?;
            // Identical branch arms deduplicate to one state; the producer
            // still joined them. An exact break rejoin is never re-keyed.
            if states.len() == 1 {
                conditional_variants = branch_joins.normalize(
                    program,
                    function,
                    block_id,
                    exact_join,
                    conditional_variants,
                    budget,
                )?;
            }
            let mut state = *states
                .into_iter()
                .next()
                .expect("replay control group is non-empty");
            state.live_order = live_order;
            state.conditional_variants = conditional_variants;
            if let Some(auth) = lexical {
                <E as lexical_boundary::Policy>::check(
                    function,
                    auth,
                    block_id,
                    &state,
                    storage,
                    &storage_regions,
                    leaves,
                    budget,
                )?;
            }
            for transition in &block.transitions {
                renewal::charge_transition(function, transition, &state, budget)?;
                execute_replay_transition::<E>(
                    program, function, transition, &mut state, storage, leaves, helpers, budget,
                )?;
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    require_normal_flow_state(function, &state, block_id)?;
                    let edge = &plan.edges[edge.0 as usize];
                    let state =
                        state_for_edge(function, state, &edge.condition, &contract_sources)?;
                    insertions.insert_owned(
                        function,
                        &mut incoming[edge.to.0 as usize],
                        state,
                        budget,
                    );
                    if insertions.failed {
                        return Err(insertions.failure().into());
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    require_normal_flow_state(function, &state, block_id)?;
                    // Initial selection receivers are paid before len/index queries.
                    // This is a new metadata refusal frontier, not clone backing.
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<Result<(), Diagnostic>>()
                            + std::mem::size_of::<&mut ReplayBudget>()
                            + std::mem::size_of::<&ResolvedFunction>()
                            + std::mem::size_of::<&str>()
                            + 4 * std::mem::size_of::<usize>()
                            + std::mem::size_of::<bool>()
                            + 3 * std::mem::size_of::<&Vec<EdgeId>>(),
                        "branch final-state selection controls",
                    )?;
                    if edges.len() == 2 {
                        let edge = &plan.edges[edges[0].0 as usize];
                        budget.charge(
                            function,
                            renewal::retained_units(&state),
                            "renewal branch history clone",
                        )?;
                        let cloned = if storage::empty_renewals::selected(&state, budget) {
                            let payload = storage::empty_renewals::reserve(
                                function,
                                &state,
                                budget,
                                "ownership branch clone",
                            )?;
                            storage::empty_renewals::copy(payload)
                        } else {
                            storage::reserve_state_clone(
                                function,
                                &state,
                                budget,
                                "ownership branch clone",
                            )?;
                            state.clone()
                        };
                        let next =
                            state_for_edge(function, cloned, &edge.condition, &contract_sources)?;
                        if let Some(auth) = lexical {
                            lexical_flow::capture_loop_entry(
                                function,
                                auth,
                                &edge.condition,
                                &next,
                                &mut loop_entries,
                                budget,
                            )?;
                        }
                        insertions.insert_owned(
                            function,
                            &mut incoming[edge.to.0 as usize],
                            next,
                            budget,
                        );
                        if insertions.failed {
                            return Err(insertions.failure().into());
                        }
                        // The earlier sibling owns its independent clone. The final
                        // sibling receives the original state, with identical work.

                        let edge = &plan.edges[edges[1].0 as usize];
                        budget.charge(
                            function,
                            renewal::retained_units(&state),
                            "renewal branch history clone",
                        )?;
                        let next =
                            state_for_edge(function, state, &edge.condition, &contract_sources)?;
                        if let Some(auth) = lexical {
                            lexical_flow::capture_loop_entry(
                                function,
                                auth,
                                &edge.condition,
                                &next,
                                &mut loop_entries,
                                budget,
                            )?;
                        }
                        insertions.insert_owned(
                            function,
                            &mut incoming[edge.to.0 as usize],
                            next,
                            budget,
                        );
                        if insertions.failed {
                            return Err(insertions.failure().into());
                        }
                    } else {
                        // Complete old fallback for low-level malformed cardinality.
                        for edge in edges {
                            let edge = &plan.edges[edge.0 as usize];
                            budget.charge(
                                function,
                                renewal::retained_units(&state),
                                "renewal branch history clone",
                            )?;
                            let cloned = if storage::empty_renewals::selected(&state, budget) {
                                let payload = storage::empty_renewals::reserve(
                                    function,
                                    &state,
                                    budget,
                                    "ownership branch clone",
                                )?;
                                storage::empty_renewals::copy(payload)
                            } else {
                                storage::reserve_state_clone(
                                    function,
                                    &state,
                                    budget,
                                    "ownership branch clone",
                                )?;
                                state.clone()
                            };
                            let next = state_for_edge(
                                function,
                                cloned,
                                &edge.condition,
                                &contract_sources,
                            )?;
                            if let Some(auth) = lexical {
                                lexical_flow::capture_loop_entry(
                                    function,
                                    auth,
                                    &edge.condition,
                                    &next,
                                    &mut loop_entries,
                                    budget,
                                )?;
                            }
                            insertions.insert_owned(
                                function,
                                &mut incoming[edge.to.0 as usize],
                                next,
                                budget,
                            );
                            if insertions.failed {
                                return Err(insertions.failure().into());
                            }
                        }
                    }
                }
                CleanupTerminator::Exit(exit_id) => {
                    let exit = &plan.exits[exit_id.0 as usize];
                    let extra = lexical
                        .and_then(|auth| auth.item_disposals.get(exit_id))
                        .unwrap_or(&no_extra_disposal);
                    match replay_exit(
                        function,
                        exit,
                        state,
                        &storage_regions,
                        storage,
                        leaves,
                        extra,
                        budget,
                    )? {
                        ExitFlow::Edge(edge, continued) => {
                            let target = plan.edges[edge.0 as usize].to;
                            if let Some(assignment) =
                                lexical.and_then(|auth| auth.wrapper_cancels.get(exit_id))
                            {
                                // The authenticated hidden Step has no
                                // one-leaf reservation. Preserve every unrelated
                                // history; unexpected authority fails closed.
                                if continued.renewals.contains_key(assignment) {
                                    return Err(replay_error(function,
                                        "owning Break conditional Step has unexpected renewal history").into());
                                }
                            }
                            if let Some(auth) = lexical {
                                lexical_flow::check_break(
                                    function,
                                    auth,
                                    exit,
                                    &continued,
                                    &loop_entries,
                                    leaves,
                                )?;
                            }
                            insertions.insert_owned(
                                function,
                                &mut incoming[target.0 as usize],
                                continued,
                                budget,
                            );
                            if insertions.failed {
                                return Err(insertions.failure().into());
                            }
                        }
                        ExitFlow::Latch(latched) => {
                            let auth = lexical.ok_or_else(|| {
                                replay_error(function, "loop latch outside the lexical profile")
                            })?;
                            if let Some(entry) = lexical_cert::latch_resume(plan, exit.id) {
                                // A certified continue enters its finite generated
                                // advancement fragment, whose own latch checks entry.
                                insertions.insert_owned(
                                    function,
                                    &mut incoming[entry.0 as usize],
                                    latched,
                                    budget,
                                );
                                if insertions.failed {
                                    return Err(insertions.failure().into());
                                }
                            } else {
                                lexical_flow::check_latch(
                                    function,
                                    auth,
                                    exit,
                                    &latched,
                                    &loop_entries,
                                )?;
                                terminal_paths =
                                    terminal_paths.checked_add(1).ok_or_else(|| {
                                        replay_error(function, "too many terminal cleanup paths")
                                    })?;
                            }
                        }
                        ExitFlow::Terminal => {
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
        )
        .into());
    }
    Ok(())
}

// Independently retained incoming-main source and complete caller comparison.
#[test]
fn complete_incoming_main_and_shipping_keep_source_results_work_and_full_drop() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    use sha2::{Digest, Sha256};
    let digest = |raw: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)));
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/18127f538186267d50e714d81076482f6bb89c3d41e7186e4d8c1de45b36929c"
    ));
    assert_eq!(original.len(), 51619);
    assert_eq!(digest(original.as_bytes()), "18127f538186267d50e714d81076482f6bb89c3d41e7186e4d8c1de45b36929c");
    assert_eq!(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/e/18127f538186267d50e714d81076482f6bb89c3d41e7186e4d8c1de45b36929c")).unwrap(), original.as_bytes());
    let current = include_str!("main3d81_before.rs");
    assert_eq!(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/main3d81_before.rs")).unwrap(), current.as_bytes());
    let method = original.split_once("#[allow(clippy::too_many_arguments)] // Keep the independently held phase index explicit.\n").unwrap().1.split_once("\ntype ControlKey =").unwrap().0;
    let acquired = current.strip_prefix("use super::*;\n\n#[allow(clippy::too_many_arguments)] // Keep the independently held phase index explicit.\n").unwrap().split_once("\n// Independently retained incoming-main source and complete caller comparison.\n").unwrap().0;
    assert_eq!(acquired, method);
    let root = include_str!("../path_join.rs");
    let helpers = original.split_once("type ControlKey =").unwrap().1.split_once("\n#[cfg(test)]\n#[rustfmt::skip]").unwrap().0;
    assert_eq!(root.matches(helpers).count(), 1);
    for (path, raw, expected) in [
        ("src/cleanup_plan/replay/path_join/boxed_states.rs", include_bytes!("boxed_states.rs").as_slice(), "1cb644157ccf8246f6a81011d0dd099a64f1809cffae15bb3e993c1d64eb93c5"),
        ("src/cleanup_plan/replay/path_join/branch_join.rs", include_bytes!("branch_join.rs").as_slice(), "d3633b8f9fd25ed9036c7387541961e94147743613423e2f945051be3c434a54"),
        ("src/cleanup_plan/replay/path_join/storage.rs", include_bytes!("storage.rs").as_slice(), "671900b2a5b62cba38ad34777f69d8f7fa2169bacac174a4237e1a24fbaf07c5"),
    ] {
        assert_eq!(digest(raw), expected);
        assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap(), raw);
    }
    let source = r#"
module boxed;
@id("f") fn f(flag:bool)->i64 {
    let scratch=bytes_zeroed(3usize);
    let view=bytes_as_slice(scratch);
    if flag && byte_len(view)==3usize {7}else{9}
}
@id("main") fn main()->i64 {f(false)}
"#;
    let checked = crate::check(source, "main3d81-held-box.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let reparsed = crate::check(&canonical, "main3d81-held-box.spx").unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    let program = crate::hir::resolve(&reparsed).unwrap();
    for function in &program.functions {
        let plan = function.cleanup_plan.clone();
        let run = |incoming| {
            with_limit_usage_refusal(64 * 1024 * 1024, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = (|| -> Result<(), Diagnostic> {
                        let storage = validate_slots_paid(function, &mut budget)?;
                        let leaves = collect_leaves_paid(function, &mut budget)?;
                        if incoming {
                            validate_path_states::<Diagnostic>(&program, function, &storage, &leaves, &mut budget, None, None)
                        } else {
                            super::validate_path_states::<Diagnostic>(&program, function, &storage, &leaves, &mut budget, None, None)
                        }
                    })();
                    let verdict = (result.is_ok(), budget.remaining, budget.skeleton_remaining, budget.skeleton_spent);
                    drop(result);
                    drop(budget);
                    verdict
                })
            })
        };
        let incoming = run(true);
        let shipping = run(false);
        assert!(incoming.0.0.0 && shipping.0.0.0);
        assert_eq!(incoming.0.0, shipping.0.0);
        for row in [&incoming, &shipping] {
            assert!(!row.1 && row.3.is_none());
            let allocation = row.0.1;
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= row.2);
        }
        eprintln!("main3d81 comparison function={} incoming_reservations={} shipping_reservations={} incoming_allocator={:?} shipping_allocator={:?}", function.id.as_str(), incoming.2, shipping.2, incoming.0.1, shipping.0.1);
        assert_eq!(function.cleanup_plan, plan);
    }
}
