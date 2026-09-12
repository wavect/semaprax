use super::*;
use crate::project::ProjectExecutionRole;

#[test]
fn untraced_entry_test_fuel_and_cancellation_match_traced_execution() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let worker = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    for role in [ProjectExecutionRole::Entry, ProjectExecutionRole::Test] {
        for max_steps in [1, 2, 1000] {
            for cancelled in [false, true] {
                let cancellation = ProjectExecutionCancellation::new();
                if cancelled {
                    cancellation.cancel();
                }
                let options = PreparedProjectExecutionOptions {
                    max_steps,
                    ..Default::default()
                };
                let before = worker.execute(role, &options, &cancellation).unwrap();
                let untraced = worker
                    .execute_untraced(role, max_steps, &cancellation)
                    .unwrap();
                let after = worker.execute(role, &options, &cancellation).unwrap();
                assert_eq!(
                    before, after,
                    "untraced execution must not change later trace bytes"
                );
                assert_eq!(untraced.role(), role);
                assert_eq!(untraced.max_steps(), max_steps);
                assert_eq!(untraced.steps_used(), before.steps_used());
                assert_eq!(untraced.outcome(), before.outcome());
            }
        }
    }
    assert_eq!(
        worker
            .execute_entry_untraced(1000, &ProjectExecutionCancellation::new())
            .unwrap()
            .outcome(),
        &ProjectPreparedExecutionOutcome::Returned(42)
    );
    assert_eq!(
        worker
            .execute_test_untraced(1000, &ProjectExecutionCancellation::new())
            .unwrap()
            .outcome(),
        &ProjectPreparedExecutionOutcome::Returned(0)
    );
}

#[test]
fn untraced_invalid_fuel_fails_without_poisoning_worker_or_relaxing_trace_bounds() {
    let _serial = real_prepare_serial();
    let worker = revision()
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    let before = worker
        .execute_entry(&PreparedProjectExecutionOptions::default(), &cancellation)
        .unwrap();
    for max_steps in [0, interpreter::MAX_STEPS_LIMIT + 1, usize::MAX] {
        assert_eq!(
            worker
                .execute_entry_untraced(max_steps, &cancellation)
                .unwrap_err()[0]
                .code,
            "SPX-F108"
        );
    }
    assert_eq!(
        PreparedProjectExecutionOptions::new(1000, 0, 0)
            .unwrap_err()
            .code,
        "SPX-F108"
    );
    let after = worker
        .execute_entry(&PreparedProjectExecutionOptions::default(), &cancellation)
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn no_trace_evaluator_collects_no_events_or_dropped_event_accounting() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let closures = super::super::origin::prepare_closures(&revision).unwrap();
    let evaluated = interpreter::evaluate_prepared_resolved_zero_arg_i64(
        revision.entry_program(),
        &closures.entry,
        1000,
        0,
        interpreter::PreparedCancellation::Never,
    )
    .unwrap();
    assert!(evaluated.events.is_empty());
    assert_eq!(evaluated.events.capacity(), 0);
    assert_eq!(evaluated.dropped_events, 0);
    assert!(evaluated.steps_used > 0);
}
