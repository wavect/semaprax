// Complete non-test fixture helpers retained for the executable main comparator.

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
