use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::*;

fn revision() -> Arc<ProjectRevision> {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
    crate::project::load_snapshot(&manifest)
        .expect("calculator Project must load")
        .retain_revision()
}

#[test]
fn one_prepared_worker_repeats_exact_entry_and_test_with_replayable_origins() {
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    let options = PreparedProjectExecutionOptions::default();
    let first = prepared.execute_entry(&options, &cancellation).unwrap();
    let second = prepared.execute_entry(&options, &cancellation).unwrap();
    assert_eq!(
        first.outcome(),
        &ProjectPreparedExecutionOutcome::Returned(42)
    );
    assert_eq!(first.trace().envelope(), second.trace().envelope());
    verify_project_source_trace(first.trace().envelope()).unwrap();
    verify_project_source_trace_against_revision(&revision, first.trace().envelope()).unwrap();

    let test = prepared.execute_test(&options, &cancellation).unwrap();
    assert_eq!(
        test.outcome(),
        &ProjectPreparedExecutionOutcome::Returned(0)
    );
    verify_project_source_trace_against_revision(&revision, test.trace().envelope()).unwrap();
}

#[test]
fn cancellation_is_a_replayable_zero_step_outcome_and_trace_saturation_is_explicit() {
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    cancellation.cancel();
    let cancelled = prepared
        .execute_entry(&PreparedProjectExecutionOptions::default(), &cancellation)
        .unwrap();
    assert_eq!(
        cancelled.outcome(),
        &ProjectPreparedExecutionOutcome::Cancelled { before_step: 1 }
    );
    assert_eq!(cancelled.steps_used(), 0);
    assert_eq!(cancelled.trace().recorded_events(), 0);
    verify_project_source_trace_against_revision(&revision, cancelled.trace().envelope()).unwrap();

    let one_event = PreparedProjectExecutionOptions::new(
        interpreter::DEFAULT_MAX_STEPS,
        DEFAULT_PROJECT_SOURCE_TRACE_BYTES,
        1,
    )
    .unwrap();
    let completed = prepared
        .execute_entry(&one_event, &ProjectExecutionCancellation::new())
        .unwrap();
    assert_eq!(completed.trace().recorded_events(), 1);
    assert!(completed.trace().truncated());
}

#[test]
fn options_and_resigned_origin_mutation_fail_closed() {
    assert_eq!(
        PreparedProjectExecutionOptions::new(0, MIN_PROJECT_SOURCE_TRACE_BYTES, 1)
            .unwrap_err()
            .code,
        "SPX-F108"
    );
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let execution = prepared
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    let hostile = execution
        .trace()
        .envelope()
        .replacen("src/app.spx", "src/bad.spx", 1);
    assert_eq!(
        verify_project_source_trace(&hostile).unwrap_err().code,
        "SPX-F110"
    );
}

#[test]
fn worker_permit_is_bounded_and_released_exactly_once() {
    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    let first = PreparedWorkerPermit::acquire(&ACTIVE, 2).unwrap();
    let second = PreparedWorkerPermit::acquire(&ACTIVE, 2).unwrap();
    assert_eq!(
        PreparedWorkerPermit::acquire(&ACTIVE, 2)
            .unwrap_err()
            .first()
            .unwrap()
            .code,
        "SPX-F107"
    );
    drop(first);
    let replacement = PreparedWorkerPermit::acquire(&ACTIVE, 2).unwrap();
    drop(second);
    drop(replacement);
    assert_eq!(ACTIVE.load(Ordering::Acquire), 0);
}

#[test]
fn execution_admission_rejects_concurrent_work_and_reopens_after_release() {
    let executing = AtomicBool::new(false);
    let first = ExecutionAdmission::acquire(&executing).unwrap();
    assert_eq!(
        ExecutionAdmission::acquire(&executing)
            .unwrap_err()
            .first()
            .unwrap()
            .code,
        "SPX-F109"
    );
    drop(first);
    drop(ExecutionAdmission::acquire(&executing).unwrap());
}

#[test]
fn duplicate_function_origin_must_be_exact_and_is_counted_once() {
    let origin = FunctionOrigin {
        path: "src/app.spx".to_owned(),
        source_revision: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
        source_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            .to_owned(),
        source_bytes: 128,
    };
    let mut origins = BTreeMap::new();
    let mut bytes = 0;
    insert_origin(&mut origins, "app.main", origin.clone(), &mut bytes).unwrap();
    let exact_bytes = bytes;
    insert_origin(&mut origins, "app.main", origin.clone(), &mut bytes).unwrap();
    assert_eq!(bytes, exact_bytes);

    let mut drifted = origin;
    drifted.source_bytes += 1;
    assert_eq!(
        insert_origin(&mut origins, "app.main", drifted, &mut bytes)
            .unwrap_err()
            .first()
            .unwrap()
            .code,
        "SPX-F107"
    );
}
