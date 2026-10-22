//! One immutable, fully paid status directory reused by consecutive ordinary passes.
//! Complete pre-retention receivers remain in the parent and execute independently.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;

// Pay both complete initialized return and receiving controls; no old Directory credit.
const CONTROL_BYTES: usize = std::mem::size_of::<Result<StatusSources<'static>, Diagnostic>>()
    + std::mem::size_of::<StatusSources<'static>>();

pub(in crate::cleanup_plan::replay) struct StatusSources<'hir> {
    function: &'hir ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    ids: Option<Directory<&'hir StatusSourceId>>,
}

pub(in crate::cleanup_plan::replay) fn validate_status_sources_retained<'hir>(
    function: &'hir ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
) -> Result<StatusSources<'hir>, Diagnostic> {
    if let Facts::Tree(tree) = facts {
        super::super::super::validate_status_sources(function, tree)?;
        budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
        return Ok(StatusSources {
            function,
            budget,
            ledger: ClassifierLedger::capture(),
            ids: None,
        });
    }
    if function.cleanup_plan.status_sources.len() >= 3 {
        return validate_status_sources_held_prefix(function, facts, budget);
    }
    let expressions: &dyn Lookup = facts;
    let mut ids = Directory::new(function, budget, "cleanup status identity directory")?;
    let prefix_owner = directory::StatusInsertionOwner::new(function, &ids, budget)?;
    for source in &function.cleanup_plan.status_sources {
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
                    ));
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
                ));
            }
        }
    }
    drop(prefix_owner);
    budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
    Ok(StatusSources {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
        ids: Some(ids),
    })
}

// The retained key proof is the sole additional borrowed argument to the complete receiver.
#[allow(clippy::too_many_arguments)]
pub(in crate::cleanup_plan::replay) fn validate_blocks_and_edges_retained(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
    validated: &StatusSources<'_>,
) -> Result<(), Diagnostic> {
    if !std::ptr::eq(function, validated.function) || !std::ptr::eq(budget, validated.budget) {
        return Err(replay_error(
            function,
            "status directory belongs to another function or budget",
        ));
    }
    if !validated.ledger.current() {
        return Err(budget.budget_refusal(
            function,
            "retained status invocation owner changed",
            "",
        ));
    }
    // Authenticate the unchanged payer even though retaining the directory allocates nothing.
    budget.reserve_bytes(function, 0, "retained cleanup status directory")?;
    budget.charge(function, 0, "retained cleanup status directory")?;
    if let Facts::Tree(tree) = facts {
        return super::super::super::validate_blocks_and_edges(
            program, function, tree, storage, leaves, helpers, budget,
        );
    }
    let expressions: &dyn Lookup = facts;
    let plan = &function.cleanup_plan;
    let statuses = validated
        .ids
        .as_ref()
        .ok_or_else(|| replay_error(function, "retained status directory has no metered keys"))?;
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
        validate_edge_condition(function, &edge.condition, expressions, statuses, budget)?;
    }

    // Preserve the original directory constructor's zero/current/sticky checks.
    budget.reserve_bytes(function, 0, "cleanup referenced exit directory")?;
    budget.charge(function, 0, "cleanup referenced exit directory")?;
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
                    helpers::validate_place(function, destination, storage, leaves, budget)?;
                }
                CleanupTransition::ReserveRenewal { at, binding } => {
                    renewal::validate_binding(program, function, at, binding, helpers, budget)?;
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, binding, storage, leaves, budget)?;
                }
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    renewal::validate_binding(program, function, at, destination, helpers, budget)?;
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, source, storage, leaves, budget)?;
                    helpers::validate_place(function, destination, storage, leaves, budget)?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    require_expression(function, expressions, at, budget)?;
                    let source_flags =
                        helpers::validate_place(function, source, storage, leaves, budget)?;
                    let destination_flags =
                        helpers::validate_place(function, destination, storage, leaves, budget)?;
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
                    helpers::validate_place(function, source, storage, leaves, budget)?;
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
                    let source_flags =
                        helpers::validate_place(function, source, storage, leaves, budget)?;
                    let destination_flags =
                        helpers::validate_place(function, destination, storage, leaves, budget)?;
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
                    helpers::validate_place(function, destination, storage, leaves, budget)?;
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
                        for flag in helpers::validate_place(
                            function,
                            &argument.source,
                            storage,
                            leaves,
                            budget,
                        )? {
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
                    require_status_paid(function, statuses, source, budget)?;
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
                validate_owned_edge_unique(function, block.id, *edge)?;
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
                    validate_owned_edge_unique(function, block.id, *edge)?;
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
                budget.reserve_bytes(function, 0, "cleanup referenced exit directory")?;
                budget.charge(function, 0, "cleanup referenced exit directory")?;
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
                        probe.previous = plan.blocks.get(probe.owner.0 as usize);
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

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_status_retention(path: &str, source: &str) -> String {
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/cleanup-status-directory-retention/exact-source-inverses.json"
    )))
    .unwrap();
    let record = records
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["source"] == path)
        .unwrap();
    let mut restored = source.to_owned();
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1, "{}", edit["name"]);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    restored
}

// Every block preceding this call has its exact contiguous enumeration ID.
// Goto has one edge; Branch rejects equal edge IDs before either call. An
// accepted immutable edge has exactly this owner, so no edge can have passed
// in an earlier distinct block. A second directory traversal proves nothing new.
fn validate_owned_edge_unique(
    function: &ResolvedFunction,
    owner: BlockId,
    edge: EdgeId,
) -> Result<(), Diagnostic> {
    let Some(item) = function.cleanup_plan.edges.get(edge.0 as usize) else {
        return Err(replay_error(
            function,
            "cleanup terminator references unknown edge",
        ));
    };
    if item.id != edge || item.from != owner {
        return Err(replay_error(
            function,
            "cleanup edge has the wrong owner or is referenced repeatedly",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod edge_before;
#[cfg(test)]
mod edge_tests;

#[cfg(test)]
mod status_before;
#[cfg(test)]
mod status_tests;

#[cfg(test)]
mod exit_before;
mod exit_prefix;
#[cfg(test)]
mod exit_tests;

mod exits;
pub(in crate::cleanup_plan::replay) use exits::validate_exits_retained;

#[cfg(test)]
mod status_before_prefix;

#[cfg(test)]
mod component_before_prefix;

// Same complete validation loop; only the owned insertion builder differs.
pub(in crate::cleanup_plan::replay) fn validate_status_sources_held_prefix<'hir>(
    function: &'hir ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
) -> Result<StatusSources<'hir>, Diagnostic> {
    if let Facts::Tree(tree) = facts {
        super::super::super::validate_status_sources(function, tree)?;
        budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
        return Ok(StatusSources {
            function,
            budget,
            ledger: ClassifierLedger::capture(),
            ids: None,
        });
    }
    let expressions: &dyn Lookup = facts;
    let mut ids = Directory::new(function, budget, "cleanup status identity directory")?;
    let mut prefix_owner = ids.held_prefix(function, budget)?;
    for source in &function.cleanup_plan.status_sources {
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
                    ));
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
                ));
            }
        }
    }
    prefix_owner.freeze(function, budget)?;
    budget.reserve_bytes(function, CONTROL_BYTES, "retained status full controls")?;
    Ok(StatusSources {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
        ids: Some(ids),
    })
}
