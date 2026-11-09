//! Compact storage controls; every measurement includes the final owning Drop.
use super::frontier::{Incoming, Insertions};
use super::*;
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
    let promotion = 2 * super::super::super::physical_storage::btree_node_upper::<Box<State>, ()>()
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
fn boxed_goto_and_final_sibling_keep_pointer_and_full_old_edge_condition_diagnostics() {
    let f = function();
    for condition in [
        EdgeCondition::Always,
        EdgeCondition::BooleanResult(f.body.id.clone(), true),
        EdgeCondition::StatusNonzero(StatusSourceId {
            expression: f.body.id.clone(),
            lane: crate::cleanup_plan::StatusLane::OperationFailure,
        }),
    ] {
        let mut b = ReplayBudget::with_physical(&f, 1 << 20).unwrap();
        let sources = BTreeMap::new();
        let mut expected = empty();
        let mut actual = Box::new(empty());
        let pointer = std::ptr::from_ref(actual.as_ref());
        for _ in 0..32 {
            expected =
                before_frontier::state_for_edge_before(&f, expected, &condition, &sources, &mut b)
                    .unwrap();
            actual = boxed_flow::state_for_edge(&f, actual, &condition, &sources, &mut b).unwrap();
            assert_eq!(std::ptr::from_ref(actual.as_ref()), pointer);
            assert_eq!(*actual, expected);
            if actual.pending_failure.is_some() {
                break;
            }
        }
    }
}

mod lexical_before;
mod scalar_before;
fn cost(
    source: &dyn ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) -> (Result<(), Diagnostic>, usize, usize) {
    let lexical = crate::hir::lexical_exit::requires(function)
        .expect("genuine checked HIR lexical classification");
    let mut b = ReplayBudget::with_physical(function, 64 * 1024 * 1024).unwrap();
    let initial = b.remaining;
    let context = crate::cleanup_plan::replay::selected_context::Context::new_paid(
        source, function, &mut b, lexical,
    );
    let result = match context {
        Err(e) => Err(e),
        Ok(context) => match (old, lexical) {
            (true, true) => lexical_before::lexical_before(&context, function, &mut b),
            (true, false) => scalar_before::scalar_before(&context, function, &mut b),
            (false, true) => crate::cleanup_plan::replay::lexical_replay::validate_paid(
                &context, function, &mut b,
            ),
            (false, false) => crate::cleanup_plan::replay::scalar_replay::validate_paid(
                &context, function, &mut b,
            ),
        },
    };
    let result = result.map_err(|e| match e {
        Error::Owned(e) => e,
        Error::Structural(r) => r.legacy(function),
        Error::Fact(r) => r.legacy(function),
        Error::Failed => panic!("fresh old owner failed without first diagnostic"),
    });
    (
        result,
        b.physical.as_ref().unwrap().charged(),
        initial - b.remaining,
    )
}
#[test]
fn directory_cold_layout_is_the_real_enum_and_old_receivers_remain_byte_exact() {
    use sha2::{Digest, Sha256};
    // The enum can cost more for mostly-Many programs. Do not normalize that
    // physical directory growth out of complete caller measurements.
    assert!(size_of::<Incoming<PathState>>() >= size_of::<BTreeSet<Box<PathState>>>());
    for (source, expected) in [
        (
            include_str!("before_frontier/walk.rs"),
            "a83992368a59d48c1d7cca887f9e184b5ec1457e985db6219efca294675302f4",
        ),
        (
            include_str!("before_frontier/queries.rs"),
            "d1aa5cfe263f3412849d4d7818481e01a682aef3f0c3f07eeec6e196d2981909",
        ),
        (
            include_str!("before_frontier/receiving.rs"),
            "d0a1faed7d1d9a2d17b8b7983aa6fdc75f95f7636724cadb6bc4433a717e1e7e",
        ),
        (
            include_str!("before_frontier/finish.rs"),
            "b864db59f35453a3991eb78f8dcb7d4c88b06c6731ef92cdde960ca18f4a90df",
        ),
    ] {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            expected
        );
    }
}

pub(super) fn owner_cost(
    source: &dyn ReplayContext,
    function: &ResolvedFunction,
) -> (Result<(), Diagnostic>, usize, usize) {
    cost(source, function, true)
}
mod full_callers;

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
            let earlier =
                finish::state_for_edge(&f, earlier, &EdgeCondition::Always, &sources, &mut b)
                    .unwrap();
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
    // Initialize only the default panic reporter's process-lifetime locks.
    // No compiler work runs here; the actual default hook and complete unwind
    // remain inside observation, and every live/Drop assertion stays strict.
    drop(std::io::stderr().lock());
    drop(std::backtrace::Backtrace::force_capture());
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
#[test]
fn zero_work_and_pending_selected_published_states_keep_full_old_edge_diagnostics() {
    fn diagnostic(e: Error, f: &ResolvedFunction) -> Diagnostic {
        match e {
            Error::Owned(d) => d,
            Error::Structural(r) => r.legacy(f),
            Error::Fact(r) => r.legacy(f),
            Error::Failed => panic!("missing first edge error"),
        }
    }
    let f = function();
    let sources = BTreeMap::new();
    for case in 0..4 {
        let mut state = empty();
        let status = StatusSourceId {
            expression: f.body.id.clone(),
            lane: crate::cleanup_plan::StatusLane::OperationFailure,
        };
        match case {
            1 => state.pending_failure = Some(status),
            2 => state.selected_failure = Some(status),
            3 => state.published = true,
            _ => {}
        }
        let mut old_budget = ReplayBudget::with_physical(&f, 1 << 20).unwrap();
        let mut new_budget = ReplayBudget::with_physical(&f, 1 << 20).unwrap();
        if case == 0 {
            old_budget.remaining = 0;
            new_budget.remaining = 0;
        }
        let old = before_frontier::state_for_edge_before(
            &f,
            state.clone(),
            &EdgeCondition::Always,
            &sources,
            &mut old_budget,
        )
        .unwrap_err();
        let new = boxed_flow::state_for_edge(
            &f,
            Box::new(state),
            &EdgeCondition::Always,
            &sources,
            &mut new_budget,
        )
        .unwrap_err();
        assert_eq!(
            diagnostic(old, &f),
            diagnostic(new, &f),
            "all six edge diagnostic fields, case {case}"
        );
    }
}
