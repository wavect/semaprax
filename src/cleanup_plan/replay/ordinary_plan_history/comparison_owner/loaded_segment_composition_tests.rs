//! Ordinary independent-segment transport, checked against the complete immediate kernel.
use super::*;

pub(super) fn complete_immediate438_invoice_counts(row: &pair_scope_census::Row) -> pair_scope_census::Row {
    assert!(row.complete && row.ended && !row.unwound && !row.truncated && !row.overflow);
    assert!(!row.old);
    let skipped = row
        .independent_turns
        .checked_sub(row.independent_runs)
        .unwrap();
    let mut old = *row;
    for count in [
        &mut old.mask_checks,
        &mut old.shared_checks,
        &mut old.shared_metadata,
    ] {
        *count = count.checked_add(skipped).unwrap();
    }
    old.value_steps = old
        .value_steps
        .checked_add(2 * row.independent_turns)
        .unwrap();
    old.pair_right_steps = old
        .pair_right_steps
        .checked_add(row.independent_turns)
        .unwrap();
    old.pair_receivers = old
        .pair_receivers
        .checked_add(row.independent_turns)
        .unwrap();
    old.pair_matches_admitted = old
        .pair_matches_admitted
        .checked_add(row.independent_turns)
        .unwrap();
    old.independent_frames = 0;
    old.independent_checks = 0;
    old.independent_runs = 0;
    old.independent_turns = 0;
    old
}
pub(super) fn complete_loaded_delta(row: &pair_scope_census::Row) -> (usize, usize) {
    assert!(row.complete && row.ended && !row.unwound && !row.truncated && !row.overflow);
    let (removed, added) = prices_unchecked(row);
    (added, removed)
}
fn prices(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {
    assert_eq!(row.independent_frames, usize::from(paths >= 2));
    prices_unchecked(row)
}
fn prices_unchecked(row: &pair_scope_census::Row) -> (usize, usize) {
    let skipped = row
        .independent_turns
        .checked_sub(row.independent_runs)
        .unwrap();
    let old_pair = paired_value::STEP + paired_value::EMPTY + paired_value::RIGHT_STEP + PAIR_MATCH;
    let old_loop = paired_value::TAIL_SELECT
        + certified_heads::BOUNDARY_CHECK
        + certified_heads::BOUNDARY_EMPTY;
    (
        old_pair * row.independent_turns + old_loop * skipped,
        paired_value::INDEPENDENT_PRIMITIVES * row.independent_frames + 3 * size_of::<bool>() * row.independent_checks
            + paired_value::INDEPENDENT_RUN_SETUP * row.independent_runs
            + paired_value::INDEPENDENT_RUN_PAYLOAD * row.independent_turns,
    )
}
fn counts(row: &pair_scope_census::Row) -> [usize; 7] {
    [
        row.mask_checks,
        row.shared_checks,
        row.shared_metadata,
        row.value_steps,
        row.pair_right_steps,
        row.pair_receivers,
        row.pair_matches_admitted,
    ]
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
                    crate::cleanup_plan::replay::ordinary_plan_history::comparison_owner::before_loaded_segments::compare(function, &mut budget, actual, expected)
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
fn loaded_segments_keep_complete_immediate438_values_work_drop_and_typed_equation() {
    let program = super::super::super::tests::program();
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
        #[cfg(feature = "unstable-workflow-profiling")]
        let capture = {
            use crate::workflow_profile::plan_comparison::{Binding, Capture};
            use sha2::{Digest, Sha256};
            Capture::begin(Binding::caller_labelled(
                "loaded independent shipping rounds",
                Sha256::digest(include_bytes!("loaded_segment_composition_tests.rs")).into(),
            )).unwrap()
        };
        let (new, new_heap, new_paid) = observe_immediate_shaped_pair(
            function,
            &expected,
            actual(),
            false,
            MAX_REPLAY_WORK_UNITS,
        );
        let row = complete_row(false);
        #[cfg(feature = "unstable-workflow-profiling")]
        {
            use crate::workflow_profile::plan_comparison::Site;
            let report = capture.finish().unwrap();
            assert!(report.counts_complete && report.detail_complete && !report.overflow);
            assert_eq!(row.shared_boundary_complete, 0);
            assert_eq!(report.counts.at(Site::Round), row.rounds as u64);
            assert_eq!(report.counts.admitted_bytes[Site::Round as usize], 0);
            assert_eq!(report.counts.started, report.counts.ended);
            assert_eq!(report.counts.unwound, 0);
        }
        assert_eq!(new, old);
        assert_eq!(new_heap, old_heap);
        assert_eq!(counts(&complete_immediate438_invoice_counts(&row)), counts(&old_row));
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
                    crate::cleanup_plan::replay::ordinary_plan_history::comparison_owner::before_loaded_segments::compare(
                        function, &mut budget, actual, expected)
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
fn loaded_segments_preserve_exact_short_floor_zero_sticky_and_projection_refusals() {
    let program = super::super::super::tests::program();
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
                assert!(std::panic::catch_unwind(|| super::pair_scope_tests::predecessor_price_counts(&bad)).is_err());
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

#[test]
fn loaded_segments_keep_entire_immediate_and_puremain_sources_and_all_unchanged_drivers() {
    use sha2::Digest;
    let hash = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(s.as_bytes()))
        )
    };
    let old = include_str!(
        "../../../../../e/e98bf0ffd40155974eeb159ffc5cb0a194c5c0b72f5735ad95381c3644d7dedd"
    );
    assert_eq!(
        hash(old),
        "e98bf0ffd40155974eeb159ffc5cb0a194c5c0b72f5735ad95381c3644d7dedd"
    );
    let mut expected = old.replacen("//! One comparison invocation owns sort and pair state. No nested order/sift calls.\nuse super::*;\n#[allow(unused_macros, reason = \"complete immediate final Option producer remains a frozen control\")]\n#[macro_use]\nmod lexical_cursor;\n#[macro_use]\nmod final_affine;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_final_affine;\n#[macro_use]\nmod paired_value;\n#[allow(\n    unused_macros,\n    reason = \"complete immediate suffix macro remains available to frozen generations\"\n)]\n#[macro_use]\nmod shared_prefix;\n#[macro_use]\nmod certified_heads;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_empty_selector;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_paired_tail;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_main_b692;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_main487;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_pair_links;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_main001;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_certified_heads;\n#[cfg(test)]\nmod before_event_prefix;\n#[cfg(test)]\nmod before_shared_steps;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_shared_runs;\n#[cfg(test)]\n#[allow(unused_imports, reason = \"complete frozen generation retains its exact original namespace imports\")]\n#[rustfmt::skip]\nmod before_reachable_primitives;\n#[cfg(test)]\nmod before_flat_view;\n#[cfg(test)]\nmod before_fused_flat;\n#[cfg(test)]\nmod before_identity_prefix;\n#[cfg(test)]\nmod before_lexical_cursor;\n#[cfg(test)]\nmod before_pair_scope;\n#[cfg(test)]\n#[rustfmt::skip]\nmod before_prepared_pairing;\n#[cfg(test)]\nmod before_retained_tail;\n#[cfg(test)]\npub(super) mod pair_scope_census;\n\n", "//! Complete immediate438 comparator; only namespace/observer identity differs.\nuse super::*;\n#[macro_use]\n#[path = \"paired_value_before_loaded_segments.rs\"]\nmod paired_value;\n#[macro_use]\n#[path = \"certified_heads_before_loaded_segments.rs\"]\nmod certified_heads;\n", 1);
    expected = expected.replacen(
        "pair_scope_census::begin(function, budget, false)",
        "pair_scope_census::begin(function, budget, true)",
        1,
    );
    expected = expected.replacen("\n#[cfg(test)]\nmod tests;\n", "\n", 1);
    assert_eq!(expected, include_str!("before_loaded_segments.rs"));
    assert_eq!(
        hash(include_str!("paired_value_before_loaded_segments.rs")),
        "0e289307a841d6ae175957fc9d0f510dc08ccf85959125748cf027b65a9b0de3"
    );
    assert_eq!(
        hash(include_str!("certified_heads_before_loaded_segments.rs")),
        "d1a229642405a06f2037a30470188c82d66ecb51913c40f30842428c3984a1d6"
    );
    assert_eq!(
        hash(include_str!("lexical_cursor.rs")),
        "c63b8f98af8b1fa55f7e305819efc87107477db4a92f57743365544885b4973c"
    );
    assert_eq!(
        hash(&crate::source_reflows_merge::main_e177_final_inputs::value_direct(
            "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix.rs",
            "faf2391a83182d4b60d1be8561c404ac7af427fcd1f59ff2bfa45c85058244d0",
            include_str!("shared_prefix.rs"),
        )),
        "faf2391a83182d4b60d1be8561c404ac7af427fcd1f59ff2bfa45c85058244d0"
    );
    assert_eq!(
        hash(include_str!("final_affine.rs")),
        "2f4387d36e1e29ab15c409438dd74071439f2cb058d3fe53aa16c533ef79780e"
    );
    let main = include_str!(
        "../../../../../e/c8f66d4154fd2ed0bb830b4bcecb2c9153949b9c636ea2d4fc8977edfbf130f8"
    );
    assert_eq!(
        hash(main),
        "c8f66d4154fd2ed0bb830b4bcecb2c9153949b9c636ea2d4fc8977edfbf130f8"
    );
    let mut expected = main.replacen(
        "mod lexical_cursor;",
        "#[path = \"main_loaded_segments_cursor.rs\"]\nmod lexical_cursor;",
        1,
    );
    for name in [
        "before_lexical_cursor",
        "before_pair_scope",
        "pair_scope_census",
    ] {
        expected = expected.replacen(&format!("#[cfg(test)]\nmod {name};\n"), "", 1);
    }
    expected = expected.replacen(
        "#[cfg(test)]\npub(super) mod paired_segments_before;\n",
        "",
        1,
    );
    expected = expected.replacen("\n#[cfg(test)]\nmod tests;\n", "\n", 1);
    assert_eq!(expected, include_str!("main_loaded_segments.rs"));
    assert_eq!(
        hash(include_str!("main_loaded_segments_cursor.rs")),
        "768c23a10fcec91ef7dfdbd67dabd64885266291f236c66efd0e80ad21b29b6e"
    );
}
