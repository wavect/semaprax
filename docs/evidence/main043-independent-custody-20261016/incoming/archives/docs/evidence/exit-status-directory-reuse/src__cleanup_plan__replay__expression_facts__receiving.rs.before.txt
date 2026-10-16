//! Complete ordinary receiving dispatch; old tree branches remain compiled independently.
use super::*;
mod directory;
mod retained;
#[cfg(test)]
pub(in crate::cleanup_plan::replay) use retained::before_status_retention;
pub(in crate::cleanup_plan::replay) use retained::{
    validate_blocks_and_edges_retained, validate_status_sources_retained,
};
mod regions_entry;
mod remaining;
use directory::Directory;
pub(in crate::cleanup_plan::replay) use regions_entry::{validate_entry, validate_regions};
pub(in crate::cleanup_plan::replay) use remaining::validate_exits;

pub(in crate::cleanup_plan::replay) fn validate_status_sources(
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if let Facts::Tree(tree) = facts {
        return super::super::validate_status_sources(function, tree);
    }
    validate_status_sources_core::<Diagnostic>(function, facts, budget)
}

pub(in crate::cleanup_plan::replay) fn validate_status_sources_core<E: semantic_refusal::Error>(
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
) -> Result<(), E> {
    if let Facts::Tree(tree) = facts {
        return super::super::validate_status_sources_core::<E>(function, tree);
    }
    let expressions: &dyn Lookup = facts;
    let mut ids = Directory::new(function, budget, "cleanup status identity directory")?;
    for source in &function.cleanup_plan.status_sources {
        if !ids.insert(function, &source.id, budget)? {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        if expressions
            .fact(function, &source.id.expression, budget)?
            .is_none()
        {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        }
        match (&source.id.lane, &source.producer) {
            (
                StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                StatusProducer::PropagatedCall { callee },
            ) => {
                let Some(Some(call)) = expressions.fact(function, &source.id.expression, budget)?
                else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    )
                    .into());
                };
                if call.callee.as_ref() != callee {
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    )
                    .into());
                }
            }
            (
                StatusLane::OperationFailure,
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                },
            ) => {
                let mut cases = Directory::new(
                    function,
                    budget,
                    "cleanup checked arithmetic case directory",
                )?;
                for case in normalized_cases {
                    cases.insert(function, *case, budget)?;
                }
                if cases.is_empty() || cases.len() != normalized_cases.len() {
                    return Err(replay_error(
                        function,
                        "checked-arithmetic status cases are empty or repeated",
                    )
                    .into());
                }
            }
            (
                StatusLane::AllocationFailure,
                StatusProducer::CheckedAllocation {
                    normalized_cases, ..
                },
            ) if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
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
                )
                .into());
            }
        }
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn validate_blocks_and_edges(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if let Facts::Tree(tree) = facts {
        return super::super::validate_blocks_and_edges(
            program, function, tree, storage, leaves, helpers, budget,
        );
    }
    let expressions: &dyn Lookup = facts;
    let plan = &function.cleanup_plan;
    let mut statuses = Directory::new(function, budget, "cleanup status source directory")?;
    for source in &plan.status_sources {
        statuses.insert(function, &source.id, budget)?;
    }
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
        validate_edge_condition(function, &edge.condition, expressions, &statuses, budget)?;
    }

    let mut referenced_edges =
        Directory::new(function, budget, "cleanup referenced edge directory")?;
    let mut referenced_exits =
        Directory::new(function, budget, "cleanup referenced exit directory")?;
    let mut committed_calls = Directory::new(function, budget, "cleanup committed call directory")?;
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
                    require_expression(function, expressions, at, budget)?;
                    validate_place(function, destination, storage, leaves)?;
                }
                CleanupTransition::ReserveRenewal { at, binding } => {
                    renewal::validate_binding(program, function, at, binding, helpers, budget)?;
                    require_expression(function, expressions, at, budget)?;
                    validate_place(function, binding, storage, leaves)?;
                }
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    renewal::validate_binding(program, function, at, destination, helpers, budget)?;
                    require_expression(function, expressions, at, budget)?;
                    validate_place(function, source, storage, leaves)?;
                    validate_place(function, destination, storage, leaves)?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    require_expression(function, expressions, at, budget)?;
                    let source_flags = validate_place(function, source, storage, leaves)?;
                    let destination_flags = validate_place(function, destination, storage, leaves)?;
                    let mut source_directory =
                        Directory::new(function, budget, "cleanup transfer source flag directory")?;
                    for flag in source_flags {
                        source_directory.insert(function, flag, budget)?;
                    }
                    let mut destination_directory = Directory::new(
                        function,
                        budget,
                        "cleanup transfer destination flag directory",
                    )?;
                    for flag in destination_flags {
                        destination_directory.insert(function, flag, budget)?;
                    }
                    let source_flags = source_directory;
                    let destination_flags = destination_directory;
                    if source_flags.len() != destination_flags.len()
                        || !source_flags.is_disjoint(&destination_flags, function, budget)?
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
                    require_expression(function, expressions, at, budget)?;
                    validate_place(function, source, storage, leaves)?;
                    let Some(cases) = program.declarations().variant_cases(variant) else {
                        return Err(replay_error(
                            function,
                            "variant authentication references a non-variant declaration",
                        ));
                    };
                    if !cases.iter().any(|candidate| candidate.id == *case) {
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
                    require_expression(function, expressions, at, budget)?;
                    let source_flags = validate_place(function, source, storage, leaves)?;
                    let destination_flags = validate_place(function, destination, storage, leaves)?;
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
                    require_expression(function, expressions, at, budget)?;
                    validate_place(function, destination, storage, leaves)?;
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
                    let Some(Some(fact)) = expressions.fact(function, call, budget)? else {
                        return Err(replay_error(
                            function,
                            "call commit does not name a call expression",
                        ));
                    };
                    // Lexical return domains duplicate postcondition bodies, so one
                    // call legitimately commits once per proof domain; the typed
                    // skeleton comparison counts those per-domain commits exactly.
                    if !committed_calls.insert(function, call, budget)?
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
                    let mut consumed =
                        Directory::new(function, budget, "cleanup consumed flag directory")?;
                    for argument in arguments {
                        if previous.is_some_and(|value| value >= argument.parameter_index) {
                            return Err(replay_error(
                                function,
                                "call-commit parameters are not in strict signature order",
                            ));
                        }
                        previous = Some(argument.parameter_index);
                        let index = usize::try_from(argument.parameter_index).map_err(|_| {
                            replay_error(function, "call-commit parameter index does not fit usize")
                        })?;
                        let Some(expected_expression) = fact.arguments.get(index) else {
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
                                && parameter_index == &argument.parameter_index
                                && value_expression == expected_expression
                                && argument.source.projections.is_empty() => {}
                            _ => {
                                return Err(replay_error(
                                    function,
                                    "call commit does not consume its matching whole argument epoch",
                                ));
                            }
                        }
                        for flag in validate_place(function, &argument.source, storage, leaves)? {
                            if !consumed.insert(function, flag, budget)? {
                                return Err(replay_error(
                                    function,
                                    "atomic call commit consumes overlapping argument epochs",
                                ));
                            }
                        }
                    }
                }
                CleanupTransition::SelectFailure { source } => {
                    require_status_paid(function, &statuses, source, budget)?;
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
                        StagedCopyResultSource::Body { expression, .. } => {
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
                            require_expression(function, expressions, expression, budget)?;
                            require_expression(function, expressions, operand, budget)?;
                        }
                    }
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                validate_owned_edge_paid(function, block.id, *edge, &mut referenced_edges, budget)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always)
                    && !(matches!(
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
                    validate_owned_edge_paid(
                        function,
                        block.id,
                        *edge,
                        &mut referenced_edges,
                        budget,
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
                    || !referenced_exits.insert(function, *exit, budget)?
                {
                    return Err(replay_error(
                        function,
                        "cleanup block references an unknown or repeated exit",
                    ));
                }
                if plan.exits[exit.0 as usize].from != block.id {
                    return Err(replay_error(
                        function,
                        "cleanup exit is owned by the wrong block",
                    ));
                }
            }
        }
    }
    let mut expected_calls = Directory::new(function, budget, "cleanup expected call directory")?;
    for (expression, fact) in expressions.entries_paid(function, budget)? {
        if fact.is_some() {
            expected_calls.insert(function, expression, budget)?;
        }
    }
    // Control-flow-dead calls after an abrupt exit are never lowered, so the
    // lexical profile checks commit coverage in the typed skeleton instead.
    if plan.lexical_control.is_none()
        && !committed_calls.same_keys(&expected_calls, function, budget)?
    {
        return Err(replay_error(
            function,
            "cleanup plan does not contain exactly one atomic commit for every call",
        ));
    }
    Ok(())
}

fn validate_edge_condition(
    function: &ResolvedFunction,
    condition: &EdgeCondition,
    expressions: &dyn Lookup,
    statuses: &Directory<&StatusSourceId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    match condition {
        EdgeCondition::Always => Ok(()),
        EdgeCondition::BooleanResult(expression, _) => {
            require_expression(function, expressions, expression, budget)
        }
        EdgeCondition::VariantCase { scrutinee, .. } => {
            require_expression(function, expressions, scrutinee, budget)
        }
        EdgeCondition::ArmSelected { scrutinee, .. } => {
            require_expression(function, expressions, scrutinee, budget)
        }
        EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
            require_status_paid(function, statuses, source, budget)
        }
    }
}

fn require_expression(
    function: &ResolvedFunction,
    expressions: &dyn Lookup,
    expression: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if expressions.fact(function, expression, budget)?.is_some() {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format_args!("cleanup plan references unknown expression `{expression}`"),
        ))
    }
}

fn validate_owned_edge_paid(
    function: &ResolvedFunction,
    owner: BlockId,
    edge: EdgeId,
    referenced: &mut Directory<EdgeId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let Some(item) = plan.edges.get(edge.0 as usize) else {
        return Err(replay_error(
            function,
            "cleanup terminator references unknown edge",
        ));
    };
    if item.id != edge || item.from != owner || !referenced.insert(function, edge, budget)? {
        return Err(replay_error(
            function,
            "cleanup edge has the wrong owner or is referenced repeatedly",
        ));
    }
    Ok(())
}

fn require_status_paid(
    function: &ResolvedFunction,
    statuses: &Directory<&StatusSourceId>,
    source: &StatusSourceId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if statuses.contains(function, &source, budget)? {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format_args!(
                "cleanup plan references unknown status source `{}`",
                source.expression
            ),
        ))
    }
}

#[cfg(test)]
mod tests;
