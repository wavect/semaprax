//! Complete immediate production comparison caller, selecting only the frozen receiver.
use super::super::super::*;
mod canonical_shared {
    pub(super) use super::super::*;
}
pub(in crate::cleanup_plan::replay::factored) fn compare_with_index_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Option<bool>, Diagnostic> {
    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    budget.reserve_bytes(
        function,
        canonical_shared::Canonicalizer::CONSTRUCTOR_CONTROLS,
        "canonical ordered dictionary owner receiving",
    )?;
    let mut canonical = canonical_shared::Canonicalizer::new(&automaton, sink);
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::Canonicalizer::COLD_NODE_CONTROLS,
            "factored cold terminal node predicate",
        )?;
    }
    if canonical.cold_dead_node(hir_root) {
        budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
        let Some(hir) = canonical.id_for_comparison(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
        let Some(plan) = canonical.id_for_comparison(function, vec![(plan_root, 1)], budget)?
        else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
            "factored held closure invocation controls",
        )?;
    }
    let mut receiving =
        canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(function, budget);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
    let Some(hir) =
        canonical.id_with_receiving(function, vec![(hir_root, 1)], budget, &mut receiving)?
    else {
        return Ok(None);
    };
    budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
    let Some(plan) =
        canonical.id_with_receiving(function, vec![(plan_root, 1)], budget, &mut receiving)?
    else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}
