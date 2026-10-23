//! Direct copying when the continuation query costs more than it can save.
use super::*;
type Pointer = std::rc::Rc<SkeletonObservation>;
// The first reservation precedes the length predicate and its initialized
// borrowed/primitive receivers. No Decision, Candidate or query is constructed.
pub(in crate::cleanup_plan::replay) const SELECT: usize = size_of::<
    Result<(), physical_budget::ByteFailure>,
>() + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<&Vec<Pointer>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<bool>();
pub(in crate::cleanup_plan::replay) const COPY: usize = plan_compact_roles::<Pointer>();
const FULL: usize =
    production_capacity::production_inline_controls::<Pointer>() + TAKE_CALL_CONTROLS;
// Maximum backing avoided by either TAKE or bounded copy is doubled length.
// The small path wins even if the old optimizer would allocate NO Vec backing.
// The real old route keeps its original bank; none of it funds this new route.
pub(in crate::cleanup_plan::replay) const LIMIT: usize =
    (FULL - COPY - SELECT - 1) / (2 * size_of::<Pointer>()) + 1;
const _: () = assert!(FULL > COPY + SELECT);
const _: () = assert!(size_of::<Pointer>() < FULL - COPY - SELECT);

// Lexical production: there is no forwarding owner, argument list or Vec Result.
// The old full-query route remains in the caller for every larger history.
macro_rules! produce {
    ($budget:ident, $function:ident, $observations:ident, $observation:ident,
     $edge:ident, $queue:ident) => {{
        physical_budget::compact_try!($budget.reserve_compact_bytes(
            $function,
            $crate::cleanup_plan::replay::branch_clone::short_plan::SELECT,
            "branch short-history selection controls",
        ));
        if $observations.len() < $crate::cleanup_plan::replay::branch_clone::short_plan::LIMIT {
            physical_budget::compact_try!($budget.reserve_compact_bytes(
                $function,
                $crate::cleanup_plan::replay::branch_clone::short_plan::COPY,
                "branch short-history copy controls",
            ));
            let mut branch;
            $crate::cleanup_plan::replay::branch_clone::direct::copy_small_plan!(
                branch,
                $observations,
                $observation,
                $budget,
                $function
            );
            skeleton_queue_push(
                $budget,
                $function,
                &mut $queue,
                ($edge.to, branch),
                "plan branch state push",
            )?;
            continue;
        }
    }};
}
pub(in crate::cleanup_plan::replay) use produce;
