//! Authored current/complete-immediate comparator owners; execution pending.
use super::*;
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
    Failure { code: error.code, severity: error.severity, message: text(&error.message),
        path: error.path.as_deref().map(text), span: error.span, help: error.help.as_deref().map(text) }
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<(), Failure>,
    retry: Option<Failure>,
    remaining: usize,
    skeleton_spent: usize,
}
type Observed = ((Outcome, Metrics), bool, usize, Option<ReservationRefusal>);
fn observe(function: &ResolvedFunction, expected: &[SkeletonPath], old: bool,
    physical: usize, floor: usize, comparison_work: usize, mutation: usize) -> Observed {
    with_limit_usage_refusal(physical, || measure(|| {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        assert!(comparison_work <= MAX_REPLAY_WORK_UNITS);
        budget.remaining = comparison_work; // initial test allowance, before any work/debit
        assert!(crate::bounded_output::set_active_floor(floor));
        frontiers::reset();
        let result = (|| {
            paid!(&mut budget, function, PRODUCE_CALL_CONTROLS, "retained test producer call")?;
            let mut actual = produce(function, &mut budget)?;
            match mutation {
                1 => actual.paths.reverse(),
                2 => { actual.paths[0].history.previous = Some(actual.arena.len()); },
                3 => { if let Some(segment) = actual.arena.first_mut() { segment.depth = 0; } },
                _ => {}
            }
            paid!(&mut budget, function, COMPARE_CALL_CONTROLS, "retained test comparison call")?;
            if old { before_comparison::compare(function, &mut budget, actual, expected) }
            else { compare(function, &mut budget, actual, expected) }
        })();
        let saved = result.as_ref().map(|_| ()).map_err(failure);
        let retry = if result.is_err() && (budget.refusal_spent || crate::bounded_output::active_refused()) {
            crate::bounded_output::clear_active_floor();
            let before = crate::bounded_output::active_remaining();
            let remaining = budget.remaining;
            let retry = budget.reserve_bytes(function, 0, "retained comparator sticky zero").unwrap_err();
            assert_eq!(crate::bounded_output::active_remaining(), before);
            assert_eq!(budget.remaining, remaining);
            let empty = Diagnostic::io("SPX-H006", String::new());
            assert_eq!(failure(&retry), failure(&empty));
            assert_eq!(result.as_ref().map(|_| ()).map_err(failure), saved);
            Some(retry)
        } else { None };
        let outcome = Outcome { result: saved, retry: retry.as_ref().map(failure),
            remaining: budget.remaining, skeleton_spent: budget.skeleton_spent };
        crate::bounded_output::clear_active_floor();
        drop(budget);
        drop(result);
        drop(retry);
        outcome
    }))
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
            let ((a, ma), _, _, _) = observe(function, &expected, true, usize::MAX, 0, MAX_REPLAY_WORK_UNITS, mutation);
            let old_swaps = frontiers::finish().last().map_or(0, |event| event.swaps);
            let ((b, mb), _, _, _) = observe(function, &expected, false, usize::MAX, 0, MAX_REPLAY_WORK_UNITS, mutation);
            let new_swaps = frontiers::finish().last().map_or(0, |event| event.swaps);
            assert_eq!(a, b, "complete output/work/status mutation{mutation}");
            assert_eq!(old_swaps, new_swaps);
            eprintln!("OPT880 mutation{mutation} old paid-frame result={:?} allocated={} peak={} current result={:?} allocated={} peak={}", a.result, ma.allocated, ma.highwater, b.result, mb.allocated, mb.highwater);
            balanced(ma); balanced(mb);
            let spent = MAX_REPLAY_WORK_UNITS - a.remaining;
            for cut in 0..=spent {
                let ((a, ma), _, _, _) = observe(function, &expected, true, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                let ((b, mb), _, _, _) = observe(function, &expected, false, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                assert_eq!(a, b, "exact original comparison work cut{cut} mutation{mutation}");
                balanced(ma); balanced(mb);
            }
        }
        for mutation in 0..4 {
            let mut changed = expected.clone();
            match mutation {
                0 => { changed.pop(); },
                1 => { if let Some(path) = changed.last().cloned() { changed.push(path); } },
                2 => { if let Some(path) = changed.last_mut() { path.terminal = SkeletonTerminal::Failure; } },
                _ => { if let Some(path) = changed.last_mut() { path.observations = Vec::new().into(); } },
            }
            changed.sort();
            let ((a, ma), _, _, _) = observe(function, &changed, true, usize::MAX, 0, MAX_REPLAY_WORK_UNITS, 0);
            frontiers::finish();
            let ((b, mb), _, _, _) = observe(function, &changed, false, usize::MAX, 0, MAX_REPLAY_WORK_UNITS, 0);
            frontiers::finish();
            assert_eq!(a, b, "changed expected paths{mutation}");
            balanced(ma); balanced(mb);
        }
    }
}
#[test]
fn retained_comparator_actual_owner_exact_short_zero_floor_sticky_and_full_drop() {
    let program = super::super::tests::program();
    let function = &program.functions[0];
    let expected = expected(&program, function);
    let ((success, metrics), refused, paid, first) = observe(function, &expected, false, usize::MAX, 0, MAX_REPLAY_WORK_UNITS, 0);
    assert!(success.result.is_ok() && !refused && first.is_none());
    balanced(metrics);
    assert!(paid >= metrics.allocated);
    let events = frontiers::finish();
    let owner = events.iter().position(|e| e.phase == "ordinary plan retained comparison owner").unwrap();
    assert!(events[owner].after.unwrap() > events[owner].before);
    let mut selected = vec![owner];
    for phase in ["ordinary plan history cursor backing", "ordinary plan history paired cursor next", "ordinary plan history HIR cursor prepare", "ordinary plan history HIR cursor next"] {
        let hits = events.iter().enumerate().filter(|(_,e)| e.phase==phase && e.after.is_some_and(|after| after > e.before)).map(|(i,_)| i).collect::<Vec<_>>();
        assert!(!hits.is_empty(), "actual frontier{phase}");
        selected.push(hits[0]); selected.push(*hits.last().unwrap());
    }
    selected.sort_unstable(); selected.dedup();
    for ordinal in selected {
        let event = events[ordinal]; let end = event.after.unwrap();
        for (limit, floor) in [(0,0),(event.before,0),(end-1,0),(end,0),(end+1,1),(end,1)] {
            let a = observe(function, &expected, false, limit, floor, MAX_REPLAY_WORK_UNITS, 0);
            let trace = frontiers::finish();
            let b = observe(function, &expected, false, limit, floor, MAX_REPLAY_WORK_UNITS, 0);
            frontiers::finish();
            assert_eq!(a.0.0, b.0.0); assert_eq!(a.1,b.1); assert_eq!(a.2,b.2); assert_eq!(a.3,b.3);
            balanced(a.0.1); balanced(b.0.1);
            if limit == end && floor == 0 { assert_eq!(trace[ordinal].after, Some(end)); }
            if (limit == end-1 && floor == 0) || (limit == end && floor == 1) || (limit == event.before && floor == 0) {
                assert!(a.0.0.result.is_err() && a.1 && a.3.is_some());
                let first = a.3.unwrap();
                assert_eq!(first.requested, end - event.before);
                assert_eq!(first.remaining, limit - event.before);
                assert_eq!(first.floor, floor);
            }
        }
    }
}

#[test]
fn complete_immediate_comparator_tariffs_and_cursor_bodies_remain_independent_and_exact() {
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/opt880-comparison-owner-v1/src__cleanup_plan__replay__ordinary_plan_history.rs.before.txt"));
    let after = include_str!("../before_comparison.rs");
    let body = before.split_once("const COMPARE_CONTROLS:").unwrap().1.split_once("\n#[cfg(test)]\nmod before_hir_compare;").unwrap().0;
    assert_eq!(after, format!("//! Complete immediate comparator and original tariffs; no candidate calls.\nuse super::*;\n\nconst COMPARE_CONTROLS:{body}\n"));
    let cursor = before.split_once("struct Cursor {").unwrap().1.split_once("\nconst COMPARE_CONTROLS:").unwrap().0;
    let current = include_str!("../../ordinary_plan_history.rs");
    assert_eq!(current.matches(&format!("struct Cursor {{{cursor}")).count(), 1);
    let actual = include_str!("../comparison_owner.rs");
    assert!(!actual.contains("paid_order!"));
    assert!(!actual.contains("paid_sift!"));
    assert!(!actual.contains("fn order("));
    assert!(!actual.contains("fn sift("));
}
