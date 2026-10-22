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

// Replace real split/earlier-iterator receiving roles within the UNCHANGED
// once-per-prefix FINAL_SPLIT_CONTROLS payment. The original two Option tuple
// slots, slice headers and newly constructed earlier iterator disappear. New
// next_back returns ONE Option<&Path>; its Some pattern binds only &Path.
const REMOVED_SPLIT: usize = 2 * size_of::<&std::slice::Iter<'_, ExprSkeletonPath>>()
    + 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<Option<(&ExprSkeletonPath, &[ExprSkeletonPath])>>()
    + size_of::<&ExprSkeletonPath>()
    + size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>();
const NEW_SPLIT: usize = size_of::<&mut std::slice::Iter<'_, ExprSkeletonPath>>()
    + size_of::<Option<&ExprSkeletonPath>>()
    + size_of::<&ExprSkeletonPath>()
    + size_of::<std::slice::Iter<'static, ExprSkeletonPath>>();
// One FIRST-sibling gate only, never a repeated per-suffix amortization. Scalar
// slots cover both allowance fields, length result and bound operands; borrowed
// slots cover one borrowed payer pattern, selected(), two Option headers and Vec headers.
const PROBE: usize = size_of::<&allocation::AllocationPayer>()
    + size_of::<&ReplayBudget>()
    + 2 * size_of::<&Option<std::rc::Rc<ObservationDag>>>()
    + 2 * size_of::<&Vec<Observation>>()
    + 6 * size_of::<usize>()
    + 12 * size_of::<bool>();
const _: () = assert!(NEW_SPLIT + PROBE <= REMOVED_SPLIT);
pub(in crate::cleanup_plan::replay) const MINIMUM_ITEMS: usize =
    CONTROLS / size_of::<Observation>() + 1;

// No forwarded helper Result or temporary owned target exists. The caller has
// already charged the first original clone work and the unchanged split frame.
macro_rules! after_work {
    ($work:ident, $prefix:expr, $suffix:expr) => {{
        if matches!(
            &$work.budget.payer,
            &$crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
        )
            && !$work.budget.selected()
            && $work.budget.skeleton_remaining >= 1
            && $work.budget.skeleton_spent < usize::MAX
            && ($prefix).prefix.is_none()
            && ($suffix).prefix.is_none()
            && !($suffix).items.is_empty()
            && ($prefix).items.len()
                >= $crate::cleanup_plan::replay::skeleton_work::prefix_fusion::MINIMUM_ITEMS
        {
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::skeleton_work::prefix_fusion::CONTROLS,
                "ordinary prefix fusion receiving controls",
            )?;
            let prefix = &$prefix;
            let suffix = &$suffix;
            // All legal Vec<Observation> lengths have byte size <= isize::MAX.
            // Therefore adding two lengths, doubling a length, and multiplying
            // the resulting geometric target by pointer size fit usize on this
            // host. Retain checked arithmetic and the complete old fallback.
            let target = prefix
                .items
                .len()
                .checked_mul(std::mem::size_of::<
                    std::rc::Rc<$crate::cleanup_plan::replay::SkeletonObservation>,
                >())
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
                });
            if let Some(target) = target {
                $crate::cleanup_plan::replay::note_skeleton_materialization();
                $work.charge(1, "skeleton suffix clone")?;
                $crate::cleanup_plan::replay::note_skeleton_materialization();
                $work
                    .budget
                    .reserve_bytes($work.function, 0, "ordinary flat suffix")?;
                $work.budget.reserve_array::<std::rc::Rc<
                                    $crate::cleanup_plan::replay::SkeletonObservation,
                                >>($work.function, target, "ordinary flat suffix")?;
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
                // Keep the borrowed prefix alive through every ordered clone.
                items.extend_from_slice(&prefix.items);
                items.extend_from_slice(&suffix.items);
                $crate::cleanup_plan::replay::Observations {
                    prefix: None,
                    items,
                }
            } else {
                $crate::cleanup_plan::replay::skeleton_work::prefix_fusion::original!(
                    $work, $prefix, $suffix
                )
            }
        } else {
            $crate::cleanup_plan::replay::skeleton_work::prefix_fusion::original!(
                $work, $prefix, $suffix
            )
        }
    }};
}
macro_rules! original {
    ($work:ident, $prefix:expr, $suffix:expr) => {{
        $work
            .budget
            .reserve_clone($work.function, &$prefix, "skeleton prefix clone")?;
        $crate::cleanup_plan::replay::note_skeleton_materialization();
        let observations = ($prefix).clone();
        $work.extend_owned_observations(observations, &$suffix, "skeleton suffix clone")?
    }};
}
pub(in crate::cleanup_plan::replay) use {after_work, original};

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
