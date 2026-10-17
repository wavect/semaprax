//! Same complete comparison pair; only the factored receiver borrows empty backing.
use super::*;
pub(super) fn cross_check<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    storage: &mut continuation_storage::Storage<'hir>,
) -> Result<(), Diagnostic> {
    let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
    let mut enumerating = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {
        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
        hir.sort();
        plan.sort();
        Ok(hir == plan)
    });
    if let Ok(true) = enumerated {
        let mut factored = ReplayBudget::with_skeleton_limit(0);
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
