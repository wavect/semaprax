//! Pair-local successor: real complete producer+comparison boundaries.
use super::*;
use super::super::before_prepared_pairing::{compare, PAYMENT_HEADER, PATH_INDEX, PAIR_SCOPE, PAIR_PRIMITIVE};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};
// Source-type coefficients are independent of measured debit differences.
fn tail_receiver_prices() -> (usize, usize, usize) {
    let old = size_of::<&History>()
        + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
        + 3 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<usize>()
        + 3 * size_of::<Option<&Rc<SkeletonObservation>>>()
        + 2 * size_of::<&Option<&Rc<SkeletonObservation>>>()
        + size_of::<bool>()
        + size_of::<usize>()
        + size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<&SkeletonObservation>()
        + 2 * size_of::<Option<&SkeletonObservation>>();
    let current = size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<&SkeletonObservation>()
        + 2 * size_of::<Option<&SkeletonObservation>>()
        + size_of::<usize>();
    let view = 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>();
    (old, current, view)
}
// Independent source-type invoice for consuming prepared links. Every existing
// before/current physical, diagnostic, ownership and work-cut gate remains.
fn prepared_link_prices() -> (usize, usize, usize, usize) {
    let old_step = 2 * size_of::<&Vec<usize>>() + size_of::<usize>();
    let new_step = 2 * size_of::<usize>();
    let old_load = 2 * size_of::<&Vec<usize>>()
        + 2 * size_of::<&[usize]>()
        + 2 * size_of::<&[usize]>()
        + size_of::<usize>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<&[usize]>()
        + 2 * size_of::<usize>()
        + size_of::<&usize>()
        + size_of::<usize>();
    let new_load = 2 * size_of::<&mut Vec<usize>>()
        + size_of::<Option<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    let setup = 4 * size_of::<usize>(); // owner fields and initial scalar values
    let limit = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<usize>();
    (old_step - new_step, old_load - new_load, setup, limit)
}
fn observe_pair(
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    old: bool,
    physical: usize,
    floor: usize,
    comparison_work: usize,
    mutation: usize,
) -> Observed {
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
                    before_pair_scope::compare(function, &mut budget, actual, expected)
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

fn complete_row(old: bool) -> pair_scope_census::Row {
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    assert_eq!(row.old, old);
    assert!(row.complete && row.ended && !row.unwound && !row.truncated && !row.overflow);
    row
}
#[test]
fn pair_local_paths_match_whole_883_exact_work_heap_swaps_and_typed_role_equation() {
    let program = super::super::super::tests::program();
    for function in &program.functions {
        let expected = expected(&program, function);
        pair_scope_census::reset();
        let ((old, old_heap), old_refused, old_paid, old_receipt) = observe_pair(
            function,
            &expected,
            true,
            usize::MAX,
            0,
            MAX_REPLAY_WORK_UNITS,
            0,
        );
        let old_swaps = frontiers::finish().last().map_or(0, |e| e.swaps);
        let old_row = complete_row(true);
        pair_scope_census::reset();
        let ((current, heap), refused, paid, receipt) = observe_pair(
            function,
            &expected,
            false,
            usize::MAX,
            0,
            MAX_REPLAY_WORK_UNITS,
            0,
        );
        let swaps = frontiers::finish().last().map_or(0, |e| e.swaps);
        let row = complete_row(false);
        assert_eq!(current, old);
        assert!(
            current.result.is_ok()
                && !refused
                && !old_refused
                && receipt.is_none()
                && old_receipt.is_none()
        );
        assert_eq!(swaps, old_swaps);
        assert_eq!(
            heap, old_heap,
            "actual paths/arena/backing and full Drop are unchanged"
        );
        balanced(heap);
        balanced(old_heap);
        assert_eq!((row.pairs, row.rounds), (old_row.pairs, old_row.rounds));
        assert_eq!(row.function, old_row.function);
        assert_eq!(row.function_len, function.id.as_str().len());
        assert_eq!(
            (row.payer, row.selected, row.invocation),
            (old_row.payer, old_row.selected, old_row.invocation)
        );
        // Independent source-type coefficients, not measured debit subtraction.
        // A successful flat comparison reads every member and its None sentinel.
        let flat_paths = expected
            .iter()
            .filter(|p| p.observations.prefix.is_none())
            .count();
        let flat_rounds: usize = expected
            .iter()
            .filter(|p| p.observations.prefix.is_none())
            .map(|p| p.observations.items.len() + 1)
            .sum();
        let removed_view = 2 * size_of::<&Observations>() + size_of::<&[Rc<SkeletonObservation>]>();
        let retained_view = 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
            + 2 * size_of::<&[Rc<SkeletonObservation>]>();
        let (old_tail, tail, tail_view) = tail_receiver_prices();
        let (step, load, setup, limit) = prepared_link_prices();
        assert_eq!(
            paid + 4 * PATH_INDEX * row.rounds
                + removed_view * flat_rounds
                + (old_tail - tail) * row.tails
                + step * row.steps
                + load * row.loads,
            old_paid
                + PAIR_SCOPE * row.pairs
                + PAIR_PRIMITIVE
                + PAYMENT_HEADER
                + retained_view * flat_paths
                + tail_view * (2 * row.pairs + expected.len())
                + setup
                + limit * (2 * row.pairs + expected.len())
        );
        eprintln!("OPT886 function={} old883/current paid={old_paid}/{paid} work={} pairs={} rounds={} old_heap={old_heap:?} heap={heap:?}; unit-build diagnostic only, no shipping/application claim",
            function.id,MAX_REPLAY_WORK_UNITS-current.remaining,row.pairs,row.rounds);
        let spent = MAX_REPLAY_WORK_UNITS - current.remaining;
        for mutation in 0..4 {
            for cut in 0..=spent {
                let ((a, ma), _, _, _) =
                    observe_pair(function, &expected, true, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                let ((b, mb), _, _, _) =
                    observe_pair(function, &expected, false, usize::MAX, 0, cut, mutation);
                frontiers::finish();
                assert_eq!(
                    a, b,
                    "six-field Diagnostic/work cut{cut} mutation{mutation}"
                );
                balanced(ma);
                balanced(mb);
            }
        }
    }
}
#[test]
fn pair_local_paths_own_exact_short_zero_floor_sticky_and_full_drop_frontiers() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let expected = expected(&program, function);
    for old in [true, false] {
        let ((success, heap), refused, paid, receipt) = observe_pair(
            function,
            &expected,
            old,
            usize::MAX,
            0,
            MAX_REPLAY_WORK_UNITS,
            0,
        );
        assert!(success.result.is_ok() && !refused && receipt.is_none());
        balanced(heap);
        assert!(paid >= heap.allocated);
        let events = frontiers::finish();
        let phases = if old {
            vec![
                "ordinary plan retained comparison owner",
                "ordinary plan history paired cursor next",
            ]
        } else {
            vec![
                "ordinary plan retained comparison owner",
                "ordinary plan history pair-local paths",
                "ordinary plan history paired cursor next",
                "ordinary plan history flat HIR retained slice",
            ]
        };
        for phase in phases {
            let selected = events
                .iter()
                .enumerate()
                .filter(|(_, e)| e.phase == phase && e.after.is_some_and(|n| n > e.before))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            assert!(!selected.is_empty(), "real frontier {phase}");
            for ordinal in [selected[0], *selected.last().unwrap()] {
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
                    let a = observe_pair(
                        function,
                        &expected,
                        old,
                        limit,
                        floor,
                        MAX_REPLAY_WORK_UNITS,
                        0,
                    );
                    let trace = frontiers::finish();
                    let b = observe_pair(
                        function,
                        &expected,
                        old,
                        limit,
                        floor,
                        MAX_REPLAY_WORK_UNITS,
                        0,
                    );
                    frontiers::finish();
                    assert_eq!(a.0 .0, b.0 .0);
                    assert_eq!((a.1, a.2, a.3), (b.1, b.2, b.3));
                    balanced(a.0 .1);
                    balanced(b.0 .1);
                    if limit == end && floor == 0 {
                        assert_eq!(trace[ordinal].after, Some(end));
                    }
                    if (limit == end - 1 && floor == 0)
                        || (limit == end && floor == 1)
                        || (limit == event.before && floor == 0)
                    {
                        assert!(a.1 && a.0 .0.result.is_err());
                        let first = a.3.unwrap();
                        assert_eq!(
                            (first.requested, first.remaining, first.floor),
                            (end - event.before, limit - event.before, floor)
                        );
                        assert_eq!(
                            a.0 .0.result.as_ref().unwrap_err(),
                            &primitive_failure(function, event, limit, floor)
                        );
                    }
                }
            }
        }
    }
}

fn observe_shaped_pair(
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
                    before_pair_scope::compare(function, &mut budget, actual, expected)
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
fn pair_local_immutable_hir_deep_empty_tail_and_malformed_composition_keeps_old_status() {
    let program = super::super::super::tests::program();
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
                    let (old, _, old_paid) = observe_shaped_pair(
                        function,
                        expected,
                        shaped_883(&flat, depth, tail, malformed),
                        true,
                        MAX_REPLAY_WORK_UNITS,
                    );
                    let (new, _, new_paid) = observe_shaped_pair(
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
                        let (old, _, _) = observe_shaped_pair(
                            function,
                            expected,
                            shaped_883(&flat, depth, tail, malformed),
                            true,
                            cut,
                        );
                        let (new, _, _) = observe_shaped_pair(
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
fn pair_local_typed_oracle_missing_foreign_and_restored_scope_keeps_ambient_unspent() {
    let program = super::super::super::tests::program();
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
                                            before_pair_scope::compare(
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
fn pair_local_wide_equal_pair_has_real_typed_net_savings_against_complete_883() {
    let program = super::super::super::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
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
    pair_scope_census::reset();
    let (old, old_heap, old_paid) =
        observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);
    let old_row = complete_row(true);
    pair_scope_census::reset();
    let (current, heap, paid) =
        observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);
    let row = complete_row(false);
    assert_eq!(current, old);
    assert!(current.result.is_ok());
    assert_eq!(heap, old_heap);
    assert_eq!((row.pairs, row.rounds), (old_row.pairs, old_row.rounds));
    let removed_view = 2 * size_of::<&Observations>() + size_of::<&[Rc<SkeletonObservation>]>();
    let retained_view = 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>();
    let flat_rounds: usize = expected
        .iter()
        .map(|p| p.observations.items.len() + 1)
        .sum();
    let (old_tail, tail, tail_view) = tail_receiver_prices();
    let (step, load, setup, limit) = prepared_link_prices();
    let removed = 4 * PATH_INDEX * row.rounds
        + removed_view * flat_rounds
        + (old_tail - tail) * row.tails
        + step * row.steps
        + load * row.loads;
    let retained = PAIR_SCOPE * row.pairs
        + PAIR_PRIMITIVE
        + PAYMENT_HEADER
        + retained_view * expected.len()
        + tail_view * (2 * row.pairs + expected.len())
        + setup
        + limit * (2 * row.pairs + expected.len());
    assert_eq!(paid + removed, old_paid + retained);
    assert!(removed > retained);
    assert!(
        paid < old_paid,
        "actual complete883 receives its exact original tariff"
    );
    eprintln!("OPT886 comparison-only equal256 pair old883/current={old_paid}/{paid} P={} N={} old_heap={old_heap:?} current_heap={heap:?}; setup excluded, no application fit claim",row.pairs,row.rounds);
}
#[test]
fn pair_census_dropped_rows_and_original_unwind_never_claim_complete() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let budget = ReplayBudget::unobserved();
    pair_scope_census::reset();
    for _ in 0..129 {
        let guard = pair_scope_census::begin(function, &budget, false);
        guard.success();
    }
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 1);
    assert_eq!(rows.len(), 128);
    assert!(rows.iter().all(|r| !r.complete));
    pair_scope_census::reset();
    let original = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = pair_scope_census::begin(function, &budget, false);
        std::panic::panic_any(886_usize);
    }))
    .unwrap_err();
    assert_eq!(original.downcast_ref::<usize>(), Some(&886));
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].ended && rows[0].unwound && !rows[0].complete);
}
#[test]
fn pair_local_zero_pairs_report_real_invocation_tradeoff_without_old_padding() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let expected = Vec::new();
    let actual = || HistorySet {
        arena: Vec::new(),
        paths: Vec::new(),
        max_depth: 0,
    };
    pair_scope_census::reset();
    let (old, old_heap, old_paid) =
        observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);
    let old_row = complete_row(true);
    pair_scope_census::reset();
    let (current, heap, paid) =
        observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);
    let row = complete_row(false);
    assert_eq!(current, old);
    assert_eq!(heap, old_heap);
    assert!(current.result.is_ok());
    assert_eq!(
        (row.pairs, row.rounds, old_row.pairs, old_row.rounds),
        (0, 0, 0, 0)
    );
    assert_eq!(
        paid,
        old_paid + PAIR_PRIMITIVE + PAYMENT_HEADER + prepared_link_prices().2,
        "all retained primitive frames are funded even on this unused path"
    );
    eprintln!("OPT886 zero-pair tradeoff old883/current={old_paid}/{paid}; no universal paid saving claim");
}
#[test]
fn pair_local_whole_883_control_cursor_prices_and_prior_tests_remain_complete() {
    // The matched generation is the entire incoming producer, not a tariff shim.
    use sha2::{Digest, Sha256};
    let mut whole = include_str!("before_prepared_pairing.rs")
        .replacen("#[path = \"lexical_cursor.rs\"]\n", "", 1)
        .replacen("#[cfg(test)]\nuse super::pair_scope_census;", "#[cfg(test)]\nmod before_lexical_cursor;\n#[cfg(test)]\nmod before_pair_scope;\n#[cfg(test)]\nmod pair_scope_census;", 1);
    for name in [
        "PAYMENT_HEADER",
        "PATH_INDEX",
        "PAIR_SCOPE",
        "PAIR_PRIMITIVE",
    ] {
        whole = whole.replacen(
            &format!("pub(super) const {name}:"),
            &format!("const {name}:"),
            1,
        );
    }
    whole.push_str("\n#[cfg(test)]\nmod tests;\n");
    assert_eq!(whole.len(), 22474);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(whole.as_bytes()))
        ),
        "41cb2cf4267b055c3aee09c6a6bb143da182c28a6b583524ca7608e590133c10"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(include_bytes!(
                "lexical_cursor_before_prepared_links.rs"
            )))
        ),
        "8a354f623f13b5e9b337dae1e8eb0199a9e67e3d81b704be8864570a8eb38b22"
    );


    let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/opt886-pair-local-paths-20261018/whole-before/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"));
    let root=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/opt886-pair-local-paths-20261018/whole-before/src/cleanup_plan/replay/ordinary_plan_history.rs"));
    let cursor_body=format!("struct Cursor {{{}",root.split_once("struct Cursor {").unwrap().1.split_once("\n#[cfg(test)]\nmod before_comparison;").unwrap().0)
        .replacen("impl Cursor {","#[allow(dead_code, reason = \"complete immediate883 Cursor retains its original unused next method\")]\nimpl Cursor {",1);
    let expected=old.replacen("use super::*;\n",&format!("use super::*;\n\n{cursor_body}\n"),1).replace("#[cfg(test)]\nmod before_lexical_cursor;\n","")
        .replace("\n#[cfg(test)]\nmod tests;\n","\n")
        .replacen("    paid!(budget, function, OWNER_CONTROLS,","    let census = super::pair_scope_census::begin(function, budget, true);\n    paid!(budget, function, OWNER_CONTROLS,",1)
        .replacen("                owner.step = Step::PairNext;","                census.pair();\n                owner.step = Step::PairNext;",1)
        .replacen("                paid!(budget, function, 4 * PATH_INDEX + PAIR_MATCH,","                census.round();\n                paid!(budget, function, 4 * PATH_INDEX + PAIR_MATCH,",1)
        .replacen("    Ok(())\n}","    census.success();\n    Ok(())\n}",1);
    assert_eq!(source_format::restore(source_format::Kind::BeforePair, include_str!("before_pair_scope/mod.rs")),expected,"entire actual883 comparator, full methods and tariffs; namespace registration/diagnostic-only insertions enumerated");
    let cursor=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/opt886-pair-local-paths-20261018/whole-before/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"));
    assert_eq!(
        source_format::restore(
            source_format::Kind::Cursor,
            include_str!("before_pair_scope/lexical_cursor.rs")
        ),
        cursor,
        "whole old macro source and every tariff exact"
    );
    assert_eq!(
        source_format::restore(
            source_format::Kind::Cursor,
            &tail_source::previous(include_str!("lexical_cursor.rs")),
        ),
        cursor,
        "entire declared retained-tail transform and complete independent old macro"
    );
    let tests=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/opt886-pair-local-paths-20261018/whole-before/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs"));
    assert!(
        source_format::restore(source_format::Kind::Tests883, include_str!("tests.rs"))
            .starts_with(tests),
        "every existing owner assertion/body retained verbatim"
    );
}
#[test]
fn pair_census_refused_prefix_keeps_original_failure_and_never_claims_full_comparison() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let expected = expected(&program, function);
    let full = observe_pair(
        function,
        &expected,
        false,
        usize::MAX,
        0,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(full.0 .0.result.is_ok());
    balanced(full.0 .1);
    let events = frontiers::finish();
    let event = *events
        .iter()
        .find(|e| e.phase == "ordinary plan history pair-local paths")
        .unwrap();
    pair_scope_census::reset();
    let refused = observe_pair(
        function,
        &expected,
        false,
        event.before,
        0,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    frontiers::finish();
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    assert_eq!(rows.len(), 1);
    assert!(refused.1 && refused.0 .0.result.is_err());
    balanced(refused.0 .1);
    assert!(
        rows[0].ended
            && !rows[0].unwound
            && !rows[0].complete
            && !rows[0].overflow
            && !rows[0].truncated
    );
    assert_eq!(
        (rows[0].pairs, rows[0].rounds),
        (1, 1),
        "first work charge and both preparations precede the new acquisition"
    );
    assert_eq!(
        refused.0 .0.result.as_ref().unwrap_err(),
        &primitive_failure(function, event, event.before, 0)
    );
}

#[test]
fn retained_flat_view_keeps_complete_long_comparisons_and_stable_mismatch_diagnostics() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    for len in [0usize, 1, 64, 256] {
        let seed: Vec<_> = (0..len)
            .map(|index| {
                Rc::new(SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: index % 2 == 0,
                })
            })
            .collect();
        let expected = vec![SkeletonPath {
            observations: seed.clone().into(),
            terminal: SkeletonTerminal::Success,
        }];
        for mutation in 0..3 {
            let mut tail = seed.clone();
            if mutation == 1 {
                if tail.pop().is_none() {
                    tail.push(Rc::new(SkeletonObservation::Boolean {
                        expression: function.body.id.clone(),
                        value: false,
                    }));
                }
            } else if mutation == 2 {
                tail.push(Rc::new(SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: true,
                }));
            }
            let observe = |old| {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        // Identical fixture backing in both observations. The seeded
                        // immutable observations stay held outside both invocations.
                        budget
                            .reserve_array::<Rc<SkeletonObservation>>(
                                function,
                                tail.len(),
                                "flat-view test tail",
                            )
                            .unwrap();
                        budget
                            .reserve_array::<PlanPath>(function, 1, "flat-view test path")
                            .unwrap();
                        let actual = HistorySet {
                            arena: Vec::new(),
                            max_depth: 0,
                            paths: vec![PlanPath {
                                history: History {
                                    previous: None,
                                    tail: tail.clone(),
                                },
                                terminal: SkeletonTerminal::Success,
                            }],
                        };
                        let result = if old {
                            before_pair_scope::compare(function, &mut budget, actual, &expected)
                        } else {
                            compare(function, &mut budget, actual, &expected)
                        };
                        let outcome = (
                            result.as_ref().map(|_| ()).map_err(failure),
                            budget.remaining,
                            budget.skeleton_spent,
                        );
                        drop(result);
                        outcome
                    })
                })
            };
            pair_scope_census::reset();
            let ((old, old_heap), old_refused, old_paid, old_receipt) = observe(true);
            frontiers::finish();
            pair_scope_census::finish();
            pair_scope_census::reset();
            let ((current, heap), refused, paid, receipt) = observe(false);
            frontiers::finish();
            let (rows, lost) = pair_scope_census::finish();
            assert_eq!(lost, 0);
            assert_eq!(rows.len(), 1);
            assert!(!rows[0].old && !rows[0].overflow && !rows[0].truncated);

            assert_eq!(
                current, old,
                "all diagnostic fields and work, len={len} mutation={mutation}"
            );
            assert!(!refused && !old_refused && receipt.is_none() && old_receipt.is_none());
            balanced(heap);
            balanced(old_heap);
            assert_eq!(
                heap, old_heap,
                "identical fixture backing and complete invocation Drop"
            );
            if mutation == 0 {
                assert!(current.0.is_ok());
                let removed =
                    2 * size_of::<&Observations>() + size_of::<&[Rc<SkeletonObservation>]>();
                let retained = 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
                    + 2 * size_of::<&[Rc<SkeletonObservation>]>();
                let (old_tail, tail, tail_view) = tail_receiver_prices();
                let (step, load, setup, limit) = prepared_link_prices();
                assert!(rows[0].complete && rows[0].ended);
                assert_eq!(rows[0].tails, len + 1);
                assert_eq!(
                    paid + (removed + old_tail - tail) * (len + 1)
                        + step * rows[0].steps
                        + load * rows[0].loads,
                    old_paid
                        + PAIR_PRIMITIVE
                        + PAYMENT_HEADER
                        + retained
                        + tail_view
                        + setup
                        + limit
                );
                if len >= 64 {
                    assert!(
                        paid < old_paid,
                        "long complete comparisons must reduce real reservations"
                    );
                }
            } else {
                assert_eq!(current.0.unwrap_err().code, "SPX-H006");
            }
        }
    }
}
