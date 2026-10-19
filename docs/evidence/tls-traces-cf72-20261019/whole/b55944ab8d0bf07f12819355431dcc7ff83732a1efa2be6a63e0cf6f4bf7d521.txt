//! Authored current/complete-immediate comparator owners; execution pending.
use super::*;
mod source_format;
mod tail_source;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{measure, Metrics};
#[derive(Clone, Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 2048], usize),
    path: Option<([u8; 2048], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 2048], usize)>,
}
fn text(value: &str) -> ([u8; 2048], usize) {
    let mut bytes = [0; 2048];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(error: &Diagnostic) -> Failure {
    Failure {
        code: error.code,
        severity: error.severity,
        message: text(&error.message),
        path: error.path.as_deref().map(text),
        span: error.span,
        help: error.help.as_deref().map(text),
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<(), Failure>,
    retry: Option<Failure>,
    remaining: usize,
    skeleton_spent: usize,
}
type Observed = ((Outcome, Metrics), bool, usize, Option<ReservationRefusal>);
fn observe(
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    old: bool,
    physical: usize,
    floor: usize,
    comparison_work: usize,
    mutation: usize,
) -> Observed {
    frontiers::prepare();
    with_limit_usage_refusal(physical, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            assert!(comparison_work <= MAX_REPLAY_WORK_UNITS);
            budget.remaining = comparison_work; // initial test allowance, before any work/debit
            assert!(crate::bounded_output::set_active_floor(floor));
            frontiers::reset();
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    PRODUCE_CALL_CONTROLS,
                    "retained test producer call"
                )?;
                let mut actual = produce(function, &mut budget)?;
                match mutation {
                    1 => actual.paths.reverse(),
                    2 => {
                        actual.paths[0].history.previous = Some(actual.arena.len());
                    }
                    3 => {
                        if let Some(segment) = actual.arena.first_mut() {
                            segment.depth = 0;
                        }
                    }
                    _ => {}
                }
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "retained test comparison call"
                )?;
                if old {
                    before_comparison::compare(function, &mut budget, actual, expected)
                } else {
                    compare(function, &mut budget, actual, expected)
                }
            })();
            let saved = result.as_ref().map(|_| ()).map_err(failure);
            let retry = if result.is_err()
                && (budget.refusal_spent || crate::bounded_output::active_refused())
            {
                crate::bounded_output::clear_active_floor();
                let before = crate::bounded_output::active_remaining();
                let remaining = budget.remaining;
                let allocation = crate::test_allocation_observer::current_metrics();
                let retry = budget
                    .reserve_bytes(function, 0, "retained comparator sticky zero")
                    .unwrap_err();
                assert_eq!(crate::bounded_output::active_remaining(), before);
                assert_eq!(budget.remaining, remaining);
                assert_eq!(
                    crate::test_allocation_observer::current_metrics(),
                    allocation
                );
                let empty = Diagnostic::io("SPX-H006", String::new());
                assert_eq!(failure(&retry), failure(&empty));
                assert_eq!(result.as_ref().map(|_| ()).map_err(failure), saved);
                Some(retry)
            } else {
                None
            };
            let outcome = Outcome {
                result: saved,
                retry: retry.as_ref().map(failure),
                remaining: budget.remaining,
                skeleton_spent: budget.skeleton_spent,
            };
            crate::bounded_output::clear_active_floor();
            drop(budget);
            drop(result);
            drop(retry);
            outcome
        })
    })
}
// Independent primitive oracle: reproduce the ACTUALLY OBSERVED ledger prefix,
// then ask the unchanged real reserve primitive for its first full Diagnostic.
// This test state replication is never production credit or a guessed bill.
fn primitive_failure(
    function: &ResolvedFunction,
    event: frontiers::Event,
    limit: usize,
    floor: usize,
) -> Failure {
    let ((saved, metrics), refused, _, receipt) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.prepare_builder(function).unwrap();
            let used = limit - crate::bounded_output::active_remaining().unwrap();
            let prefix = event
                .before
                .checked_sub(used)
                .expect("observed prefix includes builder bootstrap");
            budget
                .reserve_bytes(function, prefix, "independent observed prefix")
                .unwrap();
            assert_eq!(
                limit - crate::bounded_output::active_remaining().unwrap(),
                event.before
            );
            assert!(crate::bounded_output::set_active_floor(floor));
            let error = budget
                .reserve_bytes(function, event.after.unwrap() - event.before, event.phase)
                .unwrap_err();
            let saved = failure(&error);
            crate::bounded_output::clear_active_floor();
            drop(budget);
            drop(error);
            saved
        })
    });
    assert!(refused);
    let receipt = receipt.unwrap();
    assert_eq!(receipt.requested, event.after.unwrap() - event.before);
    assert_eq!(receipt.remaining, limit - event.before);
    assert_eq!(receipt.floor, floor);
    balanced(metrics);
    saved
}
fn balanced(metrics: Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
fn expected(program: &ResolvedProgram, function: &ResolvedFunction) -> Vec<SkeletonPath> {
    let mut budget = ReplayBudget::unobserved();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    let mut paths = before::produce(function, &mut budget).unwrap();
    paths.sort();
    paths
}
#[test]
fn retained_comparator_matches_complete_immediate_work_cuts_sort_and_six_field_failures() {
    let program = super::super::tests::program();
    for function in &program.functions {
        let expected = expected(&program, function);
        for mutation in 0..4 {
            let ((a, ma), _, _, _) = observe(
                function,
                &expected,
                true,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                mutation,
            );
            let old_swaps = frontiers::finish().last().map_or(0, |event| event.swaps);
            let ((b, mb), _, _, _) = observe(
                function,
                &expected,
                false,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                mutation,
            );
            let new_swaps = frontiers::finish().last().map_or(0, |event| event.swaps);
            assert_eq!(a, b, "complete output/work/status mutation{mutation}");
            assert_eq!(old_swaps, new_swaps);
            eprintln!("OPT880 mutation{mutation} old paid-frame result={:?} allocated={} peak={} current result={:?} allocated={} peak={}", a.result, ma.allocated, ma.highwater, b.result, mb.allocated, mb.highwater);
            balanced(ma);
            balanced(mb);
            let spent = MAX_REPLAY_WORK_UNITS - a.remaining;
            for cut in 0..=spent {
                let ((a, ma), _, _, _) =
                    observe(function, &expected, true, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                let ((b, mb), _, _, _) =
                    observe(function, &expected, false, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                assert_eq!(
                    a, b,
                    "exact original comparison work cut{cut} mutation{mutation}"
                );
                balanced(ma);
                balanced(mb);
            }
        }
        for mutation in 0..4 {
            let mut changed = expected.clone();
            match mutation {
                0 => {
                    changed.pop();
                }
                1 => {
                    if let Some(path) = changed.last().cloned() {
                        changed.push(path);
                    }
                }
                2 => {
                    if let Some(path) = changed.last_mut() {
                        path.terminal = SkeletonTerminal::Failure;
                    }
                }
                _ => {
                    if let Some(path) = changed.last_mut() {
                        path.observations = Vec::new().into();
                    }
                }
            }
            changed.sort();
            let ((a, ma), _, _, _) = observe(
                function,
                &changed,
                true,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            let ((b, mb), _, _, _) = observe(
                function,
                &changed,
                false,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            assert_eq!(a, b, "changed expected paths{mutation}");
            balanced(ma);
            balanced(mb);
        }
    }
}
#[test]
fn retained_comparator_actual_owner_exact_short_zero_floor_sticky_and_full_drop() {
    let program = super::super::tests::program();
    let function = &program.functions[0];
    let expected = expected(&program, function);
    let ((success, metrics), refused, paid, first) = observe(
        function,
        &expected,
        false,
        usize::MAX,
        0,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(success.result.is_ok() && !refused && first.is_none());
    balanced(metrics);
    assert!(paid >= metrics.allocated);
    let events = frontiers::finish();
    let owner = events
        .iter()
        .position(|e| e.phase == "ordinary plan retained comparison owner")
        .unwrap();
    assert!(events[owner].after.unwrap() > events[owner].before);
    let mut selected = vec![owner];
    for phase in [
        "ordinary plan history left cursor prepare",
        "ordinary plan history right cursor prepare",
        "ordinary plan history cursor backing",
        "ordinary plan history paired cursor next",
        "ordinary plan history HIR cursor prepare",
        "ordinary plan history HIR cursor next",
        "ordinary plan retained final Zip next receivers",
        "ordinary plan retained drain swap receivers",
        "ordinary plan retained sift swap receivers",
    ] {
        let hits = events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.phase == phase && e.after.is_some_and(|after| after > e.before))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert!(!hits.is_empty(), "actual frontier{phase}");
        selected.push(hits[0]);
        selected.push(*hits.last().unwrap());
    }
    selected.sort_unstable();
    selected.dedup();
    for ordinal in selected {
        let event = events[ordinal];
        let end = event.after.unwrap();
        for (limit, floor) in [
            (0, 0),
            (event.before, 0),
            (end - 1, 0),
            (end, 0),
            (end + 1, 1),
            (end, 1),
        ] {
            let a = observe(
                function,
                &expected,
                false,
                limit,
                floor,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            let trace = frontiers::finish();
            let b = observe(
                function,
                &expected,
                false,
                limit,
                floor,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            assert_eq!(a.0 .0, b.0 .0);
            assert_eq!(a.1, b.1);
            assert_eq!(a.2, b.2);
            assert_eq!(a.3, b.3);
            balanced(a.0 .1);
            balanced(b.0 .1);
            if limit == end && floor == 0 {
                assert_eq!(trace[ordinal].after, Some(end));
            }
            if (limit == end - 1 && floor == 0)
                || (limit == end && floor == 1)
                || (limit == event.before && floor == 0)
            {
                assert!(a.0 .0.result.is_err() && a.1 && a.3.is_some());
                let first = a.3.unwrap();
                assert_eq!(first.requested, end - event.before);
                assert_eq!(first.remaining, limit - event.before);
                assert_eq!(first.floor, floor);
                assert_eq!(
                    a.0 .0.result.as_ref().unwrap_err(),
                    &primitive_failure(function, event, limit, floor)
                );
            }
        }
    }
}

#[test]
fn complete_immediate_comparator_tariffs_and_cursor_bodies_remain_independent_and_exact() {
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/opt880-comparison-owner-v1/src__cleanup_plan__replay__ordinary_plan_history.rs.before.txt"));
    let after = source_format::restore(
        source_format::Kind::OldCompare,
        include_str!("../before_comparison.rs"),
    );
    let body = before
        .split_once("const COMPARE_CONTROLS:")
        .unwrap()
        .1
        .split_once("\n#[cfg(test)]\nmod before_hir_compare;")
        .unwrap()
        .0;
    assert_eq!(after, format!("//! Complete immediate comparator and original tariffs; no candidate calls.\nuse super::*;\n\nconst COMPARE_CONTROLS:{body}\n"));
    let cursor = before
        .split_once("struct Cursor {")
        .unwrap()
        .1
        .split_once("\nconst COMPARE_CONTROLS:")
        .unwrap()
        .0;
    let current = source_format::restore(
        source_format::Kind::Root,
        include_str!("../../ordinary_plan_history.rs"),
    );
    assert_eq!(
        current
            .matches(&format!("struct Cursor {{{cursor}"))
            .count(),
        1
    );
    let actual = include_str!("../comparison_owner.rs");
    assert!(!actual.contains("paid_order!"));
    assert!(!actual.contains("paid_sift!"));
    assert!(!actual.contains("fn order("));
    assert!(!actual.contains("fn sift("));
}

// OPT883 adds complete immediate880 controls; all preceding owners remain exact.
fn observe_883(
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    old: bool,
    physical: usize,
    floor: usize,
    comparison_work: usize,
    mutation: usize,
) -> Observed {
    frontiers::prepare();
    with_limit_usage_refusal(physical, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            assert!(comparison_work <= MAX_REPLAY_WORK_UNITS);
            budget.remaining = comparison_work; // initial test allowance, before any work/debit
            assert!(crate::bounded_output::set_active_floor(floor));
            frontiers::reset();
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    PRODUCE_CALL_CONTROLS,
                    "retained test producer call"
                )?;
                let mut actual = produce(function, &mut budget)?;
                match mutation {
                    1 => actual.paths.reverse(),
                    2 => {
                        actual.paths[0].history.previous = Some(actual.arena.len());
                    }
                    3 => {
                        if let Some(segment) = actual.arena.first_mut() {
                            segment.depth = 0;
                        }
                    }
                    _ => {}
                }
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "retained test comparison call"
                )?;
                if old {
                    before_lexical_cursor::compare(function, &mut budget, actual, expected)
                } else {
                    compare(function, &mut budget, actual, expected)
                }
            })();
            let saved = result.as_ref().map(|_| ()).map_err(failure);
            let retry = if result.is_err()
                && (budget.refusal_spent || crate::bounded_output::active_refused())
            {
                crate::bounded_output::clear_active_floor();
                let before = crate::bounded_output::active_remaining();
                let remaining = budget.remaining;
                let allocation = crate::test_allocation_observer::current_metrics();
                let retry = budget
                    .reserve_bytes(function, 0, "retained comparator sticky zero")
                    .unwrap_err();
                assert_eq!(crate::bounded_output::active_remaining(), before);
                assert_eq!(budget.remaining, remaining);
                assert_eq!(
                    crate::test_allocation_observer::current_metrics(),
                    allocation
                );
                let empty = Diagnostic::io("SPX-H006", String::new());
                assert_eq!(failure(&retry), failure(&empty));
                assert_eq!(result.as_ref().map(|_| ()).map_err(failure), saved);
                Some(retry)
            } else {
                None
            };
            let outcome = Outcome {
                result: saved,
                retry: retry.as_ref().map(failure),
                remaining: budget.remaining,
                skeleton_spent: budget.skeleton_spent,
            };
            crate::bounded_output::clear_active_floor();
            drop(budget);
            drop(result);
            drop(retry);
            outcome
        })
    })
}

#[test]
fn lexical_plan_cursor_preserves_complete_immediate_sort_work_cuts_and_six_field_failures() {
    let program = super::super::tests::program();
    for function in &program.functions {
        let expected = expected(&program, function);
        for mutation in 0..4 {
            let ((a, ma), _, _, _) = observe_883(
                function,
                &expected,
                true,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                mutation,
            );
            let old_swaps = frontiers::finish().last().map_or(0, |event| event.swaps);
            let ((b, mb), _, _, _) = observe_883(
                function,
                &expected,
                false,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                mutation,
            );
            let new_swaps = frontiers::finish().last().map_or(0, |event| event.swaps);
            assert_eq!(a, b, "complete output/work/status mutation{mutation}");
            assert_eq!(old_swaps, new_swaps);
            eprintln!("OPT883 mutation{mutation} old paid-frame result={:?} allocated={} peak={} current result={:?} allocated={} peak={}", a.result, ma.allocated, ma.highwater, b.result, mb.allocated, mb.highwater);
            balanced(ma);
            balanced(mb);
            let spent = MAX_REPLAY_WORK_UNITS - a.remaining;
            for cut in 0..=spent {
                let ((a, ma), _, _, _) =
                    observe_883(function, &expected, true, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                let ((b, mb), _, _, _) =
                    observe_883(function, &expected, false, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                assert_eq!(
                    a, b,
                    "exact original comparison work cut{cut} mutation{mutation}"
                );
                balanced(ma);
                balanced(mb);
            }
        }
        for mutation in 0..4 {
            let mut changed = expected.clone();
            match mutation {
                0 => {
                    changed.pop();
                }
                1 => {
                    if let Some(path) = changed.last().cloned() {
                        changed.push(path);
                    }
                }
                2 => {
                    if let Some(path) = changed.last_mut() {
                        path.terminal = SkeletonTerminal::Failure;
                    }
                }
                _ => {
                    if let Some(path) = changed.last_mut() {
                        path.observations = Vec::new().into();
                    }
                }
            }
            changed.sort();
            let ((a, ma), _, _, _) = observe_883(
                function,
                &changed,
                true,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            let ((b, mb), _, _, _) = observe_883(
                function,
                &changed,
                false,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            assert_eq!(a, b, "changed expected paths{mutation}");
            balanced(ma);
            balanced(mb);
        }
    }
}

#[test]
fn lexical_plan_cursor_real_payload_frontiers_exact_short_zero_floor_sticky_and_full_drop() {
    let program = super::super::tests::program();
    let function = &program.functions[0];
    let expected = expected(&program, function);
    let ((success, metrics), refused, paid, first) = observe_883(
        function,
        &expected,
        false,
        usize::MAX,
        0,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(success.result.is_ok() && !refused && first.is_none());
    balanced(metrics);
    assert!(paid >= metrics.allocated);
    let events = frontiers::finish();
    let owner = events
        .iter()
        .position(|e| e.phase == "ordinary plan retained comparison owner")
        .unwrap();
    assert!(events[owner].after.unwrap() > events[owner].before);
    let mut selected = vec![owner];
    for phase in [
        "ordinary plan history left cursor prepare",
        "ordinary plan history right cursor prepare",
        "ordinary plan history cursor backing",
        "ordinary plan history paired cursor next",
        "ordinary plan history HIR cursor prepare",
        "ordinary plan history HIR cursor next",
        "ordinary plan retained final Zip next receivers",
        "ordinary plan retained drain swap receivers",
        "ordinary plan retained sift swap receivers",
    ] {
        let hits = events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.phase == phase && e.after.is_some_and(|after| after > e.before))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert!(!hits.is_empty(), "actual frontier{phase}");
        selected.push(hits[0]);
        selected.push(*hits.last().unwrap());
    }
    selected.sort_unstable();
    selected.dedup();
    for ordinal in selected {
        let event = events[ordinal];
        let end = event.after.unwrap();
        for (limit, floor) in [
            (0, 0),
            (event.before, 0),
            (end - 1, 0),
            (end, 0),
            (end + 1, 1),
            (end, 1),
        ] {
            let a = observe_883(
                function,
                &expected,
                false,
                limit,
                floor,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            let trace = frontiers::finish();
            let b = observe_883(
                function,
                &expected,
                false,
                limit,
                floor,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            assert_eq!(a.0 .0, b.0 .0);
            assert_eq!(a.1, b.1);
            assert_eq!(a.2, b.2);
            assert_eq!(a.3, b.3);
            balanced(a.0 .1);
            balanced(b.0 .1);
            if limit == end && floor == 0 {
                assert_eq!(trace[ordinal].after, Some(end));
            }
            if (limit == end - 1 && floor == 0)
                || (limit == end && floor == 1)
                || (limit == event.before && floor == 0)
            {
                assert!(a.0 .0.result.is_err() && a.1 && a.3.is_some());
                let first = a.3.unwrap();
                assert_eq!(first.requested, end - event.before);
                assert_eq!(first.remaining, limit - event.before);
                assert_eq!(first.floor, floor);
                assert_eq!(
                    a.0 .0.result.as_ref().unwrap_err(),
                    &primitive_failure(function, event, limit, floor)
                );
            }
        }
    }
}

#[test]
fn lexical_plan_cursor_reports_complete_immediate_actual_paid_work_and_drop() {
    let program = super::super::tests::program();
    for function in &program.functions {
        let expected = expected(&program, function);
        for old in [true, false] {
            let ((outcome, heap), refused, paid, receipt) = observe_883(
                function,
                &expected,
                old,
                usize::MAX,
                0,
                MAX_REPLAY_WORK_UNITS,
                0,
            );
            frontiers::finish();
            assert!(outcome.result.is_ok() && !refused && receipt.is_none());
            assert!(paid >= heap.allocated);
            balanced(heap);
            eprintln!("OPT883 immediate880={old} function={} paid={paid} ordinary_work={} skeleton_spent={} allocated={} peak={} freed={} live={}",
                function.id, MAX_REPLAY_WORK_UNITS-outcome.remaining, outcome.skeleton_spent,
                heap.allocated, heap.highwater, heap.freed, heap.live);
        }
    }
}

#[test]
fn lexical_plan_cursor_whole_immediate_methods_prices_and_original_owners_are_exact() {
    let root = source_format::restore(
        source_format::Kind::Root,
        include_str!("../../ordinary_plan_history.rs"),
    );
    let old_root = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/opt883-lexical-plan-cursor-20261017/src__cleanup_plan__replay__ordinary_plan_history.rs.before.txt"));
    assert_eq!(
        root, old_root,
        "original Cursor and every root caller/control unchanged"
    );
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/opt883-lexical-plan-cursor-20261017/src__cleanup_plan__replay__ordinary_plan_history__comparison_owner.rs.before.txt"));
    let cursor = root
        .split_once("struct Cursor {")
        .unwrap()
        .1
        .split_once("\n#[cfg(test)]\nmod before_comparison;")
        .unwrap()
        .0;
    let before = source_format::restore(
        source_format::Kind::BeforeLexical,
        include_str!("before_lexical_cursor.rs"),
    );
    let body = old.strip_prefix("//! One comparison invocation owns sort and pair state. No nested order/sift calls.\nuse super::*;\n").unwrap().strip_suffix("\n#[cfg(test)]\nmod tests;\n").unwrap();
    assert_eq!(before, format!("//! Complete immediate880 comparator and Cursor; namespace-only test module.\nuse super::*;\n\nstruct Cursor {{{cursor}\n{body}"));
    let current = include_str!("../comparison_owner.rs");
    assert_eq!(current.matches("lexical_plan_next!(").count(), 4);
    assert!(!current.contains(".cursors[0].next("));
    assert!(!current.contains(".cursors[1].next("));
    let original_tests = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/opt883-lexical-plan-cursor-20261017/src__cleanup_plan__replay__ordinary_plan_history__comparison_owner__tests.rs.before.txt"));
    assert!(
        source_format::restore(source_format::Kind::Tests880, include_str!("tests.rs"))
            .starts_with(original_tests),
        "every prior test is retained verbatim"
    );
}

// These alternative immutable inputs are constructed outside the comparison
// boundary, identically for both arms. Original producer+comparison measured
// owners above still cover all setup and final Drop. This helper lends no proof.
fn shaped_883(paths: &[SkeletonPath], empty: usize, tail: bool, malformed: usize) -> HistorySet {
    let mut arena = Vec::new();
    let mut actual = Vec::new();
    let mut max_depth = 0;
    for path in paths {
        let mut previous = None;
        let mut depth = 0;
        for _ in 0..empty {
            depth += 1;
            let index = arena.len();
            arena.push(Segment {
                previous,
                items: Vec::new(),
                depth,
            });
            previous = Some(index);
        }
        let split = if tail {
            path.observations.items.len().saturating_sub(1)
        } else {
            path.observations.items.len()
        };
        for observation in &path.observations.items[..split] {
            depth += 1;
            let index = arena.len();
            arena.push(Segment {
                previous,
                items: vec![Rc::clone(observation)],
                depth,
            });
            previous = Some(index);
        }
        max_depth = max_depth.max(depth);
        actual.push(PlanPath {
            history: History {
                previous,
                tail: path.observations.items[split..].to_vec(),
            },
            terminal: path.terminal,
        });
    }
    actual.reverse();
    if malformed == 1 {
        actual[0].history.previous = Some(arena.len());
    }
    if malformed == 2 {
        arena[0].depth = 0;
    }
    HistorySet {
        arena,
        paths: actual,
        max_depth,
    }
}
fn observe_shaped_883(
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    actual: HistorySet,
    old: bool,
    units: usize,
) -> (Outcome, Metrics, usize) {
    let ((outcome, metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.remaining = units;
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "883 shaped comparison receiving"
                )?;
                if old {
                    before_lexical_cursor::compare(function, &mut budget, actual, expected)
                } else {
                    compare(function, &mut budget, actual, expected)
                }
            })();
            let result = result.as_ref().map(|_| ()).map_err(failure);
            let outcome = Outcome {
                result,
                retry: None,
                remaining: budget.remaining,
                skeleton_spent: budget.skeleton_spent,
            };
            drop(budget);
            outcome
        })
    });
    assert!(
        !refused && first.is_none(),
        "this comparison is byte-unlimited, including work errors"
    );
    balanced(metrics);
    assert!(paid >= metrics.allocated);
    (outcome, metrics, paid)
}
#[test]
fn lexical_plan_cursor_immutable_hir_deep_empty_tail_and_malformed_composition_keeps_old_status() {
    let program = super::super::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let flat = expected(&program, function);
    let mut shared = flat.clone();
    for path in &mut shared {
        path.observations.prefix = path.observations.freeze();
        path.observations.items.clear();
    }
    assert!(shared.iter().any(|path| path.observations.prefix.is_some()));
    for expected in [&flat, &shared] {
        for depth in [0, 32] {
            for tail in [false, true] {
                for malformed in 0..3 {
                    // Flat source values build PLAN; expected can be a distinct
                    // immutable HIR sequence with equal contents and identities.
                    let (old, _, old_paid) = observe_shaped_883(
                        function,
                        expected,
                        shaped_883(&flat, depth, tail, malformed),
                        true,
                        MAX_REPLAY_WORK_UNITS,
                    );
                    let (new, _, new_paid) = observe_shaped_883(
                        function,
                        expected,
                        shaped_883(&flat, depth, tail, malformed),
                        false,
                        MAX_REPLAY_WORK_UNITS,
                    );
                    assert_eq!(
                        new, old,
                        "deep/empty/tail/malformed complete six-field comparison"
                    );
                    assert_eq!(new.result.is_ok(), malformed == 0);
                    let spent = MAX_REPLAY_WORK_UNITS - old.remaining;
                    for cut in [0, 1, spent.saturating_sub(1), spent] {
                        let (old, _, _) = observe_shaped_883(
                            function,
                            expected,
                            shaped_883(&flat, depth, tail, malformed),
                            true,
                            cut,
                        );
                        let (new, _, _) = observe_shaped_883(
                            function,
                            expected,
                            shaped_883(&flat, depth, tail, malformed),
                            false,
                            cut,
                        );
                        assert_eq!(
                            new, old,
                            "actual original forward/segment and HIR work priority"
                        );
                    }
                    eprintln!("OPT883 shaped shared={} depth={depth} tail={tail} malformed={malformed} old_paid={old_paid} current_paid={new_paid}; input construction excluded, complete comparison and receiver Drop included", expected[0].observations.prefix.is_some());
                }
            }
        }
    }
}
#[test]
fn lexical_plan_cursor_typed_oracle_missing_foreign_and_restored_scope_keeps_ambient_unspent() {
    let program = super::super::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let foreign = function.clone();
    let expected = expected(&program, function);
    for state in 0..3 {
        let run = |old: bool| {
            let actual = shaped_883(&expected, 4, true, 0);
            let (status, refused, active_paid, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    let scope =
                        allocation::OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
                    let invocation = match state {
                        0 => Some(allocation::enter(&program, function, scope.lease()).unwrap()),
                        1 => None,
                        _ => Some(allocation::enter(&program, &foreign, scope.lease()).unwrap()),
                    };
                    let mut budget = ReplayBudget::with_skeleton_limit_oracle(
                        MAX_REPLAY_WORK_UNITS,
                        scope.lease(),
                    );
                    let before = crate::bounded_output::active_remaining();
                    // The real caller authenticates typed Oracle authority before
                    // entering either private comparator. Neither comparator mints
                    // or rechecks a program/function lease by itself.
                    let result =
                        match SkeletonWork::with_index(&program, function, &mut budget, None) {
                            Err(error) => {
                                drop(actual);
                                Err(error)
                            }
                            Ok(work) => {
                                drop(work);
                                match budget.reserve_bytes(
                                    function,
                                    COMPARE_CALL_CONTROLS,
                                    "883 typed oracle comparison call",
                                ) {
                                    Err(error) => {
                                        drop(actual);
                                        Err(error)
                                    }
                                    Ok(()) => {
                                        if old {
                                            before_lexical_cursor::compare(
                                                function,
                                                &mut budget,
                                                actual,
                                                &expected,
                                            )
                                        } else {
                                            compare(function, &mut budget, actual, &expected)
                                        }
                                    }
                                }
                            }
                        };
                    let status = result.as_ref().map(|_| ()).map_err(failure);
                    let work = (
                        budget.remaining,
                        budget.skeleton_remaining,
                        budget.skeleton_spent,
                    );
                    let receipt = scope.receipt();
                    if state == 0 {
                        assert!(status.is_ok());
                    } else {
                        assert_eq!(result, Err(Diagnostic::io("SPX-H006", String::new())));
                        assert_eq!((receipt.charged, receipt.work), (0, 0));
                        assert!(receipt.physical_failure.is_some());
                        let error = budget
                            .reserve_bytes(function, 0, "883 foreign oracle sticky")
                            .unwrap_err();
                        assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
                        assert_eq!((scope.receipt().charged, scope.receipt().work), (0, 0));
                        drop(invocation);
                        let restored =
                            allocation::enter(&program, function, scope.lease()).unwrap();
                        let retry = budget
                            .reserve_bytes(function, 0, "883 restored oracle sticky")
                            .unwrap_err();
                        assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                        assert_eq!((scope.receipt().charged, scope.receipt().work), (0, 0));
                        drop(restored);
                    }
                    assert_eq!(crate::bounded_output::active_remaining(), before);
                    drop(result);
                    drop(budget);
                    (status, work, receipt.work)
                });
            assert!(!refused && first.is_none());
            assert_eq!(
                active_paid, 0,
                "explicit Oracle never falls back to ambient payer"
            );
            status
        };
        assert_eq!(
            run(false),
            run(true),
            "typed same fixture, all six fields and exact real work"
        );
    }
}

#[test]
fn retained_arena_slice_wide_pair_measures_real_shipping880_paid_bytes_without_old_padding() {
    let program = super::super::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    // Comparator inputs are constructed before its measured boundary. These
    // are private complete-value controls, not a fabricated admitted program.
    let items = (0..256)
        .map(|_| {
            Rc::new(SkeletonObservation::Boolean {
                expression: function.body.id.clone(),
                value: true,
            })
        })
        .collect::<Vec<_>>();
    let expected = vec![
        SkeletonPath {
            observations: items.clone().into(),
            terminal: SkeletonTerminal::Success
        };
        2
    ];
    let actual = || HistorySet {
        arena: vec![Segment {
            previous: None,
            items: items.clone(),
            depth: 1,
        }],
        paths: (0..2)
            .map(|_| PlanPath {
                history: History {
                    previous: Some(0),
                    tail: Vec::new(),
                },
                terminal: SkeletonTerminal::Success,
            })
            .collect(),
        max_depth: 1,
    };
    let (old, old_heap, old_paid) =
        observe_shaped_883(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);
    let (current, heap, paid) =
        observe_shaped_883(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);
    assert_eq!(
        current, old,
        "full result and all original work; real immediate shipping baseline"
    );
    assert!(current.result.is_ok());
    assert!(
        paid < old_paid,
        "only genuinely removed recurring receivers may earn a saving"
    );
    assert_eq!(
        heap.allocated, old_heap.allocated,
        "the retained slice view adds no heap backing"
    );
    eprintln!("OPT883 retained-arena two equal256-observation paths old_shipping880_paid={old_paid} current_paid={paid} old_heap={old_heap:?} current_heap={heap:?}; comparison-only, setup excluded; no application fit claim");
}

#[path = "pair_scope_tests.rs"]
mod pair_scope_tests;
