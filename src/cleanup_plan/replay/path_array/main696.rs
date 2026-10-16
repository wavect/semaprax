//! Complete Main696 helper and current receivers; lexical adapters only.
//! Skip intermediate empty-result backings when output has a known minimum.
//! The original push loop still produces and meters every path. Acquiring the
//! same power-of-two capacity early preserves every subsequent growth target.
use super::super::*;
use super::tests;
use super::main696 as path_array;

pub(super) fn receiving_bytes() -> usize {
    use std::mem::size_of;
    2 * size_of::<Result<(), Diagnostic>>()
        + size_of::<&mut SkeletonWork<'_, '_>>()
        + size_of::<&mut Vec<ExprSkeletonPath>>()
        + size_of::<&ResolvedFunction>()
        + size_of::<&mut ReplayBudget>()
        + size_of::<&str>()
        + 3 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + 4 * size_of::<bool>()
}

#[track_caller]
pub(in crate::cleanup_plan::replay) fn minimum(
    paths: &mut Vec<ExprSkeletonPath>,
    count: usize,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if tests::original_selected() {
        return Ok(());
    }
    // Selection inspects existing headers only; it allocates and traverses
    // nothing. Small outputs retain their complete original physical route.
    if count < 8 || paths.capacity() != 0 || !work.budget.pays_allocations() {
        return Ok(());
    }
    work.budget
        .reserve_array::<u8>(work.function, receiving_bytes(), phase)?;
    let capacity = count.checked_next_power_of_two().ok_or_else(|| {
        work.budget.budget_refusal(
            work.function,
            "cleanup replay minimum path capacity overflowed",
            "",
        )
    })?;
    work.budget
        .reserve_vec(work.function, paths, capacity, phase)
}

pub(in crate::cleanup_plan::replay) fn sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    let mut combined = Vec::new();
    if !suffixes.is_empty() {
        path_array::minimum(
            &mut combined,
            prefixes.len(),
            work,
            "sequenced path capacity",
        )?;
    }
    for mut prefix in prefixes {
        if prefix.failed || prefix.residual {
            work.push_expr_path(&mut combined, prefix, "short-circuited skeleton path")?;
            continue;
        }
        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
        for suffix in suffixes.iter() {
            let observations =
                work.clone_observations(&prefix.observations, "skeleton prefix clone")?;
            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "skeleton suffix clone",
            )?;
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            work.push_expr_path(
                &mut combined,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                    continuation: lexical_skeleton::Continuation::sequenced(
                        &prefix.continuation,
                        &suffix.continuation,
                    ),
                },
                "sequenced skeleton path",
            )?;
        }
    }
    Ok(combined)
}

pub(in crate::cleanup_plan::replay) fn append_expr_paths(
    target: &mut Vec<ExprSkeletonPath>,
    paths: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    path_array::minimum(target, paths.len(), work, phase)?;
    for path in paths {
        work.push_expr_path(target, path, phase)?;
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn split_status_paths(
    paths: Vec<ExprSkeletonPath>,
    source: StatusSourceId,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut results = Vec::new();
    path_array::minimum(&mut results, paths.len(), work, "status path capacity")?;
    for mut path in paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut results, path, "short-circuited status path")?;
            continue;
        }
        let mut failure = work.clone_expr_path(&path, "status failure path clone")?;
        let failure_source = work.clone_owned(&source, "failure status-source clone")?;
        work.push_observation(
            &mut failure,
            SkeletonObservation::Status {
                source: failure_source,
                success: false,
            },
            "status failure observation",
        )?;
        failure.failed = true;
        failure.owned_source = None;
        work.push_expr_path(&mut results, failure, "status failure path")?;
        let success_source = work.clone_owned(&source, "success status-source clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Status {
                source: success_source,
                success: true,
            },
            "status success observation",
        )?;
        path.owned_source = None;
        work.push_expr_path(&mut results, path, "status success path")?;
    }
    Ok(results)
}

pub(in crate::cleanup_plan::replay) fn split_contract(
    paths: Vec<ExprSkeletonPath>,
    contract: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut results = Vec::new();
    path_array::minimum(&mut results, paths.len(), work, "contract path capacity")?;
    for mut path in paths {
        if path.failed || path.residual {
            work.push_expr_path(&mut results, path, "short-circuited contract path")?;
            continue;
        }
        let mut failure = work.clone_expr_path(&path, "contract failure path clone")?;
        let failure_expression =
            work.clone_owned(&contract.id, "contract failure expression clone")?;
        work.push_observation(
            &mut failure,
            SkeletonObservation::Boolean {
                expression: failure_expression,
                value: false,
            },
            "contract failure observation",
        )?;
        failure.failed = true;
        failure.owned_source = None;
        work.push_expr_path(&mut results, failure, "contract failure path")?;
        let success_expression =
            work.clone_owned(&contract.id, "contract success expression clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression: success_expression,
                value: true,
            },
            "contract success observation",
        )?;
        path.owned_source = None;
        work.push_expr_path(&mut results, path, "contract success path")?;
    }
    Ok(results)
}

thread_local! { static SELECTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(in crate::cleanup_plan::replay) fn selected() -> bool {
    SELECTED.with(std::cell::Cell::get)
}
pub(super) fn hir_skeleton_paths(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) { SELECTED.with(|flag| flag.set(self.0)); }
    }
    let _restore = Restore(SELECTED.with(|flag| flag.replace(true)));
    super::super::hir_skeleton_paths(program, function, budget)
}
