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
    let namespace = r##"//! Complete 67046 production receiver in an independent test namespace.
//! Source-only: this new compilation unit has not been compiled or executed.
use super::{
    physical_clone, replay_error, AllocationPayer, Automaton, BTreeMap, Diagnostic,
    Label, Rc, ReplayBudget, ResolvedFunction, Weighted, app_census, arena_tests,
    borrowed_labels, closure_receiving, empty_closure, empty_memo, empty_signature,
    intern_arena, lexical_child_scratch, ordered_alias, ordered_weights, paid_many, tests,
};
use super::inline_weighted::SmallWeighted;
"##;
    let adapters = r##"
impl Canonicalizer<'_> {
    pub(super) fn signatures(&self) -> &intern_arena::Arena<intern_arena::Signature> { &self.ids }
    pub(super) fn states(&self) -> &intern_arena::Arena<SharedWeighted> { &self.states }
}

mod terminal_leaf;
"##;
    let restored = compiled.strip_prefix(namespace).unwrap().strip_suffix(adapters).unwrap();
    assert_eq!(restored, format!("// Invoked only after preflight{production}"));
    assert_eq!(include_str!("../before_selected_labels/terminal_leaf.rs"),
        include_str!("../terminal_leaf.rs"));
    let current = include_str!("../../canonical_shared.rs");
    let restored_current = current
        .replacen("#[macro_use]\nmod selected_labels;\n#[cfg(test)]\nmod before_selected_labels;\n", "", 1)
        .replacen("AllocationPayer::SelectedReservation => {\n                    return selected_labels_successors!(function, budget, self.automaton, closed, accepted, true);\n                }", "AllocationPayer::SelectedReservation => {}", 1)
        .replacen("AllocationPayer::SelectedReservation => {\n                return selected_labels_successors!(function, budget, self.automaton, closed, accepted, false);\n            }", "AllocationPayer::SelectedReservation => {}", 1);
    assert_eq!(restored_current, before, "complete parent inverse, no selection filter");
    assert_eq!(current.matches("return selected_labels_successors!").count(), 2);
    assert_eq!(current.matches("AllocationPayer::ActiveBuilder =>").count(), 2);
    let selected = include_str!("../selected_labels.rs");
    assert!(!selected.contains("one_pass_successors("));
    assert!(selected.contains("if &*first == label"));
    assert!(selected.contains("std::mem::replace(&mut labels"));
}

// Fixed stack-only observation metadata. Presence and exact UTF-8 bytes are
// retained independently: None and Some("") are not conflated or truncated.
#[derive(Debug, Eq, PartialEq)]
struct FieldText {
    present: bool,
    len: usize,
    bytes: [u8; 1024],
}
impl FieldText {
    fn capture(value: Option<&str>) -> Self {
        let mut result = Self { present: value.is_some(), len: 0, bytes: [0; 1024] };
        if let Some(value) = value {
            assert!(value.len() <= result.bytes.len(), "diagnostic observation must remain lossless");
            result.len = value.len();
            result.bytes[..value.len()].copy_from_slice(value.as_bytes());
        }
        result
    }
}
#[derive(Debug, Eq, PartialEq)]
struct FullDiagnostic {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: FieldText,
    span: Option<crate::ast::Span>,
    path: FieldText,
    help: FieldText,
}
impl FullDiagnostic {
    fn capture(error: &Diagnostic) -> Self {
        Self {
            code: error.code, severity: error.severity,
            message: FieldText::capture(Some(error.message.as_ref())),
            span: error.span, path: FieldText::capture(error.path.as_deref()),
            help: FieldText::capture(error.help.as_deref()),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
struct FrontierOutcome {
    result: Result<usize, ()>,
    first: Option<FullDiagnostic>,
    physical_reason: Option<crate::cleanup_plan::replay::physical_storage::PhysicalRefusal>,
}
fn observe_full_frontier(function: &ResolvedFunction, graph: &Automaton,
    sink: usize, root: usize, limit: usize) -> (FrontierOutcome, Metrics) {
    let mut first = None;
    let mut physical_reason = None;
    let (result, metrics) = measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                first = Some(FullDiagnostic::capture(&error));
                drop(error);
                // There is no live budget to retry after constructor refusal.
                return Err(());
            }
        };
        let mut owner = Canonicalizer::new(graph, sink);
        let result = (|| {
            budget.reserve_array::<(usize, u64)>(function, 1, "selected inline root")?;
            let id = owner.id(function, vec![(root, 1)], &mut budget)?;
            assert!(id.is_some());
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged())
        })();
        drop(owner);
        let result = match result {
            Ok(charged) => Ok(charged),
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                let selected = FullDiagnostic::capture(&error);
                let remaining = (budget.remaining, budget.skeleton_remaining);
                let charged = budget.physical.as_ref().unwrap().charged();
                // This observes the actual retained first reason; reserve(0)
                // on an already failed reservation neither clears nor debits it.
                let reason = budget.physical.as_mut().unwrap().reserve(0).unwrap_err();
                let enclosing_remaining = crate::bounded_output::active_remaining();
                let ledger = crate::bounded_output::classifier_string::ClassifierLedger::capture();
                let enclosing_first = ledger.first_refusal();
                let allocated = current_metrics().allocated;
                let retry = budget.reserve_bytes(function, 0, "selected inline sticky retry").unwrap_err();
                let empty = Diagnostic::io("SPX-H006", String::new());
                // First reporting text remains selected in its owning error;
                // retry reports the precise stable empty six-field status.
                assert_eq!(FullDiagnostic::capture(&error), selected);
                assert_eq!(FullDiagnostic::capture(&retry), FullDiagnostic::capture(&empty));
                assert_eq!(budget.physical.as_mut().unwrap().reserve(0), Err(reason));
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                assert_eq!((budget.remaining, budget.skeleton_remaining), remaining);
                assert_eq!(crate::bounded_output::active_remaining(), enclosing_remaining);
                assert_eq!(ledger.first_refusal(), enclosing_first);
                assert_eq!(current_metrics().allocated, allocated);
                first = Some(selected);
                physical_reason = Some(reason);
                drop(empty);
                drop(retry);
                drop(ledger);
                drop(error);
                Err(())
            }
        };
        let stopped = current_metrics().allocated;
        drop(budget);
        assert_eq!(current_metrics().allocated, stopped, "whole Drop allocates no backing");
        result
    });
    (FrontierOutcome { result, first, physical_reason }, metrics)
}
#[test]
fn selected_inline_every_actual_physical_frontier_retains_full_first_status_sticky_reason_and_drop() {
    let function = fixture();
    for case in 0..=4 {
        let (graph, sink, root) = graph(&function, case);
        let (success, metrics) = observe_full_frontier(&function, &graph, sink, root, 1 << 25);
        let exact = success.result.unwrap();
        assert!(success.first.is_none() && success.physical_reason.is_none());
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        for limit in 0..=exact {
            let (first, metrics) = observe_full_frontier(&function, &graph, sink, root, limit);
            let (repeat, repeated_metrics) = observe_full_frontier(&function, &graph, sink, root, limit);
            // Complete first-diagnostic values and the actual first physical
            // reason repeat exactly; no code-only or digest comparison is used.
            assert_eq!(first, repeat, "case={case} limit={limit}/{exact}");
            assert_eq!(first.result.is_ok(), limit == exact);
            assert_eq!(first.first.is_none(), limit == exact);
            for metrics in [metrics, repeated_metrics] {
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
        }
    }
}
