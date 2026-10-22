use super::*;
use crate::cleanup_plan::replay::final_output::compact;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::mem::size_of;

#[test]
fn compact_hir_banks_change_only_the_actual_result_slots() {
    let delta = size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>();
    assert!(delta > 0);
    assert_eq!(
        super::super::HIR_SELECTION_CONTROLS - compact::HIR_SELECTION_CONTROLS,
        delta
    );
    assert_eq!(
        super::super::HIR_MOVE_CONTROLS - compact::HIR_MOVE_CONTROLS,
        2 * delta
    );
}

#[test]
fn compact_hir_work_preserves_every_work_cut_and_full_refusal() {
    let program = program();
    let function = &program.functions[0];
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        for allowance in [0, 1, 2, 64] {
            for units in [0, 1, 2, usize::MAX] {
                let run = |compact| {
                    let mut budget = ReplayBudget::with_skeleton_limit(allowance);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let outcome = if compact {
                        work.charge_compact(units, "skeleton prefix clone")
                            .map_err(|error| failure(error.into_diagnostic()))
                    } else {
                        work.charge(units, "skeleton prefix clone").map_err(failure)
                    };
                    drop(work);
                    (
                        outcome,
                        budget.remaining,
                        budget.skeleton_remaining,
                        budget.skeleton_spent,
                        budget.refusal_spent,
                    )
                };
                assert_eq!(
                    run(false),
                    run(true),
                    "allowance={allowance}, units={units}"
                );
            }
        }
    });
    assert!(!refused && first.is_none());
}

#[test]
fn compact_vector_banks_preserve_all_other_actual_receivers() {
    let delta = size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>();
    assert!(delta > 0);
    assert_eq!(
        crate::cleanup_plan::replay::sequence_vector::QUERY - compact::VECTOR_QUERY,
        delta
    );
    assert_eq!(
        crate::cleanup_plan::replay::sequence_vector::FRAME - compact::VECTOR_FRAME,
        delta
    );
}

#[test]
fn compact_path_banks_keep_every_original_count_and_capacity_receiver() {
    let delta = size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>();
    assert!(delta > 0);
    assert_eq!(
        crate::cleanup_plan::replay::path_array::count_controls::<true>()
            - compact::path_count_controls::<true>(),
        delta
    );
    assert_eq!(
        crate::cleanup_plan::replay::path_array::count_controls::<false>()
            - compact::path_count_controls::<false>(),
        delta
    );
    assert_eq!(
        crate::cleanup_plan::replay::path_array::capacity_controls()
            - compact::path_capacity_controls(),
        delta
    );
}
