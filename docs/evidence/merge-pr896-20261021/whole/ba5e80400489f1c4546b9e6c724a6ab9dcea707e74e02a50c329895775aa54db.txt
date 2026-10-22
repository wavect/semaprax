//! Complete immediate comparator versus identity-replayed current path.
use super::*;
use super::super::before_event_prefix::{compare, ComparisonOwner, shared_prefix_prices as shared_prefix};

// Independent typed site equation. No coefficient is inferred from paid totals.
pub(super) fn added(r: &pair_scope_census::Row) -> usize {
    let setup = size_of::<ComparisonOwner<'static>>()
        - size_of::<before_identity_prefix::ComparisonOwner<'static>>()
        + 3 * size_of::<usize>()
        + size_of::<bool>()
        + 7 * PAYMENT_HEADER;
    let detector = 6 * size_of::<usize>() + 8 * size_of::<bool>();
    let capture = size_of::<usize>();
    let extent = 2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<usize>();
    let replay = 2 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<usize>()
        + size_of::<Ordering>()
        + 4 * size_of::<bool>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>();
    setup
        + detector * r.identity_detectors
        + capture * r.identity_captures
        + extent * r.identity_extent_captures
        + replay * r.identity_observations
}
// Separate incoming reachable-site delta; never an identity-replay credit.
pub(super) fn reachable_delta(paths: usize) -> (usize, usize) {
    let header = 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    let selector = header + 3 * size_of::<usize>() + 2 * size_of::<bool>();
    let reachable = match paths {
        0 => 0,
        1 => 5 * header,
        _ => 20 * header,
    };
    (20 * header, selector + reachable)
}
pub(super) fn removed(r: &pair_scope_census::Row) -> usize {
    // Complete old MATCH, two loaded STEP+EMPTY packets; no LOAD or tail credit.
    let matching = 2 * size_of::<&SkeletonObservation>()
        + 2 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonTerminal>()
        + 2 * size_of::<Ordering>();
    let step = 2 * size_of::<usize>()
        + 3 * size_of::<bool>()
        + size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 2 * size_of::<&Rc<SkeletonObservation>>()
        + size_of::<&SkeletonObservation>()
        + size_of::<usize>();
    let empty = 2 * size_of::<usize>() + size_of::<bool>();
    r.identity_observations * (matching + 2 * (step + empty))
}

fn observation(n: usize, value: bool) -> Rc<SkeletonObservation> {
    Rc::new(SkeletonObservation::Boolean {
        expression: crate::hir::ExpressionId::from_owned(format!("identity-observation-{n}")),
        value,
    })
}
// Input construction is outside both observed comparisons. Different-ID equal
// contents remain distinct arena segments, never an identity certificate.
fn fixture(
    length: usize,
    depth: usize,
    shared: bool,
    fault: usize,
) -> (HistorySet, Vec<SkeletonPath>) {
    let common: Vec<_> = (0..length).map(|n| observation(n, true)).collect();
    let mut arena = Vec::new();
    for n in 0..depth.max(1) {
        arena.push(Segment {
            previous: n.checked_sub(1),
            items: if n == 0 { common.clone() } else { Vec::new() },
            depth: n + 1,
        });
    }
    let mut paths = Vec::new();
    let mut expected = Vec::new();
    for n in 0..4 {
        let tail = if n == 0 {
            Vec::new()
        } else {
            vec![observation(length + n, n != 2)]
        };
        let terminal = if n == 3 {
            SkeletonTerminal::Failure
        } else {
            SkeletonTerminal::Success
        };
        let previous = if shared {
            depth.max(1) - 1
        } else {
            let index = arena.len();
            arena.push(Segment {
                previous: None,
                items: common.clone(),
                depth: 1,
            });
            index
        };
        paths.push(PlanPath {
            history: History {
                previous: Some(previous),
                tail: tail.clone(),
            },
            terminal,
        });
        let mut observations: Vec<_> = common.iter().map(|v| Rc::new(v.as_ref().clone())).collect();
        observations.extend(tail.iter().map(|v| Rc::new(v.as_ref().clone())));
        expected.push(SkeletonPath {
            observations: observations.into(),
            terminal,
        });
    }
    match fault {
        1 => paths[0].history.previous = Some(arena.len()),
        2 => arena[0].depth = 0,
        3 => arena[0].previous = Some(0),
        4 => {
            paths[1].history.tail.push(observation(999, false));
        }
        5 => {
            paths[2].terminal = SkeletonTerminal::LoopLatch;
        }
        6 => {
            paths[2].history.tail = paths[1].history.tail.clone();
            paths[2].terminal = paths[1].terminal;
            expected[2] = expected[1].clone();
        }
        7 => {
            if let Some(first) = arena[0].items.first_mut() {
                *first = observation(999, false);
            }
        }
        8 => {
            if let Some(last) = arena[0].items.last_mut() {
                *last = observation(999, false);
            }
        }
        _ => {}
    }
    expected.sort();
    (
        HistorySet {
            arena,
            paths,
            max_depth: depth.max(1),
        },
        expected,
    )
}
struct Run {
    observed: Observed,
    events: Vec<frontiers::Event>,
    row: pair_scope_census::Row,
}
fn run(
    function: &ResolvedFunction,
    length: usize,
    depth: usize,
    shared: bool,
    fault: usize,
    old: bool,
    units: usize,
    limit: usize,
    floor: usize,
    hir_shared: bool,
) -> Run {
    run_traced(
        function, length, depth, shared, fault, old, units, limit, floor, hir_shared, false,
    )
}
fn run_traced(
    function: &ResolvedFunction,
    length: usize,
    depth: usize,
    shared: bool,
    fault: usize,
    old: bool,
    units: usize,
    limit: usize,
    floor: usize,
    hir_shared: bool,
    swaps_only: bool,
) -> Run {
    let (actual, mut expected) = fixture(length, depth, shared, fault);
    if hir_shared {
        for path in &mut expected {
            path.observations.prefix = path.observations.freeze();
            path.observations.items.clear();
        }
    }
    pair_scope_census::reset();
    if swaps_only {
        frontiers::reset_swaps();
    } else {
        frontiers::reset();
    }
    let observed = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.remaining = units;
            assert!(crate::bounded_output::set_active_floor(floor));
            let result = (|| {
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "899 complete comparison receiver"
                )?;
                if old {
                    before_identity_prefix::compare(function, &mut budget, actual, &expected)
                } else {
                    compare(function, &mut budget, actual, &expected)
                }
            })();
            let saved = result.as_ref().map(|_| ()).map_err(failure);
            let retry = if result.is_err()
                && (budget.refusal_spent || crate::bounded_output::active_refused())
            {
                crate::bounded_output::clear_active_floor();
                let before = (budget.remaining, crate::bounded_output::active_remaining());
                let retry = budget
                    .reserve_bytes(function, 0, "899 sticky zero")
                    .unwrap_err();
                assert_eq!(
                    (budget.remaining, crate::bounded_output::active_remaining()),
                    before
                );
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
            drop(result);
            drop(retry);
            drop(budget);
            outcome
        })
    });
    let events = frontiers::finish();
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    // The original call frame can refuse before begin; that is genuinely no row.
    let row = if rows.is_empty() {
        assert!(observed.1);
        assert!(observed.0 .0.result.is_err());
        pair_scope_census::empty_for_test()
    } else {
        assert_eq!(rows.len(), 1);
        assert!(rows[0].ended && !rows[0].overflow && !rows[0].truncated && !rows[0].unwound);
        rows[0]
    };
    balanced(observed.0 .1);
    Run {
        observed,
        events,
        row,
    }
}
fn swaps(run: &Run) -> Vec<(&'static str, usize)> {
    run.events
        .iter()
        .filter(|e| {
            e.phase == "ordinary plan retained drain swap receivers"
                || e.phase == "ordinary plan retained sift swap receivers"
        })
        .map(|e| (e.phase, e.swaps))
        .collect()
}
#[test]
fn shared_identity_replay_preserves_every_work_cut_six_fields_remaining_and_swaps() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    for shared in [false, true] {
        for (length, depth) in [(0, 1), (1, 1), (8, 1), (8, 8)] {
            for fault in 0..9 {
                let old = run(
                    function,
                    length,
                    depth,
                    shared,
                    fault,
                    true,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    false,
                );
                let current = run(
                    function,
                    length,
                    depth,
                    shared,
                    fault,
                    false,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    false,
                );
                assert_eq!(old.observed.0 .0, current.observed.0 .0);
                assert_eq!(swaps(&old), swaps(&current));
                let spent = MAX_REPLAY_WORK_UNITS - old.observed.0 .0.remaining;
                for cut in 0..=spent {
                    let old = run(
                        function,
                        length,
                        depth,
                        shared,
                        fault,
                        true,
                        cut,
                        usize::MAX,
                        0,
                        false,
                    );
                    let current = run(
                        function,
                        length,
                        depth,
                        shared,
                        fault,
                        false,
                        cut,
                        usize::MAX,
                        0,
                        false,
                    );
                    assert_eq!(
                        old.observed.0 .0, current.observed.0 .0,
                        "length{length} depth{depth} shared{shared} fault{fault} work{cut}"
                    );
                    assert_eq!(swaps(&old), swaps(&current));
                }
            }
        }
    }
}
#[test]
fn shared_identity_replay_has_exact_complete_cold_hot_invoices_and_actual_k() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    for (length, depth, shared) in [
        (0, 1, true),
        (1, 1, true),
        (128, 1, false),
        (128, 1, true),
        (128, 16, true),
    ] {
        for hir_shared in [false, true] {
            let old = run_traced(
                function,
                length,
                depth,
                shared,
                0,
                true,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir_shared,
                true,
            );
            let current = run_traced(
                function,
                length,
                depth,
                shared,
                0,
                false,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir_shared,
                true,
            );
            assert!(old.observed.0 .0.result.is_ok());
            assert_eq!(old.observed.0 .0, current.observed.0 .0);
            assert!(old.row.complete && current.row.complete);
            assert_eq!(old.observed.0 .1, current.observed.0 .1);
            assert_eq!(swaps(&old), swaps(&current));
            // fixture() constructs exactly four real paths in both observations.
            assert_eq!(
                current.observed.2 + removed(&current.row) + reachable_delta(4).0,
                old.observed.2 + added(&current.row) + reachable_delta(4).1
            );
            assert_eq!(
                current.row.identity_work,
                5 * current.row.identity_observations
            );
            assert_eq!(
                old.row.value_steps,
                current.row.value_steps + 2 * current.row.identity_observations
            );
            assert_eq!(
                old.row.pair_matches_admitted,
                current.row.pair_matches_admitted + current.row.identity_observations
            );
            if shared && length > 1 {
                assert!(current.row.identity_hits > 0 && current.row.identity_observations > 0);
                assert!(
                    removed(&current.row) + reachable_delta(4).0
                        > added(&current.row) + reachable_delta(4).1,
                    "real 128-observation hot witness must amortize all new costs"
                );
            } else {
                assert_eq!(current.row.identity_observations, 0);
                assert!(
                    current.observed.2 > old.observed.2,
                    "honest common-cold regression"
                );
            }
            eprintln!(
                "OPT899 length={length} depth={depth} shared={shared} hir_shared={hir_shared} old/current={}/{} K={} hits={} detector={} capture={} new={} removed={}; unit instrumented generation, no shipping/application claim",
                old.observed.2,
                current.observed.2,
                current.row.identity_observations,
                current.row.identity_hits,
                current.row.identity_detectors,
                current.row.identity_captures,
                added(&current.row),
                removed(&current.row)
            );
        }
    }
}
#[test]
fn shared_identity_replay_every_new_acquisition_exact_short_zero_floor_and_sticky_drop() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let success = run(
        function,
        16,
        1,
        true,
        0,
        false,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
    );
    assert!(success.observed.0 .0.result.is_ok() && success.row.identity_observations > 0);
    let total = success.observed.2;
    for limit in [0, total - 1, total] {
        let result = run(
            function,
            16,
            1,
            true,
            0,
            false,
            MAX_REPLAY_WORK_UNITS,
            limit,
            0,
            false,
        );
        assert_eq!(result.observed.0 .0.result.is_ok(), limit == total);
        assert_eq!(result.observed.1, limit != total);
        if limit != total {
            assert!(result.observed.0 .0.retry.is_some());
        }
    }
    let events: Vec<_> = success
        .events
        .iter()
        .filter(|e| {
            e.phase == "ordinary plan retained comparison owner"
                || e.phase.starts_with("ordinary plan shared prefix")
                || (e.phase == "ordinary plan history paired cursor next"
                    && [
                        shared_prefix::CAPTURE_LINK,
                        shared_prefix::CAPTURE_LINK + shared_prefix::CAPTURE_EXTENT,
                    ]
                    .iter()
                    .any(|capture| e.after == Some(e.before + paired_value::LOAD + capture)))
        })
        .copied()
        .collect();
    assert!(events
        .iter()
        .any(|e| e.phase == "ordinary plan shared prefix scalar receivers"));
    assert!(events
        .iter()
        .any(|e| e.phase == "ordinary plan shared prefix identity detector"));
    for event in events {
        let after = event.after.unwrap();
        for (limit, floor) in [(after - 1, 0), (after, 1)] {
            let result = run(
                function,
                16,
                1,
                true,
                0,
                false,
                MAX_REPLAY_WORK_UNITS,
                limit,
                floor,
                false,
            );
            assert!(result.observed.1 && result.observed.0 .0.result.is_err());
            assert!(result.observed.0 .0.retry.is_some());
            let receipt = result.observed.3.unwrap();
            assert_eq!(receipt.requested, after - event.before);
            assert_eq!(receipt.remaining, limit - event.before);
            assert_eq!(receipt.floor, floor);
            assert!(!result.row.complete);
        }
        let exact = run(
            function,
            16,
            1,
            true,
            0,
            false,
            MAX_REPLAY_WORK_UNITS,
            after,
            0,
            false,
        );
        assert!(
            exact.events.iter().any(|e| e.phase == event.phase
                && e.before == event.before
                && e.after == Some(after)),
            "the exact acquisition succeeds even if the next original acquisition refuses"
        );
    }
}

#[test]
fn shared_identity_replay_real_producer_and_both_hir_representations_keep_complete_result() {
    let program = super::super::super::tests::program();
    for function in &program.functions {
        let flat = expected(&program, function);
        let mut shared = flat.clone();
        for path in &mut shared {
            path.observations.prefix = path.observations.freeze();
            path.observations.items.clear();
        }
        for expected in [&flat, &shared] {
            let observe = |old| {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let result = (|| {
                            paid!(
                                &mut budget,
                                function,
                                PRODUCE_CALL_CONTROLS,
                                "899 real producer call"
                            )?;
                            let actual = produce(function, &mut budget)?;
                            paid!(
                                &mut budget,
                                function,
                                COMPARE_CALL_CONTROLS,
                                "899 real comparison call"
                            )?;
                            if old {
                                before_identity_prefix::compare(
                                    function,
                                    &mut budget,
                                    actual,
                                    expected,
                                )
                            } else {
                                compare(function, &mut budget, actual, expected)
                            }
                        })();
                        let outcome = (
                            result.as_ref().map(|_| ()).map_err(failure),
                            budget.remaining,
                            budget.skeleton_spent,
                        );
                        drop(result);
                        drop(budget);
                        outcome
                    })
                })
            };
            let old = observe(true);
            let current = observe(false);
            assert_eq!(old.0 .0, current.0 .0);
            assert!(old.0 .0 .0.is_ok());
            balanced(old.0 .1);
            balanced(current.0 .1);
        }
    }
}

#[test]
fn shared_identity_replay_retains_complete_immediate_sources_and_original_invoices() {
    use sha2::{Digest, Sha256};
    let original = &super::super::before_event_prefix::whole_source();
    let incoming = reachable_stage(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
        "ae4705bfe4032ba2674a57c1451ded8c2cf0672f8f91cad07a127e5d599303b2",
        include_str!("../comparison_owner.rs"),
    );
    let frozen = include_str!("before_reachable_primitives.rs")
        .replace("#[path = \"lexical_cursor_reachable_before.rs\"]\n", "")
        .replace(
            "#[cfg(test)]\nuse super::{before_lexical_cursor, before_pair_scope, pair_scope_census};",
            "#[cfg(test)]\nmod before_lexical_cursor;\n#[cfg(test)]\nmod before_pair_scope;\n#[cfg(test)]\nmod pair_scope_census;",
        )
        .replace("pub(super) struct ComparisonOwner<", "struct ComparisonOwner<")
        + "\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(
        frozen, incoming,
        "entire incoming current owner, only explicit namespace glue"
    );
    assert_eq!(
        include_str!("lexical_cursor_reachable_before.rs"),
        reachable_stage(
            "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
            "c63b8f98af8b1fa55f7e305819efc87107477db4a92f57743365544885b4973c",
            include_str!("lexical_cursor.rs"),
        ),
        "entire incoming macro and tariffs remain independently compiled",
    );
    // The frozen body is complete production code; only module/test namespace
    // glue and one private-type visibility token differ from the immediate root.
    let mut restored = include_str!("before_identity_prefix.rs").to_owned();
    restored = restored
        .replace("#[path = \"lexical_cursor_before_identity.rs\"]\n", "")
        .replace("#[path = \"paired_value_before_identity.rs\"]\n", "")
        .replacen(
            "pub(super) struct ComparisonOwner<",
            "struct ComparisonOwner<",
            1,
        );
    let old_modules = concat!(
        "#[cfg(test)]\nmod before_flat_view;\n#[cfg(test)]\nmod before_fused_flat;\n",
        "#[cfg(test)]\nmod before_lexical_cursor;\n#[cfg(test)]\nmod before_pair_scope;\n",
        "#[cfg(test)]\n#[rustfmt::skip]\nmod before_prepared_pairing;\n#[cfg(test)]\nmod before_retained_tail;\n",
        "#[cfg(test)]\npub(super) mod pair_scope_census;\n"
    );
    assert_eq!(
        restored.matches("use super::pair_scope_census;\n").count(),
        1
    );
    restored = restored.replacen("use super::pair_scope_census;\n", old_modules, 1);
    restored.push_str("\n#[cfg(test)]\nmod tests;\n");
    let digest = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(
        digest(restored.as_bytes()),
        "4d1b77c0e2af83e3988274975af589b9fef1cfe3b320ae1dd659764b0f2891d6"
    );
    assert_eq!(
        digest(include_bytes!("lexical_cursor_before_identity.rs")),
        "8db6b9a6110b31cdfea17414762febcdd689d9dc8889c7428ed502599f77928b"
    );
    assert_eq!(
        digest(include_bytes!("paired_value_before_identity.rs")),
        "8f9fd563b7aca230f1a68f190973c80958b770ce7ea3ca91e71e80e38df95c0e"
    );
    assert!(original.contains("replay_shared_prefix!(owner, function, budget, census, 'paired)"));
    assert_eq!(
        shared_prefix::OWNER,
        3 * size_of::<usize>() + size_of::<bool>() + 7 * PAYMENT_HEADER
    );
    assert_eq!(
        shared_prefix::DETECTOR,
        6 * size_of::<usize>() + 8 * size_of::<bool>()
    );
}

#[test]
fn shared_identity_replay_typed_oracle_missing_foreign_and_restored_scope_keeps_ambient_unspent() {
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
                                            before_identity_prefix::compare(
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
fn shared_identity_replay_unwind_drops_live_remainder_and_marks_only_reached_prefix() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let (actual, expected) = fixture(16, 1, true, 0);
    pair_scope_census::reset();
    frontiers::reset();
    pair_scope_census::unwind_next_identity_observation();
    let ((unwound, metrics), refused, _, receipt) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "899 unwind receiver"
                )
                .unwrap();
                compare(function, &mut budget, actual, &expected).unwrap();
            }));
            let unwound = outcome.is_err();
            drop(outcome);
            unwound
        })
    });
    assert!(unwound && !refused && receipt.is_none());
    balanced(metrics);
    frontiers::finish();
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].ended && rows[0].unwound && !rows[0].complete);
    assert_eq!(rows[0].identity_hits, 1);
    assert_eq!(rows[0].identity_observations, 1);
    assert_eq!(rows[0].identity_work, 4);
}

#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn shared_identity_replay_opt_in_capture_reports_actual_k_banks_and_complete_teardown() {
    use crate::workflow_profile::plan_comparison::{Binding, Capture, Site};
    use sha2::{Digest, Sha256};
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let capture = Capture::begin(Binding::caller_labelled(
        "899 synthetic shared PLAN arena",
        Sha256::digest(include_bytes!("shared_prefix_tests.rs")).into(),
    ))
    .unwrap();
    let current = run(
        function,
        16,
        1,
        true,
        0,
        false,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
    );
    let report = capture.finish().unwrap();
    assert!(report.counts_complete && report.detail_complete && !report.overflow);
    assert_eq!(
        (
            report.counts.started,
            report.counts.ended,
            report.counts.succeeded
        ),
        (1, 1, 1)
    );
    assert_eq!(
        report.counts.at(Site::IdentityHit),
        current.row.identity_hits as u64
    );
    assert_eq!(
        report.counts.at(Site::IdentityObservation),
        current.row.identity_observations as u64
    );
    assert_eq!(
        report.counts.at(Site::IdentityWork),
        current.row.identity_work as u64
    );
    assert_eq!(
        report.counts.at(Site::IdentityCapture),
        current.row.identity_captures as u64
    );
    assert_eq!(
        report.counts.admitted_bytes[Site::IdentityDetectorAdmitted as usize],
        (current.row.identity_detectors * shared_prefix::DETECTOR) as u64
    );
    assert_eq!(
        report.counts.admitted_bytes[Site::IdentityReplayAdmitted as usize],
        (current.row.identity_observations * shared_prefix::REPLAY) as u64
    );
}
