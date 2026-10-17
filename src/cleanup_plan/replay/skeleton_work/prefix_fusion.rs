//! Fuse only a freshly copied ordinary flat tail and its nonempty flat suffix.
//! The complete previous receiver remains the fallback. No intermediate Vec is
//! constructed or charged on the fused arm, and the final capacity is unchanged.
use super::*;
use std::mem::size_of;

// New lexical metadata, branch, payment, allocation and return receivers. These
// conservative slots include both checked-arithmetic Option arms, error map
// closures, the final Vec and Observations aggregate, and all borrowed headers.
// The original clone/extension/output receivers stay covered by their old fees.
pub(in crate::cleanup_plan::replay) const CONTROLS: usize = 8 * size_of::<Result<(), Diagnostic>>()
    + 8 * size_of::<Option<usize>>()
    + 20 * size_of::<usize>()
    + 12 * size_of::<bool>()
    + 12 * size_of::<&ReplayBudget>()
    + 8 * size_of::<&ResolvedFunction>()
    + 8 * size_of::<&str>()
    + 6 * size_of::<&Observations>()
    + 6 * size_of::<&Vec<Observation>>()
    + 4 * size_of::<&[Observation]>()
    + 4 * size_of::<&mut Vec<Observation>>()
    + size_of::<Vec<Observation>>()
    + size_of::<Observations>();

// This is lexical deliberately: no helper call, Result<Observations>, tuple,
// Option<Observations> or temporary owned target is introduced before payment.
// The caller has just performed the original prefix-clone work debit.
macro_rules! after_work {
    ($work:ident, $prefix:expr, $suffix:expr) => {{
        $work.budget.reserve_bytes(
            $work.function,
            $crate::cleanup_plan::replay::skeleton_work::prefix_fusion::CONTROLS,
            "ordinary prefix fusion receiving controls",
        )?;
        let prefix = &$prefix;
        let suffix = &$suffix;
        // Only a live production builder has no separately fallible oracle
        // work debit. Selected, oracle, unobserved, stale/sticky and work-short
        // receivers retain the complete original clone and extension below.
        let target = if $work.budget.payer
            == $crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
            && !$work.budget.selected()
            && !$work.budget.refusal_spent
            && $work.budget.skeleton_remaining >= 1
            && $work.budget.skeleton_spent < usize::MAX
            && prefix.prefix.is_none()
            && suffix.prefix.is_none()
            && !suffix.items.is_empty()
        {
            // Preserve clone payload arithmetic precedence: overflow declines
            // without replacing its original diagnostic. Growth is metadata
            // only here; no final backing is paid before extension work wins.
            prefix
                .items
                .len()
                .checked_mul(std::mem::size_of::<
                    std::rc::Rc<$crate::cleanup_plan::replay::SkeletonObservation>,
                >())
                .filter(|bytes| {
                    *bytes >= $crate::cleanup_plan::replay::skeleton_work::prefix_fusion::CONTROLS
                })
                .and_then(|_| prefix.items.len().checked_add(suffix.items.len()))
                .and_then(|required| {
                    $crate::cleanup_plan::replay::physical_storage::replay_growth_capacity(
                        prefix.items.len(),
                        required,
                    )
                })
                .filter(|target| {
                    target
                        .checked_mul(std::mem::size_of::<
                            std::rc::Rc<$crate::cleanup_plan::replay::SkeletonObservation>,
                        >())
                        .is_some()
                })
        } else {
            None
        };
        if let Some(target) = target {
            $crate::cleanup_plan::replay::note_skeleton_materialization();
            $work.charge(1, "skeleton suffix clone")?;
            $crate::cleanup_plan::replay::note_skeleton_materialization();
            // The original ordinary flat-growth frontier authenticates before
            // its full backing payment. The removed clone is never fabricated.
            $work
                .budget
                .reserve_bytes($work.function, 0, "ordinary flat suffix")?;
            $work
                .budget
                .reserve_array::<std::rc::Rc<$crate::cleanup_plan::replay::SkeletonObservation>>(
                    $work.function,
                    target,
                    "ordinary flat suffix",
                )?;
            let mut items = Vec::new();
            items.try_reserve_exact(target).map_err(|_| {
                $work.budget.budget_refusal(
                    $work.function,
                    "cleanup replay physical vector allocation failed",
                    "",
                )
            })?;
            if items.capacity() != target {
                return Err($work.budget.budget_refusal(
                    $work.function,
                    "cleanup replay physical vector capacity disagrees with its reservation",
                    "",
                ));
            }
            // Prefix remains borrowed and live until all handles are copied.
            // Clone handles in the same left-to-right prefix/suffix order.
            items.extend_from_slice(&prefix.items);
            items.extend_from_slice(&suffix.items);
            $crate::cleanup_plan::replay::Observations {
                prefix: None,
                items,
            }
        } else {
            $work
                .budget
                .reserve_clone($work.function, prefix, "skeleton prefix clone")?;
            $crate::cleanup_plan::replay::note_skeleton_materialization();
            let observations = prefix.clone();
            $work.extend_owned_observations(observations, suffix, "skeleton suffix clone")?
        }
    }};
}
pub(in crate::cleanup_plan::replay) use after_work;

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
