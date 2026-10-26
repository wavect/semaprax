//! OPT928 owning scalar/complete-immediate controls; execution is pending.
use super::*;

fn observe_countdown(
    function: &ResolvedFunction,
    immediate: bool,
    length: usize,
    shape: usize,
    shared: bool,
    units: usize,
    limit: usize,
    floor: usize,
    hir_shared: bool,
) -> Run {
    // The mode owner is outside the allocation/ledger observation in BOTH arms.
    let mode = certified_heads::countdown_mode(immediate);
    let run = heads_run_generation(
        function, length, 3, shared, 0, 0, units, limit, floor, hir_shared, false, shape,
    );
    drop(mode);
    run
}
fn capture_difference() -> usize {
    certified_heads::RUN_CAPTURE
        .checked_sub(certified_heads::RUN_SCALAR_CAPTURE)
        .unwrap()
}

#[test]
fn countdown_full_typed_banks_keep_the_surviving_independent_driver_paid() {
    let driver = 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 4 * size_of::<usize>()
        + 6 * size_of::<certified_heads::SharedUnitDriver>()
        + 2 * size_of::<&mut certified_heads::SharedUnitDriver>()
        + 2 * size_of::<Option<()>>();
    let scalar =
        2 * size_of::<&[Rc<SkeletonObservation>]>() + 8 * size_of::<usize>() + size_of::<bool>();
    assert_eq!(driver, certified_heads::RUN_CAPTURE);
    assert_eq!(driver, certified_heads::before_countdown::RUN_CAPTURE);
    assert_eq!(scalar, certified_heads::RUN_SCALAR_CAPTURE);
    assert_eq!(
        paired_value::INDEPENDENT_RUN_SETUP,
        driver + 2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<[usize; 8]>()
    );
    assert!(!std::mem::needs_drop::<certified_heads::SharedUnitDriver>());
    assert_eq!(
        certified_heads::OWNER,
        certified_heads::before_countdown::OWNER
    );
    assert_eq!(
        certified_heads::RUN_PUBLISH,
        certified_heads::before_countdown::RUN_PUBLISH
    );
    assert!(
        capture_difference() > 0,
        "typed-source candidate only; no application gain inference"
    );
}

#[test]
fn countdown_whole_cold_zero_singleton_many_callers_keep_exact_work_heap_and_equation() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut captured = false;
    let mut cold = false;
    for (length, shape, shared) in [
        (0, 0, true),
        (1, 0, true),
        (2, 0, true),
        (8, 0, true),
        (17, 1, true),
        (8, 4, true),
        (8, 0, false),
    ] {
        for hir_shared in [false, true] {
            let old = observe_countdown(
                function,
                true,
                length,
                shape,
                shared,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir_shared,
            );
            let current = observe_countdown(
                function,
                false,
                length,
                shape,
                shared,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir_shared,
            );
            assert_eq!(old.observed.0 .0, current.observed.0 .0);
            assert!(
                current.observed.0 .0.result.is_ok() && old.row.complete && current.row.complete
            );
            assert_eq!(old.observed.0 .1, current.observed.0 .1);
            assert_eq!(swaps(&old), swaps(&current));
            assert_eq!(
                (
                    old.row.shared_runs,
                    old.row.shared_turns,
                    old.row.shared_publishes,
                    old.row.identity_observations,
                    old.row.identity_work
                ),
                (
                    current.row.shared_runs,
                    current.row.shared_turns,
                    current.row.shared_publishes,
                    current.row.identity_observations,
                    current.row.identity_work
                )
            );
            assert_eq!(
                current.row.shared_turns,
                current.row.identity_observations + current.row.shared_publishes
            );
            assert_eq!(
                old.observed.2,
                current.observed.2 + current.row.shared_runs * capture_difference(),
                "whole caller invoice, including original controls and payment producers"
            );
            if current.row.shared_runs == 0 {
                assert_eq!(old.observed.2, current.observed.2);
                cold = true;
            } else {
                assert!(current.observed.2 < old.observed.2);
                captured = true;
            }
            eprintln!("OPT928 len={length} shape={shape} shared={shared} HIR={hir_shared} numeric_captures={} scalar_captures={} old_paid={} current_paid={} turns={} observations={} heap_old={:?} heap_current={:?}; no application claim",
                old.row.shared_runs,current.row.shared_runs,old.observed.2,current.observed.2,
                current.row.shared_turns,current.row.identity_observations,old.observed.0.1,current.observed.0.1);
        }
    }
    assert!(
        captured && cold,
        "own real admitted capture and zero-capture cold invocation"
    );
}

#[test]
fn countdown_every_original_work_cut_and_partial_body_keep_first_diagnostic_and_drop() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut partial = false;
    let complete = observe_countdown(
        function,
        true,
        4,
        0,
        true,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
    );
    let spent = MAX_REPLAY_WORK_UNITS - complete.observed.0 .0.remaining;
    for units in 0..=spent + 1 {
        let old = observe_countdown(function, true, 4, 0, true, units, usize::MAX, 0, false);
        let current = observe_countdown(function, false, 4, 0, true, units, usize::MAX, 0, false);
        assert_eq!(
            old.observed.0 .0, current.observed.0 .0,
            "all six fields and exact work cut={units}"
        );
        assert_eq!(old.observed.0 .1, current.observed.0 .1);
        assert_eq!(swaps(&old), swaps(&current));
        assert_eq!(
            (
                old.row.shared_runs,
                old.row.shared_turns,
                old.row.shared_publishes,
                old.row.identity_observations,
                old.row.identity_work
            ),
            (
                current.row.shared_runs,
                current.row.shared_turns,
                current.row.shared_publishes,
                current.row.identity_observations,
                current.row.identity_work
            )
        );
        if current.row.shared_turns
            > current.row.identity_observations + current.row.shared_publishes
        {
            assert!(!current.row.complete && current.observed.0 .0.result.is_err());
            partial = true;
        }
    }
    assert!(partial, "a work-refused scalar body is not a published run");
}

#[test]
fn countdown_capture_exact_short_floor_zero_and_sticky_receipt_are_actual_frontiers() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut reached = 0;
    for immediate in [true, false] {
        let success = observe_countdown(
            function,
            immediate,
            4,
            0,
            true,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            false,
        );
        let bank = if immediate {
            certified_heads::RUN_CAPTURE
        } else {
            certified_heads::RUN_SCALAR_CAPTURE
        };
        let event = success
            .events
            .iter()
            .find(|e| e.phase == "ordinary plan certified numeric run capture" && e.after.is_some())
            .expect("fixture reaches actual capture");
        let after = event.after.unwrap();
        assert_eq!(after - event.before, bank);
        reached += 1;
        for (limit, floor) in [
            (0, 0),
            (event.before + bank - 1, 0),
            (event.before + bank, 0),
            (success.observed.2 - 1, 0),
            (success.observed.2, 0),
            (success.observed.2 + 7, 7),
        ] {
            let run = observe_countdown(
                function,
                immediate,
                4,
                0,
                true,
                MAX_REPLAY_WORK_UNITS,
                limit,
                floor,
                false,
            );
            if run.observed.1 {
                assert!(run.observed.0 .0.result.is_err());
            }
            if let Some(retry) = run.observed.0 .0.retry.as_ref() {
                assert_eq!(Some(retry), run.observed.0 .0.result.as_ref().err());
            }
            balanced(run.observed.0 .1);
        }
    }
    assert_eq!(reached, 2);
}

#[test]
fn countdown_both_complete_generations_unwind_with_reached_prefix_and_mode_restoration() {
    frontiers::prepare();
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    for immediate in [true, false] {
        let mode = certified_heads::countdown_mode(immediate);
        let (actual, expected) = fixture(16, 1, true, 0);
        pair_scope_census::reset();
        frontiers::reset();
        pair_scope_census::unwind_next_identity_observation();
        let ((unwound, metrics), refused, _, receipt) =
            with_limit_usage_refusal(usize::MAX, || {
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
        assert_eq!(
            (
                rows[0].identity_hits,
                rows[0].identity_observations,
                rows[0].identity_work
            ),
            (1, 1, 4)
        );
        drop(mode);
        assert!(!certified_heads::immediate_countdown_selected());
    }
}

#[test]
fn countdown_full_immediate_module_inverse_and_five_work_sites_are_complete() {
    use sha2::{Digest, Sha256};
    let frozen = include_str!("certified_heads/before_countdown.rs");
    let (body, export) = frozen.split_once("\n// TEST_ONLY_928_EXPORT\n").unwrap();
    assert_eq!(
        export,
        "pub(in super::super) use certified_suffix_before_countdown;\n"
    );
    let restored = body
        .replace("certified_heads::before_countdown::", "certified_heads::")
        .replace(
            "macro_rules! certified_suffix_before_countdown {",
            "macro_rules! certified_suffix {",
        )
        .replace("pub(in super::super)", "pub(super)")
        .replace(
            "super::super::paired_value::EMPTY",
            "super::paired_value::EMPTY",
        );
    assert_eq!(restored.len(), 34368);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "7f273a44e338d7606d1fe55f51468d3e34181df656f0ea2e67678b82d2aee2ba"
    );
    // Both routes execute this complete identical production caller; the mode
    // selects only the independently compiled suffix at the original frontier.
    let caller = include_str!("../comparison_owner.rs");
    assert_eq!(caller.len(), 29922);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(caller.as_bytes()))
        ),
        "6b8839aa7294a09ffc95f9cbc9d04fc42b6e51cd611e8bbfca1bc0bb4e88dab4"
    );
    let current = include_str!("certified_heads.rs");
    let suffix = |s: &str| {
        s.split_once("// BEGIN CERTIFIED SUFFIX\n")
            .unwrap()
            .1
            .split_once("// Both cursors have already exhausted")
            .unwrap()
            .0
            .to_owned()
    };
    let old = suffix(&restored);
    let new = suffix(current);
    let sites = |s: &str| {
        s.lines()
            .filter(|line| line.contains("$budget.charge($function, 1,"))
            .map(|line| line.trim().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(sites(&old), sites(&new));
    assert_eq!(sites(&new).len(), 5);
    assert!(new.contains("if shared_remaining == 0") && new.contains("break;"));
    assert_eq!(new.matches("shared_remaining -= 1;").count(), 1);
    assert!(!new.contains("shared_work_driver") && !new.contains("checked_sub"));
    assert_eq!(new.matches("paid!(").count(), old.matches("paid!(").count());
    let paired = include_str!("paired_value.rs");
    assert!(paired.contains("certified_heads::RUN_CAPTURE +"));
    assert!(paired.contains("let paired_work_driver: certified_heads::SharedUnitDriver"));
}
