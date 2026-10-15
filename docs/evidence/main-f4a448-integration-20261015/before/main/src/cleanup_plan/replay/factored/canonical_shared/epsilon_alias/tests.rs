//! Whole old/current producers, actual held aliases, and independent budgets.
use super::super::{before_alias, Canonicalizer};
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

type ClosureVerdict = Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic>;

fn function() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
fn graph(function: &ResolvedFunction, depth: usize) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::unobserved();
    let mut a = Automaton::default();
    let sink = a.node(function, &mut budget).unwrap();
    let mut leaves = Vec::new();
    for terminal in [
        SkeletonTerminal::Success,
        SkeletonTerminal::Failure,
        SkeletonTerminal::LoopLatch,
    ] {
        leaves.push(a.end(function, &mut budget, sink, terminal).unwrap());
    }
    let joined = a.node(function, &mut budget).unwrap();
    for target in leaves {
        a.epsilon(function, &mut budget, joined, target).unwrap();
    }
    let mut chain = joined;
    for _ in 0..depth {
        let parent = a.node(function, &mut budget).unwrap();
        a.epsilon(function, &mut budget, parent, chain).unwrap();
        chain = parent;
    }
    let left = a.node(function, &mut budget).unwrap();
    let right = a.node(function, &mut budget).unwrap();
    a.epsilon(function, &mut budget, left, chain).unwrap();
    a.epsilon(function, &mut budget, right, chain).unwrap();
    let diamond = a.node(function, &mut budget).unwrap();
    a.epsilon(function, &mut budget, diamond, left).unwrap();
    a.epsilon(function, &mut budget, diamond, right).unwrap();
    let duplicate = a.node(function, &mut budget).unwrap();
    for _ in 0..2 {
        a.epsilon(function, &mut budget, duplicate, chain).unwrap();
    }
    let mixed = a.node(function, &mut budget).unwrap();
    a.epsilon(function, &mut budget, mixed, chain).unwrap();
    a.movement(
        function,
        &mut budget,
        mixed,
        Label::End(SkeletonTerminal::Success),
        sink,
    )
    .unwrap();
    let empty = a.node(function, &mut budget).unwrap();
    (
        a,
        sink,
        vec![diamond, duplicate, mixed, empty, chain, diamond],
    )
}

#[test]
fn epsilon_chain_diamond_duplicate_and_mixed_maps_keep_every_old_entry_and_actual_backing() {
    let function = function();
    let (a, sink, roots) = graph(&function, 64);
    let (((eligible, copied_entries), allocation), refused, paid, first) =
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut old = before_alias::Canonicalizer::new(&a, sink);
                let mut current = Canonicalizer::new(&a, sink);
                let mut old_budget = ReplayBudget::new();
                let mut budget = ReplayBudget::new();
                for root in &roots {
                    let expected = old.closure_for_alias_test(&function, *root, &mut old_budget);
                    let actual = current.closure(&function, *root, &mut budget);
                    assert_eq!(actual, expected);
                    assert_eq!(budget.remaining, old_budget.remaining);
                    assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
                    drop(actual);
                    drop(expected);
                }
                let mut eligible = 0usize;
                let mut copied_entries = 0usize;
                for (node, state) in a.nodes.iter().enumerate() {
                    let old_value = old.alias_view(&node);
                    let value = current.closures.get(&node);
                    assert_eq!(old_value, value);
                    if node != sink && state.moves.is_empty() && state.epsilon.len() == 1 {
                        let child = state.epsilon[0];
                        if let (Some(value), Some(inner)) = (value, current.closures.get(&child)) {
                            assert!(Rc::ptr_eq(value, inner));
                            assert!(!Rc::ptr_eq(
                                old_value.unwrap(),
                                old.alias_view(&child).unwrap()
                            ));
                            eligible += 1;
                            copied_entries += inner.len();
                        }
                    }
                }
                assert_eq!(eligible, 66);
                assert_eq!(copied_entries, 66 * 3);
                // Bounded test-only census of this actual producer, not app frequency.
                drop(old);
                drop(current);
                drop(old_budget);
                drop(budget);
                (eligible, copied_entries)
            })
        });
    eprintln!("epsilon fixture eligible={eligible} old_entry_copies={copied_entries}");
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}

fn expected_result(
    function: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    work: usize,
) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        budget.remaining = work;
        before_alias::Canonicalizer::new(a, sink).closure_for_alias_test(
            function,
            root,
            &mut budget,
        )
    })
    .0
}
#[allow(
    clippy::too_many_arguments,
    reason = "Keep the independent receiver and observation controls explicit."
)]
fn observe(
    function: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    current: bool,
    work: usize,
    bytes: usize,
    expected: Option<&ClosureVerdict>,
) -> (
    (bool, usize, Metrics),
    bool,
    usize,
    Option<ReservationRefusal>,
) {
    let ((value, allocation), refused, paid, first) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work;
            let mut new = Canonicalizer::new(a, sink);
            let mut old = before_alias::Canonicalizer::new(a, sink);
            let result = if current {
                new.closure(function, root, &mut budget)
            } else {
                old.closure_for_alias_test(function, root, &mut budget)
            };
            if let Some(expected) = expected {
                assert_eq!(&result, expected);
            }
            let ok = result.is_ok();
            drop(result);
            let used = work - budget.remaining;
            if !ok {
                let at = current_metrics().allocated;
                let retry = if current {
                    new.closure(function, root, &mut budget)
                } else {
                    old.closure_for_alias_test(function, root, &mut budget)
                };
                assert_eq!(retry, Err(Diagnostic::io("SPX-H006", String::new())));
                drop(retry);
                assert_eq!(current_metrics().allocated, at);
                assert_eq!(budget.remaining, work - used);
            }
            drop(new);
            drop(old);
            drop(budget);
            (ok, used)
        })
    });
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    ((value.0, value.1, allocation), refused, paid, first)
}
#[test]
fn epsilon_alias_full_diagnostics_every_work_frontier_and_exact_short_zero_physical_are_independent(
) {
    let function = function();
    let (a, sink, roots) = graph(&function, 3);
    let root = roots[0];
    let expected = expected_result(&function, &a, sink, root, MAX_REPLAY_WORK_UNITS);
    let ((ok, work, _), refused, _, first) = observe(
        &function,
        &a,
        sink,
        root,
        false,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        Some(&expected),
    );
    assert!(ok && !refused && first.is_none());
    for limit in 0..=work {
        let expected = expected_result(&function, &a, sink, root, limit);
        let old = observe(
            &function,
            &a,
            sink,
            root,
            false,
            limit,
            usize::MAX,
            Some(&expected),
        );
        let current = observe(
            &function,
            &a,
            sink,
            root,
            true,
            limit,
            usize::MAX,
            Some(&expected),
        );
        assert_eq!((old.0 .0, old.0 .1), (current.0 .0, current.0 .1));
        assert!(!old.1 && !current.1);
    }
    for current in [false, true] {
        let full = observe(
            &function,
            &a,
            sink,
            root,
            current,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            Some(&expected),
        );
        eprintln!(
            "epsilon current={current} cumulative={} allocation={:?}",
            full.2, full.0 .2
        );
        for bytes in [0, full.2 - 1, full.2] {
            let actual = observe(
                &function,
                &a,
                sink,
                root,
                current,
                MAX_REPLAY_WORK_UNITS,
                bytes,
                None,
            );
            assert_eq!(actual.0 .0, bytes == full.2);
            assert_eq!(actual.1, bytes < full.2);
            assert_eq!(actual.3.is_some(), bytes < full.2);
            if bytes == 0 {
                assert_eq!(actual.0 .2.allocated, 0);
            }
        }
    }
}

#[test]
fn epsilon_full_canonical_ids_signatures_checked_weights_and_repeated_roots_match_a33() {
    let function = function();
    let (a, sink, roots) = graph(&function, 24);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old = before_alias::Canonicalizer::new(&a, sink);
            let mut current = Canonicalizer::new(&a, sink);
            let mut old_budget = ReplayBudget::new();
            let mut budget = ReplayBudget::new();
            for root in &roots {
                old_budget
                    .reserve_array::<(usize, u64)>(&function, 1, "old epsilon root input")
                    .unwrap();
                budget
                    .reserve_array::<(usize, u64)>(&function, 1, "current epsilon root input")
                    .unwrap();
                assert_eq!(
                    old.id(&function, vec![(*root, 1)], &mut old_budget),
                    current.id(&function, vec![(*root, 1)], &mut budget)
                );
                assert!(old.same_snapshot(&current));
                assert_eq!(old_budget.remaining, budget.remaining);
                assert_eq!(old_budget.skeleton_remaining, budget.skeleton_remaining);
            }
            // Duplicate edges multiply route weights; their general map must stay
            // independent and overflow the checked weighted product identically.
            old_budget
                .reserve_array::<(usize, u64)>(&function, 1, "old epsilon overflow input")
                .unwrap();
            budget
                .reserve_array::<(usize, u64)>(&function, 1, "current epsilon overflow input")
                .unwrap();
            let old_result = old.id(&function, vec![(roots[1], u64::MAX)], &mut old_budget);
            let result = current.id(&function, vec![(roots[1], u64::MAX)], &mut budget);
            assert_eq!(old_result, Ok(None));
            assert_eq!(result, old_result);
            assert_eq!(old_budget.remaining, budget.remaining);
            drop(old);
            drop(current);
            drop(old_budget);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}

#[test]
fn epsilon_alias_long_chain_reports_actual_cumulative_and_peak_costs_separately() {
    let function = function();
    let (a, sink, roots) = graph(&function, 64);
    let expected = expected_result(&function, &a, sink, roots[0], MAX_REPLAY_WORK_UNITS);
    let old = observe(
        &function,
        &a,
        sink,
        roots[0],
        false,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        Some(&expected),
    );
    let current = observe(
        &function,
        &a,
        sink,
        roots[0],
        true,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        Some(&expected),
    );
    assert!(old.0 .0 && current.0 .0 && !old.1 && !current.1);
    eprintln!(
        "epsilon long-chain old cumulative={} metrics={:?}; current cumulative={} metrics={:?}",
        old.2, old.0 .2, current.2, current.0 .2
    );
    assert_eq!(old.0 .1, current.0 .1);
    // Only this full producer fixture claims a reduction, pending execution.
    assert!(current.0 .2.allocated < old.0 .2.allocated);
    assert!(current.0 .2.allocations < old.0 .2.allocations);
    assert!(current.2 < old.2);
}

#[test]
fn epsilon_alias_foreign_restored_function_budget_and_stale_owners_never_return_views() {
    let function = function();
    let wrong = function.clone();
    let (a, sink, roots) = graph(&function, 3);
    for attack in 0..3 {
        let (((), allocation), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut current = Canonicalizer::new(&a, sink);
                let mut budget = ReplayBudget::new();
                let mut foreign_budget = ReplayBudget::new();
                drop(current.closure(&function, roots[0], &mut budget).unwrap());
                if attack == 0 {
                    let (_, child_refused, child_paid, _) =
                        with_limit_usage_refusal(usize::MAX, || {
                            let at = current_metrics().allocated;
                            assert_eq!(
                                current.closure(&function, roots[0], &mut budget),
                                Err(Diagnostic::io("SPX-H006", String::new()))
                            );
                            assert_eq!(current_metrics().allocated, at);
                        });
                    assert!(!child_refused && child_paid == 0);
                } else {
                    let at = current_metrics().allocated;
                    let result = if attack == 1 {
                        current.closure(&wrong, roots[0], &mut budget)
                    } else {
                        current.closure(&function, roots[0], &mut foreign_budget)
                    };
                    assert_eq!(result, Err(Diagnostic::io("SPX-H006", String::new())));
                    assert!(foreign_budget.refusal_text.is_none() && !foreign_budget.refusal_spent);
                    assert_eq!(current_metrics().allocated, at);
                }
                let at = current_metrics().allocated;
                assert_eq!(
                    current.closure(&function, roots[0], &mut budget),
                    Err(Diagnostic::io("SPX-H006", String::new()))
                );
                assert_eq!(current_metrics().allocated, at);
                assert!(current.closures.get(&roots[0]).is_none());
                drop(current);
                drop(foreign_budget);
                drop(budget);
            })
        });
        assert_eq!(refused, attack == 0);
        // Foreign test-ledger setup and last Drop stay in the whole observer.
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
    let ((), allocation) = measure(|| {
        let ((mut current, mut budget), refused, _, _) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut current = Canonicalizer::new(&a, sink);
                let mut budget = ReplayBudget::new();
                drop(current.closure(&function, roots[0], &mut budget).unwrap());
                (current, budget)
            });
        assert!(!refused);
        let at = current_metrics().allocated;
        assert_eq!(
            current.closure(&function, roots[0], &mut budget),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        assert_eq!(current_metrics().allocated, at);
        assert!(current.closures.get(&roots[0]).is_none());
        drop(current);
        drop(budget);
    });
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn epsilon_sink_with_one_child_keeps_own_entry_and_general_backing() {
    let function = function();
    let mut budget = ReplayBudget::unobserved();
    let mut a = Automaton::default();
    let sink = a.node(&function, &mut budget).unwrap();
    let child = a
        .end(&function, &mut budget, sink, SkeletonTerminal::Success)
        .unwrap();
    a.epsilon(&function, &mut budget, sink, child).unwrap();
    drop(budget);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old = before_alias::Canonicalizer::new(&a, sink);
            let mut current = Canonicalizer::new(&a, sink);
            let mut old_budget = ReplayBudget::new();
            let mut budget = ReplayBudget::new();
            let expected = old.closure_for_alias_test(&function, sink, &mut old_budget);
            let actual = current.closure(&function, sink, &mut budget);
            assert_eq!(actual, expected);
            assert_eq!(
                actual.as_ref().unwrap().as_ref().unwrap().get(&sink),
                Some(&1)
            );
            assert_eq!(
                actual.as_ref().unwrap().as_ref().unwrap().get(&child),
                Some(&1)
            );
            assert!(!Rc::ptr_eq(
                current.closures.get(&sink).unwrap(),
                current.closures.get(&child).unwrap()
            ));
            assert_eq!(budget.remaining, old_budget.remaining);
            drop(actual);
            drop(expected);
            drop(old);
            drop(current);
            drop(old_budget);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}
