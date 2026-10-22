//! #885 whole ordinary ABI with an actual lexical status/initial-edge owner.
//! SOURCE WIP: typed receiving ledger and independent controls pending review.
use super::super::require_expression;
use super::case_certified::retained_lookup;
use super::*;
use crate::cleanup_plan::replay::expression_facts::arena::retained::Session;
#[rustfmt::skip]
pub(in crate::cleanup_plan::replay) fn validate_structure_with_budget_storage_before_receiving<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayStructure,
    );
    let plan = &function.cleanup_plan;
    if helpers.is_some_and(|index| {
        !program
            .full_program()
            .is_some_and(|full| index.matches_program(full))
    }) {
        return Err(replay_error(
            function,
            "cleanup replay helper index belongs to a different typed program",
        ));
    }
    // Reconstruct the ordinary schema first, preserving all its current-source
    // probes/error order. A private source-held projection adds allocation
    // branches; neither its attached v20 string nor an ordinary context selects it.
    let ordinary_schema =
        schema::paid_summary::selected_with_budget(program, function, helpers, budget)?;
    let expected_schema = if allocation_paths::current(program, function).is_some() {
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
    } else {
        ordinary_schema
    };
    if plan.schema != expected_schema {
        return Err(replay_error(
            function,
            format_args!(
                "uses cleanup-plan schema `{}` instead of HIR-derived `{expected_schema}`",
                plan.schema
            ),
        ));
    }

    if lexical_cert::requires_lexical(function) {
        return lexical_replay::validate_with_index(program, function, budget, helpers);
    }
    if plan.lexical_control.is_some() {
        return Err(replay_error(
            function,
            "function without lexical exits carries a lexical-control certificate",
        ));
    }
    validate_replay_size_budget(function)?;
    budget.charge(
        function,
        plan_structure_units(plan),
        "structural validation",
    )?;

    let expression_facts = expression_facts::with_budget(function, budget)?;
    ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
    let status_cases =
        expression_facts::receiving::StatusCases::validate(program, function, budget)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    let statuses = 'status_publication: {
        let facts = &expression_facts;
        let proof = &status_cases;
        let mut attempt = case_proof::begin_receiving!(proof, function, budget);
        if let Facts::Tree(tree) = facts {
            crate::cleanup_plan::replay::validate_status_sources(function, tree)?;
            budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
            attempt.complete(function, budget)?;
            break 'status_publication StatusSources {
                function,
                budget,
                ledger: ClassifierLedger::capture(),
                ids: None,
            };
        }
        let expressions: &dyn Lookup = facts;
        let mut ids = Directory::new(function, budget, "cleanup status identity directory")?;
        // This direct borrowed predicate replaces the absent complete:false
        // initializer in the unchanged, already-funded lexical begin frame.
        if matches!(
            &budget.payer,
            &crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
                | &crate::cleanup_plan::replay::allocation::AllocationPayer::LegacyUnobserved
        ) {
            // Pay the complete new lexical selection receivers before length
            // inspection. This is an additional real fee, never credit from the proof
            // or either insertion owner. The tree lane above retains its old path.
            budget.reserve_bytes(
                function,
                3 * std::mem::size_of::<Result<(), Diagnostic>>()
                    + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, &str, usize)>(
                    )
                    + std::mem::size_of::<&ReplayBudget>()
                    + std::mem::size_of::<&Vec<crate::cleanup_plan::StatusSource>>()
                    + 2 * std::mem::size_of::<usize>()
                    + 4 * std::mem::size_of::<bool>(),
                "certified held-prefix selection controls",
            )?;
            budget.charge(function, 2, "certified held-prefix selection")?;
            if function.cleanup_plan.status_sources.len() >= 3 {
                let mut prefix_owner = ids.held_prefix(function, budget)?;
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<usize>(),
                    "status case row ordinal",
                )?;
                budget.charge(function, 1, "status case row ordinal")?;
                let mut ordinal = 0usize;
                // Peel exactly the first row; all its old guards and receiving
                // payments precede new selection. The remaining old loop is intact.
                let source = &function.cleanup_plan.status_sources[ordinal];
                if !prefix_owner.insert(function, &source.id, budget)? {
                    return Err(<Diagnostic as semantic_refusal::Error>::structural(
                        function,
                        semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
                    ));
                }
                // Retain the actual first lookup's borrowed payload, only through this
                // immutable source's producer match. Pay its complete receiving pointer
                // and one assignment step; no previous result/header credit is used.
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<&Option<CallFact<'_>>>(),
                    "retained status fact receiving control",
                )?;
                budget.charge(function, 1, "retained status fact receiving assignment")?;
                // Selection and owner formation occur only at the actual
                // first old lookup frontier. No future loop is precharged.
                budget.reserve_bytes(function, 0, "expression-fact arena lookup authentication")?;
                budget.charge(function, 0, "expression-fact arena lookup authentication")?;
                budget.reserve_bytes(
                    function,
                    retained_lookup::SELECT_BYTES,
                    "retained expression-fact selection controls",
                )?;
                if matches!(
                    &budget.payer,
                    &crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
                ) && function.cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS
                {
                    if let Facts::Arena(arena) = facts {
                        if arena.len() != 0 {
                            budget.reserve_bytes(
                                function,
                                Session::OLD_SCRATCH + Session::acquisition_bytes(),
                                "retained expression-fact owner acquisition",
                            )?;
                            budget.charge(
                                function,
                                12,
                                "retained expression-fact owner initialization",
                            )?;
                            let mut session = Session::new_prepaid(arena, function, budget);
                            loop {
                                let source = &function.cleanup_plan.status_sources[ordinal];
                                let Some(fact) = session.fact(&source.id.expression)? else {
                                    return Err(
                                        <Diagnostic as semantic_refusal::Error>::structural(
                                            function,
                                            semantic_refusal::Atom::UnknownStatus(
                                                source.id.expression.clone(),
                                            ),
                                        ),
                                    );
                                };
                                let budget = &mut *session.budget;
                                match (&source.id.lane, &source.producer) {
                                    (
                                        StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                                        StatusProducer::PropagatedCall { callee },
                                    ) => {
                                        // The payload reference is a second simultaneous receiving
                                        // control; the eliminated lookup's result is not a credit.
                                        budget.reserve_bytes(
                                            function,
                                            std::mem::size_of::<&CallFact<'_>>(),
                                            "retained status call receiving control",
                                        )?;
                                        budget.charge(
                                            function,
                                            1,
                                            "retained status call receiving assignment",
                                        )?;
                                        let Some(call) = fact else {
                                            return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    ));
                                        };
                                        if call.callee.as_ref() != callee {
                                            return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    ));
                                        }
                                    }
                                    (
                                        StatusLane::OperationFailure,
                                        StatusProducer::CheckedArithmetic {
                                            normalized_cases, ..
                                        },
                                    ) => {
                                        // Complete independent discovery proved the exact held row.
                                        // No case value is read here and no second case directory exists.
                                        proof.member(function, source, ordinal, budget)?;
                                        let _ = normalized_cases;
                                    }
                                    (
                                        StatusLane::AllocationFailure,
                                        StatusProducer::CheckedAllocation {
                                            normalized_cases, ..
                                        },
                                    ) if function.cleanup_plan.schema
                                        == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
                                        && *normalized_cases
                                            == [
                                                crate::cleanup_plan::AllocationCase::PayloadRefused,
                                                crate::cleanup_plan::AllocationCase::SizeOverflow,
                                            ] => {}
                                    (
                                        StatusLane::ContractFalse,
                                        StatusProducer::ContractFalse { .. },
                                    ) => {}
                                    _ => {
                                        return Err(replay_error(
                                            function,
                                            "status source lane does not match its producer",
                                        ));
                                    }
                                }
                                budget.charge(function, 1, "status case row ordinal increment")?;
                                ordinal += 1;
                                if ordinal == function.cleanup_plan.status_sources.len() {
                                    break;
                                }
                                let source = &function.cleanup_plan.status_sources[ordinal];
                                let budget = &mut *session.budget;
                                if !prefix_owner.insert(function, &source.id, budget)? {
                                    return Err(
                                        <Diagnostic as semantic_refusal::Error>::structural(
                                            function,
                                            semantic_refusal::Atom::RepeatedStatus(
                                                source.id.expression.clone(),
                                            ),
                                        ),
                                    );
                                }
                                // Retain the actual first lookup's borrowed payload, only through this
                                // immutable source's producer match. Pay its complete receiving pointer
                                // and one assignment step; no previous result/header credit is used.
                                budget.reserve_bytes(
                                    function,
                                    std::mem::size_of::<&Option<CallFact<'_>>>(),
                                    "retained status fact receiving control",
                                )?;
                                budget.charge(
                                    function,
                                    1,
                                    "retained status fact receiving assignment",
                                )?;
                            }
                            prefix_owner.freeze(function, session.budget)?;
                            session.budget.reserve_bytes(
                                function,
                                CONTROL_BYTES,
                                "retained status full controls",
                            )?;
                            attempt.complete(function, session.budget)?;
                            let statuses = StatusSources {
                                function,
                                budget: session.budget,
                                ledger: ClassifierLedger::capture(),
                                ids: Some(ids),
                            };
                            super::super::regions_entry::validate_regions(
                                function,
                                &storage,
                                session.budget,
                            )?;
                            super::super::regions_entry::validate_entry(
                                function,
                                &storage,
                                &leaves,
                                session.budget,
                            )?;
                            if !std::ptr::eq(function, statuses.function)
                                || !std::ptr::eq(session.budget, statuses.budget)
                            {
                                return Err(replay_error(
                                    function,
                                    "status directory belongs to another function or budget",
                                ));
                            }
                            if !statuses.ledger.current() {
                                return Err(session.budget.budget_refusal(
                                    function,
                                    "retained status invocation owner changed",
                                    "",
                                ));
                            }
                            // Authenticate the unchanged payer even though retaining the directory allocates nothing.
                            session.budget.reserve_bytes(
                                function,
                                0,
                                "retained cleanup status directory",
                            )?;
                            session.budget.charge(
                                function,
                                0,
                                "retained cleanup status directory",
                            )?;
                            let status_ids = statuses.ids.as_ref().ok_or_else(|| {
                                replay_error(
                                    function,
                                    "retained status directory has no metered keys",
                                )
                            })?;
                            for (index, edge) in plan.edges.iter().enumerate() {
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
                                    } => {
                                        session.budget.reserve_bytes(
                                            function,
                                            0,
                                            "expression-fact arena lookup authentication",
                                        )?;
                                        session.budget.charge(
                                            function,
                                            0,
                                            "expression-fact arena lookup authentication",
                                        )?;
                                        session.budget.reserve_bytes(
                                            function,
                                            Session::EDGE_RECEIVING_BYTES,
                                            "retained expression-fact query result controls",
                                        )?;
                                        #[allow(
                                            clippy::redundant_pattern_matching,
                                            reason = "bare pattern avoids a new Option method receiving boundary"
                                        )]
                                        if let None = session.fact_edge_prepaid(expression)? {
                                            return Err(replay_error(function,
                        format_args!("cleanup plan references unknown expression `{expression}`")));
                                        }
                                    }
                                    EdgeCondition::StatusZero(source)
                                    | EdgeCondition::StatusNonzero(source) => {
                                        require_status_paid(
                                            function,
                                            status_ids,
                                            source,
                                            session.budget,
                                        )?;
                                    }
                                }
                            }

                            // The existing owner ends before every program-dependent transition callback.
                            let budget = session.budget;
                            let expressions: &dyn Lookup = facts;
                            {
                                let storage = &storage;
                                let leaves = &leaves;
                                let statuses = status_ids;
                                // Preserve the original directory constructor's zero/current/sticky checks.
                                budget.reserve_bytes(
                                    function,
                                    0,
                                    "cleanup referenced exit directory",
                                )?;
                                budget.charge(function, 0, "cleanup referenced exit directory")?;
                                let mut committed_calls = Directory::new(
                                    function,
                                    budget,
                                    "cleanup committed call directory",
                                )?;
                                for (index, block) in plan.blocks.iter().enumerate() {
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
                                        match transition {
                                            CleanupTransition::Initialize { at, destination } => {
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                helpers::validate_place(
                                                    function,
                                                    destination,
                                                    storage,
                                                    leaves,
                                                    budget,
                                                )?;
                                            }
                                            CleanupTransition::ReserveRenewal { at, binding } => {
                                                renewal::validate_binding(
                                                    program, function, at, binding, helpers, budget,
                                                )?;
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                helpers::validate_place(
                                                    function, binding, storage, leaves, budget,
                                                )?;
                                            }
                                            CleanupTransition::Renew {
                                                at,
                                                source,
                                                destination,
                                            } => {
                                                renewal::validate_binding(
                                                    program,
                                                    function,
                                                    at,
                                                    destination,
                                                    helpers,
                                                    budget,
                                                )?;
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                helpers::validate_place(
                                                    function, source, storage, leaves, budget,
                                                )?;
                                                helpers::validate_place(
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
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                let source_flags = helpers::validate_place(
                                                    function, source, storage, leaves, budget,
                                                )?;
                                                let destination_flags = helpers::validate_place(
                                                    function,
                                                    destination,
                                                    storage,
                                                    leaves,
                                                    budget,
                                                )?;
                                                let mut source_directory = Directory::new(
                                                    function,
                                                    budget,
                                                    "cleanup transfer source flag directory",
                                                )?;
                                                for flag in source_flags {
                                                    source_directory
                                                        .insert(function, flag, budget)?;
                                                }
                                                let mut destination_directory = Directory::new(
                                                    function,
                                                    budget,
                                                    "cleanup transfer destination flag directory",
                                                )?;
                                                for flag in destination_flags {
                                                    destination_directory
                                                        .insert(function, flag, budget)?;
                                                }
                                                let source_flags = source_directory;
                                                let destination_flags = destination_directory;
                                                if source_flags.len() != destination_flags.len()
                                                    || !source_flags.is_disjoint(
                                                        &destination_flags,
                                                        function,
                                                        budget,
                                                    )?
                                                {
                                                    return Err(replay_error(
                            function,
                            "cleanup transfer has incompatible structural places",
                        ));
                                                }
                                            }
                                            CleanupTransition::AuthenticateVariantCase {
                                                at,
                                                source,
                                                variant,
                                                case,
                                            } => {
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                helpers::validate_place(
                                                    function, source, storage, leaves, budget,
                                                )?;
                                                let Some(cases) =
                                                    program.declarations().variant_cases(variant)
                                                else {
                                                    return Err(replay_error(
                            function,
                            "variant authentication references a non-variant declaration",
                        ));
                                                };
                                                if !cases
                                                    .iter()
                                                    .any(|candidate| candidate.id == *case)
                                                {
                                                    return Err(replay_error(
                            function,
                            "variant authentication references a foreign case",
                        ));
                                                }
                                                let slot = function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == source.storage)
                        .ok_or_else(|| {
                            replay_error(function, "variant authentication source has no slot")
                        })?;
                                                if !matches!(
                                                    &slot.ty,
                                                    ResolvedType::Nominal { declaration, .. } if declaration == variant
                                                ) {
                                                    return Err(replay_error(
                            function,
                            "variant authentication source type disagrees with its variant",
                        ));
                                                }
                                            }
                                            CleanupTransition::TransferVariant {
                                                at,
                                                source,
                                                destination,
                                                variant,
                                            } => {
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                let source_flags = helpers::validate_place(
                                                    function, source, storage, leaves, budget,
                                                )?;
                                                let destination_flags = helpers::validate_place(
                                                    function,
                                                    destination,
                                                    storage,
                                                    leaves,
                                                    budget,
                                                )?;
                                                if source_flags.len() != destination_flags.len()
                        || function
                            .cleanup_plan
                            .slots
                            .iter()
                            .filter(|slot| {
                                slot.storage == source.storage
                                    || slot.storage == destination.storage
                            })
                            .any(|slot| {
                                !matches!(
                                    &slot.ty,
                                    ResolvedType::Nominal { declaration, .. }
                                        if declaration == variant
                                )
                            })
                    {
                        return Err(replay_error(
                            function,
                            "variant transfer has incompatible conditional inventories",
                        ));
                    }
                                            }
                                            CleanupTransition::InitializeVariant {
                                                at,
                                                destination,
                                                variant,
                                            } => {
                                                require_expression(
                                                    function,
                                                    expressions,
                                                    at,
                                                    budget,
                                                )?;
                                                helpers::validate_place(
                                                    function,
                                                    destination,
                                                    storage,
                                                    leaves,
                                                    budget,
                                                )?;
                                                let slot = function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == destination.storage)
                        .ok_or_else(|| {
                            replay_error(function, "variant initialization has no destination slot")
                        })?;
                                                if !destination.projections.is_empty()
                                                    || !matches!(
                                                        &slot.ty,
                                                        ResolvedType::Nominal { declaration, .. }
                                                            if declaration == variant
                                                    )
                                                    || !matches!(
                                                        &slot.field_liveness_shape,
                                                        FieldLivenessShape::Variant { declaration, .. }
                                                            if declaration == variant
                                                    )
                                                {
                                                    return Err(replay_error(
                            function,
                            "variant initialization destination disagrees with its variant",
                        ));
                                                }
                                            }
                                            CleanupTransition::CallCommit { call, arguments } => {
                                                let Some(Some(fact)) =
                                                    expressions.fact(function, call, budget)?
                                                else {
                                                    return Err(replay_error(
                            function,
                            "call commit does not name a call expression",
                        ));
                                                };
                                                // Lexical return domains duplicate postcondition bodies, so one
                                                // call legitimately commits once per proof domain; the typed
                                                // skeleton comparison counts those per-domain commits exactly.
                                                if !committed_calls
                                                    .insert(function, call, budget)?
                                                    && plan.lexical_control.is_none()
                                                {
                                                    return Err(replay_error(
                                                        function,
                                                        "call has more than one atomic commit",
                                                    ));
                                                }
                                                let params = resolved_call::resolved_call_param_view(
                        program,
                        function,
                        find_resolved_expression(function, call).ok_or_else(|| {
                            replay_error(function, "call commit expression is absent")
                        })?,
                        &fact.callee,
                        fact.instance,
                        fact.type_arguments.as_slice(),
                    )
                    .map_err(|_| {
                        replay_error(
                            function,
                            format_args!("call commit has unknown callee `{}`", fact.callee),
                        )
                    })?;
                                                if params.len() != fact.arguments.len() {
                                                    return Err(replay_error(
                            function,
                            "call commit callee signature has inconsistent arity",
                        ));
                                                }
                                                if !resolved_call::owned_parameter_indices_match(
                                                    &params,
                                                    arguments,
                                                    function,
                                                    |ty| type_needs_drop(program, function, ty),
                                                )? {
                                                    return Err(replay_error(
                            function,
                            "call commit does not contain every and only owned droppable parameter",
                        ));
                                                }
                                                let mut previous = None;
                                                let mut consumed = Directory::new(
                                                    function,
                                                    budget,
                                                    "cleanup consumed flag directory",
                                                )?;
                                                for argument in arguments {
                                                    if previous.is_some_and(|value| {
                                                        value >= argument.parameter_index
                                                    }) {
                                                        return Err(replay_error(
                                function,
                                "call-commit parameters are not in strict signature order",
                            ));
                                                    }
                                                    previous = Some(argument.parameter_index);
                                                    let index = usize::try_from(argument.parameter_index).map_err(|_| {
                            replay_error(function, "call-commit parameter index does not fit usize")
                        })?;
                                                    let Some(expected_expression) =
                                                        fact.arguments.get(index)
                                                    else {
                                                        return Err(replay_error(
                                function,
                                "call-commit parameter index exceeds call arity",
                            ));
                                                    };
                                                    match &argument.source.storage {
                                                        StorageId::CallArgument {
                                                            call: storage_call,
                                                            parameter_index,
                                                            value_expression,
                                                        } if storage_call == call
                                                            && parameter_index
                                                                == &argument.parameter_index
                                                            && value_expression
                                                                == expected_expression
                                                            && argument
                                                                .source
                                                                .projections
                                                                .is_empty() => {}
                                                        _ => {
                                                            return Err(replay_error(
                                    function,
                                    "call commit does not consume its matching whole argument epoch",
                                ));
                                                        }
                                                    }
                                                    for flag in helpers::validate_place(
                                                        function,
                                                        &argument.source,
                                                        storage,
                                                        leaves,
                                                        budget,
                                                    )? {
                                                        if !consumed
                                                            .insert(function, flag, budget)?
                                                        {
                                                            return Err(replay_error(
                                    function,
                                    "atomic call commit consumes overlapping argument epochs",
                                ));
                                                        }
                                                    }
                                                }
                                            }
                                            CleanupTransition::SelectFailure { source } => {
                                                require_status_paid(
                                                    function, statuses, source, budget,
                                                )?;
                                                if selected.replace(source).is_some() {
                                                    return Err(replay_error(
                            function,
                            "cleanup block selects more than one failure status",
                        ));
                                                }
                                            }
                                            CleanupTransition::StageCopyResult { source } => {
                                                validate_staged_target(function, source)?;
                                                match source {
                                                    StagedCopyResultSource::Body {
                                                        expression,
                                                        ..
                                                    } => {
                                                        if expression != &function.body.id {
                                                            return Err(replay_error(
                                    function,
                                    "body Copy-result stage names another expression",
                                ));
                                                        }
                                                    }
                                                    StagedCopyResultSource::TryResidual {
                                                        expression,
                                                        operand,
                                                        ..
                                                    }
                                                    | StagedCopyResultSource::TryOptionNone {
                                                        expression,
                                                        operand,
                                                        ..
                                                    } => {
                                                        require_expression(
                                                            function,
                                                            expressions,
                                                            expression,
                                                            budget,
                                                        )?;
                                                        require_expression(
                                                            function,
                                                            expressions,
                                                            operand,
                                                            budget,
                                                        )?;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    match &block.terminator {
                                        CleanupTerminator::Goto(edge) => {
                                            validate_owned_edge_unique(function, block.id, *edge)?;
                                            if !matches!(
                                                plan.edges[edge.0 as usize].condition,
                                                EdgeCondition::Always
                                            ) && !(matches!(
                                                function.cleanup_plan.schema,
                                                CLEANUP_PLAN_SCHEMA_V6
                                                    | CLEANUP_PLAN_SCHEMA_V7
                                                    | CLEANUP_PLAN_SCHEMA_V8
                                                    | CLEANUP_PLAN_SCHEMA_V9
                                                    | CLEANUP_PLAN_SCHEMA_V10
                                                    | CLEANUP_PLAN_SCHEMA_V11
                                                    | CLEANUP_PLAN_SCHEMA_V12
                                                    | CLEANUP_PLAN_SCHEMA_V13
                                                    | CLEANUP_PLAN_SCHEMA_V15
                                                    | CLEANUP_PLAN_SCHEMA_V16
                                                    | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17
                                                    | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
                                                    | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
                                                    | CLEANUP_PLAN_SCHEMA_V14
                                            ) && matches!(
                                                plan.edges[edge.0 as usize].condition,
                                                EdgeCondition::VariantCase { matches: true, .. }
                                            )) {
                                                return Err(replay_error(
                                                    function,
                                                    "goto edge is conditional",
                                                ));
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
                                                validate_owned_edge_unique(
                                                    function, block.id, *edge,
                                                )?;
                                            }
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
                                            {
                                                return Err(replay_error(
                        function,
                        "cleanup block references an unknown or repeated exit",
                    ));
                                            }
                                            // The eliminated insert authenticated even an already-paid
                                            // slot. Keep both zero checks before the original owner read.
                                            budget.reserve_bytes(
                                                function,
                                                0,
                                                "cleanup referenced exit directory",
                                            )?;
                                            budget.charge(
                                                function,
                                                0,
                                                "cleanup referenced exit directory",
                                            )?;
                                            if plan.exits[exit.0 as usize].from != block.id {
                                                // Only a malformed owner needs a bounded previous-block
                                                // proof. Pay every control and step before these new reads.
                                                budget.reserve_bytes(
                                                    function,
                                                    exit_prefix::CONTROL_BYTES,
                                                    "cleanup prior exit reference controls",
                                                )?;
                                                budget.charge(
                                                    function,
                                                    exit_prefix::PROBE_UNITS,
                                                    "cleanup prior exit reference proof",
                                                )?;
                                                let mut probe = exit_prefix::Probe {
                                                    owner: plan.exits[exit.0 as usize].from,
                                                    current: block.id,
                                                    exit: *exit,
                                                    previous: None,
                                                    repeated: false,
                                                };
                                                if probe.owner.0 < probe.current.0 {
                                                    probe.previous =
                                                        plan.blocks.get(probe.owner.0 as usize);
                                                    if let Some(previous) = probe.previous {
                                                        probe.repeated = previous.id == probe.owner
                                                            && matches!(previous.terminator,
                                    CleanupTerminator::Exit(seen) if seen == probe.exit);
                                                    }
                                                }
                                                if probe.repeated {
                                                    return Err(replay_error(
                            function,
                            "cleanup block references an unknown or repeated exit",
                        ));
                                                }
                                                return Err(replay_error(
                                                    function,
                                                    "cleanup exit is owned by the wrong block",
                                                ));
                                            }
                                        }
                                    }
                                }
                                let mut expected_calls = Directory::new(
                                    function,
                                    budget,
                                    "cleanup expected call directory",
                                )?;
                                for (expression, fact) in
                                    expressions.entries_paid(function, budget)?
                                {
                                    if fact.is_some() {
                                        expected_calls.insert(function, expression, budget)?;
                                    }
                                }
                                // Control-flow-dead calls after an abrupt exit are never lowered, so the
                                // lexical profile checks commit coverage in the typed skeleton instead.
                                if plan.lexical_control.is_none()
                                    && !committed_calls.same_keys(
                                        &expected_calls,
                                        function,
                                        budget,
                                    )?
                                {
                                    return Err(replay_error(
            function,
            "cleanup plan does not contain exactly one atomic commit for every call",
        ));
                                }
                            }
                            expression_facts::receiving::validate_exits_retained(
                                program,
                                function,
                                &storage,
                                &leaves,
                                &BTreeSet::new(),
                                budget,
                                &statuses,
                            )?;
                            structural_maps::validate_reference_coverage(function, budget)?;
                            structural_control::validate_ordinary_cfg(function, budget)?;
                            if !cleanup_inert_path_product_can_be_summarized(function)?
                                && !status_only_paths_can_be_summarized(function)
                                && !cleanup_inert_large_decisions_can_be_summarized(function)
                            {
                                validate_path_states::<Diagnostic>(
                                    program, function, &storage, &leaves, budget, None, helpers,
                                )?;
                                validate_typed_control_skeleton_with_index(
                                    program,
                                    function,
                                    budget,
                                    helpers,
                                    continuations,
                                )?;
                            }
                            return Ok(());
                        }
                    }
                }
                let Some(fact) = expressions.fact(function, &source.id.expression, budget)? else {
                    return Err(<Diagnostic as semantic_refusal::Error>::structural(
                        function,
                        semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
                    ));
                };
                match (&source.id.lane, &source.producer) {
                    (
                        StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                        StatusProducer::PropagatedCall { callee },
                    ) => {
                        // The payload reference is a second simultaneous receiving
                        // control; the eliminated lookup's result is not a credit.
                        budget.reserve_bytes(
                            function,
                            std::mem::size_of::<&CallFact<'_>>(),
                            "retained status call receiving control",
                        )?;
                        budget.charge(function, 1, "retained status call receiving assignment")?;
                        let Some(call) = fact else {
                            return Err(replay_error(
                                function,
                                "propagated-call status source does not name a call expression",
                            ));
                        };
                        if call.callee.as_ref() != callee {
                            return Err(replay_error(
                                function,
                                "propagated-call status source names the wrong callee",
                            ));
                        }
                    }
                    (
                        StatusLane::OperationFailure,
                        StatusProducer::CheckedArithmetic {
                            normalized_cases, ..
                        },
                    ) => {
                        // Complete independent discovery proved the exact held row.
                        // No case value is read here and no second case directory exists.
                        proof.member(function, source, ordinal, budget)?;
                        let _ = normalized_cases;
                    }
                    (
                        StatusLane::AllocationFailure,
                        StatusProducer::CheckedAllocation {
                            normalized_cases, ..
                        },
                    ) if function.cleanup_plan.schema
                        == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
                        && *normalized_cases
                            == [
                                crate::cleanup_plan::AllocationCase::PayloadRefused,
                                crate::cleanup_plan::AllocationCase::SizeOverflow,
                            ] => {}
                    (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
                    _ => {
                        return Err(replay_error(
                            function,
                            "status source lane does not match its producer",
                        ));
                    }
                }
                budget.charge(function, 1, "status case row ordinal increment")?;
                ordinal += 1;
                while ordinal < function.cleanup_plan.status_sources.len() {
                    let source = &function.cleanup_plan.status_sources[ordinal];
                    if !prefix_owner.insert(function, &source.id, budget)? {
                        return Err(<Diagnostic as semantic_refusal::Error>::structural(
                            function,
                            semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
                        ));
                    }
                    // Retain the actual first lookup's borrowed payload, only through this
                    // immutable source's producer match. Pay its complete receiving pointer
                    // and one assignment step; no previous result/header credit is used.
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<&Option<CallFact<'_>>>(),
                        "retained status fact receiving control",
                    )?;
                    budget.charge(function, 1, "retained status fact receiving assignment")?;
                    let Some(fact) = expressions.fact(function, &source.id.expression, budget)?
                    else {
                        return Err(<Diagnostic as semantic_refusal::Error>::structural(
                            function,
                            semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
                        ));
                    };
                    match (&source.id.lane, &source.producer) {
                        (
                            StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                            StatusProducer::PropagatedCall { callee },
                        ) => {
                            // The payload reference is a second simultaneous receiving
                            // control; the eliminated lookup's result is not a credit.
                            budget.reserve_bytes(
                                function,
                                std::mem::size_of::<&CallFact<'_>>(),
                                "retained status call receiving control",
                            )?;
                            budget.charge(
                                function,
                                1,
                                "retained status call receiving assignment",
                            )?;
                            let Some(call) = fact else {
                                return Err(replay_error(
                                    function,
                                    "propagated-call status source does not name a call expression",
                                ));
                            };
                            if call.callee.as_ref() != callee {
                                return Err(replay_error(
                                    function,
                                    "propagated-call status source names the wrong callee",
                                ));
                            }
                        }
                        (
                            StatusLane::OperationFailure,
                            StatusProducer::CheckedArithmetic {
                                normalized_cases, ..
                            },
                        ) => {
                            // Complete independent discovery proved the exact held row.
                            // No case value is read here and no second case directory exists.
                            proof.member(function, source, ordinal, budget)?;
                            let _ = normalized_cases;
                        }
                        (
                            StatusLane::AllocationFailure,
                            StatusProducer::CheckedAllocation {
                                normalized_cases, ..
                            },
                        ) if function.cleanup_plan.schema
                            == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
                            && *normalized_cases
                                == [
                                    crate::cleanup_plan::AllocationCase::PayloadRefused,
                                    crate::cleanup_plan::AllocationCase::SizeOverflow,
                                ] => {}
                        (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
                        _ => {
                            return Err(replay_error(
                                function,
                                "status source lane does not match its producer",
                            ));
                        }
                    }
                    budget.charge(function, 1, "status case row ordinal increment")?;
                    ordinal += 1;
                }
                prefix_owner.freeze(function, budget)?;
                budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
                attempt.complete(function, budget)?;
                break 'status_publication StatusSources {
                    function,
                    budget,
                    ledger: ClassifierLedger::capture(),
                    ids: Some(ids),
                };
            }
        }
        let prefix_owner = directory::StatusInsertionOwner::new(function, &ids, budget)?;
        budget.reserve_bytes(
            function,
            std::mem::size_of::<usize>(),
            "status case row ordinal",
        )?;
        budget.charge(function, 1, "status case row ordinal")?;
        let mut ordinal = 0usize;
        // This paid ordinal advances only after the complete fallible row body.
        // A direct while-index keeps that frontier without an Enumerate carrier.
        while ordinal < function.cleanup_plan.status_sources.len() {
            let source = &function.cleanup_plan.status_sources[ordinal];
            if !ids.insert_certified(function, &source.id, budget, &prefix_owner)? {
                return Err(<Diagnostic as semantic_refusal::Error>::structural(
                    function,
                    semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
                ));
            }
            // Retain the actual first lookup's borrowed payload, only through this
            // immutable source's producer match. Pay its complete receiving pointer
            // and one assignment step; no previous result/header credit is used.
            budget.reserve_bytes(
                function,
                std::mem::size_of::<&Option<CallFact<'_>>>(),
                "retained status fact receiving control",
            )?;
            budget.charge(function, 1, "retained status fact receiving assignment")?;
            let Some(fact) = expressions.fact(function, &source.id.expression, budget)? else {
                return Err(<Diagnostic as semantic_refusal::Error>::structural(
                    function,
                    semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
                ));
            };
            match (&source.id.lane, &source.producer) {
                (
                    StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                    StatusProducer::PropagatedCall { callee },
                ) => {
                    // The payload reference is a second simultaneous receiving
                    // control; the eliminated lookup's result is not a credit.
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<&CallFact<'_>>(),
                        "retained status call receiving control",
                    )?;
                    budget.charge(function, 1, "retained status call receiving assignment")?;
                    let Some(call) = fact else {
                        return Err(replay_error(
                            function,
                            "propagated-call status source does not name a call expression",
                        ));
                    };
                    if call.callee.as_ref() != callee {
                        return Err(replay_error(
                            function,
                            "propagated-call status source names the wrong callee",
                        ));
                    }
                }
                (
                    StatusLane::OperationFailure,
                    StatusProducer::CheckedArithmetic {
                        normalized_cases, ..
                    },
                ) => {
                    // Complete independent discovery proved the exact held row.
                    // No case value is read here and no second case directory exists.
                    proof.member(function, source, ordinal, budget)?;
                    let _ = normalized_cases;
                }
                (
                    StatusLane::AllocationFailure,
                    StatusProducer::CheckedAllocation {
                        normalized_cases, ..
                    },
                ) if function.cleanup_plan.schema
                    == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
                    && *normalized_cases
                        == [
                            crate::cleanup_plan::AllocationCase::PayloadRefused,
                            crate::cleanup_plan::AllocationCase::SizeOverflow,
                        ] => {}
                (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
                _ => {
                    return Err(replay_error(
                        function,
                        "status source lane does not match its producer",
                    ));
                }
            }
            budget.charge(function, 1, "status case row ordinal increment")?;
            ordinal += 1;
        }
        drop(prefix_owner);
        budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
        attempt.complete(function, budget)?;
        StatusSources {
            function,
            budget,
            ledger: ClassifierLedger::capture(),
            ids: Some(ids),
        }
    };
    expression_facts::receiving::validate_regions(function, &storage, budget)?;
    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
    expression_facts::receiving::validate_blocks_and_edges_retained(
        program,
        function,
        &expression_facts,
        &storage,
        &leaves,
        helpers,
        budget,
        &statuses,
    )?;
    expression_facts::receiving::validate_exits_retained(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
        &statuses,
    )?;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)?;
    if !cleanup_inert_path_product_can_be_summarized(function)?
        && !status_only_paths_can_be_summarized(function)
        && !cleanup_inert_large_decisions_can_be_summarized(function)
    {
        validate_path_states::<Diagnostic>(
            program, function, &storage, &leaves, budget, None, helpers,
        )?;
        validate_typed_control_skeleton_with_index(
            program,
            function,
            budget,
            helpers,
            continuations,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::duplicate_mod,
    reason = "independent historical receivers retain their own before and test modules"
)]
#[path = "ordinary_cross_phase_before.rs"]
mod before;
#[cfg(test)]
#[allow(
    clippy::duplicate_mod,
    reason = "independent historical receivers retain their own before and test modules"
)]
#[path = "ordinary_cross_phase_tests.rs"]
mod tests;
