//! Same complete comparison pair; only the factored receiver borrows empty backing.
use super::*;
pub(super) fn cross_check<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    storage: &mut continuation_storage::Storage<'hir>,
) -> Result<(), Diagnostic> {
    let _debit_route = debit_routes::route(debit_routes::Route::UnoptedCrosscheck);
    let _debit_phase = debit_routes::phase(debit_routes::Phase::EnumeratedHIR);
    let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
    let mut enumerating = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    #[cfg(feature = "unstable-workflow-profiling")]
    let paid_enumeration = enumerating
        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckEnumerated);
    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {
        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);
        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
        let _debit_phase = debit_routes::phase(debit_routes::Phase::PathComparison);
        hir.sort();
        plan.sort();
        Ok(hir == plan)
    });
    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);
    #[cfg(feature = "unstable-workflow-profiling")]
    drop(paid_enumeration);
    if let Ok(true) = enumerated {
        let mut factored = ReplayBudget::with_skeleton_limit(0);
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_crosscheck = factored
            .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckFactored);
        let verdict =
            match compare_with_index_storage(program, function, &mut factored, None, Some(storage))
            {
                // A refused ledger cannot finance a verdict. Preserve the exact
                // error and let the caller report its sticky builder refusal.
                Err(error) if crate::bounded_output::active_refused() => {
                    SKELETON_MATERIALIZATIONS.with(|count| count.set(saved));
                    return Err(error);
                }
                verdict => verdict,
            };
        assert!(
            matches!(verdict, Ok(Some(true))),
            "factored skeleton comparison disagrees with enumeration for `{}`: {:?}",
            function.id,
            verdict.map_err(|error| error.message)
        );
    }
    SKELETON_MATERIALIZATIONS.with(|count| count.set(saved));
    Ok(())
}
#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
