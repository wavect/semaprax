//! Actual composed successor producers, complete immediate and original receivers.
use super::super::{before_paid_many, Canonicalizer, Label};
use super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
use std::mem::size_of;

type Observation = Result<(Option<usize>, usize, usize), ()>;
fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
fn graph(
    function: &ResolvedFunction,
    width: usize,
    repeats: usize,
) -> (super::super::Automaton, usize, usize) {
    use super::super::super::SkeletonTerminal;
    let mut budget = ReplayBudget::unobserved();
    let mut graph = super::super::Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    // Sparse actual node identities; reversed insertion exercises flat shifts.
    let mut children = Vec::new();
    for _ in 0..width {
        graph.node(function, &mut budget).unwrap();
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
fn same_maps(current: &Canonicalizer<'_>, old: &before_paid_many::Canonicalizer<'_>) {
    assert_eq!(current.states.len(), old.states().len());
    assert!(current.states.iter().eq(old.states().iter()));
    assert_eq!(current.ids.len(), old.signatures().len());
    assert!(current.ids.iter().eq(old.signatures().iter()));
}

#[test]
fn composed_targets_keep_complete_inline_and_original_receivers_work_order_and_overflow() {
    let function = fixture();
    for width in [0, 1, 2, 4, 17] {
        for repeats in [1, 2, 4] {
            let (graph, sink, root) = graph(&function, width, repeats);
            for weight in [0, 1, 2, u64::MAX] {
                let mut full = ReplayBudget::unobserved();
                let mut old = before_paid_many::Canonicalizer::new(&graph, sink);
                let initial = full.remaining;
                old.id(&function, vec![(root, weight)], &mut full).unwrap();
                let work = initial - full.remaining;
                for remaining in [0, work.saturating_sub(1), work, initial] {
                    let mut old = before_paid_many::Canonicalizer::new(&graph, sink);
                    let mut current = Canonicalizer::new(&graph, sink);
                    let mut original =
                        super::super::super::canonical_before::Receiver::new(&graph, sink);
                    let mut a = ReplayBudget::unobserved();
                    let mut b = ReplayBudget::unobserved();
                    let mut c = ReplayBudget::unobserved();
                    a.remaining = remaining;
                    b.remaining = remaining;
                    c.remaining = remaining;
                    let expected = old.id(&function, vec![(root, weight)], &mut a);
                    let actual = current.id(&function, vec![(root, weight)], &mut b);
                    let original_result = original.id(&function, vec![(root, weight)], &mut c);
                    assert_eq!(actual, expected, "complete six-field diagnostics and IDs");
                    assert_eq!(actual, original_result, "complete original tree algorithm");
                    assert_eq!(&current.ids, original.signatures());
                    assert_eq!(current.states.len(), original.states().len());
                    for ((state, id), (expected, previous_id)) in
                        current.states.iter().zip(original.states())
                    {
                        assert_eq!(state.as_slice(), expected.as_slice());
                        assert_eq!(id, previous_id);
                    }
                    assert_eq!(
                        (a.remaining, a.skeleton_remaining),
                        (b.remaining, b.skeleton_remaining)
                    );
                    assert_eq!(
                        (a.remaining, a.skeleton_remaining),
                        (c.remaining, c.skeleton_remaining)
                    );
                    same_maps(&current, &old);
                    assert!(
                        current.cursor.is_empty()
                            && current.frames.is_empty()
                            && current.open.is_empty()
                    );
                }
            }
        }
    }
}

fn observe(
    function: &ResolvedFunction,
    graph: &super::super::Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    old: bool,
) -> (Observation, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                return Err(());
            }
        };
        let result = (|| {
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "canonical ordered dictionary owner receiving",
            )?;
            budget.reserve_array::<(usize, u64)>(function, 1, "composed canonical root")?;
            let initial = budget.remaining;
            let id = if old {
                before_paid_many::Canonicalizer::new(graph, sink).id(
                    function,
                    vec![(root, weight)],
                    &mut budget,
                )?
            } else {
                Canonicalizer::new(graph, sink).id(function, vec![(root, weight)], &mut budget)?
            };
            Ok::<_, Diagnostic>((
                id,
                budget.physical.as_ref().unwrap().charged(),
                initial - budget.remaining,
            ))
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        let result = result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
        });
        drop(budget);
        assert_eq!(
            current_metrics().allocated,
            stopped,
            "publication and whole Drop allocate no backing"
        );
        result
    })
}

#[test]
fn composed_paid_producer_measures_both_receiving_paths_and_exact_short_zero_whole_drop() {
    let function = fixture();
    for width in [0, 1, 2, 4, 17] {
        for weight in [1, 2, u64::MAX] {
            let (graph, sink, root) = graph(&function, width, 2);
            let mut tuples = Vec::new();
            for old in [true, false] {
                ENTRIES.with(|value| value.set((0, 0)));
                let (full, allocation) =
                    observe(&function, &graph, sink, root, weight, 1 << 26, old);
                let (id, exact, work) = full.unwrap();
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                assert!(allocation.allocated <= exact && allocation.highwater <= exact);
                if !old && width > 1 && weight < u64::MAX {
                    let entries = ENTRIES.with(std::cell::Cell::get);
                    assert!(
                        if weight == 1 {
                            entries.0 > 0
                        } else {
                            entries.1 > 0
                        },
                        "actual paid unit/general successor branch must promote"
                    );
                }
                for limit in [0, exact - 1, exact] {
                    let (result, physical) =
                        observe(&function, &graph, sink, root, weight, limit, old);
                    assert_eq!(result.is_ok(), limit == exact);
                    if let Ok((actual, paid, charged_work)) = result {
                        assert_eq!((actual, paid, charged_work), (id, exact, work));
                    }
                    assert!(!physical.invalid && physical.live == 0);
                    assert_eq!(physical.allocated, physical.freed);
                }
                tuples.push((id, exact, work, allocation));
            }
            assert_eq!(tuples[0].0, tuples[1].0);
            // New helper/return controls and flat shifts may be unfavorable.
            // These are actual complete owners, never a universal gain assertion.
            eprintln!("OPT870 width={width} weight={weight} immediate-inline/composed={tuples:?}");
            if width <= 1 {
                assert_eq!((tuples[0].1, tuples[0].2), (tuples[1].1, tuples[1].2));
                assert_eq!(tuples[0].3.allocated, tuples[1].3.allocated);
            }
        }
    }
}

#[test]
fn composed_actual_successors_preserve_sorted_children_weights_and_competing_overflow() {
    let function = fixture();
    let (automaton, sink, root) = graph(&function, 4, 2);
    let mut old = before_paid_many::Canonicalizer::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut a = ReplayBudget::unobserved();
    let mut b = ReplayBudget::unobserved();
    let mut old_receiving =
        super::super::closure_receiving::ClosureReceiving::from_prepaid(&function, &mut a);
    let mut current_receiving =
        super::super::closure_receiving::ClosureReceiving::from_prepaid(&function, &mut b);
    let first = current.closure(&function, root, &mut b);
    let second = current.closure_with_receiving(&function, root, &mut b, &mut current_receiving);
    assert_eq!(
        (first, second),
        old.compatibility_closures(&function, root, &mut a, &mut old_receiving)
    );
    assert_eq!(
        (a.remaining, a.skeleton_remaining),
        (b.remaining, b.skeleton_remaining)
    );

    for width in [0, 1, 2, 4, 17] {
        for repeats in [1, 2, 4] {
            let (graph, sink, root) = graph(&function, width, repeats);
            for weight in [0, 1, 2, u64::MAX] {
                // Immutable test input is outside the measured canonical owner.
                let state =
                    super::super::Rc::new(super::super::SmallWeighted::Many(vec![(root, weight)]));
                let mut old = before_paid_many::Canonicalizer::new(&graph, sink);
                let mut current = Canonicalizer::new(&graph, sink);
                let mut a = ReplayBudget::with_physical(&function, 1 << 26).unwrap();
                let mut b = ReplayBudget::with_physical(&function, 1 << 26).unwrap();
                let expected = old.successors(&function, &state, &mut a);
                let actual = current.successors(&function, &state, &mut b);
                assert_eq!(
                    actual, expected,
                    "full labels, actual child identities, weights and six diagnostic fields"
                );
                if let Ok(Some((accepted, children))) = actual {
                    assert_eq!(accepted, 0);
                    assert_eq!(children.len(), usize::from(width > 0));
                    if let Some((_, targets)) = children.first() {
                        assert_eq!(targets.len(), width);
                        assert!(targets.windows(2).all(|pair| pair[0].0 < pair[1].0));
                        assert!(targets
                            .iter()
                            .all(|(_, routes)| *routes == weight * repeats as u64));
                        if width == 1 {
                            assert_eq!(targets.capacity(), 1);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn composed_target_and_iterator_layouts_preserve_actual_singleton_receivers() {
    use super::super::successor_targets::Targets as Previous;
    assert_eq!(size_of::<Targets>(), size_of::<Previous>());
    assert_eq!(size_of::<Option<Targets>>(), size_of::<Option<Previous>>());
    assert_eq!(
        size_of::<BTreeMap<Label, Targets>>(),
        size_of::<BTreeMap<Label, Previous>>()
    );
    assert_eq!(
        size_of::<std::collections::btree_map::IntoIter<Label, Targets>>(),
        size_of::<std::collections::btree_map::IntoIter<Label, Previous>>()
    );
    assert_eq!(
        size_of::<Option<(Label, Targets)>>(),
        size_of::<Option<(Label, Previous)>>()
    );
    assert_eq!(
        size_of::<Canonicalizer<'_>>(),
        size_of::<before_paid_many::Canonicalizer<'_>>()
    );
    assert_eq!(
        Canonicalizer::CONSTRUCTOR_CONTROLS,
        before_paid_many::Canonicalizer::CONSTRUCTOR_CONTROLS
    );
    eprintln!(
        "OPT870 actual target layouts old={} current={} promotion={} add={} finish={}",
        size_of::<Previous>(),
        size_of::<Targets>(),
        PROMOTE_RECEIVING,
        ADD_RECEIVING,
        FINISH_RECEIVING
    );
}

#[test]
fn composed_flat_helpers_keep_sorted_sparse_multiplicity_overflow_and_prepublication_refusal() {
    let function = fixture();
    for reversed in [false, true] {
        let mut budget = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
        budget
            .reserve_bytes(
                &function,
                PROMOTE_RECEIVING,
                "test actual promotion receivers",
            )
            .unwrap();
        let (first, child) = if reversed { (97, 3) } else { (3, 97) };
        let Targets::PaidMany(mut flat) =
            promote(&function, &mut budget, first, 2, child, 3).unwrap()
        else {
            panic!("actual paid promotion")
        };
        let mut old = BTreeMap::from([(first, 2), (child, 3)]);
        for (child, weight) in [(41, 7), (3, 11), (17, 2), (97, 13), (41, 19)] {
            budget
                .reserve_bytes(&function, ADD_RECEIVING, "test actual add receivers")
                .unwrap();
            assert!(add(&function, &mut budget, &mut flat, child, weight).unwrap());
            *old.entry(child).or_insert(0) += weight;
            assert!(flat
                .iter()
                .copied()
                .eq(old.iter().map(|(key, value)| (*key, *value))));
        }
        budget
            .reserve_bytes(&function, ADD_RECEIVING, "test overflow receivers")
            .unwrap();
        assert!(!add(&function, &mut budget, &mut flat, 3, u64::MAX).unwrap());
        assert!(
            flat.iter()
                .copied()
                .eq(old.iter().map(|(key, value)| (*key, *value))),
            "checked overflow never mutates the selected slot"
        );
    }
    // Derive the complete actual helper price, then repeat every byte boundary.
    let mut full = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    full.reserve_bytes(
        &function,
        PROMOTE_RECEIVING,
        "test actual promotion receivers",
    )
    .unwrap();
    let start = full.physical.as_ref().unwrap().charged();
    drop(promote(&function, &mut full, 97, 2, 3, 3).unwrap());
    let price = full.physical.as_ref().unwrap().charged() - start + PROMOTE_RECEIVING;
    // with_physical owns its independently priced initial controls as well.
    let initial = start - PROMOTE_RECEIVING;
    for limit in 0..=initial + price {
        let (success, metrics) = measure(|| {
            let result = ReplayBudget::with_physical(&function, limit).and_then(|mut budget| {
                budget.reserve_bytes(
                    &function,
                    PROMOTE_RECEIVING,
                    "test actual promotion receivers",
                )?;
                promote(&function, &mut budget, 97, 2, 3, 3)
            });
            let success = result.is_ok();
            drop(result);
            success
        });
        assert_eq!(success, limit == initial + price);
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}

#[test]
fn composed_frozen_actual_comparison_keeps_full_source_trace_and_diagnostic_order() {
    use super::super::super::{compare_with_index_storage, lexical_plan_tests};
    for source in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let (program, function) = lexical_plan_tests::fixture(source);
        lexical_plan_tests::assert_plan_oracle(&function);
        let mut old = ReplayBudget::unobserved();
        let mut current = ReplayBudget::unobserved();
        let initial = old.remaining;
        let expected = before_paid_many::comparison::compare_with_index_storage(
            &program, &function, &mut old, None, None,
        );
        assert_eq!(
            compare_with_index_storage(&program, &function, &mut current, None, None),
            expected
        );
        assert_eq!(
            (current.remaining, current.skeleton_remaining),
            (old.remaining, old.skeleton_remaining)
        );
        assert_eq!(expected, Ok(Some(true)));
        for old in [true, false] {
            let observe = |limit| {
                measure(|| {
                    let mut budget = match ReplayBudget::with_physical(&function, limit) {
                        Ok(budget) => budget,
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            return Err(());
                        }
                    };
                    let result = if old {
                        before_paid_many::comparison::compare_with_index_storage(
                            &program,
                            &function,
                            &mut budget,
                            None,
                            None,
                        )
                    } else {
                        compare_with_index_storage(&program, &function, &mut budget, None, None)
                    };
                    let paid = budget.physical.as_ref().unwrap().charged();
                    assert!(
                        current_metrics().allocated <= paid,
                        "whole actual comparison caller backing"
                    );
                    let stopped = current_metrics().allocated;
                    let result = result
                        .map(|equal| {
                            assert_eq!(equal, Some(true));
                            paid
                        })
                        .map_err(|error| {
                            assert_eq!(error.code, "SPX-H006");
                        });
                    drop(budget);
                    assert_eq!(current_metrics().allocated, stopped);
                    result
                })
            };
            let (result, metrics) = observe(1 << 26);
            let exact = result.unwrap();
            eprintln!("OPT870 actual comparison immediate={old} paid={exact} physical={metrics:?}");
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
            for limit in [0, exact - 1, exact] {
                let (result, metrics) = observe(limit);
                assert_eq!(result.is_ok(), limit == exact);
                assert!(
                    !metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed
                );
            }
        }
        let work = initial - old.remaining;
        for remaining in [0, work.saturating_sub(1), work] {
            let mut old = ReplayBudget::unobserved();
            let mut current = ReplayBudget::unobserved();
            old.remaining = remaining;
            current.remaining = remaining;
            assert_eq!(
                compare_with_index_storage(&program, &function, &mut current, None, None),
                before_paid_many::comparison::compare_with_index_storage(
                    &program, &function, &mut old, None, None
                )
            );
            assert_eq!(
                (current.remaining, current.skeleton_remaining),
                (old.remaining, old.skeleton_remaining)
            );
        }
    }
}

#[test]
fn composed_paid_memo_rejects_foreign_function_budget_and_ledger_then_stays_poisoned() {
    use crate::bounded_output::{
        active_remaining, reserve_active_required, with_limit_usage_refusal,
        with_limit_usage_refusal_site,
    };
    let function = fixture();
    let wrong = function.clone();
    for old in [true, false] {
        for attack in 0..4 {
            let (graph, sink, root) = graph(&function, 4, 2);
            let (((), metrics), refused, _, first, site) =
                with_limit_usage_refusal_site(1 << 26, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let mut foreign = ReplayBudget::new();
                        budget
                            .reserve_bytes(
                                &function,
                                Canonicalizer::CONSTRUCTOR_CONTROLS,
                                "canonical ordered dictionary owner receiving",
                            )
                            .unwrap();
                        macro_rules! exercise {
                            ($owner:expr) => {{
                                let mut owner = $owner;
                                budget
                                    .reserve_array::<(usize, u64)>(
                                        &function,
                                        1,
                                        "composed bound root",
                                    )
                                    .unwrap();
                                assert!(owner
                                    .id(&function, vec![(root, 1)], &mut budget)
                                    .unwrap()
                                    .is_some());
                                budget
                                    .reserve_array::<(usize, u64)>(
                                        &function,
                                        1,
                                        "composed hostile root",
                                    )
                                    .unwrap();
                                let input = vec![(root, 1)];
                                budget
                                    .reserve_array::<(usize, u64)>(
                                        &function,
                                        1,
                                        "composed restored root",
                                    )
                                    .unwrap();
                                let retry = vec![(root, 1)];
                                if attack == 0 {
                                    assert!(!reserve_active_required(
                                        active_remaining().unwrap() + 1
                                    ));
                                }
                                let remaining = active_remaining();
                                let work = budget.remaining;
                                let allocated = current_metrics().allocated;
                                let error = match attack {
                                    1 => owner.id(&wrong, input, &mut budget),
                                    2 => owner.id(&function, input, &mut foreign),
                                    3 => {
                                        let (result, nested_refused, paid, first) =
                                            with_limit_usage_refusal(1 << 26, || {
                                                let at = current_metrics().allocated;
                                                let result =
                                                    owner.id(&function, input, &mut budget);
                                                assert_eq!(current_metrics().allocated, at);
                                                result
                                            });
                                        assert!(!nested_refused && paid == 0 && first.is_none());
                                        result
                                    }
                                    _ => owner.id(&function, input, &mut budget),
                                };
                                assert_eq!(error, Err(Diagnostic::io("SPX-H006", String::new())));
                                assert_eq!(active_remaining(), remaining);
                                assert_eq!(budget.remaining, work);
                                if attack != 3 {
                                    assert_eq!(current_metrics().allocated, allocated);
                                }
                                let allocated = current_metrics().allocated;
                                assert_eq!(
                                    owner.id(&function, retry, &mut budget),
                                    Err(Diagnostic::io("SPX-H006", String::new()))
                                );
                                assert_eq!(current_metrics().allocated, allocated);
                                assert_eq!(active_remaining(), remaining);
                                assert_eq!(budget.remaining, work);
                                assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                            }};
                        }
                        if old {
                            exercise!(before_paid_many::Canonicalizer::new(&graph, sink));
                        } else {
                            exercise!(Canonicalizer::new(&graph, sink));
                        }
                    })
                });
            assert_eq!(refused, attack == 0);
            assert_eq!(first.is_some(), attack == 0);
            assert_eq!(site.is_some(), attack == 0);
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn composed_promotion_work_refusal_keeps_six_fields_first_order_and_zero_allocation_sticky_retry() {
    let function = fixture();
    for work in [0, 1, 2] {
        let phase = if work == 0 {
            "factored flat promotion key comparison"
        } else {
            "factored flat promotion insertions"
        };
        let expected = Diagnostic::io("SPX-H006", format!(
            "cleanup plan for function `{}` failed independent replay: cleanup replay work budget exhausted during {phase}",
            function.id.as_str()));
        let ((), metrics) = measure(|| {
            let mut budget = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
            budget
                .reserve_bytes(
                    &function,
                    PROMOTE_RECEIVING,
                    "test actual promotion receivers",
                )
                .unwrap();
            budget.remaining = work;
            let allocated = current_metrics().allocated;
            let error = match promote(&function, &mut budget, 97, 2, 3, 3) {
                Ok(_) => panic!("work refuses before flat backing or publication"),
                Err(error) => error,
            };
            assert_eq!(
                error, expected,
                "all six fields, first comparison then insertions"
            );
            assert_eq!(
                current_metrics().allocated,
                allocated,
                "prepaid first diagnostic only"
            );
            let charged = budget.physical.as_ref().unwrap().charged();
            let error = match promote(&function, &mut budget, 97, 2, 3, 3) {
                Ok(_) => panic!("selected work failure stays sticky"),
                Err(error) => error,
            };
            assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
            assert_eq!(current_metrics().allocated, allocated);
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
            assert_eq!((budget.remaining, budget.skeleton_remaining), (0, 0));
        });
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
}


#[test]
fn memo_identity_gate_rejects_nested_ledger_without_spending_or_poisoning_either_payer() {
    use crate::bounded_output::{active_remaining, with_limit_usage_refusal, with_limit_usage_refusal_site};
    let function = fixture();
    for old in [true, false] {
        for authenticate in [false, true] {
            for nested_limit in [0, 1 << 26] {
                let (((), metrics), refused, _paid, first, site) =
                    with_limit_usage_refusal_site(1 << 26, || {
                        measure(|| {
                            let mut budget = ReplayBudget::new();
                            macro_rules! exercise {
                                ($memo:expr) => {{
                                    let mut memo = $memo;
                                    memo.authenticate(&function, &mut budget).unwrap();
                                    memo.insert_empty_bound(&function, 0, &mut budget).unwrap();
                                    assert_eq!(memo.state(&0), 1);
                                    let remaining = active_remaining();
                                    let work = (budget.remaining, budget.skeleton_remaining);
                                    let allocated = current_metrics().allocated;
                                    let ((), nested_refused, nested_paid, nested_first) =
                                        with_limit_usage_refusal(nested_limit, || {
                                            let nested_remaining = active_remaining();
                                            if authenticate {
                                                assert_eq!(memo.authenticate(&function, &mut budget),
                                                    Err(Diagnostic::io("SPX-H006", String::new())));
                                            } else {
                                                assert!(memo.preflight(&function, &budget));
                                                assert!(memo.failed());
                                            }
                                            assert_eq!(memo.state(&0), 0, "poisoned views stay unavailable");
                                            assert_eq!(memo.authenticate(&function, &mut budget),
                                                Err(Diagnostic::io("SPX-H006", String::new())));
                                            assert_eq!(active_remaining(), nested_remaining);
                                        });
                                    assert!(!nested_refused && nested_paid == 0 && nested_first.is_none());
                                    assert_eq!(active_remaining(), remaining);
                                    assert_eq!((budget.remaining, budget.skeleton_remaining), work);
                                    assert_eq!(current_metrics().allocated, allocated);
                                    // Restoring the original invocation must not revive the Memo.
                                    assert_eq!(memo.authenticate(&function, &mut budget),
                                        Err(Diagnostic::io("SPX-H006", String::new())));
                                    assert!(!memo.preflight(&function, &budget));
                                    assert_eq!(memo.state(&0), 0);
                                    assert_eq!(active_remaining(), remaining);
                                    assert_eq!((budget.remaining, budget.skeleton_remaining), work);
                                    assert_eq!(current_metrics().allocated, allocated);
                                    drop(memo);
                                }};
                            }
                            if old {
                                exercise!(before_identity::Memo::<()>::new());
                            } else {
                                exercise!(super::super::empty_memo::Memo::<()>::new());
                            }
                            drop(budget);
                        })
                    });
                // The complete old Memo remains an executable negative witness:
                // its unchanged current() gateway poisons the captured outer ledger.
                assert_eq!(refused, old);
                assert_eq!(first.is_some(), old);
                assert_eq!(site.is_some(), old);
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
        }
    }
}

// Complete immediate-old Memo and prices remain independently compiled.
mod before_identity {
//! Closure-only memo copy: fixed slot bitmap records explicit empty success.
//! Ordinary Memo/RetainedMemo remain byte-exact independent owners.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;
const WIDTH: usize = 8;

struct Chunk<T> {
    values: Option<Box<[Option<T>; WIDTH]>>,
    empty: u8,
}
const _: () = assert!(WIDTH <= u8::BITS as usize);

pub(super) struct Memo<T> {
    chunks: Vec<Chunk<T>>,
    pub(super) binding: Option<Binding>,
    pub(super) failed: bool,
}
pub(super) struct Binding {
    pub(super) function: *const ResolvedFunction,
    pub(super) budget: *const ReplayBudget,
    pub(super) ledger: ClassifierLedger,
}
impl<T> Memo<T> {
    pub(super) fn new() -> Self {
        Self {
            chunks: Vec::new(),
            binding: None,
            failed: false,
        }
    }
    pub(super) fn authenticate(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        if let Some(binding) = &self.binding {
            // Read no foreign payer, even to construct a refusal report. The
            // exact caller keeps both borrowed receivers at stable addresses.
            let ledger_matches = binding.ledger.current();
            if binding.function != std::ptr::from_ref(function)
                || binding.budget != std::ptr::from_ref(budget)
                || !ledger_matches
            {
                self.failed = true;
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
        }
        if let Err(error) =
            budget.reserve_bytes(function, 0, "factored closure memo authentication")
        {
            self.failed = true;
            return Err(error);
        }
        if budget.pays_allocations() {
            if let Err(error) = budget.reserve_bytes(
                function,
                size_of::<Result<(), Diagnostic>>(),
                "factored closure memo authorization receipt",
            ) {
                self.failed = true;
                return Err(error);
            }
        }
        if self.binding.is_none() {
            // One live Memo receiver (including its binding slot) and the
            // separately named Option<Binding> initializer below.
            // The old Canonicalizer constructor created only allocation-free
            // inert fields; activation pays its new inline receiver once.
            if budget.pays_allocations() {
                if let Err(error) = budget.reserve_bytes(
                    function,
                    size_of::<Self>() + size_of::<Option<Binding>>(),
                    "factored closure memo controls",
                ) {
                    self.failed = true;
                    return Err(error);
                }
            }
            let binding = Some(Binding {
                function: std::ptr::from_ref(function),
                budget: std::ptr::from_ref(budget),
                ledger: ClassifierLedger::capture(),
            });
            self.binding = binding;
        }
        Ok(())
    }
    // Identity-only preflight precedes acquiring a fresh invocation destination.
    // It neither asks a foreign payer nor constructs a diagnostic receiver.
    pub(super) fn preflight(&mut self, function: &ResolvedFunction, budget: &ReplayBudget) -> bool {
        if self.failed {
            return false;
        }
        if let Some(binding) = &self.binding {
            let ledger_matches = binding.ledger.current();
            if binding.function != std::ptr::from_ref(function)
                || binding.budget != std::ptr::from_ref(budget)
                || !ledger_matches
            {
                self.failed = true;
            } else if budget.refusal_spent
                || (budget.pays_allocations() && crate::bounded_output::active_refused())
            {
                // Identity is already bound before reading this same account.
                // A warm poisoned owner must not consume its report or reset
                // work through a fresh zero-byte payer authentication.
                self.failed = true;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn unpublished(&self) -> bool { self.chunks.is_empty() }
    #[cfg(test)]
    pub(super) fn payload_chunks(&self) -> usize {
        self.chunks.iter().filter(|chunk| chunk.values.is_some()).count()
    }
    pub(super) fn failed(&self) -> bool {
        self.failed
    }
    // Caller already holds the selected error; retain it unchanged while
    // invalidating views without querying any payer or allocating a report.
    pub(super) fn reject_after_error(&mut self) {
        self.failed = true;
    }
    pub(super) fn contains_key(&self, key: &usize) -> bool {
        self.state(key) != 0
    }
    // Replaces the old contains_key boolean result with a one-byte state code.
    // No owned Option/Rc/enum is transported by this borrowed metadata query.
    pub(super) fn state(&self, key: &usize) -> u8 {
        if self.failed {
            return 0;
        }
        match self.chunks.get(*key / WIDTH) {
            Some(chunk) => {
                if chunk.values.as_ref().is_some_and(|values| values[*key % WIDTH].is_some()) {
                    2
                } else if chunk.empty & (1_u8 << (*key % WIDTH)) != 0 {
                    1
                } else {
                    0
                }
            }
            None => 0,
        }
    }
    pub(super) fn get(&self, key: &usize) -> Option<&T> {
        if self.failed {
            return None;
        }
        self.chunks
            .get(*key / WIDTH)?
            .values
            .as_ref()?
            .get(*key % WIDTH)?
            .as_ref()
    }
    #[cfg(test)]
    pub(super) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        self.authenticate(function, budget)?;
        let result = self.insert_bound(function, key, budget, make);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(super) fn insert_bound(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        let index = key / WIDTH;
        // WIDTH > 1, so quotient + 1 cannot overflow usize.
        let required = index + 1;
        if self.chunks.len() < required {
            let additional = required - self.chunks.len();
            budget.reserve_vec(
                function,
                &mut self.chunks,
                additional,
                "factored closure memo directory",
            )?;
            // One actual inline slot initializer; receiving directory backing
            // is separately retained by reserve_vec above.
            budget.reserve_array::<Chunk<T>>(function, additional, "factored closure memo slot initializer")?;
            self.chunks.resize_with(required, || Chunk { values: None, empty: 0 });
        }
        if self.chunks[index].values.is_none() {
            if budget.pays_allocations() {
                // Full array initializer and heap array, Box local and Some
                // initializer. No payload backing exists for empty-only slots.
                budget.reserve_bytes(function,
                    2 * size_of::<[Option<T>; WIDTH]>() + 2 * size_of::<Box<[Option<T>; WIDTH]>>(),
                    "factored closure memo payload promotion")?;
            }
            let values = Box::new(std::array::from_fn(|_| None));
            self.chunks[index].values = Some(values);
        }
        self.chunks[index].values.as_mut().expect("paid payload")[key % WIDTH] = Some(make());
        Ok(())
    }
    pub(super) fn insert_empty_bound(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        let index = key / WIDTH;
        // WIDTH > 1, so quotient + 1 cannot overflow usize.
        let required = index + 1;
        if self.chunks.len() < required {
            let additional = required - self.chunks.len();
            budget.reserve_vec(
                function,
                &mut self.chunks,
                additional,
                "factored closure memo directory",
            )?;
            // One actual inline slot initializer; receiving directory backing
            // is separately retained by reserve_vec above.
            budget.reserve_array::<Chunk<T>>(function, additional, "factored closure memo slot initializer")?;
            self.chunks.resize_with(required, || Chunk { values: None, empty: 0 });
        }
        // The original empty bits survive later paid payload promotion.
        // Only unpublished keys reach this producer, exactly as before.
        self.chunks[index].empty |= 1_u8 << (key % WIDTH);
        Ok(())
    }
}
}
