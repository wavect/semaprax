//! Complete immediate-old authenticated receiver; unchanged producer dependencies.
use super::*;

pub(in crate::cleanup_plan::replay) fn validate_authenticated(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    expression_count: usize,
) -> Result<(), Diagnostic> {
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
            // The actual census attempt remains spent. The legacy ordinary
            // route keeps its exact path/work ceilings and enumerating proof.
            let upper =
                lexical_census::validate_size_budget_and_skeleton(program, function, || {
                    Ok(expression_count)
                })?;
            reserve_enumerated(function, budget, upper)?;
            validate_typed_control_skeleton_with_index(program, function, budget, helpers, None)
        }
        Strategy::LexicalFactored => {
            let BuiltComparison {
                automaton,
                sink,
                hir_root,
                plan_root,
            } = &candidate.built;
            let mut canonical = canonical_shared::Canonicalizer::new(automaton, *sink);
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                    "factored held closure invocation controls",
                )?;
            }
            let mut receiving = canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(
                function, budget,
            );
            budget.reserve_array::<(usize, u64)>(function, 1, "lexical HIR weighted root")?;
            let hir =
                canonical.id_with_receiving(function, vec![(*hir_root, 1)], budget, &mut receiving);
            #[cfg(test)]
            instrumentation::phase(function, budget, "HIR canonicalization", None);
            let hir = hir?;
            budget.reserve_array::<(usize, u64)>(function, 1, "lexical plan weighted root")?;
            let plan = canonical.id_with_receiving(
                function,
                vec![(*plan_root, 1)],
                budget,
                &mut receiving,
            );
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
        Strategy::RefusedArithmeticOverflow => unreachable!("overflow census already refused"),
    }
}
