//! Compact storage controls; every measurement includes the final owning Drop.
use super::frontier::{Incoming, Insertions};
use super::*;
// Keep every original direct helper control against its complete old producer.
use super::held_edge_before as boxed_flow;
use crate::bounded_output::{
    active_remaining, reserve_active_required, set_active_floor, with_limit_usage_refusal,
};
use crate::test_allocation_observer::{current_metrics, measure};
use std::{cell::Cell, mem::size_of};
thread_local! { static DROPS:Cell<usize> = const { Cell::new(0) }; }
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug)]
struct State([u64; 50]);
impl Drop for State {
    fn drop(&mut self) {
        DROPS.with(|n| n.set(n.get() + 1));
    }
}
fn function() -> ResolvedFunction {
    crate::hir::resolve(
        &crate::check(
            "module compact; @id(\"main\") fn main()->i64 {0}",
            "compact.spx",
        )
        .unwrap(),
    )
    .unwrap()
    .functions
    .remove(0)
}
fn scalar(value: u64) -> State {
    State([value; 50])
}
fn pointer<T: Ord>(value: &Incoming<T>) -> *const T {
    std::ptr::from_ref(value.one().as_ref())
}
fn bootstrap(f: &ResolvedFunction) -> usize {
    f.id.as_str().len() + 512
}
fn empty() -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: Vec::new(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
        returned: None,
    }
}
#[test]
fn empty_one_duplicate_promotion_both_orders_and_tree_splits_keep_exact_owners() {
    let f = function();
    for reverse in [false, true] {
        DROPS.with(|n| n.set(0));
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(1 << 24, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut receiver = Insertions::acquire(&f, &mut budget).unwrap();
                let mut target = Incoming::default();
                assert!(target.is_empty());
                let first = if reverse { 127 } else { 0 };
                receiver.insert_owned(&f, &mut target, scalar(first), &mut budget);
                assert!(!receiver.failed);
                let original = pointer(&target);
                receiver.insert_owned(&f, &mut target, scalar(first), &mut budget);
                assert!(!receiver.failed);
                assert_eq!(pointer(&target), original);
                assert_eq!(DROPS.with(Cell::get), 1);
                for step in 1..128 {
                    let n = if reverse { 127 - step } else { step };
                    receiver.insert_owned(&f, &mut target, scalar(n), &mut budget);
                    assert!(!receiver.failed);
                }
                assert!(matches!(target, Incoming::Many(_)));
                assert_eq!(target.len(), 128);
                assert_eq!(
                    target.iter().map(|s| s.0[0]).collect::<Vec<_>>(),
                    (0..128).collect::<Vec<_>>()
                );
                assert!(target
                    .iter()
                    .any(|s| std::ptr::from_ref(s.as_ref()) == original));
                drop(target);
                drop(receiver);
                drop(budget);
                assert_eq!(DROPS.with(Cell::get), 129);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}
#[test]
fn held_singleton_transfer_and_equal_held_duplicate_never_rebox_or_replace_existing_owner() {
    let f = function();
    DROPS.with(|n| n.set(0));
    let (((), allocation), refused, paid, _) = with_limit_usage_refusal(1 << 20, || {
        measure(|| {
            let mut b = ReplayBudget::new();
            let mut r = Insertions::acquire(&f, &mut b).unwrap();
            let mut source = Incoming::default();
            let mut target = Incoming::default();
            r.insert_owned(&f, &mut source, scalar(7), &mut b);
            let original = pointer(&source);
            let held = std::mem::take(&mut source).into_one().next().unwrap();
            let allocations = current_metrics().allocations;
            r.insert_held(&f, &mut target, held, &mut b);
            assert!(!r.failed);
            assert_eq!(pointer(&target), original);
            assert_eq!(current_metrics().allocations, allocations);
            r.insert_owned(&f, &mut source, scalar(7), &mut b);
            r.insert_held(&f, &mut target, source.into_one().next().unwrap(), &mut b);
            assert!(!r.failed);
            assert_eq!(pointer(&target), original);
            assert_eq!(DROPS.with(Cell::get), 1);
            drop(target);
            drop(r);
            drop(b);
            assert_eq!(DROPS.with(Cell::get), 2);
        })
    });
    assert!(!refused);
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}
#[test]
fn each_constructor_one_and_promotion_payment_frontier_preserves_partial_drop_and_sticky_first_report(
) {
    let f = function();
    let acquisition =
        bootstrap(&f) + Insertions::<State>::controls() + Insertions::<State>::entry_controls();
    let one =
        size_of::<State>() + 2 * size_of::<Box<State>>() + Insertions::<State>::entry_controls();
    let promotion =
        2 * crate::cleanup_plan::replay::physical_storage::btree_node_upper::<Box<State>, ()>()
            .unwrap()
            + one;
    for limit in [
        0,
        acquisition - 1,
        acquisition,
        acquisition + one - 1,
        acquisition + one,
        acquisition + one + promotion - 1,
        acquisition + one + promotion,
    ] {
        let ((success, a), refused, paid, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                let mut target = Incoming::default();
                let mut r = match Insertions::acquire(&f, &mut b) {
                    Ok(r) => r,
                    Err(e) => {
                        drop(e);
                        return false;
                    }
                };
                r.insert_owned(&f, &mut target, scalar(1), &mut b);
                if !r.failed {
                    r.insert_owned(&f, &mut target, scalar(2), &mut b);
                }
                let success = !r.failed;
                if !success {
                    let before = target.len();
                    let allocations = current_metrics().allocations;
                    r.insert_owned(&f, &mut target, scalar(3), &mut b);
                    assert_eq!(target.len(), before);
                    assert_eq!(current_metrics().allocations, allocations);
                    drop(r.failure());
                }
                drop(target);
                drop(r);
                drop(b);
                success
            })
        });
        assert_eq!(success, limit == acquisition + one + promotion);
        assert_eq!(refused, !success);
        assert_eq!(first.is_some(), !success);
        assert!(paid <= limit);
        assert!(!a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
        assert!(a.allocated <= paid);
    }
}
#[test]
fn warm_empty_one_many_and_duplicates_keep_zero_authentication_floor_and_prior_refusal() {
    let f = function();
    for count in 0..3 {
        for floor in [false, true] {
            let (((), a), _, _, _) = with_limit_usage_refusal(1 << 20, || {
                measure(|| {
                    let mut b = ReplayBudget::new();
                    let mut r = Insertions::acquire(&f, &mut b).unwrap();
                    let mut target = Incoming::default();
                    for n in 0..count {
                        r.insert_owned(&f, &mut target, scalar(n), &mut b);
                        assert!(!r.failed);
                    }
                    if floor {
                        assert!(set_active_floor(active_remaining().unwrap()));
                    } else {
                        assert!(!reserve_active_required(active_remaining().unwrap() + 1));
                    }
                    let allocations = current_metrics().allocations;
                    r.insert_owned(&f, &mut target, scalar(0), &mut b);
                    assert!(r.failed);
                    assert_eq!(target.len(), count as usize);
                    assert_eq!(current_metrics().allocations, allocations);
                    let first = r.failure();
                    r.insert_owned(&f, &mut target, scalar(1), &mut b);
                    assert!(r.failed);
                    assert_eq!(target.len(), count as usize);
                    drop(first);
                    drop(target);
                    drop(r);
                    drop(b);
                })
            });
            assert!(!a.invalid && a.live == 0);
            assert_eq!(a.allocated, a.freed);
        }
    }
}
#[test]
fn foreign_function_budget_and_nested_scope_cannot_publish_or_revive_original_owner() {
    let f = function();
    let foreign = f.clone();
    for case in 0..3 {
        let (((), a), _, _, _) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                let mut other = ReplayBudget::new();
                let mut r = Insertions::acquire(&f, &mut b).unwrap();
                let mut target = Incoming::default();
                match case {
                    0 => r.insert_owned(&foreign, &mut target, scalar(1), &mut b),
                    1 => r.insert_owned(&f, &mut target, scalar(1), &mut other),
                    _ => {
                        with_limit_usage_refusal(1 << 16, || {
                            r.insert_owned(&f, &mut target, scalar(1), &mut b)
                        });
                    }
                }
                assert!(r.failed && target.is_empty());
                drop(r.failure());
                r.insert_owned(&f, &mut target, scalar(2), &mut b);
                assert!(r.failed && target.is_empty());
                drop(target);
                drop(r);
                drop(other);
                drop(b);
            })
        });
        assert!(!a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
    }
}
#[test]
fn actual_paid_goto_chain_and_final_sibling_move_the_same_box_through_frontiers() {
    let f = function();
    let (((), a), refused, _, first) = with_limit_usage_refusal(1 << 24, || {
        measure(|| {
            let mut b = ReplayBudget::with_physical(&f, 1 << 23).unwrap();
            let mut receiver = Insertions::<PathState>::acquire(&f, &mut b).unwrap();
            let mut current = Incoming::default();
            receiver.insert_owned(&f, &mut current, empty(), &mut b);
            assert!(!receiver.failed);
            let original = pointer(&current);
            let sources = BTreeMap::new();
            for _ in 0..32 {
                let state = current.into_one().next().unwrap();
                let state =
                    boxed_flow::state_for_edge(&f, state, &EdgeCondition::Always, &sources, &mut b)
                        .unwrap();
                current = Incoming::default();
                receiver.insert_held(&f, &mut current, state, &mut b);
                assert!(!receiver.failed);
                assert_eq!(pointer(&current), original);
            }
            let state = current.into_one().next().unwrap();
            storage::reserve_state_clone(&f, &state, &mut b, "ownership branch clone").unwrap();
            let earlier = state.as_ref().clone();
            let earlier = state_for_edge(&f, earlier, &EdgeCondition::Always, &sources).unwrap();
            let mut sibling = Incoming::default();
            receiver.insert_owned(&f, &mut sibling, earlier, &mut b);
            assert!(!receiver.failed);
            assert_ne!(pointer(&sibling), original);
            let final_sibling =
                boxed_flow::state_for_edge(&f, state, &EdgeCondition::Always, &sources, &mut b)
                    .unwrap();
            let mut final_target = Incoming::default();
            receiver.insert_held(&f, &mut final_target, final_sibling, &mut b);
            assert!(!receiver.failed);
            assert_eq!(pointer(&final_target), original);
            drop(sibling);
            drop(final_target);
            drop(receiver);
            drop(b);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!a.invalid && a.live == 0);
    assert_eq!(a.allocated, a.freed);
}

#[test]
fn promotion_comparison_unwind_drops_both_owners_once_after_prepublication_payment() {
    thread_local! { static COMPARES:Cell<usize> = const { Cell::new(0) }; }
    #[derive(Eq, PartialEq)]
    struct Unwind(State);
    impl Ord for Unwind {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            COMPARES.with(|n| {
                let next = n.get() + 1;
                n.set(next);
                assert_ne!(next, 2, "injected publication comparison unwind");
            });
            self.0.cmp(&other.0)
        }
    }
    impl PartialOrd for Unwind {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    let f = function();
    DROPS.with(|n| n.set(0));
    COMPARES.with(|n| n.set(0));
    let (((), a), refused, _, _) = with_limit_usage_refusal(1 << 20, || {
        measure(|| {
            let mut b = ReplayBudget::new();
            let mut receiver = Insertions::<Unwind>::acquire(&f, &mut b).unwrap();
            let mut target = Incoming::default();
            receiver.insert_owned(&f, &mut target, Unwind(scalar(1)), &mut b);
            assert!(!receiver.failed);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                receiver.insert_owned(&f, &mut target, Unwind(scalar(2)), &mut b)
            }));
            assert!(result.is_err());
            drop(result);
            assert!(target.is_empty());
            assert_eq!(DROPS.with(Cell::get), 2);
            drop(target);
            drop(receiver);
            drop(b);
        })
    });
    assert!(!refused);
    assert!(!a.invalid && a.live == 0);
    assert_eq!(a.allocated, a.freed);
}

mod custody;
mod whole;

#[test]
fn final_original_box_reaches_either_branch_destination_with_equal_duplicate_preserved() {
    let f = function();
    for destination in 0..2 {
        let (((), a), refused, paid, first) = with_limit_usage_refusal(1 << 24, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                let mut receiver = Insertions::<PathState>::acquire(&f, &mut b).unwrap();
                let mut initial = Incoming::default();
                receiver.insert_owned(&f, &mut initial, empty(), &mut b);
                assert!(!receiver.failed);
                let original = pointer(&initial);
                let state = initial.into_one().next().unwrap();
                boxed_flow::reserve_block(&f, &mut b).unwrap();
                storage::reserve_state_clone(&f, &state, &mut b, "ownership branch clone").unwrap();
                let clone = state.as_ref().clone();
                let mut targets = [Incoming::default(), Incoming::default()];
                receiver.insert_owned(&f, &mut targets[1 - destination], clone, &mut b);
                assert!(!receiver.failed);
                let state = boxed_flow::state_for_edge(
                    &f,
                    state,
                    &EdgeCondition::Always,
                    &BTreeMap::new(),
                    &mut b,
                )
                .unwrap();
                receiver.insert_held(&f, &mut targets[destination], state, &mut b);
                assert!(!receiver.failed);
                assert_eq!(pointer(&targets[destination]), original);
                assert_ne!(pointer(&targets[1 - destination]), original);
                receiver.insert_owned(&f, &mut targets[destination], empty(), &mut b);
                assert!(!receiver.failed);
                assert_eq!(pointer(&targets[destination]), original);
                drop(targets);
                drop(receiver);
                drop(b);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
        assert!(a.allocated <= paid);
    }
}

#[test]
fn same_box_edge_preserves_all_original_conditions_and_six_field_diagnostics() {
    let f = function();
    let status = StatusSourceId {
        expression: f.body.id.clone(),
        lane: StatusLane::OperationFailure,
    };
    let mut sources = BTreeMap::new();
    sources.insert(f.body.id.clone(), status.clone());
    for condition in [
        EdgeCondition::Always,
        EdgeCondition::BooleanResult(f.body.id.clone(), false),
        EdgeCondition::BooleanResult(f.body.id.clone(), true),
        EdgeCondition::StatusZero(status.clone()),
        EdgeCondition::StatusNonzero(status.clone()),
    ] {
        for case in 0..4 {
            let mut value = empty();
            match case {
                1 => value.pending_failure = Some(status.clone()),
                2 => value.selected_failure = Some(status.clone()),
                3 => value.published = true,
                _ => {}
            }
            let expected = before_frontier::edge_before(&f, value.clone(), &condition, &sources);
            let mut b = ReplayBudget::unobserved();
            let boxed = Box::new(value);
            let original = std::ptr::from_ref(boxed.as_ref());
            let actual = boxed_flow::state_for_edge(&f, boxed, &condition, &sources, &mut b);
            if let Ok(value) = &actual {
                assert_eq!(std::ptr::from_ref(value.as_ref()), original);
            }
            assert_eq!(actual.map(|v| *v), expected);
        }
    }
}

#[test]
fn many_keeps_full_sorted_prefix_validation_and_join_in_both_promotion_orders() {
    let program = crate::hir::resolve(
        &crate::check(
            "module ordinary.many; @id(\"main\") fn main()->i64 {0}",
            "many.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let f = &program.functions[0];
    for reverse in [false, true] {
        let mut b = ReplayBudget::unobserved();
        let mut receiver = Insertions::acquire(f, &mut b).unwrap();
        let mut incoming = Incoming::default();
        let mut original = BTreeSet::new();
        for history in if reverse {
            [[1, 3], [1, 2]]
        } else {
            [[1, 2], [1, 3]]
        } {
            let mut state = empty();
            state.live_order = history.into_iter().map(LivenessFlagId).collect();
            original.insert(Box::new(state.clone()));
            receiver.insert_owned(f, &mut incoming, state, &mut b);
            assert!(!receiver.failed);
        }
        assert_eq!(incoming.len(), 2);
        let states = incoming.into_many();
        assert_eq!(states, original);
        let mut old = ReplayBudget::unobserved();
        let mut current = ReplayBudget::unobserved();
        let expected = before_frontier::validate_group_before(f, &original, BlockId(7), &mut old);
        assert_eq!(
            validate_control_group_with_budget(f, &states, BlockId(7), &mut current),
            expected
        );
        assert_eq!(current.remaining, old.remaining);
        assert_eq!(
            joined_ownership_state(&program, f, &states, &mut current),
            before_frontier::join_before(&program, f, &original, &mut old)
        );
        assert_eq!(current.remaining, old.remaining);
        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
    }
}

#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn actual_ordinary_driver_reports_bound_body_frontier_and_complete_terminal_arity() {
    use crate::bounded_output::exclusive_debits::{attempt, path_census::Event};
    use crate::workflow_profile::plan_comparison::{Binding, Capture};
    let program=crate::hir::resolve(&crate::check(
        "module census; @id(\"f\") fn f(flag:bool)->i64 {if flag {1}else{2}} @id(\"main\") fn main()->i64 {f(false)}",
        "census.spx").unwrap()).unwrap();
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    let c = Capture::begin(Binding::caller_labelled(
        "actual ordinary path census",
        [0; 32],
    ))
    .unwrap();
    let ((outcome, remaining), refused, paid, _) =
        crate::bounded_output::with_limit_usage_refusal(64 << 20, || {
            let mut a = attempt(0, 0, 0);
            let mut budget = ReplayBudget::new();
            let outcome = (|| {
                let storage = validate_slots_paid(function, &mut budget)?;
                let leaves = collect_leaves_paid(function, &mut budget)?;
                validate_path_states::<Diagnostic>(
                    &program,
                    function,
                    &storage,
                    &leaves,
                    &mut budget,
                    None,
                    None,
                )
            })();
            let remaining = budget.remaining;
            a.complete(outcome.is_ok());
            (outcome, remaining)
        });
    assert!(outcome.is_ok() && !refused && remaining < MAX_REPLAY_WORK_UNITS && paid > 0);
    let r = c.finish().unwrap().resolved_core;
    assert!(r.complete && !r.invalid);
    let row = r.attempts[0].path_census[1];
    assert_eq!(
        (row.entries, row.successes, row.events[Event::Body as usize]),
        (1, 1, 1)
    );
    assert!(row.events[Event::InputOne as usize] > 0 && row.events[Event::OwnedBox as usize] > 0);
    assert!(row.terminal_paths > 0 && row.successful_paths > 0 && row.admitted_requests > 0);
}
