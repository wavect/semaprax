//! Complete immediate899 versus event-driven first-head suffix controls.
use super::*;
fn event_inputs(length: usize, shape: usize) -> (HistorySet, Vec<SkeletonPath>) {
    let (mut actual, expected) = fixture(length, 1, shape != 1, 0);
    if shape == 2 {
        let values = actual.arena[0].items.clone();
        let split = length / 2;
        let prefix = actual.arena.len();
        actual.arena.push(Segment {
            previous: None,
            items: values[..split].to_vec(),
            depth: 1,
        });
        let suffix = actual.arena.len();
        actual.arena.push(Segment {
            previous: Some(prefix),
            items: values[split..].to_vec(),
            depth: 2,
        });
        actual.paths[1].history.previous = Some(suffix);
        actual.paths[3].history.previous = Some(suffix);
        actual.max_depth = 2;
    } else if shape == 3 {
        let empty = actual.arena.len();
        actual.arena.push(Segment {
            previous: Some(0),
            items: Vec::new(),
            depth: 2,
        });
        actual.paths[1].history.previous = Some(empty);
        actual.paths[3].history.previous = Some(empty);
        actual.max_depth = 2;
    }
    (actual, expected)
}
fn event_run(
    function: &ResolvedFunction,
    length: usize,
    depth: usize,
    old: bool,
    units: usize,
    limit: usize,
    floor: usize,
    hir_shared: bool,
    swaps_only: bool,
) -> Run {
    let (actual, mut expected) = event_inputs(length, depth);
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
                    before_event_prefix::compare(function, &mut budget, actual, &expected)
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
fn event_observe(
    function: &ResolvedFunction,
    length: usize,
    shape: usize,
    old: bool,
    units: usize,
    trace: bool,
) -> Run {
    event_run(
        function,
        length,
        shape,
        old,
        units,
        usize::MAX,
        0,
        false,
        !trace,
    )
}
// Independent full immediate899 typed invoice. These are its actual old banks,
// never replaced with the new event constants or inferred from observed deltas.
fn old_added(r: &pair_scope_census::Row) -> usize {
    let setup = size_of::<before_event_prefix::ComparisonOwner<'static>>()
        - size_of::<before_identity_prefix::ComparisonOwner<'static>>()
        + 3 * size_of::<usize>()
        + size_of::<bool>()
        + 7 * PAYMENT_HEADER;
    let detector = 6 * size_of::<usize>() + 8 * size_of::<bool>();
    let capture = size_of::<usize>();
    let extent = 2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<usize>();
    let body = 2 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<usize>()
        + size_of::<Ordering>()
        + 4 * size_of::<bool>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>();
    setup
        + detector * r.identity_detectors
        + capture * r.identity_captures
        + extent * r.identity_extent_captures
        + body * r.identity_observations
}
#[test]
fn fresh_entry_suffix_has_independent_full_old_new_invoices_and_real_q_g_h() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    for length in [0, 1, 2, 8, 128] {
        for shape in 0..4 {
            let old = event_observe(function, length, shape, true, MAX_REPLAY_WORK_UNITS, false);
            let current =
                event_observe(function, length, shape, false, MAX_REPLAY_WORK_UNITS, false);
            assert_eq!(old.observed.0 .0, current.observed.0 .0);
            assert!(current.observed.0 .0.result.is_ok());
            assert_eq!(old.observed.0 .1, current.observed.0 .1);
            assert_eq!(swaps(&old), swaps(&current));
            assert!(old.row.complete && current.row.complete);
            assert_eq!(current.row.identity_probes, current.row.shared_runs);
            assert_eq!(current.row.identity_bodies, 0);
            assert_eq!(
                current.row.shared_turns,
                current.row.identity_observations + current.row.shared_runs
            );
            assert_eq!(current.row.shared_runs, current.row.shared_publishes);
            assert_eq!(current.row.identity_empty_probes, 0);
            assert_eq!(
                current.row.empty_selectors,
                current.row.identity_hits + current.row.shared_boundary_complete
            );
            assert_eq!(
                current.row.empty_selector_skips + current.row.shared_runs,
                current.row.empty_selectors
            );
            if length == 1 && shape == 0 {
                assert!(current.row.identity_hits > 0);
                assert_eq!(
                    current.row.identity_observations, 0,
                    "same-ID singleton is a paid empty suffix, never G"
                );
            }
            assert_eq!(
                (
                    current.row.identity_captures,
                    current.row.identity_extent_captures
                ),
                (0, 0)
            );
            assert_eq!(
                current.observed.2 + removed(&current.row) + old_added(&old.row),
                old.observed.2 + added(&current.row) + removed(&old.row)
            );
            if length == 0 {
                assert!(
                    current.observed.2 > old.observed.2,
                    "actual added cold sites must remain visible"
                );
            }
            if shape == 1 {
                assert_eq!(
                    current.row.identity_observations, 0,
                    "different-ID equal contents never certify"
                );
            }
            if shape == 0 && length == 128 {
                assert!(current.row.identity_observations > 0);
                assert!(
                    current.observed.2 < old.observed.2,
                    "real long suffix pays all new sites"
                );
            }
            eprintln!(
                "OPT899_EVENT length={length} shape={shape} old899/current={}/{} old_K={} old_D={} old_C={} old_E={} new_Q={} new_G={} new_H={} new_P={} new_B={} new_empty={} old_added={} new_added={} old_removed={} new_removed={}; complete unit comparison only, input construction excluded, full Drop included, no shipping/application claim",
                old.observed.2,
                current.observed.2,
                old.row.identity_observations,
                old.row.identity_detectors,
                old.row.identity_captures,
                old.row.identity_extent_captures,
                current.row.identity_detectors,
                current.row.identity_observations,
                current.row.identity_hits,
                current.row.identity_probes,
                current.row.identity_bodies,
                current.row.identity_empty_probes,
                old_added(&old.row),
                added(&current.row),
                removed(&old.row),
                removed(&current.row)
            );
        }
    }
}
#[test]
fn fresh_entry_suffix_asymmetric_empty_and_every_work_cut_preserve_immediate_six_fields() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    for (length, shape) in [(2, 0), (2, 1), (2, 2), (0, 3), (8, 3)] {
        let complete = event_observe(function, length, shape, true, MAX_REPLAY_WORK_UNITS, false);
        let ordinary_spent = MAX_REPLAY_WORK_UNITS - complete.observed.0 .0.remaining;
        for units in 0..=ordinary_spent + 1 {
            let old = event_observe(function, length, shape, true, units, false);
            let current = event_observe(function, length, shape, false, units, false);
            assert_eq!(
                old.observed.0.0, current.observed.0.0,
                "same six fields, remaining work, and sticky outcome at length={length} shape={shape} cut={units}"
            );
            assert_eq!(swaps(&old), swaps(&current));
        }
    }
}
#[test]
fn fresh_entry_suffix_refused_prefix_prices_admitted_bodies_independently_of_g() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    let complete = event_observe(function, 2, 0, false, MAX_REPLAY_WORK_UNITS, true);
    let mut partial_body = false;
    let ordinary_spent = MAX_REPLAY_WORK_UNITS - complete.observed.0 .0.remaining;
    for units in 0..=ordinary_spent {
        let current = event_observe(function, 2, 0, false, units, true);
        let bytes = |phase| {
            current
                .events
                .iter()
                .filter(|e| e.phase == phase)
                .filter_map(|e| e.after.map(|after| after - e.before))
                .sum::<usize>()
        };
        assert_eq!(
            bytes("ordinary plan fresh segment identity event"),
            current.row.identity_detectors * (2 * size_of::<usize>() + size_of::<bool>())
        );
        assert_eq!(
            bytes("ordinary plan shared suffix pattern probe"),
            current.row.identity_probes * (3 * size_of::<&[Rc<SkeletonObservation>]>())
        );
        assert_eq!(
            bytes("ordinary plan shared suffix scalar receivers"),
            current.row.identity_bodies
                * (2 * size_of::<&SkeletonObservation>()
                    + 2 * size_of::<usize>()
                    + size_of::<Ordering>())
        );
        assert_eq!(
            bytes("ordinary plan certified numeric run capture"),
            current.row.shared_runs * certified_heads::RUN_CAPTURE
        );
        assert_eq!(
            bytes("ordinary plan certified numeric run turn"),
            current.row.shared_turns * certified_heads::RUN_TURN
        );
        assert_eq!(
            bytes("ordinary plan certified numeric run publication"),
            current.row.shared_publishes * certified_heads::RUN_PUBLISH
        );
        assert_eq!(current.row.identity_bodies, 0);
        // Each published run has one paid terminal turn after its last
        // observation. Only an additional turn can identify a work-refused body.
        let completed_turns = current.row.identity_observations + current.row.shared_publishes;
        assert!(current.row.shared_turns >= completed_turns);
        if current.row.complete {
            assert_eq!(current.row.shared_turns, completed_turns);
            assert_eq!(current.row.shared_runs, current.row.shared_publishes);
        }
        if current.row.shared_turns > completed_turns {
            assert!(!current.row.complete && current.observed.0 .0.result.is_err());
            partial_body = true;
        }
    }
    assert!(
        partial_body,
        "a reached paid body must also be observed when its original work refuses"
    );
}

fn main99_run(
    function: &ResolvedFunction,
    length: usize,
    depth: usize,
    units: usize,
    limit: usize,
    floor: usize,
    hir_shared: bool,
    swaps_only: bool,
) -> Run {
    let (actual, mut expected) = event_inputs(length, depth);
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
                super::super::super::before_shared_steps::compare(
                    function,
                    &mut budget,
                    actual,
                    &expected,
                )
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

#[test]
fn fresh_suffix_supersedes_complete_shared_step_generation_without_polling() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut genuine_poll = false;
    for length in [0, 1, 2, 8, 128] {
        for shape in 0..4 {
            for hir_shared in [false, true] {
                let old = main99_run(
                    function,
                    length,
                    shape,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    hir_shared,
                    true,
                );
                let current = event_run(
                    function,
                    length,
                    shape,
                    false,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    hir_shared,
                    true,
                );
                assert_eq!(old.observed.0 .0, current.observed.0 .0);
                assert!(current.observed.0 .0.result.is_ok());
                assert_eq!(swaps(&old), swaps(&current));
                assert_eq!((old.observed.0 .1.live, current.observed.0 .1.live), (0, 0));
                assert!(old.row.complete && current.row.complete);
                assert_eq!(
                    (current.row.shared_patterns, current.row.shared_steps),
                    (0, 0),
                    "the superseded nonempty loaded-item poll is physically absent"
                );
                assert!(
                    current.row.shared_checks >= current.row.shared_metadata,
                    "the distinct loaded-empty boundary checks are actually observed"
                );
                if length > 1 && shape == 0 {
                    assert!(
                        old.row.shared_steps > 0,
                        "genuine incoming loaded-item route executes"
                    );
                    assert!(
                        current.row.identity_observations > 0,
                        "current invokes the real fresh-head suffix, not the incoming oracle"
                    );
                    genuine_poll = true;
                }
                eprintln!(
                    "OPT899_SHARED_GENERATIONS length={length} shape={shape} hir_shared={hir_shared} incoming99_paid={} current_paid={} old_shared_steps={} current_Q={} current_G={} current_H={}; both complete producers, full Drop; separate independent invoices remain owning controls; no application gain claim",
                    old.observed.2,
                    current.observed.2,
                    old.row.shared_steps,
                    current.row.identity_detectors,
                    current.row.identity_observations,
                    current.row.identity_hits
                );
            }
        }
    }
    assert!(genuine_poll);
}
#[test]
fn fresh_suffix_and_complete_shared_step_generation_keep_every_work_refusal_and_swap() {
    let program = super::super::super::super::tests::program();
    let function = &program.functions[0];
    for (length, shape) in [(2, 0), (2, 1), (2, 2), (0, 3), (8, 3)] {
        let complete = main99_run(
            function,
            length,
            shape,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            false,
            true,
        );
        let ordinary_spent = MAX_REPLAY_WORK_UNITS - complete.observed.0 .0.remaining;
        for units in 0..=ordinary_spent + 1 {
            let old = main99_run(function, length, shape, units, usize::MAX, 0, false, true);
            let current = event_run(
                function,
                length,
                shape,
                false,
                units,
                usize::MAX,
                0,
                false,
                true,
            );
            assert_eq!(
                old.observed.0 .0, current.observed.0 .0,
                "six diagnostic fields, sticky result and remaining work: {length}/{shape}/{units}"
            );
            assert_eq!(swaps(&old), swaps(&current));
            assert_eq!((old.observed.0 .1.live, current.observed.0 .1.live), (0, 0));
        }
    }
}
