//! Complete predecessor algebra and real singleton/promotion accounting.
use super::super::SkeletonTerminal;
use super::{
    before_targets, successor_targets::Targets, Automaton, Canonicalizer, Diagnostic, Label,
    ReplayBudget, ResolvedFunction,
};
use crate::test_allocation_observer::{measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}
fn automaton(
    function: &ResolvedFunction,
    width: usize,
    repeats: usize,
) -> (Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    let mut children = Vec::new();
    for _ in 0..width {
        children.push(
            graph
                .end(function, &mut budget, sink, SkeletonTerminal::Failure)
                .unwrap(),
        );
    }
    for child in children.into_iter().rev() {
        for _ in 0..repeats {
            graph
                .movement(
                    function,
                    &mut budget,
                    root,
                    Label::End(SkeletonTerminal::Success),
                    child,
                )
                .unwrap();
        }
    }
    (graph, sink, root)
}
#[test]
fn singleton_targets_complete_before_receiver_keeps_widths_repeats_signatures_and_overflow_work() {
    let function = fixture();
    for width in 0..=4 {
        for repeats in [1, 2, 4] {
            let (graph, sink, root) = automaton(&function, width, repeats);
            let mut old = before_targets::Receiver::new(&graph, sink);
            let mut current = Canonicalizer::new(&graph, sink);
            let mut old_budget = ReplayBudget::unobserved();
            let mut budget = ReplayBudget::unobserved();
            for weight in [1, 2, u64::MAX] {
                let expected = old.id(&function, vec![(root, weight)], &mut old_budget);
                let actual = current.id(&function, vec![(root, weight)], &mut budget);
                assert_eq!(
                    actual, expected,
                    "full six fields, width={width} repeats={repeats}"
                );
                assert_eq!(budget.remaining, old_budget.remaining);
                assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
                assert_eq!(&current.ids, old.signatures());
                super::inline_weighted::assert_states(&current.states, old.states());
            }
        }
    }
}
fn probe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    root: usize,
    limit: usize,
    old: bool,
) -> (Result<usize, ()>, Metrics) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    let admission = budget.physical.as_ref().unwrap().charged();
    measure(|| {
        let result = (|| {
            budget.reserve_array::<(usize, u64)>(function, 1, "singleton control root")?;
            let id = if old {
                let mut receiver = before_targets::Receiver::new(graph, sink);
                receiver.id(function, vec![(root, 1)], &mut budget)?
            } else {
                let mut receiver = Canonicalizer::new(graph, sink);
                receiver.id(function, vec![(root, 1)], &mut budget)?
            };
            assert!(id.is_some());
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged() - admission)
        })();
        result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        })
    })
}
#[test]
fn singleton_targets_actual_before_current_reserved_allocated_peak_and_complete_drop() {
    let function = fixture();
    for width in 0..=4 {
        let (graph, sink, root) = automaton(&function, width, 1);
        let mut tuples = Vec::new();
        for old in [true, false] {
            let (paid, allocation) = probe(&function, &graph, sink, root, 1 << 24, old);
            let paid = paid.unwrap();
            assert!(
                !allocation.invalid && allocation.allocated <= paid && allocation.highwater <= paid
            );
            assert_eq!(allocation.live, 0);
            assert_eq!(allocation.allocated, allocation.freed);
            tuples.push((paid, allocation));
        }
        eprintln!("singleton-targets width={width} old/current={tuples:?}");
        // No universal net-benefit premise: larger inline layouts/control fees
        // may regress many targets. Only the real singleton removes its tree.
        if width == 1 {
            assert!(tuples[1].1.allocations < tuples[0].1.allocations);
        }
    }
}
#[test]
fn singleton_targets_full_receiver_exact_short_zero_bytes_and_work_keep_all_drop() {
    let function = fixture();
    for width in [1, 2, 4] {
        let (graph, sink, root) = automaton(&function, width, 2);
        let admission = ReplayBudget::with_physical(&function, 1 << 24)
            .unwrap()
            .physical
            .as_ref()
            .unwrap()
            .charged();
        let (full, _) = probe(&function, &graph, sink, root, 1 << 24, false);
        let paid = full.unwrap();
        for limit in [admission + paid, admission + paid - 1, admission] {
            let (result, allocation) = probe(&function, &graph, sink, root, limit, false);
            assert_eq!(result.is_ok(), limit == admission + paid);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
        }
        let mut full = ReplayBudget::unobserved();
        let before = full.remaining;
        Canonicalizer::new(&graph, sink)
            .id(&function, vec![(root, 1)], &mut full)
            .unwrap();
        let work = before - full.remaining;
        for limit in [work, work - 1, 0] {
            let mut budget = ReplayBudget::unobserved();
            budget.remaining = limit;
            let mut old_budget = ReplayBudget::unobserved();
            old_budget.remaining = limit;
            assert_eq!(
                Canonicalizer::new(&graph, sink).id(&function, vec![(root, 1)], &mut budget),
                before_targets::Receiver::new(&graph, sink).id(
                    &function,
                    vec![(root, 1)],
                    &mut old_budget
                )
            );
            assert_eq!(budget.remaining, old_budget.remaining);
            assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        }
    }
}
#[test]
fn singleton_target_promotions_sort_and_preserve_same_child_overflow_before_later_acquisition() {
    let function = fixture();
    let mut budget = ReplayBudget::unobserved();
    let mut one = Targets::one(&function, &mut budget, 9, 4).unwrap();
    one.promote(&function, &mut budget, 2, 7).unwrap();
    let output = one.finish(&function, &mut budget).unwrap();
    assert_eq!(&**output, &[(2, 7), (9, 4)]);
    // The overflow path is exercised through the ACTUAL caller, including its
    // next labels/targets, rather than a test-only duplicate accumulation loop.
    let (graph, sink, root) = automaton(&function, 2, 2);
    let mut current = Canonicalizer::new(&graph, sink);
    let mut old = before_targets::Receiver::new(&graph, sink);
    let mut old_budget = ReplayBudget::unobserved();
    let actual = current.id(&function, vec![(root, u64::MAX)], &mut budget);
    let expected = old.id(&function, vec![(root, u64::MAX)], &mut old_budget);
    assert_eq!(actual, expected);
    assert_eq!(actual.unwrap(), None);
    assert_eq!(budget.remaining, old_budget.remaining);
    assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
    // Two distinct children promote first, then the first child repeats and
    // overflows inside Many. A later cyclic label must never replace None.
    let (mut graph, sink, root) = automaton(&function, 2, 1);
    let first = graph.nodes[root].moves[0].1;
    let mut build = ReplayBudget::unobserved();
    graph
        .movement(
            &function,
            &mut build,
            root,
            Label::End(SkeletonTerminal::Success),
            first,
        )
        .unwrap();
    graph
        .movement(
            &function,
            &mut build,
            root,
            Label::End(SkeletonTerminal::LoopLatch),
            root,
        )
        .unwrap();
    let mut budget = ReplayBudget::unobserved();
    let mut old_budget = ReplayBudget::unobserved();
    let actual =
        Canonicalizer::new(&graph, sink).id(&function, vec![(root, u64::MAX)], &mut budget);
    let old = before_targets::Receiver::new(&graph, sink).id(
        &function,
        vec![(root, u64::MAX)],
        &mut old_budget,
    );
    assert_eq!(
        actual, old,
        "Many overflow wins over the later cyclic label"
    );
    assert_eq!(actual.unwrap(), None);
    assert_eq!(budget.remaining, old_budget.remaining);
    assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
}

#[test]
fn singleton_promotion_acquires_both_tree_nodes_before_first_allocation_and_stays_inline_on_refusal(
) {
    use std::mem::size_of;
    let function = fixture();
    let admission = ReplayBudget::with_physical(&function, 1 << 20)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let one_fee = 2 * size_of::<Targets>()
        + size_of::<Result<Targets, Diagnostic>>()
        + size_of::<Option<u64>>()
        + size_of::<Option<Targets>>();
    let promotion_fee = size_of::<Result<(), Diagnostic>>()
        + 2 * size_of::<std::collections::BTreeMap<usize, u64>>()
        + size_of::<Targets>()
        + 2 * size_of::<Option<u64>>()
        + 2 * size_of::<(usize, u64)>();
    let node = crate::cleanup_plan::replay_btree_node_upper::<usize, u64>().unwrap();
    eprintln!("singleton-layout old={} new={} one_fee={one_fee} promotion_fee={promotion_fee} node={node}",
        size_of::<std::collections::BTreeMap<usize, u64>>(), size_of::<Targets>());
    eprintln!("outer-label-node old={} new={}",
        crate::cleanup_plan::replay_btree_node_upper::<Label, std::collections::BTreeMap<usize, u64>>().unwrap(),
        crate::cleanup_plan::replay_btree_node_upper::<Label, Targets>().unwrap());
    for available in [
        0,
        promotion_fee - 1,
        promotion_fee + node - 1,
        promotion_fee + 2 * node - 1,
    ] {
        let mut budget =
            ReplayBudget::with_physical(&function, admission + one_fee + available).unwrap();
        let mut targets = Targets::one(&function, &mut budget, 9, 4).unwrap();
        let ((), allocation) = measure(|| {
            let error = targets.promote(&function, &mut budget, 2, 7).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            drop(error);
            assert!(matches!(
                targets,
                Targets::One {
                    child: 9,
                    weight: 4
                }
            ));
            let selected = budget.physical.as_mut().unwrap().reserve(0).unwrap_err();
            assert_eq!(budget.physical.as_mut().unwrap().reserve(1), Err(selected));
            let at = budget.physical.as_ref().unwrap().charged();
            drop(targets.finish(&function, &mut budget).unwrap_err());
            assert_eq!(budget.physical.as_ref().unwrap().charged(), at);
        });
        assert!(!allocation.invalid);
        assert_eq!(
            allocation.allocated, 0,
            "neither insert allocates before both reservations"
        );
        assert_eq!(allocation.live, 0);
    }
    let mut budget =
        ReplayBudget::with_physical(&function, admission + one_fee + promotion_fee + 2 * node)
            .unwrap();
    let mut targets = Targets::one(&function, &mut budget, 9, 4).unwrap();
    let ((), allocation) = measure(|| {
        targets.promote(&function, &mut budget, 2, 7).unwrap();
        assert!(matches!(targets, Targets::Many(_)));
        drop(targets);
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert!(allocation.allocated > 0 && allocation.allocated <= 2 * node);
    assert_eq!(allocation.allocated, allocation.freed);
}
#[test]
fn singleton_actual_receiver_foreign_restored_function_budget_and_stale_owner_fail_before_new_targets(
) {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::current_metrics;
    let function = fixture();
    let wrong = function.clone();
    let (graph, sink, root) = automaton(&function, 1, 2);
    for attack in 0..3 {
        let (((), allocation), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut current = Canonicalizer::new(&graph, sink);
                let mut budget = ReplayBudget::new();
                let mut foreign = ReplayBudget::new();
                budget
                    .reserve_array::<(usize, u64)>(&function, 1, "singleton owner root")
                    .unwrap();
                current.id(&function, vec![(root, 1)], &mut budget).unwrap();
                let input = vec![(root, 1)];
                if attack == 0 {
                    let (_, child_refused, child_paid, _) =
                        with_limit_usage_refusal(usize::MAX, || {
                            let at = current_metrics().allocated;
                            assert_eq!(
                                current.id(&function, input, &mut budget),
                                Err(Diagnostic::io("SPX-H006", String::new()))
                            );
                            assert_eq!(current_metrics().allocated, at);
                        });
                    assert!(!child_refused && child_paid == 0);
                } else {
                    let at = current_metrics().allocated;
                    let result = if attack == 1 {
                        current.id(&wrong, input, &mut budget)
                    } else {
                        current.id(&function, input, &mut foreign)
                    };
                    assert_eq!(result, Err(Diagnostic::io("SPX-H006", String::new())));
                    assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                    assert_eq!(current_metrics().allocated, at);
                }
                let input = vec![(root, 1)];
                let at = current_metrics().allocated;
                assert_eq!(
                    current.id(&function, input, &mut budget),
                    Err(Diagnostic::io("SPX-H006", String::new()))
                );
                assert_eq!(current_metrics().allocated, at);
                drop(current);
                drop(foreign);
                drop(budget);
            })
        });
        assert_eq!(refused, attack == 0);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    let ((), allocation) = measure(|| {
        let ((mut current, mut budget), refused, _, _) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut current = Canonicalizer::new(&graph, sink);
                let mut budget = ReplayBudget::new();
                budget
                    .reserve_array::<(usize, u64)>(&function, 1, "singleton stale root")
                    .unwrap();
                current.id(&function, vec![(root, 1)], &mut budget).unwrap();
                (current, budget)
            });
        assert!(!refused);
        let input = vec![(root, 1)];
        let at = current_metrics().allocated;
        assert_eq!(
            current.id(&function, input, &mut budget),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        assert_eq!(current_metrics().allocated, at);
        drop(current);
        drop(budget);
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

#[test]
fn singleton_finish_every_real_carrier_vector_and_rc_boundary_refuses_before_unpaid_backing() {
    use std::mem::size_of;
    let function = fixture();
    let admission = ReplayBudget::with_physical(&function, 1 << 20)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let one = 2 * size_of::<Targets>()
        + size_of::<Result<Targets, Diagnostic>>()
        + size_of::<Option<u64>>()
        + size_of::<Option<Targets>>();
    let finish = size_of::<Targets>()
        + size_of::<Result<std::rc::Rc<Vec<(usize, u64)>>, Diagnostic>>()
        + 2 * size_of::<std::rc::Rc<Vec<(usize, u64)>>>()
        + size_of::<Vec<(usize, u64)>>()
        + size_of::<(usize, u64)>()
        + size_of::<std::iter::Once<(usize, u64)>>();
    let vector = size_of::<(usize, u64)>();
    let header = super::physical_clone::rc_backing_bytes::<Vec<(usize, u64)>>();
    let full = one + finish + vector + header;
    for available in [
        0,
        one - 1,
        one,
        one + finish - 1,
        one + finish,
        one + finish + vector - 1,
        one + finish + vector,
        full - 1,
        full,
    ] {
        let mut budget = ReplayBudget::with_physical(&function, admission + available).unwrap();
        let ((), allocation) = measure(|| {
            let result = Targets::one(&function, &mut budget, 3, 7)
                .and_then(|target| target.finish(&function, &mut budget));
            if available == full {
                let state = result.unwrap();
                assert_eq!(&**state, &[(3, 7)]);
                assert_eq!(state.capacity(), 1);
                drop(state);
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let first = budget.physical.as_mut().unwrap().reserve(0).unwrap_err();
                let charged = budget.physical.as_ref().unwrap().charged();
                assert_eq!(budget.physical.as_mut().unwrap().reserve(1), Err(first));
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
            }
        });
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        let expected = if available == full {
            vector + header
        } else if available >= one + finish + vector {
            vector
        } else {
            0
        };
        assert_eq!(allocation.allocated, expected);
        assert_eq!(allocation.reallocations, 0);
    }
}
