//! Actual production-reservation boundaries; fixed instrumentation grants no authority.
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{measure, Metrics};
#[derive(Clone, Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    let mut bytes = [0; 1024];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<(), Failure>,
    sticky_zero: Option<Failure>,
    spent: usize,
}
type Observed = ((Outcome, Metrics), bool, usize, Option<ReservationRefusal>);
fn observe(
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    limit: usize,
    floor: usize,
) -> Observed {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            assert!(crate::bounded_output::set_active_floor(floor));
            frontiers::reset();
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    PRODUCE_CALL_CONTROLS,
                    "frontier producer call"
                )?;
                let actual = produce(function, &mut budget)?;
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "frontier comparison call"
                )?;
                compare(function, &mut budget, actual, expected)
            })();
            let result = result.map_err(failure);
            let spent = budget.skeleton_spent;
            let sticky_zero = if result.is_err() {
                crate::bounded_output::clear_active_floor();
                let before = crate::bounded_output::active_remaining();
                let refused = budget
                    .reserve_bytes(function, 0, "frontier sticky zero")
                    .unwrap_err();
                assert_eq!(
                    crate::bounded_output::active_remaining(),
                    before,
                    "zero cannot refund or consume after first refusal"
                );
                Some(failure(refused))
            } else {
                None
            };
            crate::bounded_output::clear_active_floor();
            drop(budget);
            Outcome {
                result,
                sticky_zero,
                spent,
            }
        })
    })
}
fn flat(program: &ResolvedProgram, function: &ResolvedFunction) -> Vec<SkeletonPath> {
    let mut budget = ReplayBudget::unobserved();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    let mut flat = before::produce(function, &mut budget).unwrap();
    flat.sort();
    flat
}
fn balanced(allocation: Metrics) {
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

#[test]
fn each_new_frame_backing_and_post_swap_frontier_has_exact_short_zero_floor_and_drop_controls() {
    let program = super::tests::program();
    let function = &program.functions[0];
    let expected = flat(&program, function);
    let ((success, allocation), refused, _, first) = observe(function, &expected, usize::MAX, 0);
    assert!(success.result.is_ok() && !refused && first.is_none());
    balanced(allocation);
    let events = frontiers::finish();
    let phases = [
        "frontier producer call",
        "cleanup-plan root state push",
        "ordinary plan history queue receivers",
        "ordinary plan history path receivers",
        "plan success path push",
        "plan branch state push",
        "ordinary plan history comparator controls",
        "ordinary plan history comparison receivers",
        "ordinary plan history sort receivers",
        "ordinary plan history producer controls",
        "ordinary plan history branch controls",
        "ordinary plan history arena growth",
        "plan branch observation push",
        "frontier comparison call",
        "ordinary plan history cursor backing",
        "ordinary plan history sift call",
        "ordinary plan history order call",
        "ordinary plan history left cursor prepare",
        "ordinary plan history right cursor prepare",
        "ordinary plan history paired cursor next",
        "ordinary plan history HIR cursor prepare",
        "ordinary plan history HIR cursor next",
    ];
    let mut selected = Vec::new();
    for phase in phases {
        let matching = events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event.phase == phase && event.after.is_some_and(|after| after > event.before)
            })
            .collect::<Vec<_>>();
        assert!(!matching.is_empty(), "missing actual frontier {phase}");
        selected.push(matching[0].0);
        if phase == "ordinary plan history arena growth"
            || phase == "ordinary plan history cursor backing"
            || phase == "plan branch observation push"
        {
            assert!(
                matching.len() > 1,
                "first and later acquisition required for {phase}"
            );
            selected.push(matching.last().unwrap().0);
        }
    }
    let after_swap = events
        .iter()
        .enumerate()
        .find(|(_, event)| event.swaps > 0 && event.phase == "ordinary plan history order call")
        .expect("a later sort comparison follows a real array swap")
        .0;
    selected.push(after_swap);
    selected.sort_unstable();
    selected.dedup();
    for ordinal in selected {
        let event = events[ordinal];
        let end = event.after.unwrap();
        let ((_, allocation), _, _, _) = observe(function, &expected, end, 0);
        balanced(allocation);
        let exact = frontiers::finish();
        assert_eq!(exact[ordinal].phase, event.phase);
        assert_eq!(
            exact[ordinal].after,
            Some(end),
            "exact acquisition must complete at {}",
            event.phase
        );
        for (limit, floor) in [(end - 1, 0), (event.before, 0), (end, 1)] {
            let ((failed, allocation), refused, paid, first) =
                observe(function, &expected, limit, floor);
            balanced(allocation);
            assert!(
                refused
                    && first.is_some()
                    && failed.result.is_err()
                    && failed.sticky_zero.is_some()
            );
            let short = frontiers::finish();
            assert_eq!(short[ordinal].phase, event.phase);
            assert_eq!(
                short[ordinal].after, None,
                "refusal must precede formation at {}",
                event.phase
            );
            assert!(paid <= limit);
            if floor == 1 {
                assert_eq!(first.unwrap().floor, 1);
            }
            // Repeat the exact invocation: all six Diagnostic fields, old work,
            // complete first reservation receipt and sticky zero are stable.
            let ((again, allocation), again_refused, again_paid, again_first) =
                observe(function, &expected, limit, floor);
            balanced(allocation);
            frontiers::finish();
            assert_eq!(again, failed);
            assert_eq!(again_refused, refused);
            assert_eq!(again_paid, paid);
            assert_eq!(again_first, first);
        }
    }
}
