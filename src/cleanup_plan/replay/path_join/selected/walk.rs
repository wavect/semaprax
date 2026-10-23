use super::*;
pub(in crate::cleanup_plan::replay) fn validate_path_states(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    lexical: Option<&lexical_cert::LexicalAuth>,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), Error> {
    #[cfg(feature = "unstable-workflow-profiling")]
    crate::bounded_output::exclusive_debits::path_census::event(
        crate::bounded_output::exclusive_debits::path_census::Event::Body,
    );
    budget.reserve_array::<receiving::WalkBody<'_>>(
        function,
        1,
        "selected path validate_path_states body receiving",
    )?;
    budget.charge(
        function,
        85,
        "selected path validate_path_states body receiving",
    )?;
    budget.charge(function, 6, "compact incoming additional body receiving")?;
    budget.reserve_array::<(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedFunction,
        &BTreeSet<StorageId>,
        &Leaves,
        &mut ReplayBudget,
        Option<&lexical_cert::LexicalAuth>,
        Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
        Result<(), Error>,
    )>(
        function,
        1,
        "selected path validate_path_states formal receiving",
    )?;
    budget.charge(
        function,
        8,
        "selected path validate_path_states formal receiving",
    )?;
    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected path permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    #[cfg(test)]
    {
        budget.reserve_array::<(
            crate::bounded_output::stage_debits::Operation,
            crate::bounded_output::stage_debits::OperationGuard,
            crate::bounded_output::stage_debits::OperationGuard,
        )>(function, 1, "selected path stage guard receiving")?;
        budget.charge(function, 3, "selected path stage guard receiving")?;
    }
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayPathStates,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayPathStates,
    );
    if let Some(auth) = lexical {
        let mut cancel_entries = auth.wrapper_cancels.values();
        while let Some(assignment) = dynamic_next!(
            budget,
            function,
            cancel_entries,
            receiving::CancelEntriesNext<'_>,
            3
        ) {
            #[allow(clippy::redundant_pattern_matching)]
            if let Some(_) = receive!(
                budget,
                function,
                (
                    &crate::cleanup_plan::ReplayRenewalBindings<'_>,
                    &ExpressionId,
                    Option<&crate::hir::ResolvedBinding>
                ),
                3,
                "selected wrapper renewal query receiving",
                selected_renewals!(budget, function, program)
                    .expect("held selected renewal lease")
                    .get(assignment)
            ) {
                return Err(path_error!(
                    permit,
                    Reason::Finite(Kind::OwningBreakConditionalStepHasUnexpectedRenewalAuthority,)
                ));
            }
        }
    }
    let plan = &function.cleanup_plan;
    let storage_regions = retained_child!(
        budget,
        function,
        StorageRegions,
        3,
        storage::storage_regions(function, budget)
    )?;
    let mut loop_entries = BTreeMap::<usize, PathState>::new();
    let no_extra_disposal = BTreeSet::<StorageId>::new();
    let mut contract_sources = BTreeMap::new();
    let mut status_entries = plan.status_sources.iter();
    while let Some(source) = dynamic_next!(
        budget,
        function,
        status_entries,
        receiving::ContractSourcesNext<'_>,
        3
    ) {
        if source.id.lane != StatusLane::ContractFalse {
            continue;
        }
        retained_insert!(
            budget,
            function,
            ContractMapInsert,
            7,
            storage::insert_map(
                function,
                &mut contract_sources,
                clone_value!(budget, function, source.id.expression, ExpressionId),
                clone_value!(budget, function, source.id, StatusSourceId),
                budget,
                "contract status lookup",
            )
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
    let mut entry_places = plan.entry_state.live_owned_parameters.iter();
    while let Some(place) = dynamic_next!(
        budget,
        function,
        entry_places,
        receiving::EntryPlacesNext<'_>,
        3
    ) {
        let flags = place_flags!(budget, function, place, storage, leaves)?;
        budget.reserve_vec(
            function,
            &mut initial.live_order,
            flags.len(),
            "entry live flags",
        )?;
        path_child!(
            budget,
            function,
            AppendFlagsCall,
            6,
            finish::append_flags(
                function,
                &mut initial,
                flags,
                Kind::EntryStateInitializesAnEmptyOrLiveCleanupPlace,
                budget,
            )
        )?;
    }
    let mut conditional_entries = plan.entry_state.conditional_owned_parameters.iter();
    while let Some(entry) = dynamic_next!(
        budget,
        function,
        conditional_entries,
        receiving::ConditionalEntriesNext<'_>,
        3
    ) {
        budget.reserve_array::<(DeclarationId, Vec<LivenessFlagId>)>(
            function,
            entry.cases.len(),
            "conditional entry cases",
        )?;
        let mut cases = Vec::with_capacity(entry.cases.len());
        let mut conditional_cases = entry.cases.iter();
        while let Some(case) = dynamic_next!(
            budget,
            function,
            conditional_cases,
            receiving::ConditionalCasesNext<'_>,
            3
        ) {
            budget.reserve_array::<LivenessFlagId>(
                function,
                case.live_places.len(),
                "conditional entry flags",
            )?;
            let mut flags = Vec::with_capacity(case.live_places.len());
            let mut conditional_places = case.live_places.iter();
            while let Some(place) = dynamic_next!(
                budget,
                function,
                conditional_places,
                receiving::ConditionalPlacesNext<'_>,
                3
            ) {
                let under = place_flags!(budget, function, place, storage, leaves)?;
                if under.len() != 1
                    || dynamic_get!(
                        budget,
                        function,
                        place.projections,
                        &Vec<DeclarationId>,
                        0,
                        usize,
                        Option<&DeclarationId>,
                        (&DeclarationId),
                        4
                    ) != Some(&case.case)
                {
                    return Err(path_error!(
                        permit,
                        Reason::Finite(
                            Kind::ConditionalEntryCaseDoesNotNameExactCaseQualifiedLeaves,
                        )
                    ));
                }
                flags.push(flag_at!(budget, function, under, 0));
            }
            budget.reserve_bytes(
                function,
                case.case.as_str().len(),
                "conditional entry case identity",
            )?;
            cases.push((
                clone_value!(budget, function, case.case, DeclarationId),
                flags,
            ));
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
                storage: clone_value!(budget, function, entry.storage, StorageId),
                projections: Vec::new(),
            },
            variant: clone_value!(budget, function, entry.variant, DeclarationId),
            cases,
        });
    }

    budget.reserve_array::<frontier::Incoming<PathState>>(
        function,
        plan.blocks.len(),
        "incoming state frontier",
    )?;
    let mut incoming = Vec::with_capacity(plan.blocks.len());
    budget.reserve_array::<receiving::DirectoryCallback<'_>>(
        function,
        plan.blocks.len(),
        "compact incoming repeated default callback receiving",
    )?;
    budget.charge(
        function,
        plan.blocks.len().saturating_mul(5),
        "compact incoming repeated default callback receiving",
    )?;
    receive!(
        budget,
        function,
        receiving::DirectoryResize<'_>,
        8,
        "compact incoming resize method receiving",
        incoming.resize_with(
            plan.blocks.len(),
            frontier::Incoming::default as fn() -> frontier::Incoming<PathState>
        )
    );
    let mut incoming_insertions = frontier::Insertions::<PathState>::acquire(function, budget)?;
    let mut insertions = boxed_states::Insertions::<PathState>::acquire(function, budget)?;
    incoming_insertions.insert_owned(
        function,
        incoming_at!(budget, function, incoming, plan.entry.0 as usize),
        initial,
        budget,
    );
    if incoming_insertions.failed {
        return Err(incoming_insertions.failure().into());
    }
    budget.reserve_array::<usize>(function, plan.blocks.len(), "cleanup predecessor counters")?;
    let mut remaining_predecessors = vec![0_usize; plan.blocks.len()];
    let mut plan_blocks = plan.blocks.iter();
    while let Some(block) = dynamic_next!(
        budget,
        function,
        plan_blocks,
        receiving::PlanBlocksNext<'_>,
        3
    ) {
        let mut successor_entries = path_child!(
            budget,
            function,
            SuccessorsCall,
            4,
            successors(function, block.id, budget)
        )?;
        while let Some(successor) = dynamic_next!(
            budget,
            function,
            successor_entries,
            receiving::SuccessorEntriesNext<'_>,
            3
        ) {
            *counter_at!(
                budget,
                function,
                remaining_predecessors,
                successor.0 as usize
            ) = index!(
                budget,
                function,
                remaining_predecessors,
                successor.0 as usize,
                usize
            )
            .checked_add(1)
            .ok_or_else(path_refusal!(
                budget,
                function,
                permit,
                usize,
                Kind::TooManyCleanupPredecessors
            ))?;
        }
    }
    budget.reserve_array::<BlockId>(function, plan.blocks.len(), "cleanup ready queue")?;
    let mut queue = VecDeque::with_capacity(plan.blocks.len());
    queue.push_back(plan.entry);
    let mut terminal_paths = 0_usize;
    let mut successful_paths = 0_usize;
    let mut branch_joins = branch_join::BranchJoins::default();

    while let Some(block_id) = receive!(
        budget,
        function,
        (&mut VecDeque<BlockId>, Option<BlockId>, BlockId),
        3,
        "selected path ready pop receiving",
        queue.pop_front()
    ) {
        let states = std::mem::take(incoming_at!(
            budget,
            function,
            incoming,
            block_id.0 as usize
        ));
        #[cfg(feature = "unstable-workflow-profiling")]
        states.observe_incoming();
        if states.is_empty() {
            return Err(path_error!(
                permit,
                Reason::Finite(Kind::ReachableCleanupBlockHasNoIncomingOwnershipState,)
            ));
        }
        receive!(
            budget,
            function,
            usize,
            1,
            "selected path retained total receiving",
            ()
        );
        let mut retained = 1usize;
        let mut retained_states = states.iter();
        while let Some(state) = dynamic_next!(
            budget,
            function,
            retained_states,
            receiving::FrontierEntriesNext<'_>,
            3
        ) {
            let units = retained_units!(budget, function, state);
            retained = retained
                .saturating_add(state.live_order.len())
                .saturating_add(units);
        }
        let join_units = states.len().saturating_mul(retained);
        budget.charge(function, join_units, "all-path ownership replay")?;
        if states.len() == 1 {
            let block = plan_at!(budget, function, plan, blocks, block_id.0 as usize);
            let exact_join = exact_join!(budget, function, lexical, block_id);
            if exact_join && states.len() != 1 {
                return Err(path_error!(
                    permit,
                    Reason::Finite(
                        Kind::OwningBreakFragmentDoesNotRejoinItsParentWithExactOwnershipState,
                    )
                ));
            }
            path_child!(
                budget,
                function,
                SingletonGroupCall,
                5,
                boxed_flow::validate_singleton(function, states.one(), block_id, budget)
            )?;
            let mut singleton_states = states.into_one();
            let mut state = dynamic_next!(
                budget,
                function,
                singleton_states,
                receiving::FrontierOneNext<'_>,
                3
            )
            .expect("replay control group is non-empty");
            receive!(
                budget,
                function,
                (
                    &mut Vec<ReplayConditionalVariant>,
                    Vec<ReplayConditionalVariant>,
                    Vec<ReplayConditionalVariant>,
                    Vec<ReplayConditionalVariant>
                ),
                4,
                "same-box normalization replacement and moved vector receiving",
                ()
            );
            let variants = std::mem::take(&mut state.conditional_variants);
            state.conditional_variants = retained_child!(
                budget,
                function,
                BranchNormalize,
                8,
                branch_joins.normalize(program, function, block_id, exact_join, variants, budget)
            )?;
            if let Some(auth) = lexical {
                retained_child!(
                    budget,
                    function,
                    BoundaryCheck,
                    9,
                    <Error as lexical_boundary::Policy>::check(
                        function,
                        auth,
                        block_id,
                        &state,
                        storage,
                        &storage_regions,
                        leaves,
                        budget,
                    )
                )?;
            }
            let mut block_transitions = block.transitions.iter();
            while let Some(transition) = dynamic_next!(
                budget,
                function,
                block_transitions,
                receiving::BlockTransitionsNext<'_>,
                3
            ) {
                retained_child!(
                    budget,
                    function,
                    TransitionCharge,
                    5,
                    renewal::charge_transition(function, transition, &state, budget,)
                )?;
                path_child!(
                    budget,
                    function,
                    ExecuteCall,
                    9,
                    execution::execute(
                        program, function, transition, &mut state, storage, leaves, helpers,
                        budget,
                    )
                )?;
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    path_child!(
                        budget,
                        function,
                        RequireNormalFlowStateCall,
                        5,
                        finish::require_normal_flow_state(function, &state, block_id, budget)
                    )?;
                    let edge = plan_at!(budget, function, plan, edges, edge.0 as usize);
                    let state = path_child!(
                        budget,
                        function,
                        BoxedEdgeCall,
                        6,
                        boxed_flow::state_for_edge(
                            function,
                            state,
                            &edge.condition,
                            &contract_sources,
                            budget,
                        )
                    )?;
                    incoming_insertions.insert_held(
                        function,
                        incoming_at!(budget, function, incoming, edge.to.0 as usize),
                        state,
                        budget,
                    );
                    if incoming_insertions.failed {
                        return Err(incoming_insertions.failure().into());
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    path_child!(
                        budget,
                        function,
                        RequireNormalFlowStateCall,
                        5,
                        finish::require_normal_flow_state(function, &state, block_id, budget)
                    )?;
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<Result<(), Error>>()
                            + std::mem::size_of::<&mut ReplayBudget>()
                            + std::mem::size_of::<&ResolvedFunction>()
                            + std::mem::size_of::<&str>()
                            + 4 * std::mem::size_of::<usize>()
                            + std::mem::size_of::<bool>()
                            + 3 * std::mem::size_of::<&Vec<EdgeId>>(),
                        "branch final-state selection controls",
                    )?;
                    if edges.len() == 2 {
                        let edge = plan_at!(
                            budget,
                            function,
                            plan,
                            edges,
                            index!(budget, function, edges, 0, EdgeId).0 as usize
                        );
                        charge_retained_units!(
                            budget,
                            function,
                            &state,
                            "renewal branch history clone"
                        )?;
                        let cloned = {
                            retained_child!(
                                budget,
                                function,
                                StateClone,
                                5,
                                storage::reserve_state_clone(
                                    function,
                                    &state,
                                    budget,
                                    "ownership branch clone",
                                )
                            )?;
                            clone_state!(budget, function, state)
                        };
                        let next = path_child!(
                            budget,
                            function,
                            StateForEdgeCall,
                            6,
                            finish::state_for_edge(
                                function,
                                cloned,
                                &edge.condition,
                                &contract_sources,
                                budget,
                            )
                        )?;
                        if let Some(auth) = lexical {
                            path_child!(
                                budget,
                                function,
                                CaptureLoopEntryCall,
                                7,
                                lexical_flow::capture_loop_entry(
                                    function,
                                    auth,
                                    &edge.condition,
                                    &next,
                                    &mut loop_entries,
                                    budget,
                                )
                            )?;
                        }
                        incoming_insertions.insert_owned(
                            function,
                            incoming_at!(budget, function, incoming, edge.to.0 as usize),
                            next,
                            budget,
                        );
                        if incoming_insertions.failed {
                            return Err(incoming_insertions.failure().into());
                        }

                        let edge = plan_at!(
                            budget,
                            function,
                            plan,
                            edges,
                            index!(budget, function, edges, 1, EdgeId).0 as usize
                        );
                        charge_retained_units!(
                            budget,
                            function,
                            &state,
                            "renewal branch history clone"
                        )?;
                        let next = path_child!(
                            budget,
                            function,
                            BoxedEdgeCall,
                            6,
                            boxed_flow::state_for_edge(
                                function,
                                state,
                                &edge.condition,
                                &contract_sources,
                                budget,
                            )
                        )?;
                        if let Some(auth) = lexical {
                            path_child!(
                                budget,
                                function,
                                CaptureLoopEntryCall,
                                7,
                                lexical_flow::capture_loop_entry(
                                    function,
                                    auth,
                                    &edge.condition,
                                    &next,
                                    &mut loop_entries,
                                    budget,
                                )
                            )?;
                        }
                        incoming_insertions.insert_held(
                            function,
                            incoming_at!(budget, function, incoming, edge.to.0 as usize),
                            next,
                            budget,
                        );
                        if incoming_insertions.failed {
                            return Err(incoming_insertions.failure().into());
                        }
                    } else {
                        let mut branch_edges = edges.iter();
                        while let Some(edge) = dynamic_next!(
                            budget,
                            function,
                            branch_edges,
                            receiving::BranchEdgesNext<'_>,
                            3
                        ) {
                            let edge = plan_at!(budget, function, plan, edges, edge.0 as usize);
                            charge_retained_units!(
                                budget,
                                function,
                                &state,
                                "renewal branch history clone"
                            )?;
                            let cloned = {
                                retained_child!(
                                    budget,
                                    function,
                                    StateClone,
                                    5,
                                    storage::reserve_state_clone(
                                        function,
                                        &state,
                                        budget,
                                        "ownership branch clone",
                                    )
                                )?;
                                clone_state!(budget, function, state)
                            };
                            let next = path_child!(
                                budget,
                                function,
                                StateForEdgeCall,
                                6,
                                finish::state_for_edge(
                                    function,
                                    cloned,
                                    &edge.condition,
                                    &contract_sources,
                                    budget,
                                )
                            )?;
                            if let Some(auth) = lexical {
                                path_child!(
                                    budget,
                                    function,
                                    CaptureLoopEntryCall,
                                    7,
                                    lexical_flow::capture_loop_entry(
                                        function,
                                        auth,
                                        &edge.condition,
                                        &next,
                                        &mut loop_entries,
                                        budget,
                                    )
                                )?;
                            }
                            incoming_insertions.insert_owned(
                                function,
                                incoming_at!(budget, function, incoming, edge.to.0 as usize),
                                next,
                                budget,
                            );
                            if incoming_insertions.failed {
                                return Err(incoming_insertions.failure().into());
                            }
                        }
                    }
                }
                CleanupTerminator::Exit(exit_id) => {
                    let exit = plan_at!(budget, function, plan, exits, exit_id.0 as usize);
                    let extra = lexical_lookup!(
                        budget,
                        function,
                        lexical,
                        exit_id,
                        item_disposals,
                        BTreeSet<StorageId>
                    )
                    .unwrap_or(&no_extra_disposal);
                    match path_child!(
                        budget,
                        function,
                        ExitCall,
                        9,
                        execution::exit(
                            function,
                            exit,
                            *state,
                            &storage_regions,
                            storage,
                            leaves,
                            extra,
                            budget,
                        )
                    )? {
                        ExitFlow::Edge(edge, continued) => {
                            let target =
                                plan_at!(budget, function, plan, edges, edge.0 as usize).to;
                            if let Some(assignment) = lexical_lookup!(
                                budget,
                                function,
                                lexical,
                                exit_id,
                                wrapper_cancels,
                                ExpressionId
                            ) {
                                if continued.renewals.contains_key(assignment) {
                                    return Err(path_error!(permit, Reason::Finite(
                                        Kind::OwningBreakConditionalStepHasUnexpectedRenewalHistory,
                                    )));
                                }
                            }
                            if let Some(auth) = lexical {
                                path_child!(
                                    budget,
                                    function,
                                    CheckBreakCall,
                                    8,
                                    lexical_flow::check_break(
                                        function,
                                        auth,
                                        exit,
                                        &continued,
                                        &loop_entries,
                                        leaves,
                                        budget,
                                    )
                                )?;
                            }
                            incoming_insertions.insert_owned(
                                function,
                                incoming_at!(budget, function, incoming, target.0 as usize),
                                continued,
                                budget,
                            );
                            if incoming_insertions.failed {
                                return Err(incoming_insertions.failure().into());
                            }
                        }
                        ExitFlow::Latch(latched) => {
                            let auth = lexical.ok_or_else(path_refusal!(
                                budget,
                                function,
                                permit,
                                &lexical_cert::LexicalAuth,
                                Kind::LoopLatchOutsideTheLexicalProfile
                            ))?;
                            if let Some(entry) = lexical_cert::latch_resume(plan, exit.id) {
                                incoming_insertions.insert_owned(
                                    function,
                                    incoming_at!(budget, function, incoming, entry.0 as usize),
                                    latched,
                                    budget,
                                );
                                if incoming_insertions.failed {
                                    return Err(incoming_insertions.failure().into());
                                }
                            } else {
                                path_child!(
                                    budget,
                                    function,
                                    CheckLatchCall,
                                    7,
                                    lexical_flow::check_latch(
                                        function,
                                        auth,
                                        exit,
                                        &latched,
                                        &loop_entries,
                                        budget,
                                    )
                                )?;
                                terminal_paths =
                                    terminal_paths.checked_add(1).ok_or_else(path_refusal!(
                                        budget,
                                        function,
                                        permit,
                                        usize,
                                        Kind::TooManyTerminalCleanupPaths
                                    ))?;
                            }
                        }
                        ExitFlow::Terminal => {
                            terminal_paths =
                                terminal_paths.checked_add(1).ok_or_else(path_refusal!(
                                    budget,
                                    function,
                                    permit,
                                    usize,
                                    Kind::TooManyTerminalCleanupPaths
                                ))?;
                            if matches!(
                                exit.continuation,
                                ExitContinuation::CommitResult { .. }
                                    | ExitContinuation::ReturnUnit
                            ) {
                                successful_paths =
                                    successful_paths.checked_add(1).ok_or_else(path_refusal!(
                                        budget,
                                        function,
                                        permit,
                                        usize,
                                        Kind::TooManySuccessfulCleanupPaths
                                    ))?;
                            }
                        }
                    }
                }
            }
            let mut successor_entries = path_child!(
                budget,
                function,
                SuccessorsCall,
                4,
                successors(function, block_id, budget)
            )?;
            while let Some(successor) = dynamic_next!(
                budget,
                function,
                successor_entries,
                receiving::SuccessorEntriesNext<'_>,
                3
            ) {
                let remaining = counter_at!(
                    budget,
                    function,
                    remaining_predecessors,
                    successor.0 as usize
                );
                *remaining = remaining.checked_sub(1).ok_or_else(path_refusal!(
                    budget,
                    function,
                    permit,
                    usize,
                    Kind::CleanupPredecessorCountUnderflowed
                ))?;
                if *remaining == 0 {
                    queue.push_back(successor);
                }
            }
            continue;
        }
        let mut groups = BTreeMap::<ControlKey, BTreeSet<Box<PathState>>>::new();
        let mut owned_states = states.into_many().into_iter();
        while let Some(state) = dynamic_next!(
            budget,
            function,
            owned_states,
            receiving::OwnedStatesNext<'_>,
            3
        ) {
            let key = {
                retained_child!(
                    budget,
                    function,
                    ControlClone,
                    4,
                    storage::reserve_control_clone(function, &state, budget)
                )?;
                control_key!(budget, function, state)
            };
            if !groups.contains_key(&key) {
                retained_child!(
                    budget,
                    function,
                    NodeReserve,
                    4,
                    storage::reserve_btree_node::<ControlKey, BTreeSet<Box<PathState>>>(
                        function,
                        budget,
                        "ownership control groups",
                    )
                )?;
            }
            let group = group_at!(budget, function, groups, key);
            insertions.insert_held(function, group, state, budget);
            if insertions.failed {
                return Err(insertions.failure().into());
            }
        }
        let block = plan_at!(budget, function, plan, blocks, block_id.0 as usize);
        let mut control_groups = groups.into_values();
        while let Some(states) = dynamic_next!(
            budget,
            function,
            control_groups,
            receiving::ControlGroupsNext<'_>,
            3
        ) {
            let exact_join = exact_join!(budget, function, lexical, block_id);
            if exact_join && states.len() != 1 {
                return Err(path_error!(
                    permit,
                    Reason::Finite(
                        Kind::OwningBreakFragmentDoesNotRejoinItsParentWithExactOwnershipState,
                    )
                ));
            }
            path_child!(
                budget,
                function,
                ValidateControlGroupWithBudgetCall,
                5,
                joins::validate_control_group_with_budget(function, &states, block_id, budget)
            )?;
            let (live_order, mut conditional_variants) = path_child!(
                budget,
                function,
                JoinedOwnershipStateCall,
                5,
                joins::joined_ownership_state(program, function, &states, budget)
            )?;
            if states.len() == 1 {
                conditional_variants = retained_child!(
                    budget,
                    function,
                    BranchNormalize,
                    8,
                    branch_joins.normalize(
                        program,
                        function,
                        block_id,
                        exact_join,
                        conditional_variants,
                        budget,
                    )
                )?;
            }
            let mut singleton_states = states.into_iter();
            let mut state = dynamic_next!(
                budget,
                function,
                singleton_states,
                receiving::OwnedStatesNext<'_>,
                3
            )
            .expect("replay control group is non-empty");
            state.live_order = live_order;
            state.conditional_variants = conditional_variants;
            if let Some(auth) = lexical {
                retained_child!(
                    budget,
                    function,
                    BoundaryCheck,
                    9,
                    <Error as lexical_boundary::Policy>::check(
                        function,
                        auth,
                        block_id,
                        &state,
                        storage,
                        &storage_regions,
                        leaves,
                        budget,
                    )
                )?;
            }
            let mut block_transitions = block.transitions.iter();
            while let Some(transition) = dynamic_next!(
                budget,
                function,
                block_transitions,
                receiving::BlockTransitionsNext<'_>,
                3
            ) {
                retained_child!(
                    budget,
                    function,
                    TransitionCharge,
                    5,
                    renewal::charge_transition(function, transition, &state, budget,)
                )?;
                path_child!(
                    budget,
                    function,
                    ExecuteCall,
                    9,
                    execution::execute(
                        program, function, transition, &mut state, storage, leaves, helpers,
                        budget,
                    )
                )?;
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    path_child!(
                        budget,
                        function,
                        RequireNormalFlowStateCall,
                        5,
                        finish::require_normal_flow_state(function, &state, block_id, budget)
                    )?;
                    let edge = plan_at!(budget, function, plan, edges, edge.0 as usize);
                    let state = path_child!(
                        budget,
                        function,
                        BoxedEdgeCall,
                        6,
                        boxed_flow::state_for_edge(
                            function,
                            state,
                            &edge.condition,
                            &contract_sources,
                            budget,
                        )
                    )?;
                    incoming_insertions.insert_held(
                        function,
                        incoming_at!(budget, function, incoming, edge.to.0 as usize),
                        state,
                        budget,
                    );
                    if incoming_insertions.failed {
                        return Err(incoming_insertions.failure().into());
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    path_child!(
                        budget,
                        function,
                        RequireNormalFlowStateCall,
                        5,
                        finish::require_normal_flow_state(function, &state, block_id, budget)
                    )?;
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<Result<(), Error>>()
                            + std::mem::size_of::<&mut ReplayBudget>()
                            + std::mem::size_of::<&ResolvedFunction>()
                            + std::mem::size_of::<&str>()
                            + 4 * std::mem::size_of::<usize>()
                            + std::mem::size_of::<bool>()
                            + 3 * std::mem::size_of::<&Vec<EdgeId>>(),
                        "branch final-state selection controls",
                    )?;
                    if edges.len() == 2 {
                        let edge = plan_at!(
                            budget,
                            function,
                            plan,
                            edges,
                            index!(budget, function, edges, 0, EdgeId).0 as usize
                        );
                        charge_retained_units!(
                            budget,
                            function,
                            &state,
                            "renewal branch history clone"
                        )?;
                        let cloned = {
                            retained_child!(
                                budget,
                                function,
                                StateClone,
                                5,
                                storage::reserve_state_clone(
                                    function,
                                    &state,
                                    budget,
                                    "ownership branch clone",
                                )
                            )?;
                            clone_state!(budget, function, state)
                        };
                        let next = path_child!(
                            budget,
                            function,
                            StateForEdgeCall,
                            6,
                            finish::state_for_edge(
                                function,
                                cloned,
                                &edge.condition,
                                &contract_sources,
                                budget,
                            )
                        )?;
                        if let Some(auth) = lexical {
                            path_child!(
                                budget,
                                function,
                                CaptureLoopEntryCall,
                                7,
                                lexical_flow::capture_loop_entry(
                                    function,
                                    auth,
                                    &edge.condition,
                                    &next,
                                    &mut loop_entries,
                                    budget,
                                )
                            )?;
                        }
                        incoming_insertions.insert_owned(
                            function,
                            incoming_at!(budget, function, incoming, edge.to.0 as usize),
                            next,
                            budget,
                        );
                        if incoming_insertions.failed {
                            return Err(incoming_insertions.failure().into());
                        }

                        let edge = plan_at!(
                            budget,
                            function,
                            plan,
                            edges,
                            index!(budget, function, edges, 1, EdgeId).0 as usize
                        );
                        charge_retained_units!(
                            budget,
                            function,
                            &state,
                            "renewal branch history clone"
                        )?;
                        let next = path_child!(
                            budget,
                            function,
                            BoxedEdgeCall,
                            6,
                            boxed_flow::state_for_edge(
                                function,
                                state,
                                &edge.condition,
                                &contract_sources,
                                budget,
                            )
                        )?;
                        if let Some(auth) = lexical {
                            path_child!(
                                budget,
                                function,
                                CaptureLoopEntryCall,
                                7,
                                lexical_flow::capture_loop_entry(
                                    function,
                                    auth,
                                    &edge.condition,
                                    &next,
                                    &mut loop_entries,
                                    budget,
                                )
                            )?;
                        }
                        incoming_insertions.insert_held(
                            function,
                            incoming_at!(budget, function, incoming, edge.to.0 as usize),
                            next,
                            budget,
                        );
                        if incoming_insertions.failed {
                            return Err(incoming_insertions.failure().into());
                        }
                    } else {
                        let mut branch_edges = edges.iter();
                        while let Some(edge) = dynamic_next!(
                            budget,
                            function,
                            branch_edges,
                            receiving::BranchEdgesNext<'_>,
                            3
                        ) {
                            let edge = plan_at!(budget, function, plan, edges, edge.0 as usize);
                            charge_retained_units!(
                                budget,
                                function,
                                &state,
                                "renewal branch history clone"
                            )?;
                            let cloned = {
                                retained_child!(
                                    budget,
                                    function,
                                    StateClone,
                                    5,
                                    storage::reserve_state_clone(
                                        function,
                                        &state,
                                        budget,
                                        "ownership branch clone",
                                    )
                                )?;
                                clone_state!(budget, function, state)
                            };
                            let next = path_child!(
                                budget,
                                function,
                                StateForEdgeCall,
                                6,
                                finish::state_for_edge(
                                    function,
                                    cloned,
                                    &edge.condition,
                                    &contract_sources,
                                    budget,
                                )
                            )?;
                            if let Some(auth) = lexical {
                                path_child!(
                                    budget,
                                    function,
                                    CaptureLoopEntryCall,
                                    7,
                                    lexical_flow::capture_loop_entry(
                                        function,
                                        auth,
                                        &edge.condition,
                                        &next,
                                        &mut loop_entries,
                                        budget,
                                    )
                                )?;
                            }
                            incoming_insertions.insert_owned(
                                function,
                                incoming_at!(budget, function, incoming, edge.to.0 as usize),
                                next,
                                budget,
                            );
                            if incoming_insertions.failed {
                                return Err(incoming_insertions.failure().into());
                            }
                        }
                    }
                }
                CleanupTerminator::Exit(exit_id) => {
                    let exit = plan_at!(budget, function, plan, exits, exit_id.0 as usize);
                    let extra = lexical_lookup!(
                        budget,
                        function,
                        lexical,
                        exit_id,
                        item_disposals,
                        BTreeSet<StorageId>
                    )
                    .unwrap_or(&no_extra_disposal);
                    match path_child!(
                        budget,
                        function,
                        ExitCall,
                        9,
                        execution::exit(
                            function,
                            exit,
                            *state,
                            &storage_regions,
                            storage,
                            leaves,
                            extra,
                            budget,
                        )
                    )? {
                        ExitFlow::Edge(edge, continued) => {
                            let target =
                                plan_at!(budget, function, plan, edges, edge.0 as usize).to;
                            if let Some(assignment) = lexical_lookup!(
                                budget,
                                function,
                                lexical,
                                exit_id,
                                wrapper_cancels,
                                ExpressionId
                            ) {
                                if continued.renewals.contains_key(assignment) {
                                    return Err(path_error!(permit, Reason::Finite(
                                        Kind::OwningBreakConditionalStepHasUnexpectedRenewalHistory,
                                    )));
                                }
                            }
                            if let Some(auth) = lexical {
                                path_child!(
                                    budget,
                                    function,
                                    CheckBreakCall,
                                    8,
                                    lexical_flow::check_break(
                                        function,
                                        auth,
                                        exit,
                                        &continued,
                                        &loop_entries,
                                        leaves,
                                        budget,
                                    )
                                )?;
                            }
                            incoming_insertions.insert_owned(
                                function,
                                incoming_at!(budget, function, incoming, target.0 as usize),
                                continued,
                                budget,
                            );
                            if incoming_insertions.failed {
                                return Err(incoming_insertions.failure().into());
                            }
                        }
                        ExitFlow::Latch(latched) => {
                            let auth = lexical.ok_or_else(path_refusal!(
                                budget,
                                function,
                                permit,
                                &lexical_cert::LexicalAuth,
                                Kind::LoopLatchOutsideTheLexicalProfile
                            ))?;
                            if let Some(entry) = lexical_cert::latch_resume(plan, exit.id) {
                                incoming_insertions.insert_owned(
                                    function,
                                    incoming_at!(budget, function, incoming, entry.0 as usize),
                                    latched,
                                    budget,
                                );
                                if incoming_insertions.failed {
                                    return Err(incoming_insertions.failure().into());
                                }
                            } else {
                                path_child!(
                                    budget,
                                    function,
                                    CheckLatchCall,
                                    7,
                                    lexical_flow::check_latch(
                                        function,
                                        auth,
                                        exit,
                                        &latched,
                                        &loop_entries,
                                        budget,
                                    )
                                )?;
                                terminal_paths =
                                    terminal_paths.checked_add(1).ok_or_else(path_refusal!(
                                        budget,
                                        function,
                                        permit,
                                        usize,
                                        Kind::TooManyTerminalCleanupPaths
                                    ))?;
                            }
                        }
                        ExitFlow::Terminal => {
                            terminal_paths =
                                terminal_paths.checked_add(1).ok_or_else(path_refusal!(
                                    budget,
                                    function,
                                    permit,
                                    usize,
                                    Kind::TooManyTerminalCleanupPaths
                                ))?;
                            if matches!(
                                exit.continuation,
                                ExitContinuation::CommitResult { .. }
                                    | ExitContinuation::ReturnUnit
                            ) {
                                successful_paths =
                                    successful_paths.checked_add(1).ok_or_else(path_refusal!(
                                        budget,
                                        function,
                                        permit,
                                        usize,
                                        Kind::TooManySuccessfulCleanupPaths
                                    ))?;
                            }
                        }
                    }
                }
            }
        }
        let mut successor_entries = path_child!(
            budget,
            function,
            SuccessorsCall,
            4,
            successors(function, block_id, budget)
        )?;
        while let Some(successor) = dynamic_next!(
            budget,
            function,
            successor_entries,
            receiving::SuccessorEntriesNext<'_>,
            3
        ) {
            let remaining = counter_at!(
                budget,
                function,
                remaining_predecessors,
                successor.0 as usize
            );
            *remaining = remaining.checked_sub(1).ok_or_else(path_refusal!(
                budget,
                function,
                permit,
                usize,
                Kind::CleanupPredecessorCountUnderflowed
            ))?;
            if *remaining == 0 {
                queue.push_back(successor);
            }
        }
    }

    if terminal_paths == 0 || successful_paths == 0 {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::CleanupCfgHasNoReplayableTerminalSuccessPath,)
        ));
    }
    #[cfg(feature = "unstable-workflow-profiling")]
    crate::bounded_output::exclusive_debits::path_census::finish(terminal_paths, successful_paths);
    Ok(())
}

pub(super) type ControlKey = (
    Option<StatusSourceId>,
    Option<StatusSourceId>,
    Option<StagedCopyResultSource>,
    bool,
    Option<ReturnedSource>,
);
