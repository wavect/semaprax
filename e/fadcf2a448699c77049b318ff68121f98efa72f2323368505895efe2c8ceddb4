//! Ordinary independent-segment transport, checked against the complete immediate kernel.
use super::*;
// This retained incoming equation qualifies the complete puremain599 kernel.
use super::super::super::main_loaded_segments::compare;

pub(super) fn predecessor_price_counts(row: &pair_scope_census::Row) -> pair_scope_census::Row {
    assert!(row.complete && row.ended && !row.unwound && !row.truncated && !row.overflow);
    assert!(!row.old);
    let skipped = row
        .independent_turns
        .checked_sub(row.independent_runs)
        .unwrap();
    let mut quoted = *row;
    for count in [
        &mut quoted.mask_checks,
        &mut quoted.shared_checks,
        &mut quoted.shared_patterns,
        &mut quoted.shared_metadata,
    ] {
        *count = count.checked_add(skipped).unwrap();
    }
    for count in [
        &mut quoted.pair_left_steps,
        &mut quoted.pair_right_steps,
        &mut quoted.pair_receivers,
    ] {
        *count = count.checked_add(row.independent_turns).unwrap();
    }
    quoted
}

pub(super) fn prices(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {
    let header = 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    let empty = 2 * size_of::<usize>() + size_of::<bool>();
    let left = 2 * size_of::<usize>()
        + 3 * size_of::<bool>()
        + size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<usize>()
        + empty;
    let right = 2 * size_of::<usize>()
        + 3 * size_of::<bool>()
        + size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<usize>()
        + empty;
    let pair = 4 * size_of::<&Rc<SkeletonObservation>>()
        + 6 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonTerminal>()
        + 2 * size_of::<Ordering>();
    let checks = 10 * size_of::<bool>() + 4 * size_of::<usize>();
    let setup = 4 * size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<[usize; 12]>()
        + 6 * size_of::<std::iter::Take<std::iter::Repeat<()>>>()
        + 2 * size_of::<&mut std::iter::Take<std::iter::Repeat<()>>>()
        + 2 * size_of::<Option<()>>();
    let payload = 6 * size_of::<&Rc<SkeletonObservation>>()
        + 4 * size_of::<&[Rc<SkeletonObservation>]>()
        + 6 * size_of::<&SkeletonObservation>()
        + 2 * size_of::<usize>()
        + 4 * size_of::<Ordering>()
        + 3 * size_of::<bool>();
    let skipped = row
        .independent_turns
        .checked_sub(row.independent_runs)
        .unwrap();
    (
        (left + right + pair) * row.independent_turns + checks * skipped,
        if paths >= 2 { 8 * header } else { 0 }
            + 3 * size_of::<bool>() * row.independent_checks
            + setup * row.independent_runs
            + payload * row.independent_turns,
    )
}
fn counts(row: &pair_scope_census::Row) -> [usize; 7] {
    [
        row.mask_checks,
        row.shared_checks,
        row.shared_patterns,
        row.shared_metadata,
        row.pair_left_steps,
        row.pair_right_steps,
        row.pair_receivers,
    ]
}

fn observe_immediate_shaped_pair(
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
                    super::super::paired_segments_before::compare(
                        function,
                        &mut budget,
                        actual,
                        expected,
                    )
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
fn independent_segments_keep_complete_before_values_work_drop_and_source_price_projection() {
    let program = crate::cleanup_plan::replay::ordinary_plan_history::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let item = Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: true,
    });
    for width in [1usize, 4, 16, 64] {
        let expected = vec![
            SkeletonPath {
                observations: vec![item.clone(); width].into(),
                terminal: SkeletonTerminal::Success,
            };
            2
        ];
        let actual = || HistorySet {
            arena: (0..2)
                .map(|_| Segment {
                    previous: None,
                    depth: 1,
                    items: vec![item.clone(); width],
                })
                .collect(),
            paths: (0..2)
                .map(|side| PlanPath {
                    history: History {
                        previous: Some(side),
                        tail: Vec::new(),
                    },
                    terminal: SkeletonTerminal::Success,
                })
                .collect(),
            max_depth: 1,
        };
        pair_scope_census::reset();
        let (old, old_heap, old_paid) = observe_immediate_shaped_pair(
            function,
            &expected,
            actual(),
            true,
            MAX_REPLAY_WORK_UNITS,
        );
        let old_row = complete_row(true);
        pair_scope_census::reset();
        let (new, new_heap, new_paid) = observe_immediate_shaped_pair(
            function,
            &expected,
            actual(),
            false,
            MAX_REPLAY_WORK_UNITS,
        );
        let row = complete_row(false);
        assert_eq!(new, old);
        assert_eq!(new_heap, old_heap);
        assert_eq!(counts(&predecessor_price_counts(&row)), counts(&old_row));
        assert_eq!(
            (row.pairs, row.rounds, row.steps, row.loads, row.tails),
            (
                old_row.pairs,
                old_row.rounds,
                old_row.steps,
                old_row.loads,
                old_row.tails
            )
        );
        let (removed, retained) = prices(expected.len(), &row);
        assert_eq!(new_paid + removed, old_paid + retained);
        if width >= 16 {
            assert!(row.independent_runs > 0 && row.independent_turns >= row.independent_runs);
        }
        if width == 64 {
            assert!(new_paid < old_paid);
        }
        let spent = MAX_REPLAY_WORK_UNITS - old.remaining;
        for cut in 0..=spent {
            pair_scope_census::reset();
            let (old, old_heap, _) =
                observe_immediate_shaped_pair(function, &expected, actual(), true, cut);
            pair_scope_census::reset();
            let (new, heap, _) =
                observe_immediate_shaped_pair(function, &expected, actual(), false, cut);
            assert_eq!(new, old, "complete immediate predecessor work cut {cut}");
            assert_eq!(heap, old_heap);
        }
    }
}

#[test]
fn complete_immediate_kernel_has_only_named_namespace_changes() {
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paired-segments-5d46-20261019/before-owner.rs.txt"
    ));
    let mut expected = old.to_owned();
    for name in [
        "before_lexical_cursor",
        "before_pair_scope",
        "pair_scope_census",
    ] {
        let token = format!("#[cfg(test)]\nmod {name};\n");
        assert_eq!(expected.matches(&token).count(), 1);
        expected = expected.replacen(&token, "", 1);
    }
    let test_module = "\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(expected.matches(test_module).count(), 1);
    expected = expected.replacen(test_module, "", 1);
    expected = expected.replacen(
        "mod lexical_cursor;",
        "#[path = \"paired_segments_before_cursor.rs\"]\nmod lexical_cursor;",
        1,
    );
    expected = expected.replacen(
        "pair_scope_census::begin(function, budget, false)",
        "pair_scope_census::begin(function, budget, true)",
        1,
    );
    assert_eq!(expected, include_str!("../paired_segments_before.rs"));
    assert_eq!(
        include_str!("../paired_segments_before_cursor.rs"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paired-segments-5d46-20261019/before-macro.rs.txt"
        ))
    );
}

fn observe_funded(
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    actual: HistorySet,
    old: bool,
    physical: usize,
    floor: usize,
) -> Observed {
    with_limit_usage_refusal(physical, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            assert!(crate::bounded_output::set_active_floor(floor));
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "883 shaped comparison receiving"
                )?;
                if old {
                    super::super::paired_segments_before::compare(
                        function,
                        &mut budget,
                        actual,
                        expected,
                    )
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
fn independent_segments_exact_short_floor_zero_sticky_and_projection_refusals_are_real() {
    let program = crate::cleanup_plan::replay::ordinary_plan_history::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let item = Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: true,
    });
    let expected = vec![
        SkeletonPath {
            observations: vec![item.clone(); 64].into(),
            terminal: SkeletonTerminal::Success,
        };
        2
    ];
    let actual = || HistorySet {
        arena: (0..2)
            .map(|_| Segment {
                previous: None,
                depth: 1,
                items: vec![item.clone(); 64],
            })
            .collect(),
        paths: (0..2)
            .map(|side| PlanPath {
                history: History {
                    previous: Some(side),
                    tail: Vec::new(),
                },
                terminal: SkeletonTerminal::Success,
            })
            .collect(),
        max_depth: 1,
    };
    for old in [true, false] {
        pair_scope_census::reset();
        let ((full, heap), refused, paid, first) =
            observe_funded(function, &expected, actual(), old, usize::MAX, 0);
        let row = complete_row(old);
        assert!(full.result.is_ok() && !refused && first.is_none());
        balanced(heap);
        if !old {
            assert!(row.independent_runs > 0);
            for change in 0..5 {
                let mut bad = row;
                match change {
                    0 => bad.complete = false,
                    1 => bad.ended = false,
                    2 => bad.unwound = true,
                    3 => bad.overflow = true,
                    _ => bad.independent_runs = bad.independent_turns + 1,
                }
                assert!(std::panic::catch_unwind(|| predecessor_price_counts(&bad)).is_err());
            }
        }
        for (limit, floor, success) in [
            (paid, 0, true),
            (paid - 1, 0, false),
            (0, 0, false),
            (paid + 32, 32, true),
            (paid + 31, 32, false),
        ] {
            pair_scope_census::reset();
            let ((outcome, heap), refused, charged, receipt) =
                observe_funded(function, &expected, actual(), old, limit, floor);
            balanced(heap);
            assert!(charged <= limit - floor);
            assert_eq!(outcome.result.is_ok(), success);
            assert_eq!(refused, !success);
            if success {
                assert_eq!(outcome, full);
                assert!(outcome.retry.is_none() && receipt.is_none());
            } else {
                assert!(outcome.retry.is_some());
                let first = receipt.expect("actual first physical refusal");
                assert_eq!(first.floor, floor);
                assert!(first.requested > 0 && first.requested + floor > first.remaining);
            }
        }
    }
}
