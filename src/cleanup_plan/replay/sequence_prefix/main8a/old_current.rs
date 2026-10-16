//! Complete Main-f1 receiver, bound to the preserved wrapper.
use super::before_wrapper as sequence_prefix;
use super::path_array;
use crate::cleanup_plan::replay::{lexical_skeleton, Diagnostic, ExprSkeletonPath, SkeletonWork};
pub(super) fn sequence_skeleton_paths(
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
            let observations = match sequence_prefix::try_take(
                &mut prefix.observations,
                &suffix.observations,
                suffixes.len(),
                work.function,
                work.budget,
                "skeleton prefix clone",
            )? {
                Some(observations) => observations,
                None => work.clone_observations(&prefix.observations, "skeleton prefix clone")?,
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
