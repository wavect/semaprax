//! Complete immediate-old actual ordinary constructor caller.
use super::super::*;

#[rustfmt::skip]
pub(in crate::cleanup_plan::replay) fn hir_skeleton_paths_with_index_storage_before<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayHirSkeleton,
    );
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
    // This is the original consuming for-loop's IntoIter/Option state, made
    // explicit to query its remaining length only after the first old push work.
    let mut paths = paths.into_iter();
    let path = match paths.next() {
        Some(path) => path,
        None => return Ok(completed),
    };
    let terminal = lexical_skeleton::terminal(&path);
    path_array::completed_first!(
        work,
        paths,
        completed,
        SkeletonPath {
            observations: path.observations,
            terminal
        },
        "completed HIR skeleton path"
    );
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
}
