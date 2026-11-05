//! Actual Selected route: complete predecessors, exact small fallback and Drop.
use super::*;
use crate::cleanup_plan::replay::merge_census::{failure, Failure};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}
fn graph(function: &ResolvedFunction, width: usize, repeats: usize) -> (Automaton, usize, usize) {
    let mut setup = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut setup).unwrap();
    let root = graph.node(function, &mut setup).unwrap();
    for _ in 0..width {
        let child = graph
            .end(
                function,
                &mut setup,
                sink,
                super::super::SkeletonTerminal::Failure,
            )
            .unwrap();
        for _ in 0..repeats {
            graph
                .movement(
                    function,
                    &mut setup,
                    root,
                    Label::End(super::super::SkeletonTerminal::Success),
                    child,
                )
                .unwrap();
        }
    }
    (graph, sink, root)
}

#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<Option<usize>, Failure>,
    paid: usize,
    work: usize,
}
fn observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    route: usize,
    floor: Option<usize>,
) -> (Outcome, Metrics) {
    crate::bounded_output::with_limit_usage_refusal(1 << 28, || {
        measure(|| {
            let mut budget = match ReplayBudget::with_physical(function, limit) {
                Ok(value) => value,
                Err(error) => {
                    return Outcome {
                        result: Err(failure(&error)),
                        paid: 0,
                        work: 0,
                    }
                }
            };
            let initial = budget.remaining;
            let result = (|| {
                budget.reserve_array::<(usize, u64)>(function, 1, "selected route root")?;
                let input = vec![(root, weight)];
                if let Some(allowance) = floor {
                    assert!(crate::bounded_output::set_active_floor(
                        crate::bounded_output::active_remaining()
                            .unwrap()
                            .saturating_sub(allowance)
                    ));
                }
                match route {
                    0 => Canonicalizer::new(graph, sink).id(function, input, &mut budget),
                    1 => before_selected_route::Canonicalizer::new(graph, sink).id(
                        function,
                        input,
                        &mut budget,
                    ),
                    2 => before_paid_many::Canonicalizer::new(graph, sink).id(
                        function,
                        input,
                        &mut budget,
                    ),
                    _ => unreachable!("closed test route"),
                }
            })();
            let result = result.map_err(|error| failure(&error));
            let paid = budget.physical.as_ref().unwrap().charged();
            let work = initial - budget.remaining;
            assert!(
                current_metrics().allocated <= paid,
                "all actual backing remains paid"
            );
            let stopped = current_metrics();
            crate::bounded_output::clear_active_floor();
            if result.is_err() {
                let error = budget
                    .reserve_bytes(function, 0, "selected route sticky retry")
                    .unwrap_err();
                assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
                assert_eq!(budget.physical.as_ref().unwrap().charged(), paid);
                assert_eq!(budget.remaining, initial - work);
                assert_eq!(
                    current_metrics(),
                    stopped,
                    "restoring allowance cannot add backing"
                );
            }
            drop(budget);
            assert_eq!(
                current_metrics().allocated,
                stopped.allocated,
                "whole Drop adds no backing"
            );
            Outcome { result, paid, work }
        })
    })
    .0
}
fn settled(metrics: Metrics) {
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn selected_tree_fallback_has_exact_immediate_small_bills_work_and_whole_drop() {
    let function = fixture();
    for width in [0, 1] {
        let (graph, sink, root) = graph(&function, width, 2);
        for weight in [1, 2, u64::MAX] {
            let (old, old_heap) = observe(&function, &graph, sink, root, weight, 1 << 26, 2, None);
            let (current, heap) = observe(&function, &graph, sink, root, weight, 1 << 26, 0, None);
            let mut logical_budget = ReplayBudget::unobserved();
            let mut logical = Canonicalizer::new(&graph, sink);
            assert_eq!(
                logical
                    .id(&function, vec![(root, weight)], &mut logical_budget)
                    .unwrap(),
                *current.result.as_ref().unwrap()
            );
            // Cold-dead publication fills both maps without entering the
            // frame loop. This fresh one-root owner has no preexisting cache.
            let nonempty_exits = if logical.scratch_paid {
                logical
                    .states
                    .iter()
                    .filter(|(_, id)| {
                        logical
                            .ids
                            .iter()
                            .any(|((_, edges), stored)| stored == *id && !edges.is_empty())
                    })
                    .count()
            } else {
                0
            };
            let misses = if logical.scratch_paid {
                logical
                    .ids
                    .iter()
                    .filter(|((_, edges), _)| !edges.is_empty())
                    .count()
            } else {
                0
            };
            assert!(logical.ids.iter().all(|((_, edges), _)| edges.len() <= 1));
            let removed_backing = if nonempty_exits > 0 {
                std::mem::size_of::<usize>()
            } else {
                0
            };
            let old_removed = nonempty_exits * empty_signature::BRANCH_CONTROLS
                + nonempty_exits
                    * (intern_arena::SignatureQuery::EXIT_CONTROLS
                        + std::mem::size_of::<[usize; 2]>())
                + misses
                    * (intern_arena::SignatureQuery::MISS_CONTROLS
                        + std::mem::size_of::<[&usize; 2]>())
                + removed_backing;
            let new_added = if logical.scratch_paid {
                Canonicalizer::BORROWED_EXIT_LOCAL
            } else {
                0
            } + if nonempty_exits > 0 {
                Canonicalizer::BORROWED_EXIT_PRIMITIVE
            } else {
                0
            } + nonempty_exits * Canonicalizer::BORROWED_EXIT_WARM;
            assert_eq!(
                current.result, old.result,
                "same real small producer and all six error fields"
            );
            assert_eq!(current.work, old.work);
            assert_eq!(
                old.paid + new_added,
                current.paid + old_removed,
                "exact source-role equation; this direct entry forms no constructor fee request"
            );
            if nonempty_exits == 0 {
                assert_eq!(heap, old_heap, "no child-ID allocation existed to remove");
            } else {
                assert_eq!(old_heap.allocations, heap.allocations + 1);
                assert_eq!(old_heap.allocated, heap.allocated + removed_backing);
                assert_eq!(old_heap.freed, heap.freed + removed_backing);
                assert_eq!(old_heap.reallocations, heap.reallocations);
                assert!(
                    heap.highwater <= old_heap.highwater
                        && old_heap.highwater <= heap.highwater + removed_backing,
                    "source-tight peak range for one removed persistent allocation"
                );
            }
            eprintln!("selected-small composition width={width} weight={weight} old={old:?}/{old_heap:?} current={current:?}/{heap:?}");
            settled(heap);
            settled(old_heap);
            for (route, full) in [(0, &current), (2, &old)] {
                for limit in [0, full.paid - 1, full.paid] {
                    let (actual, physical) =
                        observe(&function, &graph, sink, root, weight, limit, route, None);
                    let (again, repeated_physical) =
                        observe(&function, &graph, sink, root, weight, limit, route, None);
                    assert_eq!(
                        actual, again,
                        "complete six fields, exact receipt and work at each own frontier"
                    );
                    assert_eq!(physical, repeated_physical);
                    assert_eq!(actual.result.is_ok(), limit == full.paid);
                    if limit == full.paid {
                        assert_eq!(&actual, full);
                    }
                    settled(physical);
                    settled(repeated_physical);
                }
            }
        }
    }
}

#[test]
fn selected_removed_group_owners_keep_complete_old_work_and_real_receipt_frontiers() {
    let function = fixture();
    for width in [0, 1, 2, 4] {
        let (graph, sink, root) = graph(&function, width, 2);
        for weight in [1, 2] {
            let (old, old_heap) = observe(&function, &graph, sink, root, weight, 1 << 26, 1, None);
            let (full, heap) = observe(&function, &graph, sink, root, weight, 1 << 26, 0, None);
            assert_eq!(full.result, old.result);
            assert_eq!(full.work, old.work);
            eprintln!("selected-group-removal width={width} weight={weight} old={old:?}/{old_heap:?} current={full:?}/{heap:?}");
            settled(heap);
            settled(old_heap);
            for limit in [0, full.paid - 1, full.paid] {
                let (actual, heap) = observe(&function, &graph, sink, root, weight, limit, 0, None);
                assert_eq!(actual.result.is_ok(), limit == full.paid);
                if limit == full.paid {
                    assert_eq!(actual, full);
                }
                settled(heap);
            }
            let (floored, heap) =
                observe(&function, &graph, sink, root, weight, 1 << 26, 0, Some(0));
            assert!(floored.result.is_err());
            settled(heap);
        }
    }
}

#[test]
fn selected_group_removal_retains_whole_immediate_body_and_unchanged_selected_tariffs() {
    let source = include_str!("before_selected_route.rs");
    let prefix = include_str!("../../../../../docs/evidence/opt831-870-selected-route-repair-20261017/before_selected_route.imports.txt");
    let suffix = include_str!("../../../../../docs/evidence/opt831-870-selected-route-repair-20261017/before_selected_route.suffix.txt");
    let body = source
        .strip_prefix(prefix)
        .unwrap()
        .strip_suffix(suffix)
        .unwrap();
    let whole = include_str!("../../../../../docs/evidence/opt831-870-selected-route-repair-20261017/canonical_shared.whole-before.rs.txt");
    let start = whole.find("// Invoked only after preflight").unwrap();
    let end = whole[start..].find("\n#[cfg(test)]\nmod tests;").unwrap() + start;
    assert_eq!(
        body,
        &whole[start..end],
        "complete immediate methods, all original selected banks"
    );
    assert_eq!(
        body.matches("return selected_labels_successors!").count(),
        2
    );
    let current = include_str!("../canonical_shared.rs");
    assert_eq!(
        current
            .matches("return selected_labels_successors!")
            .count(),
        0
    );
    assert_eq!(
        current
            .matches("AllocationPayer::SelectedReservation => {}")
            .count(),
        2
    );
    assert_eq!(include_str!("selected_labels.rs"), include_str!("../../../../../docs/evidence/opt831-870-selected-route-repair-20261017/selected_labels.whole-before.rs.txt"));
    assert_eq!(include_str!("paid_many.rs"), include_str!("../../../../../docs/evidence/opt831-870-selected-route-repair-20261017/paid_many.whole-before.rs.txt"));
}
