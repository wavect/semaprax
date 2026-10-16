//! Complete incoming Main44 receivers; only namespace bindings are adapted.
use super::*;
pub(in crate::cleanup_plan::replay) fn sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    if sequence_vector::can_reuse(sequence_vector::Candidate {
        prefixes: &prefixes,
        suffixes: &suffixes,
        budget: work.budget,
    }) {
        work.budget.reserve_array::<u8>(
            work.function,
            sequence_vector::receiving_bytes(),
            "single-successor receiving frame",
        )?;
        return sequence_vector::single(prefixes, &suffixes, work, share_prefix);
    }
    let mut combined = Vec::new();
    if !suffixes.is_empty() {
        path_array::sequence::minimum(
            &mut combined,
            &prefixes,
            &suffixes,
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
        // This is the original for-loop's Iter/Option state. Earlier
        // successors borrow the prefix; the last one can take it intact.
        let mut remaining_suffixes = suffixes.iter();
        while let Some(suffix) = remaining_suffixes.next() {
            let observations = if sequence_prefix::can_take(sequence_prefix::Candidate {
                prefix: &prefix.observations,
                suffix: &suffix.observations,
                remaining_suffixes: remaining_suffixes.len(),
                budget: work.budget,
            }) {
                // Retain the original clone's work and materialization, while
                // funding the pure probe before transferring any owned field.
                work.charge(1, "skeleton prefix clone")?;
                work.budget.reserve_array::<u8>(
                    work.function,
                    sequence_prefix::RECEIVING_BYTES,
                    "skeleton prefix clone",
                )?;
                note_skeleton_materialization();
                std::mem::take(&mut prefix.observations)
            } else {
                work.clone_observations(&prefix.observations, "skeleton prefix clone")?
            };
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

pub(super) fn append_expr_paths(
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

pub(super) fn split_status_paths(
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

pub(super) fn split_contract(
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

pub(super) fn hir_skeleton_paths_with_index_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    with_active(|| {

    let mut work = SkeletonWork::checked_with_index(program, function, budget, helpers)?;
    work.continuations = continuations;
    let mut paths = work.singleton_path(empty_expr_path(), "HIR root path")?;
    for contract in &function.requires {
        paths = sequence_expression(program, function, paths, contract, &mut work)?;
        paths = split_contract(paths, contract, &mut work)?;
    }
    paths = sequence_expression(program, function, paths, &function.body, &mut work)?;
    let has_residual = paths.iter().any(|path| path.residual);
    let owned_result = nested_shape::type_needs_drop_with_budget(
        program,
        function,
        &function.return_type,
        work.budget,
    )?;
    if has_residual && !owned_result {
        if !function.cleanup_plan.slots.is_empty() {
            return Err(replay_error(
                function,
                "postfix `?` cleanup skeleton contains resource slots",
            ));
        }
        for path in &mut paths {
            if path.failed {
                continue;
            }
            if !path.residual {
                let expression =
                    work.clone_owned(&function.body.id, "body result expression clone")?;
                let instance = work.clone_owned(&function.return_type, "body result type clone")?;
                work.push_observation(
                    path,
                    SkeletonObservation::StageCopyResult(CopySource::Body {
                        expression,
                        instance,
                    }),
                    "body result staging",
                )?;
            }
            path.residual = false;
        }
    }
    if owned_result {
        let body_id = work.clone_owned(&function.body.id, "owned result expression clone")?;
        paths = transfer_completed_paths(
            function,
            paths,
            body_id,
            CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            },
            "owned function result",
            &mut work,
        )?;
    }
    if has_residual && owned_result {
        let provisional = CleanupPlace::whole(StorageId::ProvisionalResult);
        for path in &mut paths {
            if path.residual {
                if path.owned_source.as_ref() != Some(&provisional) {
                    return Err(replay_error(
                        function,
                        "owned postfix `?` residual does not stage the provisional result",
                    ));
                }
                path.residual = false;
            }
        }
    }
    // A lexical `return` has already staged its result and left its contour;
    // it now runs every postcondition exactly like a normally completed body.
    lexical_skeleton::release_returns(&mut paths);
    for contract in &function.ensures {
        paths = sequence_expression(program, function, paths, contract, &mut work)?;
        paths = split_contract(paths, contract, &mut work)?;
    }
    let mut completed = Vec::new();
    path_array::completed_minimum(
        &mut completed,
        paths.len(),
        &mut work,
        "completed HIR skeleton capacity",
    )?;
    for path in paths {
        let terminal = lexical_skeleton::terminal(&path);
        work.push_skeleton_path(
            &mut completed,
            SkeletonPath {
                observations: path.observations,
                terminal,
            },
            "completed HIR skeleton path",
        )?;
    }
    Ok(completed)
    })
}
