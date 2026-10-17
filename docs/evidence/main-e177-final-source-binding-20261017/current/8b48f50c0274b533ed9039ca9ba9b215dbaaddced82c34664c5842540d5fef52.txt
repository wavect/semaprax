//! One real local binary-search owner; no retained source/payer or cached verdict.
use super::*;
use std::cmp::Ordering;
use std::mem::size_of;

pub(in crate::cleanup_plan::replay) struct SearchOwner {
    pub low: usize, pub high: usize, pub mid: usize, pub group: usize,
    pub position: usize, pub found: bool, pub ordering: Ordering,
}
// Three lexical reserve sites, but only TWO new caller frames: query and step.
// The path/call setup uses the existing subsequent acquisition already prepaid
// by DIRECTORY_CONTROLS (same &mut budget/function/usize/str ABI as the removed
// reserve_array call). Owned rows enlarge their EXISTING header call instead;
// construct! creates no function/caller/Result. OWNER_CONTROLS below pays both
// new query/step headers BEFORE either is formed, never its own initializer.
// At each sequential primitive site success is (), a sole owning error exits;
// no compound search Result or closure is constructed or retained.
const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>();
pub(in crate::cleanup_plan::replay) const OWNER_CONTROLS: usize = size_of::<SearchOwner>()
    + 2 * PAYMENT;
// Each query's Vec::len actual/formal refs and returned usize. Zero/false
// literals initialize retained fields directly; no function/Result is formed.
pub(in crate::cleanup_plan::replay) const QUERY_PAYLOAD: usize =
    2 * size_of::<&Vec<usize>>() + size_of::<usize>()
    // Found-output `order[position]`: distinct original lexical receiver.
    + 2 * size_of::<&Vec<usize>>() + 2 * size_of::<usize>() + size_of::<&usize>();
// Actual source-level Index ABI (same criterion as comparison_owner::PATH_INDEX).
// No charges for nonexistent Vec/slice forwarding bodies or other key kinds.
pub(in crate::cleanup_plan::replay) const ORDER_INDEX: usize =
    2 * size_of::<&Vec<usize>>() + 2 * size_of::<usize>() + size_of::<&usize>();
pub(in crate::cleanup_plan::replay) const PATH_INDEX: usize =
    2 * size_of::<&Vec<(ExprSkeletonPath, Vec<Observations>)>>()
    + 2 * size_of::<usize>() + size_of::<&(ExprSkeletonPath, Vec<Observations>)>();
pub(in crate::cleanup_plan::replay) const CALL_INDEX: usize =
    // Two distinct lexical group index operations: path then equal commit list.
    2 * (2 * size_of::<&Vec<(CallSkeletonState, Vec<Observations>)>>()
        + 2 * size_of::<usize>() + size_of::<&(CallSkeletonState, Vec<Observations>)>());
pub(in crate::cleanup_plan::replay) const ROW_INDEX: usize =
    2 * size_of::<&Vec<(usize, Vec<Observations>)>>()
    + 2 * size_of::<usize>() + size_of::<&(usize, Vec<Observations>)>()
    + 2 * size_of::<&[ExprSkeletonPath]>() + 2 * size_of::<usize>() + size_of::<&ExprSkeletonPath>();
// Complete frozen path-comparison receiving inventory, acquired for each actual
// step before compare_paths; no callback or binary-search Result survives.
pub(in crate::cleanup_plan::replay) const PATH_COMPARE: usize =
    4 * size_of::<&ExprSkeletonPath>() + 4 * size_of::<&Option<CleanupPlace>>()
    + 8 * size_of::<&bool>() + 4 * size_of::<&lexical_skeleton::Continuation>()
    // Four std cmp returns and four immediate receivers, plus the private
    // compare_paths return/caller receiver. Same complete frozen helper tariff.
    + 10 * size_of::<Ordering>() + 2 * size_of::<&usize>()
    // Distinct `other` pattern bindings in all three nested matches; prepaid
    // branch slots, never aliases of the return/receiving pairs above.
    + 3 * size_of::<Ordering>();
pub(in crate::cleanup_plan::replay) const CALL_COMPARE: usize = PATH_COMPARE
    + 4 * size_of::<&Vec<(u32, CleanupPlace)>>() // commit cmp actual/formal pairs
    + 2 * size_of::<Ordering>() // commit cmp return/receiver
    + size_of::<Ordering>(); // caller's distinct non-Equal `other` binding
// Four builtin operations have two copied operands and one result each:
// high-low, difference/2, low+quotient, mid+1. The terminal bound test has
// two operands and its bool. Even terminal/no-probe iterations acquire the
// full declared branch bank first; this conservative cost is never refunded.
pub(in crate::cleanup_plan::replay) const STEP_PAYLOAD: usize =
    4 * (2 * size_of::<usize>() + size_of::<usize>())
    + 2 * size_of::<usize>() + size_of::<bool>();

macro_rules! initialize {
    ($work:ident, $owner:ident) => {
        $work.budget.reserve_bytes($work.function,
            $crate::cleanup_plan::replay::skeleton_work::merge_search::OWNER_CONTROLS,
            "factored merge fee computation controls")?;
        $crate::cleanup_plan::replay::skeleton_work::merge_search::construct!($owner);
    };
}
pub(in crate::cleanup_plan::replay) use initialize;
macro_rules! construct {
    ($owner:ident) => {
        let mut $owner = $crate::cleanup_plan::replay::skeleton_work::merge_search::SearchOwner {
            low: 0, high: 0, mid: 0, group: 0, position: 0, found: false,
            ordering: std::cmp::Ordering::Equal,
        };
    };
}
pub(in crate::cleanup_plan::replay) use construct;
macro_rules! search {
    ($work:ident, $owner:ident, $order:ident, $index:expr, $comparison:expr, $compare:expr) => {{
        $work.budget.reserve_bytes($work.function,
            $crate::cleanup_plan::replay::skeleton_work::merge_search::QUERY_PAYLOAD,
            "factored retained merge query")?;
        $owner.low = 0; $owner.high = $order.len(); $owner.found = false;
        $owner.position = 0;
        loop {
            $work.budget.reserve_bytes($work.function,
                $crate::cleanup_plan::replay::skeleton_work::merge_search::ORDER_INDEX
                    + $index + $comparison
                    + $crate::cleanup_plan::replay::skeleton_work::merge_search::STEP_PAYLOAD,
                "factored retained merge comparison")?;
            if $owner.low >= $owner.high { $owner.position = $owner.low; break; }
            // low<=high<=len; no addition can exceed high. Search owns no key.
            $owner.mid = $owner.low + ($owner.high - $owner.low) / 2;
            $owner.group = $order[$owner.mid];
            $owner.ordering = $compare;
            match $owner.ordering {
                std::cmp::Ordering::Less => $owner.low = $owner.mid + 1,
                std::cmp::Ordering::Greater => $owner.high = $owner.mid,
                std::cmp::Ordering::Equal => { $owner.found = true; $owner.position = $owner.mid; break; }
            }
        }
    }};
}
pub(in crate::cleanup_plan::replay) use search;
