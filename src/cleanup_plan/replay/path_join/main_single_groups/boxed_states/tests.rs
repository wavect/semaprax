use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use std::cell::Cell;

thread_local! { static DROPS: Cell<usize> = const { Cell::new(0) }; }
// Deliberately not Clone: frontier transfers must move the complete allocation.
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct State([u64; 50]);
impl Drop for State {
    fn drop(&mut self) {
        DROPS.with(|drops| drops.set(drops.get() + 1));
    }
}
fn reserve(bytes: usize, _: &'static str) -> Result<(), ()> {
    reserve_active_required(bytes).then_some(()).ok_or(())
}

#[test]
fn exact_box_and_node_are_paid_before_either_allocation_and_refusal_is_sticky() {
    let exact = btree_node_upper::<Box<State>, ()>().unwrap() + size_of::<State>();
    assert!(exact < btree_node_upper::<State, ()>().unwrap());
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut states = BTreeSet::new();
                let result = insert_owned(&mut states, State([17; 50]), &mut reserve);
                let success = result.is_ok();
                if success {
                    assert_eq!(states.len(), 1);
                    assert_eq!(states.first().unwrap().0, [17; 50]);
                    let charged = crate::bounded_output::reserve_active_required(1);
                    assert!(!charged);
                    // A duplicate also needs live authority after a refusal.
                    assert!(insert_owned(&mut states, State([17; 50]), &mut reserve).is_err());
                    assert_eq!(states.len(), 1);
                } else {
                    assert!(states.is_empty());
                    assert!(insert_owned(&mut states, State([19; 50]), &mut reserve).is_err());
                }
                drop(states);
                success
            })
        });
        assert_eq!(success, limit == exact);
        assert!(refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        if success {
            assert_eq!(paid, exact);
        } else {
            // Even a paid node request is not constructed before the box debit.
            assert_eq!(allocation.allocated, 0);
        }
    }
}

#[test]
fn held_state_moves_preserve_pointers_and_duplicates_without_cloning_or_refunds() {
    DROPS.with(|drops| drops.set(0));
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut source = BTreeSet::new();
            let mut destination = BTreeSet::new();
            assert_eq!(
                insert_owned(&mut source, State([3; 50]), &mut reserve),
                Ok(true)
            );
            assert_eq!(
                insert_owned(&mut source, State([3; 50]), &mut reserve),
                Ok(false)
            );
            assert_eq!(DROPS.with(Cell::get), 1);
            let held = source.pop_first().unwrap();
            let pointer = std::ptr::from_ref(held.as_ref());
            assert_eq!(insert_held(&mut destination, held, &mut reserve), Ok(true));
            assert_eq!(
                std::ptr::from_ref(destination.first().unwrap().as_ref()),
                pointer
            );
            assert_eq!(DROPS.with(Cell::get), 1);
            // An independently paid identical box must be dropped on dedup.
            assert_eq!(
                insert_owned(&mut source, State([3; 50]), &mut reserve),
                Ok(true)
            );
            assert_eq!(
                insert_held(&mut destination, source.pop_first().unwrap(), &mut reserve),
                Ok(false)
            );
            assert_eq!(DROPS.with(Cell::get), 2);
            drop(source);
            assert_eq!(DROPS.with(Cell::get), 2);
            drop(destination);
            assert_eq!(DROPS.with(Cell::get), 3);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    let node = btree_node_upper::<Box<State>, ()>().unwrap();
    assert_eq!(paid, 3 * node + 2 * size_of::<State>());
    assert!(allocation.allocated <= paid);

    // A failed transfer drops its already paid input without allocating a new
    // box/node or modifying the destination. The first refusal stays selected.
    DROPS.with(|drops| drops.set(0));
    let exact = node + size_of::<State>();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let mut source = BTreeSet::new();
            let mut destination = BTreeSet::new();
            assert_eq!(
                insert_owned(&mut source, State([7; 50]), &mut reserve),
                Ok(true)
            );
            let allocated = crate::test_allocation_observer::current_metrics().allocated;
            assert!(
                insert_held(&mut destination, source.pop_first().unwrap(), &mut reserve).is_err()
            );
            assert!(destination.is_empty());
            assert_eq!(
                crate::test_allocation_observer::current_metrics().allocated,
                allocated
            );
            assert_eq!(DROPS.with(Cell::get), 1);
            assert!(reserve(0, "after failed transfer").is_err());
            drop(source);
            drop(destination);
        })
    });
    assert!(refused && first.is_some());
    assert_eq!(paid, exact);
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn boxed_order_matches_inline_values_through_tree_splits_and_every_prefix() {
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut boxed = BTreeSet::new();
            for (index, key) in (0..127).rev().chain(0..127).enumerate() {
                assert_eq!(
                    insert_owned(&mut boxed, State([key; 50]), &mut reserve),
                    Ok(index < 127)
                );
            }
            // Independently generated expected ordering; no allocation needed.
            assert!(boxed.iter().map(|state| state.0[0]).eq(0..127));
            for state in &boxed {
                assert!(boxed
                    .range::<Box<State>, _>(..state)
                    .map(|prior| prior.0[0])
                    .eq(0..state.0[0]));
            }
            drop(boxed);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}
