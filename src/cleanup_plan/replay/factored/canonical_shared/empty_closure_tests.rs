//! Actual unit production query; compatibility adapters are excluded from costs.
use super::{
    before_empty, closure_receiving::ClosureReceiving, Automaton, Canonicalizer, Rc, ReplayBudget,
    ResolvedFunction,
};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}
fn graph(function: &ResolvedFunction) -> (Automaton, usize, usize, usize) {
    let mut setup = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut setup).unwrap();
    let empty = graph.node(function, &mut setup).unwrap();
    let alias = graph.node(function, &mut setup).unwrap();
    graph.epsilon(function, &mut setup, alias, empty).unwrap();
    (graph, sink, empty, alias)
}

#[test]
fn direct_empty_bitmap_and_alias_cached_queries_keep_work_without_receiving_allocation_or_debit() {
    let function = fixture();
    let (graph, sink, empty, alias) = graph(&function);
    let ((), allocation) = measure(|| {
        let mut old = before_empty::Canonicalizer::new(&graph, sink);
        let mut current = Canonicalizer::new(&graph, sink);
        let mut a = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
        let mut b = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
        b.reserve_bytes(
            &function,
            ClosureReceiving::DIRECT_CONTROLS,
            "direct empty test owner",
        )
        .unwrap();
        let mut receiving = ClosureReceiving::from_prepaid(&function, &b);
        for root in [empty, alias] {
            let expected = old.closure(&function, root, &mut a).unwrap().unwrap();
            assert_eq!(expected.len(), 0);
            drop(expected);
            current.closure_query(&function, root, &mut b, &mut receiving);
            assert!(receiving.ready && receiving.empty && !receiving.overflow);
            assert!(receiving.refusal.is_none());
            assert!(current.closures.contains_key(&root));
            assert!(current.closures.get(&root).is_none());
            assert_eq!(a.remaining, b.remaining);
            assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
            receiving.ready = false;
            let paid = b.physical.as_ref().unwrap().charged();
            let allocated = current_metrics().allocated;
            for _ in 0..8 {
                let work = b.remaining;
                current.closure_query(&function, root, &mut b, &mut receiving);
                assert!(receiving.ready && receiving.empty && !receiving.overflow);
                assert!(receiving.refusal.is_none());
                assert_eq!(b.remaining, work - 1);
                receiving.ready = false;
                let expected = old.closure(&function, root, &mut a).unwrap().unwrap();
                assert_eq!(expected.len(), 0);
                drop(expected);
                assert_eq!(a.remaining, b.remaining);
                assert_eq!(b.physical.as_ref().unwrap().charged(), paid);
                assert_eq!(current_metrics().allocated, allocated);
            }
        }
        drop(receiving);
        drop(current);
        drop(old);
        drop(a);
        drop(b);
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

#[test]
fn direct_nonempty_cached_query_borrows_original_rc_and_keeps_all_semantic_work_failures() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    for root in roots.into_iter().chain([sink]) {
        for work in [0, 1, 2, 4, 16, 128, 1 << 20] {
            let mut old = before_empty::Canonicalizer::new(&automaton, sink);
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut a = ReplayBudget::unobserved();
            let mut b = ReplayBudget::unobserved();
            a.remaining = work;
            b.remaining = work;
            let mut receiving = ClosureReceiving::from_prepaid(&function, &b);
            let expected = old.closure(&function, root, &mut a);
            current.closure_query(&function, root, &mut b, &mut receiving);
            assert!(receiving.ready);
            match expected {
                Err(expected) => assert_eq!(receiving.refusal.take(), Some(expected)),
                Ok(None) => assert!(receiving.overflow && receiving.refusal.is_none()),
                Ok(Some(expected)) => {
                    assert!(!receiving.overflow && receiving.refusal.is_none());
                    if let Some(value) = current.closures.get(&root) {
                        assert!(value.iter().eq(expected.iter()));
                    } else {
                        assert!(receiving.empty && expected.len() == 0);
                    }
                }
            }
            assert_eq!(a.remaining, b.remaining);
            assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
        }
    }
    let (graph, sink, _, _) = graph(&function);
    let ((), allocation) = measure(|| {
        let mut current = Canonicalizer::new(&graph, sink);
        let mut budget = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
        budget
            .reserve_bytes(
                &function,
                ClosureReceiving::DIRECT_CONTROLS,
                "direct shared test owner",
            )
            .unwrap();
        let mut receiving = ClosureReceiving::from_prepaid(&function, &budget);
        current.closure_query(&function, sink, &mut budget, &mut receiving);
        assert!(
            receiving.ready
                && !receiving.empty
                && !receiving.overflow
                && receiving.refusal.is_none()
        );
        receiving.ready = false;
        let count = Rc::strong_count(current.closures.get(&sink).unwrap());
        let paid = budget.physical.as_ref().unwrap().charged();
        let allocated = current_metrics().allocated;
        for _ in 0..8 {
            let work = budget.remaining;
            current.closure_query(&function, sink, &mut budget, &mut receiving);
            assert!(
                receiving.ready
                    && !receiving.empty
                    && !receiving.overflow
                    && receiving.refusal.is_none()
            );
            receiving.ready = false;
            assert_eq!(budget.remaining, work - 1);
            assert_eq!(
                Rc::strong_count(current.closures.get(&sink).unwrap()),
                count
            );
            assert_eq!(budget.physical.as_ref().unwrap().charged(), paid);
            assert_eq!(current_metrics().allocated, allocated);
        }
        drop(receiving);
        drop(current);
        drop(budget);
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

fn observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    root: usize,
    limit: usize,
    old: bool,
) -> (Result<usize, ()>, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                return Err(());
            }
        };
        let success = if old {
            let mut owner = before_empty::Canonicalizer::new(graph, sink);
            let result = owner.closure(function, root, &mut budget);
            let success = result
                .as_ref()
                .is_ok_and(|value| value.as_ref().is_some_and(|value| value.len() == 0));
            if let Err(error) = &result {
                assert_eq!(error.code, "SPX-H006");
            }
            drop(result);
            drop(owner);
            success
        } else {
            let mut owner = Canonicalizer::new(graph, sink);
            let success = match budget.reserve_bytes(
                function,
                ClosureReceiving::DIRECT_CONTROLS,
                "direct empty test owner",
            ) {
                Err(error) => {
                    assert_eq!(error.code, "SPX-H006");
                    drop(error);
                    false
                }
                Ok(()) => {
                    let mut receiving = ClosureReceiving::from_prepaid(function, &budget);
                    owner.closure_query(function, root, &mut budget, &mut receiving);
                    let success = receiving.ready
                        && receiving.empty
                        && !receiving.overflow
                        && receiving.refusal.is_none();
                    if let Some(error) = receiving.refusal.take() {
                        assert_eq!(error.code, "SPX-H006");
                    }
                    drop(receiving);
                    success
                }
            };
            drop(owner);
            success
        };
        let paid = budget.physical.as_ref().unwrap().charged();
        assert!(current_metrics().allocated <= paid);
        let at = current_metrics().allocated;
        drop(budget);
        assert_eq!(
            current_metrics().allocated,
            at,
            "whole final Drop allocates nothing"
        );
        if success {
            Ok(paid)
        } else {
            Err(())
        }
    })
}

#[test]
fn direct_whole_empty_owner_costs_and_all_short_frontiers_cover_new_bitmap_backing_and_drop() {
    let function = fixture();
    let (graph, sink, empty, alias) = graph(&function);
    for root in [empty, alias] {
        let mut tuples = Vec::new();
        for old in [true, false] {
            let (paid, allocation) = observe(&function, &graph, sink, root, 1 << 24, old);
            let paid = paid.unwrap();
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid && allocation.highwater <= paid);
            tuples.push((paid, allocation));
        }
        eprintln!("bitmap-empty root={root} immediate/current={tuples:?}");
        assert!(tuples[1].1.allocations < tuples[0].1.allocations);
        assert!(tuples[1].1.allocated < tuples[0].1.allocated);
        let exact = tuples[1].0;
        for limit in 0..=exact {
            let (result, allocation) = observe(&function, &graph, sink, root, limit, false);
            assert_eq!(
                result.is_ok(),
                limit == exact,
                "root={root} limit={limit}/{exact}"
            );
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
        }
    }
}

#[test]
fn direct_overflow_is_not_empty_and_the_same_paid_status_owner_can_receive_empty_afterward() {
    let function = fixture();
    let (mut graph, sink, empty, _) = graph(&function);
    let mut setup = ReplayBudget::unobserved();
    let mut root = sink;
    for _ in 0..64 {
        let next = graph.node(&function, &mut setup).unwrap();
        graph.epsilon(&function, &mut setup, next, root).unwrap();
        graph.epsilon(&function, &mut setup, next, root).unwrap();
        root = next;
    }
    let mut current = Canonicalizer::new(&graph, sink);
    let mut old = before_empty::Canonicalizer::new(&graph, sink);
    let mut a = ReplayBudget::unobserved();
    let mut b = ReplayBudget::unobserved();
    let mut receiving = ClosureReceiving::from_prepaid(&function, &b);
    assert!(old.closure(&function, root, &mut a).unwrap().is_none());
    current.closure_query(&function, root, &mut b, &mut receiving);
    assert!(receiving.ready && receiving.overflow && receiving.refusal.is_none());
    assert_eq!(a.remaining, b.remaining);
    receiving.ready = false;
    current.closure_query(&function, empty, &mut b, &mut receiving);
    assert!(
        receiving.ready && receiving.empty && !receiving.overflow && receiving.refusal.is_none()
    );
    assert!(old.closure(&function, empty, &mut a).unwrap().is_some());
    assert_eq!(a.remaining, b.remaining);
    assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
}

#[test]
fn direct_primed_foreign_inputs_and_nested_scope_never_query_foreign_payer_or_restore_memo() {
    use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
    let function = fixture();
    let foreign_function = fixture();
    let (graph, sink, empty, _) = graph(&function);
    for mode in 0..3 {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 24, || {
            measure(|| {
                let mut current = Canonicalizer::new(&graph, sink);
                let mut budget = ReplayBudget::new();
                let mut other = ReplayBudget::new();
                budget
                    .reserve_bytes(
                        &function,
                        ClosureReceiving::DIRECT_CONTROLS,
                        "direct identity test owner",
                    )
                    .unwrap();
                let mut receiving = ClosureReceiving::from_prepaid(&function, &budget);
                current.closure_query(&function, empty, &mut budget, &mut receiving);
                assert!(receiving.ready && receiving.empty && receiving.refusal.is_none());
                receiving.ready = false;
                if mode == 2 {
                    let ((), child_refused, child_paid, child_first) =
                        with_limit_usage_refusal(0, || {
                            let at = current_metrics().allocated;
                            current.closure_query(&function, empty, &mut budget, &mut receiving);
                            assert_eq!(active_remaining(), Some(0));
                            assert_eq!(current_metrics().allocated, at);
                        });
                    assert!(!child_refused && child_paid == 0 && child_first.is_none());
                } else {
                    let paid = active_remaining();
                    let at = current_metrics().allocated;
                    if mode == 0 {
                        current.closure_query(
                            &foreign_function,
                            empty,
                            &mut budget,
                            &mut receiving,
                        );
                    } else {
                        current.closure_query(&function, empty, &mut other, &mut receiving);
                    }
                    assert_eq!(active_remaining(), paid);
                    assert_eq!(current_metrics().allocated, at);
                }
                assert!(current.closures.failed());
                assert_eq!(
                    receiving.refusal.take(),
                    Some(super::Diagnostic::io("SPX-H006", String::new()))
                );
                receiving.ready = false;
                let at = current_metrics().allocated;
                let paid = active_remaining();
                current.closure_query(&function, empty, &mut budget, &mut receiving);
                assert_eq!(
                    receiving.refusal.take(),
                    Some(super::Diagnostic::io("SPX-H006", String::new()))
                );
                assert_eq!(active_remaining(), paid);
                assert_eq!(current_metrics().allocated, at);
                drop(receiving);
                drop(current);
                drop(other);
                drop(budget);
            })
        });
        assert_eq!(refused, mode == 2);
        assert_eq!(first.is_some(), mode == 2);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}
