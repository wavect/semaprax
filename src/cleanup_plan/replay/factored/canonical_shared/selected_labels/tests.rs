//! Actual Selected canonical entry; no dispatch into the Active label producer.
use super::super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(
        super::super::super::lexical_plan_tests::WHILE,
    ).1
}
fn graph(function: &ResolvedFunction, case: usize) -> (Automaton, usize, usize) {
    use super::super::super::super::{SkeletonObservation, SkeletonTerminal};
    let mut setup = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut setup).unwrap();
    let root = graph.node(function, &mut setup).unwrap();
    let first = graph.end(function, &mut setup, sink, SkeletonTerminal::Failure).unwrap();
    let second = graph.end(function, &mut setup, sink, SkeletonTerminal::LoopLatch).unwrap();
    let a = Label::End(SkeletonTerminal::Success);
    let b = Label::End(SkeletonTerminal::Failure);
    let c = Label::End(SkeletonTerminal::LoopLatch);
    let mut edges = match case {
        0 => vec![],
        1 => vec![(a.clone(), first), (a, first)],
        2 => vec![(a.clone(), first), (a, second)],
        3 => vec![(b.clone(), second), (a.clone(), first), (b, second), (c, first), (a, second)],
        _ => {
            let left = Rc::new(SkeletonObservation::Boolean {
                expression: function.body.id.clone(), value: true,
            });
            let right = Rc::new((*left).clone());
            assert!(!Rc::ptr_eq(&left, &right));
            assert_eq!(left, right);
            vec![(Label::Observation(left), first), (Label::Observation(right), first)]
        }
    };
    for (label, child) in edges.drain(..) {
        graph.movement(function, &mut setup, root, label, child).unwrap();
    }
    (graph, sink, root)
}

#[test]
fn selected_inline_actual_parent_preserves_whole_ids_states_work_and_six_field_refusal() {
    let function = fixture();
    for case in 0..=4 {
        let (graph, sink, root) = graph(&function, case);
        for weight in [0, 1, 2, u64::MAX] {
            for work in [0, 1, 2, 4, 8, 16, 32, 128, 4096] {
                with_limit_usage_refusal(1 << 26, || {
                    let mut old_budget = ReplayBudget::with_physical(&function, 1 << 25).unwrap();
                    let mut budget = ReplayBudget::with_physical(&function, 1 << 25).unwrap();
                    old_budget.remaining = work;
                    budget.remaining = work;
                    let mut old = before_selected_labels::Canonicalizer::new(&graph, sink);
                    let mut current = Canonicalizer::new(&graph, sink);
                    let expected = old.id(&function, vec![(root, weight)], &mut old_budget);
                    let actual = current.id(&function, vec![(root, weight)], &mut budget);
                    // Diagnostic derives equality over code/severity/message/
                    // span/path/help; retain every field, including ordering.
                    assert_eq!(actual, expected, "case={case} weight={weight} work={work}");
                    assert_eq!((budget.remaining, budget.skeleton_remaining),
                        (old_budget.remaining, old_budget.skeleton_remaining));
                    assert!(current.ids.iter().eq(old.signatures().iter()));
                    assert!(current.states.iter().eq(old.states().iter()));
                });
            }
        }
    }
}

fn observe(function: &ResolvedFunction, graph: &Automaton, sink: usize, root: usize,
    limit: usize, old: bool) -> (Result<usize, ()>, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => { assert_eq!(error.code, "SPX-H006"); drop(error); return Err(()); }
        };
        let result = (|| {
            budget.reserve_array::<(usize, u64)>(function, 1, "selected inline root")?;
            let result = if old {
                let mut owner = before_selected_labels::Canonicalizer::new(graph, sink);
                owner.id(function, vec![(root, 1)], &mut budget)
            } else {
                let mut owner = Canonicalizer::new(graph, sink);
                owner.id(function, vec![(root, 1)], &mut budget)
            }?;
            assert!(result.is_some());
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged())
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        let result = result.map_err(|error| { assert_eq!(error.code, "SPX-H006"); drop(error); });
        drop(budget);
        assert_eq!(current_metrics().allocated, stopped, "whole Drop allocates no backing");
        result
    })
}

#[test]
fn selected_inline_reports_current_old_actual_tuples_and_every_byte_cut_through_drop() {
    let function = fixture();
    for case in 0..=4 {
        let (graph, sink, root) = graph(&function, case);
        let (old, old_metrics) = observe(&function, &graph, sink, root, 1 << 25, true);
        let (current, metrics) = observe(&function, &graph, sink, root, 1 << 25, false);
        let exact = current.unwrap();
        eprintln!("selected-inline-labels case={case} old={:?} current={:?}",
            (old.unwrap(), old_metrics), (exact, metrics));
        // No predicted charged-byte gain or historical tuple is an expectation.
        // The original strict widths0..4 owner remains unchanged and required.
        for limit in 0..=exact {
            let (result, metrics) = observe(&function, &graph, sink, root, limit, false);
            assert_eq!(result.is_ok(), limit == exact, "case={case} limit={limit}/{exact}");
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        for metrics in [old_metrics, metrics] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn selected_inline_warm_parent_rejects_foreign_function_budget_scope_and_restoration() {
    let function = fixture();
    let foreign_function = fixture();
    assert_eq!(function.id, foreign_function.id);
    let (graph, sink, root) = graph(&function, 1);
    for kind in 0..3 {
        with_limit_usage_refusal(1 << 26, || {
            let mut budget = ReplayBudget::with_physical(&function, 1 << 25).unwrap();
            let mut current = Canonicalizer::new(&graph, sink);
            assert!(current.id(&function, vec![(root, 1)], &mut budget).unwrap().is_some());
            let work = budget.remaining;
            let charged = budget.physical.as_ref().unwrap().charged();
            // Root argument construction is the unchanged external test caller.
            let error = if kind == 0 {
                current.id(&foreign_function, vec![(root, 1)], &mut budget).unwrap_err()
            } else if kind == 1 {
                let mut foreign = ReplayBudget::with_physical(&function, 1 << 25).unwrap();
                let foreign_work = foreign.remaining;
                let foreign_paid = foreign.physical.as_ref().unwrap().charged();
                let error = current.id(&function, vec![(root, 1)], &mut foreign).unwrap_err();
                assert_eq!(foreign.remaining, foreign_work);
                assert_eq!(foreign.physical.as_ref().unwrap().charged(), foreign_paid);
                error
            } else {
                let (error, refused, used, first) = with_limit_usage_refusal(0, || {
                    current.id(&function, vec![(root, 1)], &mut budget).unwrap_err()
                });
                assert!(!refused && used == 0 && first.is_none());
                error
            };
            assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
            assert_eq!(budget.remaining, work);
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
            let restored = current.id(&function, vec![(root, 1)], &mut budget).unwrap_err();
            assert_eq!(restored, error);
            assert_eq!(budget.remaining, work);
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
        });
    }
}

#[test]
fn complete_before_receiver_is_source_exact_with_only_test_wiring() {
    let before = include_str!("canonical.before.txt");
    let production = before.split("// Invoked only after preflight").nth(1).unwrap()
        .split("\n#[cfg(test)]\nmod tests;").next().unwrap();
    let compiled = include_str!("../before_selected_labels.rs");
    assert!(compiled.contains(&format!("// Invoked only after preflight{production}")));
    let current = include_str!("../../canonical_shared.rs");
    assert_eq!(current.matches("return selected_labels_successors!").count(), 2);
    assert_eq!(current.matches("AllocationPayer::ActiveBuilder =>").count(), 2);
    let selected = include_str!("../selected_labels.rs");
    assert!(!selected.contains("one_pass_successors("));
    assert!(selected.contains("if &*first == label"));
    assert!(selected.contains("std::mem::replace(&mut labels"));
}
