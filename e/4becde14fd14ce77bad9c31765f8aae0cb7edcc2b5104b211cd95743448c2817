//! Complete immediate-old authenticated lexical comparison caller.
use super::super::*;

pub(in crate::cleanup_plan::replay) fn validate_authenticated(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    expression_count: usize,
) -> Result<(), Diagnostic> {
    // Explicit test forcing retains the complete old double-enumeration receiver.
    // Shipping builds construct no forwarding Result or borrowed arguments here.
    #[cfg(test)]
    if forced_strategy() == Some(Strategy::Enumerated) {
        return before_fitting::validate_authenticated(
            program,
            function,
            budget,
            helpers,
            expression_count,
        );
    }
    let built = build_comparison(program, function, budget, helpers, None)?;
    let mut candidate = Candidate::from_built(function, built, budget)?;
    if candidate.strategy == Strategy::RefusedArithmeticOverflow {
        return Err(replay_error(
            function,
            "cleanup replay exact terminal multiplicity overflowed",
        ));
    }
    // Selected physical replay has no unmetered legacy census fallback.
    // Its independently closed provider/type profile is unchanged.
    if budget.selected() {
        candidate.strategy = Strategy::LexicalFactored;
    }
    #[cfg(test)]
    if let Some(forced) = forced_strategy() {
        candidate.strategy = forced;
    }
    match candidate.strategy {
        Strategy::Enumerated => {
            if budget.selected() {
                return Err(replay_error(
                    function,
                    "selected physical replay requires its metered factored strategy",
                ));
            }
            // Preserve the complete legacy enumerating admission/preflight and
            // held reservation. The proof below reuses this exact Candidate;
            // no held skeleton units are refunded or lent to canonical work.
            let upper =
                lexical_census::validate_size_budget_and_skeleton(program, function, || {
                    Ok(expression_count)
                })?;
            reserve_enumerated(function, budget, upper)?;
        }
        Strategy::LexicalFactored => {}
        Strategy::RefusedArithmeticOverflow => unreachable!("overflow census already refused"),
    }
    // Both admission classes now consume the independently built exact language.
    // Counts never authenticate equality. This tail is the complete existing
    // weighted proof, with all original root/invocation/receiver payments.
    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = &candidate.built;
    let mut canonical = canonical_shared::inline_singleton::Canonicalizer::new(automaton, *sink);
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::inline_singleton::Canonicalizer::COLD_NODE_CONTROLS,
            "factored cold terminal node predicate",
        )?;
    }
    if canonical.cold_dead_node(*hir_root) {
        budget.reserve_array::<(usize, u64)>(function, 1, "lexical HIR weighted root")?;
        let hir = canonical.id_for_comparison(function, vec![(*hir_root, 1)], budget);
        #[cfg(test)]
        instrumentation::phase(function, budget, "HIR canonicalization", None);
        let hir = hir?;
        budget.reserve_array::<(usize, u64)>(function, 1, "lexical plan weighted root")?;
        let plan = canonical.id_for_comparison(function, vec![(*plan_root, 1)], budget);
        #[cfg(test)]
        instrumentation::phase(function, budget, "plan canonicalization", None);
        return match (hir, plan?) {
            (Some(hir), Some(plan)) if hir == plan => Ok(()),
            (Some(_), Some(_)) => Err(skeleton_mismatch(function)),
            _ => Err(replay_error(
                function,
                "cleanup replay factored trace multiplicity overflowed",
            )),
        };
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
    budget.reserve_array::<(usize, u64)>(function, 1, "lexical HIR weighted root")?;
    let hir = canonical.id_with_receiving(function, vec![(*hir_root, 1)], budget, &mut receiving);
    #[cfg(test)]
    instrumentation::phase(function, budget, "HIR canonicalization", None);
    let hir = hir?;
    budget.reserve_array::<(usize, u64)>(function, 1, "lexical plan weighted root")?;
    let plan = canonical.id_with_receiving(function, vec![(*plan_root, 1)], budget, &mut receiving);
    #[cfg(test)]
    instrumentation::phase(function, budget, "plan canonicalization", None);
    let plan = plan?;
    match (hir, plan) {
        (Some(hir), Some(plan)) if hir == plan => Ok(()),
        (Some(_), Some(_)) => Err(skeleton_mismatch(function)),
        _ => Err(replay_error(
            function,
            "cleanup replay factored trace multiplicity overflowed",
        )),
    }
}
