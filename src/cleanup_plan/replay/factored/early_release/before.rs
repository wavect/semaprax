// Complete immediate previous owner, independently executable.
use super::*;
#[rustfmt::skip]
fn build_comparison_before_release<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<BuiltComparison, Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase = budget
        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::FactoredConstruction);

    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayFactoredConstruction,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayFactoredConstruction,
    );
    // Both independent constructors debit skeleton helpers as well as ordinary
    // constructors. Keep one active allowance through the HIR walk AND plan
    // automaton construction; the held program reservation belongs to other
    // enumerated functions and cannot pay for this deferred attempt.
    #[cfg(test)]
    instrumentation::begin(function, budget);
    let skeleton_reserve = budget.skeleton_remaining;
    budget.skeleton_remaining = std::mem::take(&mut budget.remaining);
    budget.lent_skeleton = true;
    let result = (|| {
        budget.merge_paths = true;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::FactoredHIR);
        let hir_paths = hir_skeleton_paths_with_index_storage(
            program,
            function,
            budget,
            helpers,
            continuations,
        );
        budget.merge_paths = false;
        #[cfg(test)]
        instrumentation::phase(
            function,
            budget,
            "HIR skeleton",
            hir_paths.as_ref().map(|paths| paths.as_slice()).ok(),
        );
        let hir_paths = hir_paths?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::HIRAutomaton);
        let mut automaton = Automaton::default();
        let sink = automaton.node(function, budget)?;
        let success = automaton.end(function, budget, sink, SkeletonTerminal::Success)?;
        let failure = automaton.end(function, budget, sink, SkeletonTerminal::Failure)?;
        let latch = automaton.end(function, budget, sink, SkeletonTerminal::LoopLatch)?;
        let hir_root = automaton.node(function, budget)?;
        let mut memo = dag_memo::new(program, function, budget)?;
        for path in &hir_paths {
            let end = automaton.add_observations(
                function,
                &path.observations,
                hir_root,
                &mut memo,
                budget,
            )?;
            let terminal = match path.terminal {
                SkeletonTerminal::Success => success,
                SkeletonTerminal::Failure => failure,
                SkeletonTerminal::LoopLatch => latch,
            };
            automaton.epsilon(function, budget, end, terminal)?;
        }
        #[cfg(test)]
        instrumentation::phase(function, budget, "HIR automaton", None);
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::PLANAutomaton);
        let plan_root = automaton.add_plan(function, success, failure, latch, budget)?;
        #[cfg(test)]
        instrumentation::phase(function, budget, "plan automaton", None);
        Ok(BuiltComparison {
            automaton,
            sink,
            hir_root,
            plan_root,
        })
    })();
    // Restore only the unused allowance. Successful debits from either
    // constructor remain spent, including an attempt ending in refusal.
    budget.merge_paths = false;
    budget.lent_skeleton = false;
    budget.remaining = std::mem::replace(&mut budget.skeleton_remaining, skeleton_reserve);
    #[cfg(test)]
    instrumentation::phase(
        function,
        budget,
        if result.is_ok() {
            "constructed"
        } else {
            "constructor refusal"
        },
        None,
    );
    result
}
