//! Topological cleanup-CFG replay and ownership-only join normalization.

use super::*;
use std::borrow::Borrow;

#[allow(clippy::too_many_arguments)] // Keep the independently held phase index explicit.
pub(in crate::cleanup_plan::replay) fn validate_path_states<E: semantic_refusal::Error>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    lexical: Option<&lexical_cert::LexicalAuth>,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), E> {
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
            let mut state = *states
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
                lexical_boundary::check(
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
            for successor in block_successors(function, block_id) {
                let remaining = &mut remaining_predecessors[successor.0 as usize];
                *remaining = remaining
                    .checked_sub(1)
                    .ok_or_else(|| replay_error(function, "cleanup predecessor count underflowed"))?;
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
                lexical_boundary::check(
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

type ControlKey = (
    Option<StatusSourceId>,
    Option<StatusSourceId>,
    Option<StagedCopyResultSource>,
    bool,
    Option<ReturnedSource>,
);

/// The borrowed prefix is exactly the sorted set previously copied into
/// `checked`. Keep the complete history union: pairwise checks alone miss a
/// cycle assembled from three or more individually compatible histories.
#[cfg(test)]
fn validate_control_group(
    function: &ResolvedFunction,
    states: &BTreeSet<PathState>,
    block: BlockId,
) -> Result<(), Diagnostic> {
    validate_control_group_with_budget(function, states, block, &mut ReplayBudget::new())
}
fn validate_control_group_with_budget<K: Ord + Borrow<PathState>>(
    function: &ResolvedFunction,
    states: &BTreeSet<K>,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    for state in states {
        validate_join_compatibility(
            function,
            states.range::<K, _>(..state).map(|prior| prior.borrow()),
            state.borrow(),
            block,
            budget,
        )?;
    }
    Ok(())
}


fn joined_ownership_state<K: Ord + Borrow<PathState>>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    states: &BTreeSet<K>,
    budget: &mut ReplayBudget,
) -> Result<(Vec<LivenessFlagId>, Vec<ReplayConditionalVariant>), Diagnostic> {
    let mut states = states.iter().map(|state| state.borrow());
    let first = states
        .next()
        .ok_or_else(|| replay_error(function, "cleanup join has no incoming states"))?;
    storage::reserve_ownership_clone(function, first, budget)?;
    let mut live_order = first.live_order.clone();
    let mut conditional_variants = first.conditional_variants.clone();
    for state in states {
        live_order = join_live_orders(function, &live_order, &state.live_order, budget)?;
        conditional_variants = join_conditional_variants(
            program,
            function,
            &conditional_variants,
            &state.conditional_variants,
            budget,
        )?;
    }
    Ok((live_order, conditional_variants))
}

fn join_live_orders(
    function: &ResolvedFunction,
    left: &[LivenessFlagId],
    right: &[LivenessFlagId],
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    let mut flags = BTreeSet::new();
    for flag in left.iter().chain(right).copied() {
        storage::insert_set(function, &mut flags, flag, budget, "joined live flags")?;
    }
    let mut successors = BTreeMap::new();
    let mut indegree = BTreeMap::new();
    for flag in &flags {
        storage::insert_map(
            function,
            &mut successors,
            *flag,
            BTreeSet::new(),
            budget,
            "join successor lookup",
        )?;
        storage::insert_map(
            function,
            &mut indegree,
            *flag,
            0u32,
            budget,
            "join indegrees",
        )?;
    }
    for history in [left, right] {
        for pair in history.windows(2) {
            if storage::insert_set(
                function,
                successors
                    .get_mut(&pair[0])
                    .expect("joined replay flag is indexed"),
                pair[1],
                budget,
                "join successor edge",
            )? {
                *indegree
                    .get_mut(&pair[1])
                    .expect("joined replay flag is indexed") += 1;
            }
        }
    }
    let mut ready = BTreeSet::new();
    for (flag, degree) in &indegree {
        if *degree == 0 {
            storage::insert_set(function, &mut ready, *flag, budget, "join ready flags")?;
        }
    }
    budget.reserve_array::<LivenessFlagId>(function, flags.len(), "joined live history")?;
    let mut joined = Vec::with_capacity(flags.len());
    while let Some(flag) = ready.pop_first() {
        joined.push(flag);
        for successor in successors[&flag].iter().copied() {
            let degree = indegree
                .get_mut(&successor)
                .expect("joined replay flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                storage::insert_set(function, &mut ready, successor, budget, "join ready flags")?;
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
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    left: &[ReplayConditionalVariant],
    right: &[ReplayConditionalVariant],
    budget: &mut ReplayBudget,
) -> Result<Vec<ReplayConditionalVariant>, Diagnostic> {
    let left_len = left.len();
    let right_len = right.len();
    let mut left_index = BTreeMap::new();
    let mut right_index = BTreeMap::new();
    for variant in left {
        storage::insert_map(
            function,
            &mut left_index,
            &variant.root,
            variant,
            budget,
            "left conditional roots",
        )?;
    }
    for variant in right {
        storage::insert_map(
            function,
            &mut right_index,
            &variant.root,
            variant,
            budget,
            "right conditional roots",
        )?;
    }
    let left = left_index;
    let right = right_index;
    if left.len() != left_len || right.len() != right_len || left.keys().ne(right.keys()) {
        return Err(replay_error(
            function,
            "cleanup join disagrees on conditional variant roots",
        ));
    }
    budget.reserve_array::<ReplayConditionalVariant>(
        function,
        left.len(),
        "joined conditional variants",
    )?;
    let mut joined = Vec::with_capacity(left.len());
    for (root, left_variant) in left {
        let right_variant = right[root];
        if left_variant.variant != right_variant.variant {
            return Err(replay_error(
                function,
                "cleanup join disagrees on conditional variant identity",
            ));
        }
        let mut left_cases = BTreeMap::new();
        let mut right_cases = BTreeMap::new();
        for (case, flags) in &left_variant.cases {
            storage::insert_map(
                function,
                &mut left_cases,
                case,
                flags,
                budget,
                "left conditional cases",
            )?;
        }
        for (case, flags) in &right_variant.cases {
            storage::insert_map(
                function,
                &mut right_cases,
                case,
                flags,
                budget,
                "right conditional cases",
            )?;
        }
        let domain = program
            .declarations()
            .variant_cases(&left_variant.variant)
            .ok_or_else(|| replay_error(function, "cleanup join variant has no closed domain"))?;
        let mut cases = Vec::new();
        for declared in domain {
            let flags = match (left_cases.get(&declared.id), right_cases.get(&declared.id)) {
                (Some(left), Some(right)) if left == right => *left,
                (Some(left), Some(right)) if left.is_empty() => *right,
                (Some(left), Some(right)) if right.is_empty() => *left,
                (Some(_), Some(_)) => {
                    return Err(replay_error(
                        function,
                        "cleanup join disagrees on conditional case payload liveness",
                    ));
                }
                (Some(flags), None) | (None, Some(flags)) => *flags,
                (None, None) => continue,
            };
            budget.reserve_vec(function, &mut cases, 1, "joined conditional cases")?;
            budget.reserve_array::<LivenessFlagId>(
                function,
                flags.len(),
                "joined conditional case flags",
            )?;
            budget.reserve_bytes(function, declared.id.as_str().len(), "joined case identity")?;
            cases.push((declared.id.clone(), flags.clone()));
        }
        let mut case_union = BTreeSet::new();
        for case in left_cases.keys().chain(right_cases.keys()) {
            storage::insert_set(
                function,
                &mut case_union,
                *case,
                budget,
                "joined case domain",
            )?;
        }
        if cases.len() != case_union.len() {
            return Err(replay_error(
                function,
                "cleanup join conditional state references a foreign case",
            ));
        }
        storage::reserve_variant_identity(function, root, &left_variant.variant, budget)?;
        joined.push(ReplayConditionalVariant {
            root: root.clone(),
            variant: left_variant.variant.clone(),
            cases,
        });
    }
    Ok(joined)
}




