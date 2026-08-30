use super::super::tests::{real_prepare_serial, revision};
use super::*;
use replacement::TestHook;

fn install_hook(worker: &PreparedProjectInterpreter, hook: TestHook) {
    *worker.replacement_hook.lock().unwrap() = Some(hook);
}

fn assert_terminal(worker: &PreparedProjectInterpreter, revision: &Arc<ProjectRevision>) {
    assert_eq!(
        worker
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new()
            )
            .unwrap_err()[0]
            .code,
        "SPX-F109"
    );
    assert_eq!(
        worker
            .replace_revision(revision.project_revision(), Arc::clone(revision))
            .unwrap_err()[0]
            .code,
        "SPX-F109"
    );
}

#[test]
fn replacement_uses_the_actual_worker_and_excludes_both_concurrent_operations() {
    let _serial = real_prepare_serial();
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        0
    );
    let revision = revision();
    let worker = prepare_project_interpreter(
        Arc::clone(&revision),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    let thread_id = worker.worker.as_ref().unwrap().thread().id();
    let baseline = worker
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    let (entered, observing) = mpsc::sync_channel(0);
    let (resume, held) = mpsc::sync_channel(0);
    install_hook(
        &worker,
        TestHook::Pause {
            entered,
            resume: held,
        },
    );
    std::thread::scope(|scope| {
        // Unwind must drop these endpoints before the scope joins its child.
        // Otherwise an assertion failure would retain the resume sender while
        // joining a request blocked on the worker's paused preparation hook.
        let resume = resume;
        let observing = observing;
        let replace = scope
            .spawn(|| worker.replace_revision(revision.project_revision(), Arc::clone(&revision)));
        assert_eq!(
            observing
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap(),
            thread_id
        );
        assert_eq!(
            ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
            1
        );
        let execute = worker
            .execute_test(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new(),
            )
            .unwrap_err();
        assert_eq!(execute[0].code, "SPX-F109");
        assert_eq!(
            worker
                .replace_revision(revision.project_revision(), Arc::clone(&revision))
                .unwrap_err()[0]
                .code,
            "SPX-F109"
        );
        resume.send(()).unwrap();
        replace.join().unwrap().unwrap();
    });
    assert_eq!(worker.worker.as_ref().unwrap().thread().id(), thread_id);
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        1
    );
    assert_eq!(
        worker
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new()
            )
            .unwrap(),
        baseline
    );
    drop(worker);
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        0
    );
}

#[test]
fn stale_base_is_checked_before_candidate_preparation_or_faults() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let worker = prepare_project_interpreter(
        Arc::clone(&revision),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    let baseline = worker
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    let stale = format!(
        "sha256:{}",
        if revision.project_revision() == format!("sha256:{}", "0".repeat(64)) {
            "1".repeat(64)
        } else {
            "0".repeat(64)
        }
    );
    install_hook(&worker, TestHook::PanicBeforePrepare);
    assert_eq!(
        worker
            .replace_revision(&stale, Arc::clone(&revision))
            .unwrap_err()[0]
            .code,
        "SPX-F108"
    );
    assert_eq!(
        worker
            .execute_entry(
                &PreparedProjectExecutionOptions::default(),
                &ProjectExecutionCancellation::new()
            )
            .unwrap(),
        baseline
    );
    // The hook is consumed by the stale request, not deferred to a later one.
    worker
        .replace_revision(revision.project_revision(), Arc::clone(&revision))
        .unwrap();
}

#[test]
fn replacement_panics_make_the_worker_terminal() {
    let _serial = real_prepare_serial();
    let revision = revision();
    for hook in [TestHook::PanicBeforePrepare, TestHook::PanicAfterCommit] {
        let worker = prepare_project_interpreter(
            Arc::clone(&revision),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        install_hook(&worker, hook);
        assert_eq!(
            worker
                .replace_revision(revision.project_revision(), Arc::clone(&revision))
                .unwrap_err()[0]
                .code,
            "SPX-F109"
        );
        assert_terminal(&worker, &revision);
        // The permit belongs to the prepared handle until its worker is joined.
        assert_eq!(
            ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
            1
        );
        drop(worker);
        assert_eq!(
            ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
            0
        );
    }
}

#[test]
fn expected_revision_has_a_closed_bounded_grammar() {
    for token in [
        String::new(),
        "sha256:".to_owned(),
        format!("sha256:{}", "A".repeat(64)),
        format!("sha256:{}", "g".repeat(64)),
        format!("sha256:{}", "0".repeat(65)),
        "x".repeat(65536),
    ] {
        assert_eq!(
            replacement::validate_expected_revision(&token).unwrap_err()[0].code,
            "SPX-F108"
        );
    }
    replacement::validate_expected_revision(&format!("sha256:{}", "0123456789abcdef".repeat(4)))
        .unwrap();
}

#[test]
fn disconnected_replacement_receiver_is_terminal_after_the_real_send_failure() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let worker = prepare_project_interpreter(
        Arc::clone(&revision),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    let admission = ExecutionAdmission::acquire(&worker.executing).unwrap();
    let (reply, response) = mpsc::sync_channel(0);
    drop(response);
    worker
        .sender
        .send(WorkerMessage::Replace(ReplacementRequest {
            expected: revision.project_revision().to_owned(),
            revision: Arc::clone(&revision),
            reply,
            hook: None,
        }))
        .unwrap();
    drop(admission);
    assert_terminal(&worker, &revision);
    drop(worker);
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        0
    );
}
