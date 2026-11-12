//! Exact post-953 and current invoices through both real sorting entries.
use super::*;

fn assert_retained_contents(
    paths: &[SkeletonPath],
    expected: &[(Vec<SkeletonObservation>, SkeletonTerminal)],
) {
    fn item(actual: &Observation, expected: &mut &[SkeletonObservation]) {
        let (first, rest) = expected.split_first().expect("next retained observation");
        assert_eq!(actual.as_ref(), first);
        *expected = rest;
    }
    fn prefix(node: &ObservationDag, expected: &mut &[SkeletonObservation]) {
        match &node.kind {
            ObservationDagKind::Seq(parent, items) => {
                if let Some(parent) = parent {
                    prefix(parent, expected);
                }
                for value in items {
                    item(value, expected);
                }
            }
            ObservationDagKind::Cat(children) | ObservationDagKind::Alt(children) => {
                for child in children {
                    prefix(child, expected);
                }
            }
        }
    }
    assert_eq!(paths.len(), expected.len());
    for (path, (observations, terminal)) in paths.iter().zip(expected) {
        assert_eq!(path.terminal, *terminal);
        let mut remaining = observations.as_slice();
        if let Some(node) = &path.observations.prefix {
            prefix(node, &mut remaining);
        }
        for value in &path.observations.items {
            item(value, &mut remaining);
        }
        assert!(remaining.is_empty());
    }
}

fn observe(
    before: bool,
    paths: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
    floor: usize,
    lent: bool,
) -> (Run, [usize; 4]) {
    let _epoch = frame_renewal_epoch(before);
    BRANCH_FRAME_EVENTS.with(|events| events.set([0; 4]));
    let ((result, snapshot, state, sticky), refused, paid, first) =
        with_limit_usage_refusal(bytes, || {
            let mut input = paths.to_vec();
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            assert!(matches!(budget.payer, AllocationPayer::ActiveBuilder));
            budget.remaining = work;
            budget.lent_skeleton = lent;
            assert!(crate::bounded_output::set_active_floor(floor));
            let result = super::super::super::sort(&mut input, function, &mut budget);
            let state = (
                budget.remaining,
                budget.skeleton_remaining,
                budget.skeleton_spent,
                budget.refusal_spent,
            );
            let sticky =
                (budget.refusal_spent || crate::bounded_output::active_refused()).then(|| {
                    budget
                        .reserve_compact_bytes(function, 0, "post-sort sticky control")
                        .unwrap_err()
                        .into_diagnostic()
                });
            (result, contents(&input), state, sticky)
        });
    let events = BRANCH_FRAME_EVENTS.with(std::cell::Cell::get);

    // Include the returned diagnostic, the prepaid refusal buffer and any
    // flat output backing transferred to the caller's path slice.
    BRANCH_FRAME_EVENTS.with(|events| events.set([0; 4]));
    let (settled, settled_refused, settled_paid, settled_first) =
        with_limit_usage_refusal(bytes, || {
            let mut input = paths.to_vec();
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            assert!(matches!(budget.payer, AllocationPayer::ActiveBuilder));
            budget.remaining = work;
            budget.lent_skeleton = lent;
            assert!(crate::bounded_output::set_active_floor(floor));
            let ((ok, remaining, skeleton_remaining, skeleton_spent, refusal_spent), heap) =
                measure(|| {
                    let ok = super::super::super::sort(&mut input, function, &mut budget).is_ok();
                    let state = (
                        ok,
                        budget.remaining,
                        budget.skeleton_remaining,
                        budget.skeleton_spent,
                        budget.refusal_spent,
                    );
                    if budget.refusal_spent || crate::bounded_output::active_refused() {
                        let settled_sticky = budget
                            .reserve_compact_bytes(function, 0, "post-sort sticky control")
                            .unwrap_err()
                            .into_diagnostic();
                        assert_eq!(Some(&settled_sticky), sticky.as_ref());
                    }
                    assert_retained_contents(&input, &snapshot);
                    drop(input);
                    drop(budget);
                    state
                });
            assert_eq!(ok, result.is_ok());
            assert_eq!(
                (remaining, skeleton_remaining, skeleton_spent, refusal_spent),
                state
            );
            heap
        });
    assert_eq!(
        (settled_refused, settled_paid, settled_first),
        (refused, paid, first)
    );
    assert_eq!(BRANCH_FRAME_EVENTS.with(std::cell::Cell::get), events);
    clean(settled);
    // Diagnostic serialization has its own builder tariff. Preserve the full
    // diagnostic here, after both replay accounts have produced their receipts.
    (
        Run {
            result: result.map_err(|error| error.json()),
            paths: snapshot,
            remaining: state.0,
            skeleton_remaining: state.1,
            skeleton_spent: state.2,
            refusal_spent: state.3,
            refused,
            paid,
            heap: settled,
            sticky: sticky.map(|error| error.json()),
        },
        events,
    )
}

fn clean(heap: Metrics) {
    assert!(!heap.invalid);
    assert_eq!(heap.live, 0);
    assert_eq!(heap.allocated, heap.freed);
}

#[test]
fn every_paid_frame_including_exhaustion_has_the_exact_typed_reduction() {
    let removed_arguments = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>();
    assert_eq!(PAYMENT - POSITIVE_BYTE_PAYMENT, removed_arguments);
    assert_eq!(FRAME_COMMON - LEXICAL_FRAME_COMMON, 2 * removed_arguments);
    let program = program();
    let function = &program.functions[0];
    let mut reached = [0; 4];
    let mut exhausted = 0;
    for (count, width) in [(0, 0), (1, 0), (2, 0), (3, 4), (12, 17), (32, 1), (128, 1)] {
        let paths = fixture(function, count, width);
        for work in [usize::MAX, 0, 1, 3, 17, 64] {
            let (old, old_events) = observe(true, &paths, function, work, usize::MAX, 0, false);
            let (new, events) = observe(false, &paths, function, work, usize::MAX, 0, false);
            same_work(&old, &new);
            assert_eq!(old.heap, new.heap);
            clean(new.heap);
            assert_eq!(old_events, events);
            assert_eq!(
                old.paid,
                new.paid + events[0] * (FRAME_COMMON - LEXICAL_FRAME_COMMON)
            );
            if count < 2 {
                assert_eq!(events, [0; 4]);
            }
            if work == usize::MAX {
                exhausted += events[0] - events[1..].iter().sum::<usize>();
                for (total, count) in reached.iter_mut().zip(events) {
                    *total += count;
                }
            }
        }
    }
    assert!(reached.iter().all(|&count| count > 0));
    assert!(exhausted > 0);
}

#[test]
fn exact_short_zero_floor_lent_and_expired_accounts_keep_their_refusals() {
    use crate::cleanup_plan::replay::allocation::OracleScope;
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 3, 4);
    for before in [true, false] {
        let (complete, _) = observe(before, &paths, function, usize::MAX, usize::MAX, 0, false);
        assert!(complete.result.is_ok());
        for floor in [0, 17] {
            let bytes = complete.paid + floor;
            let (exact, _) = observe(before, &paths, function, usize::MAX, bytes, floor, false);
            same_work(&complete, &exact);
            assert_eq!(complete.paid, exact.paid);
            let (short, _) = observe(
                before,
                &paths,
                function,
                usize::MAX,
                bytes - 1,
                floor,
                false,
            );
            assert!(short.result.is_err());
            assert!(short.refused || short.refusal_spent);
            assert!(short.sticky.is_some());
            clean(short.heap);
        }
        let (zero, events) = observe(before, &paths, function, usize::MAX, 0, 0, false);
        assert!(zero.result.is_err() && zero.sticky.is_some());
        assert_eq!(events, [0; 4]);
    }
    let (old, _) = observe(true, &paths, function, usize::MAX, usize::MAX, 0, true);
    let (new, _) = observe(false, &paths, function, usize::MAX, usize::MAX, 0, true);
    same_work(&old, &new);

    let expired = |before| {
        let _epoch = frame_renewal_epoch(before);
        let owner = OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, owner.lease());
        assert!(owner.finish().first_refusal.is_none());
        let foreign = OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
        let receipt = foreign.receipt();
        let mut paths = paths.clone();
        let failure = super::super::super::sort(&mut paths, function, &mut budget).unwrap_err();
        let sticky = budget
            .reserve_compact_bytes(function, 0, "foreign frame renewal retry")
            .unwrap_err()
            .into_diagnostic();
        assert_eq!(foreign.finish(), receipt);
        (failure, sticky, contents(&paths))
    };
    assert_eq!(expired(true), expired(false));
}

#[test]
fn whole_shipping_caller_preserves_every_output_diagnostic_and_paid_frame() {
    use crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping;
    let checked = crate::check(
        "module cursor.renewal; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}",
        "cursor-renewal.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = &program.functions[0];
    let input = positive_bytes::shipping_input(&program, function);
    assert!(input.len() > 1);
    let invoke = |before, fault, work| {
        let _epoch = frame_renewal_epoch(before);
        let mut paths = input.clone();
        match fault {
            0 => (),
            1 => {
                paths.pop();
            }
            2 => paths[0].terminal = SkeletonTerminal::LoopLatch,
            3 => paths[0].observations.items.push(item(function, false)),
            _ => unreachable!(),
        }
        let mut settled_paths = paths.clone();
        BRANCH_FRAME_EVENTS.with(|events| events.set([0; 4]));
        let (observed, refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            budget.remaining = work;
            let result = shipping::compare(function, &mut budget, &mut paths);
            (
                result,
                contents(&paths),
                budget.remaining,
                budget.skeleton_spent,
            )
        });
        let frames = BRANCH_FRAME_EVENTS.with(std::cell::Cell::get);

        BRANCH_FRAME_EVENTS.with(|events| events.set([0; 4]));
        let (heap, settled_refused, settled_paid, settled_first) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::with_skeleton_limit(work);
                budget.remaining = work;
                let ((ok, remaining, skeleton_spent), heap) = measure(|| {
                    let ok = shipping::compare(function, &mut budget, &mut settled_paths).is_ok();
                    let state = (ok, budget.remaining, budget.skeleton_spent);
                    assert_retained_contents(&settled_paths, &observed.1);
                    drop(settled_paths);
                    drop(budget);
                    state
                });
                assert_eq!(ok, observed.0.is_ok());
                assert_eq!(remaining, observed.2);
                assert_eq!(skeleton_spent, observed.3);
                heap
            });
        assert_eq!(
            (settled_refused, settled_paid, settled_first),
            (refused, paid, first)
        );
        assert_eq!(BRANCH_FRAME_EVENTS.with(std::cell::Cell::get), frames);
        clean(heap);
        ((observed, refused, first, heap), paid, frames)
    };
    for fault in 0..4 {
        for work in [usize::MAX, 0, 1, 3, 17, 64] {
            let (old, old_paid, old_frames) = invoke(true, fault, work);
            let (new, paid, frames) = invoke(false, fault, work);
            assert_eq!(old, new);
            assert_eq!(old_frames, frames);
            assert_eq!(
                old_paid,
                paid + frames[0] * (FRAME_COMMON - LEXICAL_FRAME_COMMON)
            );
            if work == usize::MAX {
                assert!(frames[0] > 0);
                assert_eq!(new.0 .0.is_ok(), fault == 0);
            }
        }
    }
}

#[test]
fn paid_frame_unwind_releases_all_cursors_and_owned_inputs() {
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) {
            FRAME_UNWIND.with(|slot| slot.set(self.0));
        }
    }
    let program = program();
    let function = &program.functions[0];
    for before in [true, false] {
        let _epoch = frame_renewal_epoch(before);
        for cut in [0, 1, 3, 7] {
            let (((), heap), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut paths = fixture(function, 3, 4);
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let _reset = Reset(FRAME_UNWIND.with(|slot| slot.replace(Some(cut))));
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        super::super::super::sort(&mut paths, function, &mut budget)
                    }));
                    assert!(result.is_err());
                    drop(result);
                    drop(paths);
                    drop(budget);
                })
            });
            assert!(!refused && first.is_none());
            clean(heap);
        }
    }
}

#[test]
fn whole_immediate_owner_and_executable_predecessor_are_source_bound() {
    use crate::source_reflows_merge::main_e177_final_inputs::declared_history_inputs as history;
    const PATH: &str = "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs";
    const BEFORE_SHA: &str = "9b0a44837b96345bec274eae1594041194ac6cee80b7d3861216e88a79b29d81";
    let current = include_str!("../receiving_sort.rs");
    let before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/9b0a44837b96345bec274eae1594041194ac6cee80b7d3861216e88a79b29d81"
    ));
    assert_eq!(
        history::expected_value(PATH, BEFORE_SHA, current).as_deref(),
        Some(before)
    );
    let retained = include_str!("before_frame_renewal.rs");
    let prefix = before.split_once("\n#[cfg(test)]\n#[allow(\n").unwrap().0;
    let visible = "pub(in crate::cleanup_plan::replay::skeleton_work::enumerated_prefix) fn sort(";
    assert_eq!(retained.matches(visible).count(), 1);
    assert_eq!(
        retained
            .strip_suffix('\n')
            .unwrap()
            .replacen(visible, "pub(super) fn sort(", 1),
        prefix
    );
}
