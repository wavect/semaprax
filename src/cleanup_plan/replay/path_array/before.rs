//! Complete independently compiled original path receivers.
use super::super::*;

pub(super) fn sequence_skeleton_paths(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let prefixes = work.merged_paths(prefixes)?;
    let suffixes = work.merged_suffixes(suffixes)?;
    let share_prefix = work.shares_prefix_tail(&suffixes)?;
    let mut combined = Vec::new();
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

pub(super) fn append_expr_paths(
    target: &mut Vec<ExprSkeletonPath>,
    paths: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
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
