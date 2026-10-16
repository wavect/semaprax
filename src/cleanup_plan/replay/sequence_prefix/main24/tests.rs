use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::cell::Cell;
thread_local! { static ORIGINAL: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(Cell::get)
}
fn with_original<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL.with(|flag| flag.replace(true)));
    run()
}
fn minimum_items() -> usize {
    MINIMUM_ITEMS
}
// Direct physical/pointer controls use the same admission and transfer as the
// live arm. Full receiver tests below execute that arm itself, not this adapter.
fn try_take(
    prefix: &mut Observations,
    suffix: &Observations,
    suffix_count: usize,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<Option<Observations>, Diagnostic> {
    if !can_take(Candidate {
        prefix,
        suffix,
        suffix_count,
        budget,
    }) {
        return Ok(None);
    }
    budget.charge_skeleton(function, 1, phase)?;
    budget.reserve_array::<u8>(function, RECEIVING_BYTES, phase)?;
    note_skeleton_materialization();
    Ok(Some(std::mem::take(prefix)))
}
fn program() -> ResolvedProgram {
    let source =
        "module prefix_move; @id(\"f\") fn f()->i64 {0} @id(\"main\") fn main()->i64 {f()}";
    let checked = crate::check(source, "prefix-move.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "prefix-move.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn seed(
    count: usize,
    capacity: usize,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<ExprSkeletonPath, Diagnostic> {
    let mut path = empty_expr_path();
    work.budget.reserve_vec(
        work.function,
        &mut path.observations.items,
        capacity,
        "prefix fixture array",
    )?;
    for n in 0..count {
        let expression = work.clone_owned(&work.function.body.id, "prefix fixture identity")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Status {
                source: StatusSourceId {
                    expression,
                    lane: StatusLane::OperationFailure,
                },
                success: n % 2 == 0,
            },
            "prefix fixture observation",
        )?;
    }
    Ok(path)
}
fn same(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(a.observations, b.observations);
        assert_eq!(a.owned_source, b.owned_source);
        assert_eq!(a.failed, b.failed);
        assert_eq!(a.residual, b.residual);
        assert_eq!(a.continuation, b.continuation);
    }
}
#[test]
fn complete_original_sequence_keeps_order_path_fields_work_and_materializations() {
    let program = program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [
                0,
                1,
                minimum_items() - 1,
                minimum_items(),
                minimum_items() + 9,
            ] {
                for fanout in [0, 1, 2] {
                    for stopped in [0, 1, 2] {
                        let mut old_budget =
                            ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        let capacity = (count + 8).next_power_of_two();
                        let mut old_prefix = seed(count, capacity, &mut old_work).unwrap();
                        let mut prefix = seed(count, capacity, &mut work).unwrap();
                        old_prefix.failed = stopped == 1;
                        prefix.failed = stopped == 1;
                        old_prefix.residual = stopped == 2;
                        prefix.residual = stopped == 2;
                        let mut old_inputs = Vec::new();
                        let mut inputs = Vec::new();
                        old_work
                            .push_expr_path(&mut old_inputs, old_prefix, "prefix fixture path")
                            .unwrap();
                        work.push_expr_path(&mut inputs, prefix, "prefix fixture path")
                            .unwrap();
                        let mut old_suffix = Vec::new();
                        let mut suffix = Vec::new();
                        for n in 0..fanout {
                            let mut a = seed(n + 1, n + 1, &mut old_work).unwrap();
                            let mut b = seed(n + 1, n + 1, &mut work).unwrap();
                            a.owned_source =
                                Some(CleanupPlace::whole(StorageId::ProvisionalResult));
                            b.owned_source =
                                Some(CleanupPlace::whole(StorageId::ProvisionalResult));
                            old_work
                                .push_expr_path(&mut old_suffix, a, "suffix fixture path")
                                .unwrap();
                            work.push_expr_path(&mut suffix, b, "suffix fixture path")
                                .unwrap();
                        }
                        let before = skeleton_materializations();
                        let expected =
                            before::sequence_skeleton_paths(old_inputs, &old_suffix, &mut old_work)
                                .unwrap();
                        let old_materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = sequence_skeleton_paths(inputs, &suffix, &mut work).unwrap();
                        assert_eq!(skeleton_materializations() - before, old_materializations);
                        same(&actual, &expected);
                        assert_eq!(work.budget.remaining, old_work.budget.remaining);
                        assert_eq!(
                            work.budget.skeleton_remaining,
                            old_work.budget.skeleton_remaining
                        );
                    }
                }
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid);
}
#[test]
fn transfer_reuses_full_backing_and_preserves_each_observation_identity() {
    let program = program();
    let function = &program.functions[0];
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let count = minimum_items() + 9;
            let mut input = seed(count, count + 16, &mut work).unwrap().observations;
            let suffix = Observations::default();
            let pointer = input.items.as_ptr();
            let capacity = input.items.capacity();
            let before = work.budget.skeleton_remaining;
            let materialized = skeleton_materializations();
            let out = try_take(&mut input, &suffix, 1, function, work.budget, "move test")
                .unwrap()
                .unwrap();
            assert!(input.items.is_empty() && input.prefix.is_none());
            assert_eq!(out.items.as_ptr(), pointer);
            assert_eq!(out.items.capacity(), capacity);
            assert_eq!(out.items.len(), count);
            for (n, item) in out.items.iter().enumerate() {
                assert!(
                    matches!(item.as_ref(),SkeletonObservation::Status{source,success}
                if source.expression==function.body.id&&source.lane==StatusLane::OperationFailure&&*success==(n%2==0))
                );
            }
            assert_eq!(work.budget.skeleton_remaining, before - 1);
            assert_eq!(skeleton_materializations(), materialized + 1);
            drop(out);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}
#[test]
fn original_receiver_source_remains_complete() {
    use sha2::{Digest, Sha256};
    let source = include_str!("before.rs");
    let original = source.split_once("pub(super) ").unwrap().1;
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "74f2835c0420877dd4a0accb6b6440a92ac6a8cb6f12793a44b5b0438ce43c6e"
    );
}

#[test]
fn complete_transfer_receiver_pays_before_allocation_at_zero_exact_and_short_limits() {
    let program = program();
    let function = &program.functions[0];
    let observe = |original, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let result = (|| {
                    let mut work = SkeletonWork::with_index(&program, function, &mut budget, None)?;
                    let count = minimum_items();
                    let input = seed(count, count + 16, &mut work)?;
                    let mut inputs = Vec::new();
                    work.push_expr_path(&mut inputs, input, "finite prefix")?;
                    let tail = seed(3, 3, &mut work)?;
                    let mut suffixes = Vec::new();
                    work.push_expr_path(&mut suffixes, tail, "finite suffix")?;
                    let output = if original {
                        before::sequence_skeleton_paths(inputs, &suffixes, &mut work)?
                    } else {
                        sequence_skeleton_paths(inputs, &suffixes, &mut work)?
                    };
                    assert_eq!(output.len(), 1);
                    assert_eq!(output[0].observations.items.len(), count + 3);
                    let count = output[0].observations.items.len();
                    drop(output);
                    Ok::<_, Diagnostic>(count)
                })();
                let out = result.as_ref().ok().copied();
                drop(result);
                let logical = (
                    budget.remaining,
                    budget.skeleton_remaining,
                    skeleton_materializations() - before,
                );
                drop(budget);
                (out, logical)
            })
        })
    };
    let ((expected, old_alloc), old_refused, old_paid, first) = observe(true, usize::MAX);
    assert!(!old_refused && first.is_none());
    let ((actual, new_alloc), refused, exact, first) = observe(false, usize::MAX);
    assert!(!refused && first.is_none());
    assert_eq!(actual, expected);
    assert!(new_alloc.allocated < old_alloc.allocated);
    assert!(exact < old_paid);
    for limit in [0, exact - 1, exact] {
        let ((out, alloc), refused, paid, first) = observe(false, limit);
        assert_eq!(out.0.is_some(), limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_some(), limit != exact);
        assert!(!alloc.invalid && alloc.live == 0);
        assert_eq!(alloc.allocated, alloc.freed);
        assert!(alloc.allocated <= paid, "limit={limit} exact={exact} paid={paid} alloc={alloc:?} refused={refused} first={first:?}");
        if limit == 0 {
            assert_eq!(alloc.allocated, 0);
        }
    }
}
#[test]
fn bypasses_and_sticky_refusals_leave_the_complete_input_untouched() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let count = minimum_items() + 9;
        let mut input = seed(count, count + 16, &mut work).unwrap().observations;
        let saved = input.clone();
        let pointer = input.items.as_ptr();
        let suffix = Observations::default();
        for fanout in [0, 2] {
            assert!(
                try_take(&mut input, &suffix, fanout, function, work.budget, "bypass")
                    .unwrap()
                    .is_none()
            );
            assert_eq!(input, saved);
            assert_eq!(input.items.as_ptr(), pointer);
        }
        assert!(with_original(|| try_take(
            &mut input,
            &suffix,
            1,
            function,
            work.budget,
            "old control"
        ))
        .unwrap()
        .is_none());
        let mut legacy = ReplayBudget::unobserved();
        assert!(
            try_take(&mut input, &suffix, 1, function, &mut legacy, "legacy")
                .unwrap()
                .is_none()
        );
        work.budget.refusal_spent = true;
        assert!(try_take(&mut input, &suffix, 1, function, work.budget, "sticky").is_err());
        assert_eq!(input, saved);
        assert_eq!(input.items.as_ptr(), pointer);
    });
}

#[test]
fn thin_header_and_complete_shipped_wrapper_and_tests_stay_source_bound() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        RECEIVING_BYTES,
        2 * std::mem::size_of::<Candidate<'static>>() + 2 * std::mem::size_of::<bool>()
    );
    assert!((MINIMUM_ITEMS - 1) * ITEM_BYTES < RECEIVING_BYTES);
    assert!(MINIMUM_ITEMS * ITEM_BYTES >= RECEIVING_BYTES);
    let full = include_str!("before_wrapper.rs")
        .replace("#[path = \"before.rs\"]\n", "")
        .replace("#[path = \"before_wrapper_tests.rs\"]\n", "");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(full.as_bytes()))
        ),
        "bd6e2b4576dbacb62f8b2698591a4afcf15809d2994c6b01010b5a0a397b8218"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                crate::source_reflows_merge::current344::project(
                    "src/cleanup_plan/replay/sequence_prefix/main24/before_wrapper_tests.rs",
                    include_str!("before_wrapper_tests.rs"),
                ).as_bytes()
            ))
        ),
        "38f11a53d0a21dc60470dc7ac46fda2a86d191ade68bd7abf5c40ab4da783846"
    );
}
