//! Reuse the existing authenticated complete status directory.
use super::*;

pub(super) fn validate(
    function: &ResolvedFunction,
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
    // Ineligible routes return to the unchanged original owner before paying
    // any retained-route control, so their bytes and work stay exactly as before.
    let Some(statuses) = validated.ids.as_ref() else {
        return structural_maps::validate_reference_coverage(function, budget);
    };
    if budget.selected()
        || !matches!(
            budget.payer,
            AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved
        )
        || statuses.len() < 8
    {
        return structural_maps::validate_reference_coverage(function, budget);
    }
    budget.reserve_bytes(function, 0, "retained reference directory authentication")?;
    budget.charge(function, 0, "retained reference directory authentication")?;
    budget.reserve_bytes(
        function,
        std::mem::size_of::<Result<(), Diagnostic>>()
            + 4 * std::mem::size_of::<bool>()
            + 2 * std::mem::size_of::<&StatusSources<'_>>()
            + std::mem::size_of::<Option<&Directory<&StatusSourceId>>>(),
        "retained reference selection controls",
    )?;
    let plan = &function.cleanup_plan;
    budget.reserve_array::<usize>(function, plan.edges.len(), "edge reference vector")?;
    let mut edge_references = vec![0_usize; plan.edges.len()];
    budget.reserve_array::<usize>(function, plan.exits.len(), "exit reference vector")?;
    let mut exit_references = vec![0_usize; plan.exits.len()];
    let mut status_references = BTreeSet::new();

    for block in &plan.blocks {
        for transition in &block.transitions {
            if let CleanupTransition::SelectFailure { source } = transition {
                insert_status(function, &mut status_references, source, budget)?;
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => edge_references[edge.0 as usize] += 1,
            CleanupTerminator::Branch(edges) => {
                for edge in edges {
                    edge_references[edge.0 as usize] += 1;
                }
            }
            CleanupTerminator::Exit(exit) => exit_references[exit.0 as usize] += 1,
        }
    }
    for edge in &plan.edges {
        match &edge.condition {
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                insert_status(function, &mut status_references, source, budget)?;
            }
            EdgeCondition::Always
            | EdgeCondition::BooleanResult(_, _)
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. } => {}
        }
    }
    for exit in &plan.exits {
        match &exit.continuation {
            ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => {
                edge_references[edge.0 as usize] += 1
            }
            ExitContinuation::ReturnFailure { source } => {
                insert_status(function, &mut status_references, source, budget)?;
            }
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnUnit
            | ExitContinuation::LoopLatch { .. } => {}
        }
    }
    if edge_references.iter().any(|count| *count != 1) {
        return Err(replay_error(
            function,
            "each cleanup edge must be referenced by exactly one control-flow owner",
        ));
    }
    if exit_references.iter().any(|count| *count != 1) {
        return Err(replay_error(
            function,
            "each cleanup exit must be referenced by exactly one block",
        ));
    }
    if !statuses.same_btree_keys(&status_references, function, budget)? {
        return Err(replay_error(
            function,
            "declared and referenced cleanup status sources differ",
        ));
    }
    Ok(())
}

fn insert_status<'plan>(
    function: &ResolvedFunction,
    statuses: &mut BTreeSet<&'plan StatusSourceId>,
    source: &'plan StatusSourceId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !statuses.contains(source) {
        // Both sets are local to validation and all identities belong to the
        // immutable plan. Reference order still compares every identity field.
        budget.reserve_btree_node::<&StatusSourceId, ()>(function, "status reference set")?;
        statuses.insert(source);
    }
    Ok(())
}
