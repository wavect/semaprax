//! Whole immediate/current callers, not a model in place of either algorithm.
use super::*;

struct Mode;
impl Drop for Mode {
    fn drop(&mut self) {
        IMMEDIATE_BRANCH_FRAMES.with(|mode| mode.set(false));
    }
}
fn select_mode(old: bool) -> Mode {
    IMMEDIATE_BRANCH_FRAMES.with(|mode| assert!(!mode.replace(old)));
    Mode
}
fn immediate(
    input: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
    floor: usize,
    lent: bool,
) -> Run {
    let _mode = select_mode(true);
    run(true, input, function, work, bytes, floor, lent)
}
fn current(
    input: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
    floor: usize,
    lent: bool,
) -> (Run, [usize; 4]) {
    BRANCH_FRAME_EVENTS.with(|events| events.set([0; 4]));
    let result = run(true, input, function, work, bytes, floor, lent);
    (result, BRANCH_FRAME_EVENTS.with(std::cell::Cell::get))
}
fn bank_equation(old: &Run, new: &Run, events: [usize; 4]) {
    same_work(old, new);
    assert_eq!(old.heap, new.heap);
    let [attempts, roots, sequences, nodes] = events;
    let exhausted = attempts.checked_sub(roots + sequences + nodes).unwrap();
    let old_frames = attempts.checked_mul(FRAME_STEP).unwrap();
    let new_frames =
        attempts * FRAME_COMMON + roots * ROOT_FRAME + sequences * SEQ_FRAME + nodes * NODE_FRAME;
    assert_eq!(
        old.paid.checked_sub(old_frames).unwrap() + new_frames,
        new.paid
    );
    assert_eq!(
        old_frames - new_frames,
        roots * (FRAME_STEP - FRAME_COMMON - ROOT_FRAME)
            + sequences * (FRAME_STEP - FRAME_COMMON - SEQ_FRAME)
            + nodes * (FRAME_STEP - FRAME_COMMON - NODE_FRAME)
            + exhausted * (FRAME_STEP - FRAME_COMMON)
    );
    assert!(new.paid <= old.paid);
}

#[test]
fn complete_branch_bank_map_pays_new_producer_before_each_constructor() {
    // Preserve every former common role. PAYMENT is the new branch reserve's
    // actual/formal arguments and two Result controls, renewed before pop.
    let common_without_new_producer = 2 * PAYMENT
        + 2 * size_of::<&mut Vec<Frame>>()
        + 2 * size_of::<&mut Option<Frame>>()
        + 2 * size_of::<Frame>()
        + 2 * size_of::<Option<Frame>>()
        + 4 * size_of::<Rc<ObservationDag>>()
        + size_of::<[usize; 8]>()
        + 8 * size_of::<bool>();
    assert_eq!(FRAME_COMMON, common_without_new_producer + PAYMENT);
    assert_eq!(
        FRAME_STEP,
        common_without_new_producer + NODE_FRAME + ROOT_FRAME
    );
    assert_eq!(SEQ_FRAME, ROOT_FRAME + 4 * size_of::<&ObservationDagKind>());
    assert!(FRAME_COMMON + ROOT_FRAME < FRAME_STEP);
    assert!(FRAME_COMMON + SEQ_FRAME <= FRAME_STEP);
    assert!(FRAME_COMMON + NODE_FRAME <= FRAME_STEP);
    assert!(FRAME_COMMON < FRAME_STEP);
    if size_of::<usize>() == 8 {
        assert_eq!(
            (FRAME_STEP, FRAME_COMMON, ROOT_FRAME, SEQ_FRAME, NODE_FRAME),
            (776, 616, 128, 160, 160)
        );
    }
}

#[test]
fn complete_immediate_sort_counts_all_branch_events_and_discloses_cold_totals() {
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 2, 3, 12, 32] {
        for width in [0, 1, 17] {
            let paths = fixture(function, count, width);
            let old = immediate(&paths, function, usize::MAX, usize::MAX, 0, false);
            let (new, events) = current(&paths, function, usize::MAX, usize::MAX, 0, false);
            bank_equation(&old, &new, events);
            if count == 32 && width == 17 {
                assert!(
                    events[1] > 0,
                    "the whole fixture must reach root-item frames"
                );
                assert!(
                    new.paid < old.paid,
                    "whole immediate/current invocation must improve"
                );
            }
            eprintln!("OPT926 count={count} width={width} events={events:?} old_paid={} current_paid={} old_heap={:?} current_heap={:?} old_result={:?} current_result={:?} work={}/{}/{}", old.paid, new.paid, old.heap, new.heap, old.result, new.result, new.remaining, new.skeleton_remaining, new.skeleton_spent);
            let exact = current(&paths, function, usize::MAX, new.paid, 0, false).0;
            same_work(&new, &exact);
            if new.paid != 0 {
                let short = current(&paths, function, usize::MAX, new.paid - 1, 0, false).0;
                assert!(short.result.is_err());
                assert!(short.refused || short.refusal_spent);
                assert!(short.sticky.is_some());
            }
            let same_cap = current(&paths, function, usize::MAX, old.paid, 0, false).0;
            same_work(&old, &same_cap);
        }
    }
}

#[test]
fn immediate_and_current_keep_every_work_cut_and_lent_first_diagnostic() {
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 2, 7] {
        let paths = fixture(function, count, 3);
        let old = immediate(&paths, function, usize::MAX, usize::MAX, 0, false);
        for work in (0..=usize::MAX - old.remaining + 1).chain([usize::MAX]) {
            same_work(
                &immediate(&paths, function, work, usize::MAX, 0, false),
                &current(&paths, function, work, usize::MAX, 0, false).0,
            );
        }
        same_work(
            &immediate(&paths, function, usize::MAX, usize::MAX, 0, true),
            &current(&paths, function, usize::MAX, usize::MAX, 0, true).0,
        );
    }
}

#[test]
fn changed_physical_frontier_keeps_sticky_floor_zero_and_malformed_cleanup() {
    let program = program();
    let function = &program.functions[0];
    let mut paths = fixture(function, 2, 3);
    for malformed in [false, true] {
        if malformed {
            paths[0].observations.prefix = Some(dag(ObservationDagKind::Alt(Vec::new())));
        }
        let old = immediate(&paths, function, usize::MAX, usize::MAX, 0, false);
        let (new, _) = current(&paths, function, usize::MAX, usize::MAX, 0, false);
        same_work(&old, &new);
        for bytes in [
            0,
            1,
            FRAME_COMMON - 1,
            FRAME_COMMON,
            FRAME_STEP,
            new.paid.saturating_sub(1),
            new.paid,
        ] {
            for floor in [0, 1, bytes] {
                if floor > bytes {
                    // A floor cannot exceed the whole allowance. Keep this
                    // rejected setup distinct from a paid sort refusal.
                    let ((), refused, paid, receipt) = with_limit_usage_refusal(bytes, || {
                        assert!(!crate::bounded_output::set_active_floor(floor));
                        assert_eq!(crate::bounded_output::active_remaining(), Some(bytes));
                        assert_eq!(
                            crate::bounded_output::active_usable_remaining(),
                            Some(bytes)
                        );
                        assert!(!crate::bounded_output::active_refused());
                    });
                    assert!(!refused);
                    assert_eq!(paid, 0);
                    assert!(receipt.is_none());
                    continue;
                }
                let result = current(&paths, function, usize::MAX, bytes, floor, false).0;
                if result.refused || result.refusal_spent {
                    assert!(result.sticky.is_some());
                }
                assert!(!result.heap.invalid);
                // Byte cuts intentionally need not select the old frame label:
                // the common pop precedes the new branch payment.
                eprintln!("OPT926 physical malformed={malformed} bytes={bytes} floor={floor} result={:?} paid={} heap={:?} sticky={:?}", result.result, result.paid, result.heap, result.sticky);
            }
        }
    }
}

#[test]
fn whole_cold_invocation_and_unwind_balance_all_input_and_cursor_owners() {
    let program = program();
    let function = &program.functions[0];
    let cold = |old| {
        let _mode = select_mode(old);
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut paths = fixture(function, 12, 17);
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::sort(
                    &mut paths,
                    function,
                    &mut budget,
                )
                .unwrap();
                drop(paths);
                drop(budget);
            })
        })
    };
    let (((), old), or, op, _) = cold(true);
    let (((), new), nr, np, _) = cold(false);
    assert!(!or && !nr && !old.invalid && !new.invalid);
    assert_eq!(old.live, 0);
    assert_eq!(new.live, 0);
    assert_eq!(old, new);
    assert!(np <= op);
    eprintln!("OPT926 full-drop old_paid={op} current_paid={np} old={old:?} current={new:?}");
    for bytes in [0, FRAME_COMMON - 1, np.saturating_sub(1), np] {
        for old in [true, false] {
            let _mode = select_mode(old);
            let (((), heap), _, _, _) = with_limit_usage_refusal(bytes, || {
                measure(|| {
                    let mut paths = fixture(function, 12, 17);
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let result =
                        crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::sort(
                            &mut paths,
                            function,
                            &mut budget,
                        );
                    drop(result);
                    drop(paths);
                    drop(budget);
                })
            });
            assert!(!heap.invalid);
            assert_eq!(
                heap.live, 0,
                "partial refusal must release all original roots too"
            );
        }
    }
    let paths = fixture(function, 2, 3);
    let root = paths[0].observations.prefix.as_ref().unwrap();
    let count = Rc::strong_count(root);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        budget
            .reserve_compact_bytes(
                function,
                RECEIVERS + PREPARE_CALL + ADVANCE_CALL,
                "test branch unwind complete receiving bank",
            )
            .unwrap();
        let mut pair = Pair::new();
        pair.left
            .prepare(&paths[0].observations, function, &mut budget)
            .unwrap();
        pair.left
            .next(&paths[0].observations, function, &mut budget)
            .unwrap();
        panic!("branch frame unwind");
    }));
    assert!(result.is_err());
    assert_eq!(Rc::strong_count(root), count);
}

#[test]
fn full_immediate_source_body_and_all_old_controls_are_preserved() {
    let captured = include_str!("before_branch_frames.rs.txt");
    assert_eq!(captured.len(), 15369);
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(captured.as_bytes()))
        ),
        "56a40d51a09c3bf21f7b4683aee142dbe29524c2c97cac97e685bf2773747549"
    );
    let compiled = include_str!("before_branch_frames.rs");
    let body = compiled
        .split_once("\n// TEST_ONLY_926_EXACT_BANK_ACCESS\n")
        .unwrap()
        .0;
    let original = captured.split_once("\n#[cfg(test)]\n#[allow(").unwrap().0;
    assert_eq!(
        body.replace("use super::super::*;", "use super::*;")
            .replace(
                "pub(in crate::cleanup_plan::replay::skeleton_work::enumerated_prefix) fn sort(",
                "pub(super) fn sort("
            ),
        original
    );
    assert!(original.contains(
        "FRAME_STEP,\n                \"enumerated HIR cursor frame receiving renewal\""
    ));
    assert!(original.contains("CAT_CHILD_STEP"));
    assert!(original.contains("NODE_STEP"));
}
