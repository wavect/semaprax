use super::*;
use crate::test_allocation_observer::{measure, Metrics};
use std::rc::Rc;
mod before;

fn fixture() -> ResolvedProgram {
    let source = "module paths.reuse; @id(\"paths.main\") fn main()->i64 {let mut n=0; while n<2 {n=n+1; break;} n}";
    let parsed = crate::check(source, "isolated-paths.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    assert!(program.functions[0].cleanup_plan.lexical_control.is_some());
    program
}

struct Probe {
    accepted: bool,
    setup: usize,
    method: usize,
    work: usize,
    observed: Metrics,
    refused: bool,
    first: Option<crate::bounded_output::ReservationRefusal>,
}
#[allow(clippy::too_many_arguments)]
fn probe(
    program: &ResolvedProgram,
    count: usize,
    capacity: usize,
    limit: usize,
    floor: usize,
    work_limit: usize,
    old: bool,
) -> Probe {
    let function = &program.functions[0];
    let (result, refused, _, first) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            let mut budget = ReplayBudget::new();
            budget.merge_paths = true;
            budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
            budget.prepare_builder(function).unwrap();
            budget
                .reserve_array::<u8>(
                    function,
                    std::mem::size_of::<SkeletonObservation>() + 2 * std::mem::size_of::<usize>(),
                    "test held observation",
                )
                .unwrap();
            let item = Rc::new(SkeletonObservation::Boolean {
                expression: function.body.id.clone(),
                value: true,
            });
            budget
                .reserve_array::<ExprSkeletonPath>(function, capacity, "test input paths")
                .unwrap();
            let mut paths = Vec::with_capacity(capacity);
            let mut pointers = [0usize; 64];
            for (index, pointer) in pointers.iter_mut().enumerate().take(count) {
                let mut path = empty_expr_path();
                budget
                    .reserve_array::<Observation>(function, 1, "test input observations")
                    .unwrap();
                path.observations.items = vec![item.clone()];
                *pointer = path.observations.items.as_ptr() as usize;
                path.failed = index % 2 == 0;
                path.residual = index % 3 == 0;
                if index % 2 != 0 {
                    path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
                }
                path.continuation = match index % 4 {
                    0 => lexical_skeleton::Continuation::Advancing(item.clone()),
                    1 => lexical_skeleton::Continuation::Exit(item.clone()),
                    2 => lexical_skeleton::Continuation::Latch(item.clone()),
                    _ => lexical_skeleton::Continuation::Normal,
                };
                paths.push(path);
            }
            assert_eq!(paths.len(), count);
            assert_eq!(paths.capacity(), capacity);
            let pointer = paths.as_ptr();
            let mut work = SkeletonWork::with_index(program, function, &mut budget, None).unwrap();
            let selected = can_reuse(&work, &paths);
            assert!(selected, "the paid isolated route must be exercised");
            work.budget.remaining = work_limit;
            work.budget.skeleton_remaining = work_limit;
            work.budget.skeleton_spent = 0;
            let start = crate::bounded_output::active_remaining().unwrap();
            assert!(crate::bounded_output::set_active_floor(floor));
            let (accepted, observed) = measure(|| {
                let result = if old {
                    work.saved_merged_paths(paths)
                } else {
                    work.merged_paths(paths)
                };
                if let Ok(paths) = &result {
                    assert_eq!(paths.len(), count);
                    if !old {
                        assert_eq!(paths.as_ptr(), pointer);
                        assert_eq!(paths.capacity(), capacity);
                    }
                    for (index, path) in paths.iter().enumerate() {
                        assert_eq!(path.failed, index % 2 == 0);
                        assert_eq!(path.residual, index % 3 == 0);
                        assert_eq!(path.owned_source.is_some(), index % 2 != 0);
                        if let Some(place) = &path.owned_source {
                            assert_eq!(place.storage, StorageId::ProvisionalResult);
                            assert!(place.projections.is_empty());
                        }
                        assert_eq!(path.observations.items.as_ptr() as usize, pointers[index]);
                        assert_eq!(path.observations.items.len(), 1);
                        assert!(Rc::ptr_eq(&path.observations.items[0], &item));
                        assert!(path.observations.prefix.is_none());
                        match (&path.continuation, index % 4) {
                            (lexical_skeleton::Continuation::Advancing(value), 0)
                            | (lexical_skeleton::Continuation::Exit(value), 1)
                            | (lexical_skeleton::Continuation::Latch(value), 2) => {
                                assert!(Rc::ptr_eq(value, &item));
                            }
                            (lexical_skeleton::Continuation::Normal, 3) => {}
                            _ => panic!("complete original continuation must survive"),
                        }
                    }
                } else {
                    assert_eq!(result.as_ref().err().unwrap().code, "SPX-H006");
                }
                let accepted = result.is_ok();
                drop(result);
                accepted
            });
            if !accepted {
                let remaining = crate::bounded_output::active_remaining();
                let work_remaining = work.budget.skeleton_remaining;
                let spent = work.budget.skeleton_spent;
                assert!(work
                    .budget
                    .reserve_bytes(function, 0, "sticky isolated retry")
                    .is_err());
                assert_eq!(crate::bounded_output::active_remaining(), remaining);
                assert_eq!(work.budget.skeleton_remaining, work_remaining);
                assert_eq!(work.budget.skeleton_spent, spent);
            }
            crate::bounded_output::clear_active_floor();
            (
                accepted,
                limit - start,
                start - crate::bounded_output::active_remaining().unwrap(),
                work.budget.skeleton_spent,
                observed,
            )
        });
    Probe {
        accepted: result.0,
        setup: result.1,
        method: result.2,
        work: result.3,
        observed: result.4,
        refused,
        first,
    }
}

#[test]
fn complete_isolated_paths_keep_every_field_backing_work_and_drop() {
    let program = fixture();
    for count in [3usize, 4, 8, 16, 32, 64] {
        let capacity = count.next_power_of_two();
        let old = probe(
            &program,
            count,
            capacity,
            usize::MAX,
            0,
            MAX_REPLAY_WORK_UNITS,
            true,
        );
        let new = probe(
            &program,
            count,
            capacity,
            usize::MAX,
            0,
            MAX_REPLAY_WORK_UNITS,
            false,
        );
        assert!(old.accepted && new.accepted && !old.refused && !new.refused);
        assert_eq!(old.work, new.work);
        assert_eq!(new.work, if new.accepted { count } else { 0 });
        assert_eq!(new.method, RECEIVING_BYTES);
        assert!(new.method < old.method);
        assert_eq!(new.observed, Metrics::default());
        assert!(old.observed.allocations > 0);
        assert_eq!(old.observed.allocated, old.observed.freed);
        assert_eq!(old.observed.live, 0);
        assert!(!old.observed.invalid);
    }
}

#[test]
fn isolated_transfer_exact_short_zero_floor_and_work_refusal_remain_sticky() {
    let program = fixture();
    let complete = probe(
        &program,
        32,
        32,
        usize::MAX,
        0,
        MAX_REPLAY_WORK_UNITS,
        false,
    );
    for floor in [0usize, 37] {
        for available in [0, RECEIVING_BYTES - 1, RECEIVING_BYTES] {
            let current = probe(
                &program,
                32,
                32,
                complete.setup + floor + available,
                floor,
                MAX_REPLAY_WORK_UNITS,
                false,
            );
            assert_eq!(current.accepted, available == RECEIVING_BYTES);
            assert_eq!(current.refused, available != RECEIVING_BYTES);
            if let Some(first) = current.first {
                assert_eq!(first.requested, RECEIVING_BYTES);
                assert_eq!(first.remaining, floor + available);
                assert_eq!(first.floor, floor);
            }
            assert_eq!(current.observed, Metrics::default());
        }
    }
    for units in [0usize, 31, 32] {
        let old = probe(&program, 32, 32, usize::MAX, 0, units, true);
        let new = probe(&program, 32, 32, usize::MAX, 0, units, false);
        assert_eq!(old.accepted, new.accepted);
        assert_eq!(old.work, new.work);
        assert_eq!(new.accepted, units == 32);
        assert_eq!(new.work, if units == 32 { 32 } else { 0 });
        assert!(!old.refused && !new.refused);
        if units < 32 {
            assert_eq!(new.method, 0);
        }
    }
}

#[test]
fn all_other_merge_modes_and_unbounded_spare_capacity_keep_the_original_route() {
    let program = fixture();
    let function = &program.functions[0];
    crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        budget.merge_paths = true;
        let mut work = SkeletonWork::with_index(&program, function, &mut budget, None).unwrap();
        let mut paths = Vec::with_capacity(32);
        for _ in 0..32 {
            paths.push(empty_expr_path());
        }
        assert!(can_reuse(&work, &paths));
        work.active_copy_region = Some(function.body.id.clone());
        assert!(!can_reuse(&work, &paths));
        work.active_copy_region = None;
        work.budget.merge_paths = false;
        assert!(!can_reuse(&work, &paths));
        work.budget.merge_paths = true;
        work.budget.admission = allocation::AdmissionMode::SelectedRetained;
        assert!(!can_reuse(&work, &paths));
        work.budget.admission = allocation::AdmissionMode::Ordinary;
        work.budget.payer = allocation::AllocationPayer::LegacyUnobserved;
        assert!(!can_reuse(&work, &paths));
        work.budget.payer = allocation::AllocationPayer::SelectedReservation;
        assert!(!can_reuse(&work, &paths));
        work.budget.payer = allocation::AllocationPayer::ActiveBuilder;
        paths.truncate(2);
        assert!(!can_reuse(&work, &paths));
        paths.truncate(1);
        assert!(!can_reuse(&work, &paths));
        paths.clear();
        assert!(!can_reuse(&work, &paths));
    });
    let mut normal = program.clone();
    normal.functions[0].cleanup_plan.lexical_control = None;
    crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        budget.merge_paths = true;
        let work =
            SkeletonWork::with_index(&normal, &normal.functions[0], &mut budget, None).unwrap();
        let paths = (0..32).map(|_| empty_expr_path()).collect();
        assert!(!can_reuse(&work, &paths));
    });
}
