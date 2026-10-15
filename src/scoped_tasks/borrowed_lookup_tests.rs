//! Borrowed task/scope queries, sticky cancellation and full original receiver.
use super::*;
use crate::test_allocation_observer::measure;
#[allow(
    dead_code,
    reason = "complete historical receiver retained for independent test comparisons"
)]
mod old;
fn current_model() -> ScopedTaskModel {
    ScopedTaskModel::try_new(
        vec![ScopeSpec::root("root"), ScopeSpec::child("child", "root")],
        vec![
            TaskSpec::new(
                "α",
                "root",
                SendableMark::Sendable,
                ShareableMark::NotShareable,
                TaskOutcome::Fail(FailureKind::Semantic),
            ),
            TaskSpec::new(
                "β",
                "child",
                SendableMark::Sendable,
                ShareableMark::NotShareable,
                TaskOutcome::Succeed,
            ),
        ],
        vec![],
        vec![ScopeJoin::new("root", "child")],
    )
    .unwrap()
}
fn original_model() -> old::ScopedTaskModel {
    old::ScopedTaskModel::try_new(
        vec![
            old::ScopeSpec::root("root"),
            old::ScopeSpec::child("child", "root"),
        ],
        vec![
            old::TaskSpec::new(
                "α",
                "root",
                old::SendableMark::Sendable,
                old::ShareableMark::NotShareable,
                old::TaskOutcome::Fail(old::FailureKind::Semantic),
            ),
            old::TaskSpec::new(
                "β",
                "child",
                old::SendableMark::Sendable,
                old::ShareableMark::NotShareable,
                old::TaskOutcome::Succeed,
            ),
        ],
        vec![],
        vec![old::ScopeJoin::new("root", "child")],
    )
    .unwrap()
}
#[test]
fn borrowed_task_scope_and_failure_queries_acquire_no_identity_backing() {
    let model = current_model();
    let original = original_model();
    let mut run = model.prepare_run();
    let mut before = original.prepare_run();
    for _ in 0..2 {
        run.step().unwrap();
        before.step().unwrap();
    }
    for key in ["α", "β", "missing", "", "α\0tail"] {
        assert_eq!(
            format!("{:?}", run.task_phase(key)),
            format!("{:?}", before.task_phase(key))
        );
    }
    for key in ["root", "child", "missing", "", "root\0tail"] {
        assert_eq!(run.is_scope_cancelled(key), before.is_scope_cancelled(key));
        assert_eq!(
            format!("{:?}", run.first_failure(key)),
            format!("{:?}", before.first_failure(key))
        );
    }
    let (_, actual) = measure(|| {
        for _ in 0..128 {
            for key in ["α", "β", "missing", "", "α\0tail"] {
                std::hint::black_box(run.task_phase(key));
            }
            for key in ["root", "child", "missing", "", "root\0tail"] {
                std::hint::black_box(run.is_scope_cancelled(key));
                std::hint::black_box(run.first_failure(key));
            }
        }
    });
    assert!(!actual.invalid);
    assert_eq!(actual.allocated, 0);
    assert_eq!(actual.live, 0);
    let (_, saved) = measure(|| {
        std::hint::black_box(before.task_phase("α"));
        std::hint::black_box(before.is_scope_cancelled("root"));
        std::hint::black_box(before.first_failure("root"));
    });
    assert!(!saved.invalid && saved.allocated > 0);
    assert_eq!(saved.live, 0);
    assert_eq!(saved.allocated, saved.freed);
}
#[test]
fn unknown_cancel_avoids_backing_and_preserves_complete_original_error_order() {
    let model = current_model();
    let original = original_model();
    let mut run = model.prepare_run();
    let mut before = original.prepare_run();
    let (actual, storage) = measure(|| run.cancel_scope("unknown"));
    assert_eq!(actual, Err(ScopedTasksError::UnknownScope));
    assert_eq!(
        format!("{:?}", actual),
        format!("{:?}", before.cancel_scope("unknown"))
    );
    assert!(!storage.invalid);
    assert_eq!(storage.allocated, 0);
    assert_eq!(storage.live, 0);
    while !run.is_complete() {
        run.step().unwrap();
        before.step().unwrap();
    }
    assert_eq!(
        run.cancel_scope("unknown"),
        Err(ScopedTasksError::RunAlreadyComplete)
    );
    assert_eq!(
        format!("{:?}", run.cancel_scope("unknown")),
        format!("{:?}", before.cancel_scope("unknown"))
    );
}
#[test]
fn complete_original_scoped_cancellation_failure_trace_and_receiving_drop_match() {
    for cancelled in [false, true] {
        let model = current_model();
        let original = original_model();
        assert_eq!(model.canonical_json(), original.canonical_json());
        assert_eq!(model.fingerprint(), original.fingerprint());
        let mut run = model.prepare_run();
        let mut before = original.prepare_run();
        if cancelled {
            assert_eq!(
                run.cancel_scope("child").unwrap(),
                before.cancel_scope("child").unwrap()
            );
            assert_eq!(
                run.cancel_scope("child").unwrap(),
                before.cancel_scope("child").unwrap()
            );
        }
        while !run.is_complete() {
            assert_eq!(format!("{:?}", run.step()), format!("{:?}", before.step()));
            assert_eq!(run.trace_canonical_json(), before.trace_canonical_json());
            assert_eq!(run.trace_digest(), before.trace_digest());
            assert_eq!(run.is_complete(), before.is_complete());
        }
        assert_eq!(
            format!("{:?}", run.finish()),
            format!("{:?}", before.finish())
        );
    }
    for saved in [false, true] {
        let (_, storage) = measure(|| {
            if saved {
                let model = original_model();
                let mut run = model.prepare_run();
                run.cancel_scope("child").unwrap();
                while run.step().unwrap().is_some() {}
                drop(run);
                drop(model);
            } else {
                let model = current_model();
                let mut run = model.prepare_run();
                run.cancel_scope("child").unwrap();
                while run.step().unwrap().is_some() {}
                drop(run);
                drop(model);
            }
        });
        assert!(!storage.invalid && storage.allocated > 0);
        assert_eq!(storage.live, 0);
        assert_eq!(storage.allocated, storage.freed);
    }
}
