//! Caller-funded direct clone and bounded current-HIR continuation query.
use super::*;

struct Bound {
    required: usize,
    forked: bool,
}
struct Query<'a> {
    plan: &'a crate::cleanup_plan::CleanupPlan,
    next: BlockId,
    remaining: usize,
    pushes: usize,
    forked: bool,
    block: Option<&'a crate::cleanup_plan::CleanupBlock>,
    edge: Option<&'a crate::cleanup_plan::CleanupEdge>,
    exit: Option<&'a crate::cleanup_plan::ExitTarget>,
}
// Full new query input actual/formal, result initializer/receiver, state and
// checked-expression receivers. The earlier inline payment owns this bank;
// neither this function nor a future skipped backing payment funds entry.
pub(in crate::cleanup_plan::replay) const QUERY_CONTROLS: usize = size_of::<Query<'static>>()
    + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
    + 2 * size_of::<BlockId>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Option<Bound>>()
    + 2 * size_of::<Bound>() // terminal initializer and returned caller binding
    + 6 * CHECKED_ARITHMETIC // transition, goto, branch, lexical marker, terminal2
    + 7 * size_of::<usize>() // transition len and three get actual/formal index pairs
    + 2 * size_of::<Option<&crate::cleanup_plan::CleanupBlock>>()
    + 2 * size_of::<Option<&crate::cleanup_plan::CleanupEdge>>()
    + 2 * size_of::<Option<&crate::cleanup_plan::ExitTarget>>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupBlock>>()
    + 2 * size_of::<&[crate::cleanup_plan::CleanupBlock]>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupEdge>>()
    + 2 * size_of::<&[crate::cleanup_plan::CleanupEdge]>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::ExitTarget>>()
    + 2 * size_of::<&[crate::cleanup_plan::ExitTarget]>()
    + 2 * size_of::<&Vec<crate::cleanup_plan::CleanupTransition>>()
    + 2 * size_of::<&Vec<EdgeId>>()
    + 2 * size_of::<&[EdgeId]>()
    + 2 * size_of::<Option<&EdgeId>>()
    + 5 * size_of::<&EdgeId>() // Goto, last edge, two lexical variants, common edge
    + 2 * size_of::<&crate::cleanup_plan::CleanupBlock>()
    + 2 * size_of::<&crate::cleanup_plan::CleanupEdge>()
    + 2 * size_of::<&crate::cleanup_plan::ExitTarget>()
    + size_of::<&crate::cleanup_plan::CleanupTerminator>()
    + size_of::<&crate::cleanup_plan::ExitContinuation>()
    + size_of::<&BlockId>()
    + size_of::<&crate::cleanup_plan::ExitTargetId>()
    + 8 * size_of::<bool>();

// checked_add/sub/mul take their receiver and operand by value: each has an
// actual and formal usize, plus the Option return before pattern consumption.
const CHECKED_ARITHMETIC: usize = 4 * size_of::<usize>() + size_of::<Option<usize>>();
// Separate actual current-probe additions, before its first input is formed.
// No callback/closure values exist: every result is consumed by a named match.
pub(in crate::cleanup_plan::replay) const PROBE_CONTROLS: usize = QUERY_CONTROLS
    + 3 * CHECKED_ARITHMETIC // required checked_add, avoided checked_mul, spare checked_sub
    + 4 * size_of::<usize>() // growth-capacity actual/formal inputs
    + size_of::<Option<usize>>() // growth-capacity return
    + 6 * size_of::<usize>() // required,growth,avoided binding,spare binding,two len values
    + 2 * size_of::<Option<usize>>() // named avoided/spare receivers
    + size_of::<bool>(); // named sufficiently_large receiver

fn bound(plan: &crate::cleanup_plan::CleanupPlan, target: BlockId, prefix: usize) -> Option<Bound> {
    let mut query = Query {
        plan,
        next: target,
        remaining: prefix,
        pushes: 0,
        forked: false,
        block: None,
        edge: None,
        exit: None,
    };
    while query.remaining != 0 {
        query.remaining -= 1;
        query.block = query.plan.blocks.get(query.next.0 as usize);
        let block = query.block?;
        query.pushes = query.pushes.checked_add(block.transitions.len())?;
        let edge = match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                query.pushes = query.pushes.checked_add(1)?;
                edge
            }
            CleanupTerminator::Branch(edges) => {
                if query.forked || edges.len() < 2 {
                    return None;
                }
                query.pushes = query.pushes.checked_add(1)?;
                query.forked = true;
                edges.last()?
            }
            CleanupTerminator::Exit(exit) => {
                query.exit = query.plan.exits.get(exit.0 as usize);
                match &query.exit?.continuation {
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnUnit
                    | ExitContinuation::ReturnFailure { .. } => {
                        return Some(Bound {
                            required: prefix.checked_add(1)?.checked_add(query.pushes)?,
                            forked: query.forked,
                        });
                    }
                    // The shipping enumerator follows these exact edges.
                    // A lexical continuation emits at most one marker, whereas
                    // Continue emits none. Neither is a terminal publication.
                    ExitContinuation::FunctionReturn { edge, .. }
                    | ExitContinuation::LoopBreak { edge, .. } => {
                        query.pushes = query.pushes.checked_add(1)?;
                        edge
                    }
                    ExitContinuation::Continue(edge) => edge,
                    ExitContinuation::LoopLatch { .. } => return None,
                }
            }
        };
        query.edge = query.plan.edges.get(edge.0 as usize);
        query.next = query.edge?.to;
    }
    None
}

pub(in crate::cleanup_plan::replay) fn probe(
    input: TakeCandidate<'_>,
) -> production_capacity::Decision {
    #[cfg(test)]
    if super::tests::original_selected() {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    }
    if input.budget.payer != AllocationPayer::ActiveBuilder || input.budget.selected() {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    }
    let Some(bound) = bound(input.plan, input.target, input.values.len()) else {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    };
    let Some(required) = input.values.len().checked_add(1) else {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    };
    let Some(growth) = physical_storage::replay_growth_capacity(input.values.len(), required)
    else {
        return production_capacity::Decision {
            take: false,
            copy_capacity: None,
        };
    };
    let avoided = growth.checked_mul(size_of::<std::rc::Rc<SkeletonObservation>>());
    let sufficiently_large = match avoided {
        Some(bytes) => bytes >= TAKE_BYTES,
        None => false,
    };
    let spare = growth.checked_sub(bound.required);
    production_capacity::Decision {
        take: !bound.forked
            && input.remaining_edges == 0
            && input.edge_count >= 2
            && input.values.capacity() > input.values.len()
            && input.values.capacity() <= growth
            && bound.required <= input.values.capacity()
            && sufficiently_large,
        copy_capacity: match spare {
            Some(spare) if spare >= production_capacity::COPY_SLOTS => Some(bound.required),
            _ => None,
        },
    }
}

// The direct generic call is real: all argument/formal receivers and the Vec
// Result initializer/receiver exist. Fund them at the earlier caller payment.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn copy_controls<T: Clone, V>() -> usize {
    QUERY_CONTROLS
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&Vec<T>>()
        + 2 * size_of::<V>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
        + 2 * size_of::<BlockId>()
        + 2 * size_of::<Result<Vec<T>, Diagnostic>>()
        + CHECKED_ARITHMETIC // required checked_add actual/formal/Option
        + 4 * size_of::<usize>() // growth-capacity actual/formal inputs
        + 4 * size_of::<Option<usize>>() // required/original/capacity initializer/receivers
        + 7 * size_of::<usize>() // required/original/capacity bindings and four len results
        + 4 * size_of::<&AllocationPayer>() // equality actual/formal left and right
        + 2 * size_of::<&ReplayBudget>() // selected actual/formal
        + 8 * size_of::<&Vec<T>>() // four len actual/formal pairs
        + 5 * size_of::<bool>() // payer, selected, lazy result, enabled, capacity fit
}

// Whole literal old inline census plus the direct bank. This replaces the
// prior zero-argument fee expression without an additional runtime fee call,
// argument, arithmetic helper Result or unpriced receiving usize.
pub(in crate::cleanup_plan::replay) const fn inline_controls<T: Clone, V>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
    + (
QUERY_CONTROLS
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&Vec<T>>()
        + 2 * size_of::<V>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
        + 2 * size_of::<BlockId>()
        + 2 * size_of::<Result<Vec<T>, Diagnostic>>()
        + CHECKED_ARITHMETIC // required checked_add actual/formal/Option
        + 4 * size_of::<usize>() // growth-capacity actual/formal inputs
        + 4 * size_of::<Option<usize>>() // required/original/capacity initializer/receivers
        + 7 * size_of::<usize>() // required/original/capacity bindings and four len results
        + 4 * size_of::<&AllocationPayer>() // equality actual/formal left and right
        + 2 * size_of::<&ReplayBudget>() // selected actual/formal
        + 8 * size_of::<&Vec<T>>() // four len actual/formal pairs
        + 5 * size_of::<bool>() // payer, selected, lazy result, enabled, capacity fit
    )
}

#[track_caller]
pub(in crate::cleanup_plan::replay) fn copy_prepaid<
    T: Clone + ClonePayload,
    V: physical_clone::IntoReplay<T>,
>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    observation: V,
    phase: &str,
    plan: &crate::cleanup_plan::CleanupPlan,
    target: BlockId,
) -> Result<Vec<T>, Diagnostic> {
    let required = values.len().checked_add(1);
    let original = match required {
        Some(required) => physical_storage::replay_growth_capacity(values.len(), required),
        None => None,
    };
    // Keep explicit Option arms: Option::map would add callback receivers outside this bank.
    #[allow(clippy::manual_map)]
    let capacity = if budget.payer == AllocationPayer::ActiveBuilder && !budget.selected() {
        #[cfg(test)]
        let enabled = !super::tests::original_selected();
        #[cfg(not(test))]
        let enabled = true;
        if enabled {
            match bound(plan, target, values.len()) {
                Some(bound) => Some(bound.required),
                None => None,
            }
        } else {
            None
        }
    } else {
        None
    };
    physical_budget::compact_try!(budget.charge_compact_skeleton(function, 1, phase));
    physical_budget::compact_try!(budget.reserve_compact_clone(function, values, phase));
    note_skeleton_materialization();
    // The immediate push work must refuse before every extra capacity debit,
    // exactly as in the complete predecessor producer.
    physical_budget::compact_try!(budget.charge_compact_skeleton(
        function,
        1,
        "plan branch observation push"
    ));
    note_skeleton_materialization();
    let original = match original {
        Some(original) => original,
        None => {
            return Err(budget.budget_refusal(
                function,
                "cleanup replay branch capacity overflowed",
                "",
            ));
        }
    };
    let capacity = match capacity {
        Some(capacity) if capacity <= original => capacity,
        _ => original,
    };
    physical_budget::compact_try!(budget.reserve_compact_array::<T>(
        function,
        capacity - values.len(),
        "plan branch observation push",
    ));
    physical_budget::compact_try!(budget.reserve_compact_bytes(
        function,
        physical_clone::conversion_bytes::<T, _>(&observation),
        "plan branch observation push",
    ));
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
    result.push(observation.into());
    #[cfg(test)]
    if capacity != original {
        note_plan_copy();
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod before;

// Const-only copy of the complete unchanged inline role bank. It introduces no
// runtime fee call; the real caller still acquires super::inline_controls.
const fn small_inline_controls<T: Clone>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
}

// Paid in the existing initial selection call, before the moved payer test or
// the new length predicate. No old role or later backing payment is credited.
pub(in crate::cleanup_plan::replay) const SMALL_SELECTION_CONTROLS: usize = 2 * size_of::<&ReplayBudget>() // pays_allocations actual/formal reborrow
    + 4 * size_of::<&AllocationPayer>() // inequality actual/formal left and right
    + size_of::<Vec<std::rc::Rc<SkeletonObservation>>>() // moved uninitialized named branch
    + 2 * size_of::<&Vec<std::rc::Rc<SkeletonObservation>>>() // len actual/formal
    + 4 * size_of::<usize>() // length result, comparison inputs and cutoff receiver
    + 6 * size_of::<bool>(); // inequality, return/receive, negation, length and condition

// The complete original initial bank plus the separately typed new receivers.
pub(in crate::cleanup_plan::replay) const PLAN_SMALL_SELECTION_CONTROLS: usize =
    super::super::final_output::PLAN_SELECTION_CONTROLS + SMALL_SELECTION_CONTROLS;

// For shorter histories even the largest possible reduction from doubled
// capacity to len+1 cannot cover the full direct-call/query bank and selection.
// This is a storage choice, never a program admission or work limit.
pub(in crate::cleanup_plan::replay) const SMALL_COPY_LIMIT: usize =
    (inline_controls::<std::rc::Rc<SkeletonObservation>, SkeletonObservation>()
        - small_inline_controls::<std::rc::Rc<SkeletonObservation>>()
        - SMALL_SELECTION_CONTROLS
        - 1)
        / size_of::<std::rc::Rc<SkeletonObservation>>()
        + 2;

// Statement expansion owns no helper inputs or Vec/Diagnostic return. Its
// complete unchanged inline census is acquired by the caller before expansion.
macro_rules! copy_small_plan {
    ($branch:ident, $values:ident, $observation:ident, $budget:ident, $function:ident) => {{
        physical_budget::compact_try!($budget.charge_compact_skeleton(
            $function,
            1,
            "cleanup-plan skeleton branch clone"
        ));
        physical_budget::compact_try!($budget.reserve_compact_clone(
            $function,
            &$values,
            "cleanup-plan skeleton branch clone"
        ));
        note_skeleton_materialization();
        // Keep the original push-work refusal ahead of additional backing.
        physical_budget::compact_try!($budget.charge_compact_skeleton(
            $function,
            1,
            "plan branch observation push"
        ));
        note_skeleton_materialization();
        let capacity = if $values.is_empty() {
            1
        } else {
            if $values.len() > usize::MAX / 2 {
                return Err($budget.budget_refusal(
                    $function,
                    "cleanup replay branch capacity overflowed",
                    "",
                ));
            }
            $values.len() * 2
        };
        physical_budget::compact_try!($budget
            .reserve_compact_array::<std::rc::Rc<SkeletonObservation>>(
                $function,
                capacity - $values.len(),
                "plan branch observation push",
            ));
        physical_budget::compact_try!($budget.reserve_compact_bytes(
            $function,
            physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(&$observation),
            "plan branch observation push",
        ));
        $branch = Vec::with_capacity(capacity);
        $branch.extend($values.iter().cloned());
        $branch.push($observation.into());
        #[cfg(test)]
        $crate::cleanup_plan::replay::branch_clone::direct::note_small_copy();
    }};
}
pub(in crate::cleanup_plan::replay) use copy_small_plan;

// Descriptive fixed-size observer; it supplies no physical or work authority.
#[cfg(test)]
std::thread_local! {
    static SMALL_COPIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn note_small_copy() {
    SMALL_COPIES.with(|slot| slot.set(slot.get().checked_add(1).unwrap()));
}
#[cfg(test)]
fn take_small_copies() -> usize {
    SMALL_COPIES.with(|slot| slot.replace(0))
}

// Current compact producer receiving roles; original banks above fund retained controls.
pub(in crate::cleanup_plan::replay) const fn compact_roles<T: Clone, V>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), physical_budget::ByteFailure>>() // compact clone/push work, census/conversion
        + 2 * size_of::<Result<(), physical_budget::ByteFailure>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, physical_budget::ByteFailure>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
    + (
QUERY_CONTROLS
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&Vec<T>>()
        + 2 * size_of::<V>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
        + 2 * size_of::<BlockId>()
        + 2 * size_of::<Result<Vec<T>, Diagnostic>>()
        + CHECKED_ARITHMETIC // required checked_add actual/formal/Option
        + 4 * size_of::<usize>() // growth-capacity actual/formal inputs
        + 4 * size_of::<Option<usize>>() // required/original/capacity initializer/receivers
        + 7 * size_of::<usize>() // required/original/capacity bindings and four len results
        + 4 * size_of::<&AllocationPayer>() // equality actual/formal left and right
        + 2 * size_of::<&ReplayBudget>() // selected actual/formal
        + 8 * size_of::<&Vec<T>>() // four len actual/formal pairs
        + 5 * size_of::<bool>() // payer, selected, lazy result, enabled, capacity fit
    )
}

const fn compact_small_roles<T: Clone>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), physical_budget::ByteFailure>>() // compact clone/push work, census/conversion
        + 2 * size_of::<Result<(), physical_budget::ByteFailure>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, physical_budget::ByteFailure>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
}

// Both current banks remove the same seven Result envelopes. Their difference,
// and therefore the existing cost-derived small-copy boundary, is unchanged.
const _: () = assert!(
    inline_controls::<std::rc::Rc<SkeletonObservation>, SkeletonObservation>()
        - small_inline_controls::<std::rc::Rc<SkeletonObservation>>()
        == compact_roles::<std::rc::Rc<SkeletonObservation>, SkeletonObservation>()
            - compact_small_roles::<std::rc::Rc<SkeletonObservation>>()
);

// Complete current bank with only its new caller payment Result narrowed.
pub(in crate::cleanup_plan::replay) const fn plan_compact_roles<T: Clone, V>() -> usize {
    size_of::<Result<(), physical_budget::ByteFailure>>() // actual compact caller payment result
        + 4 * size_of::<Result<(), physical_budget::ByteFailure>>() // compact clone/push work, census/conversion
        + 2 * size_of::<Result<(), physical_budget::ByteFailure>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, physical_budget::ByteFailure>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
    + (
QUERY_CONTROLS
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&Vec<T>>()
        + 2 * size_of::<V>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<&crate::cleanup_plan::CleanupPlan>()
        + 2 * size_of::<BlockId>()
        + 2 * size_of::<Result<Vec<T>, Diagnostic>>()
        + CHECKED_ARITHMETIC // required checked_add actual/formal/Option
        + 4 * size_of::<usize>() // growth-capacity actual/formal inputs
        + 4 * size_of::<Option<usize>>() // required/original/capacity initializer/receivers
        + 7 * size_of::<usize>() // required/original/capacity bindings and four len results
        + 4 * size_of::<&AllocationPayer>() // equality actual/formal left and right
        + 2 * size_of::<&ReplayBudget>() // selected actual/formal
        + 8 * size_of::<&Vec<T>>() // four len actual/formal pairs
        + 5 * size_of::<bool>() // payer, selected, lazy result, enabled, capacity fit
    )
}
