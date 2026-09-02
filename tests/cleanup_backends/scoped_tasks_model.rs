//! Deterministic Scoped Task Model v1 integration evidence.
//!
//! These tests pin the bounded hidden-model semantics of
//! `semaprax::scoped_tasks`: canonical scheduling known-answer digests,
//! hostile rejections, determinism under input permutation, sticky failure
//! and cancellation selection, children-before-parents cleanup, and
//! domain-separated canonical JSON serialization. They prove proof data only;
//! no threads, scheduler integration, language syntax, or target execution
//! exists.

use semaprax::scoped_tasks::{
    DependencyEdge, FailureKind, RunTotals, ScopeExitOutcome, ScopeJoin, ScopeSpec,
    ScopedTaskModel, ScopedTaskRun, ScopedTasksError, SendableMark, ShareableMark, TaskEvent,
    TaskOutcome, TaskPhase, TaskSpec, SCOPED_TASKS_MODEL_V1, SCOPED_TASKS_TRACE_V1,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MODEL_DIGEST_DOMAIN: &[u8] = b"semaprax.scoped-tasks-model-fingerprint.v1\0";

/// Canonical join-all scenario: three sibling tasks drain in stable-id order
/// and finalize in exact reverse completion order.
pub const JOIN_ALL_TRACE_DIGEST: &str =
    "c2c1ac40d3ce622bd1ac07984a88978dba13c79be2a568b9680943ccb07dbb91";

/// Cancellation mid-scope: cancelling the root after one completion cancels
/// both pending siblings before any new work starts.
pub const CANCELLATION_MID_SCOPE_TRACE_DIGEST: &str =
    "98a5bf2f423a7a4d82f5edf2fb9f5374821e478b988871ef5a3635329c2d256b";

/// Failure drain: the first failure wins stickily, a dependent is abandoned,
/// and independent siblings still run to completion.
pub const FAILURE_DRAIN_TRACE_DIGEST: &str =
    "b51cf73d42c73a97bc71a1e54b791896d053cdd5bcf4e61bfac6c2de545c6f6c";

/// Nested scopes: the child scope fully drains and finalizes before the
/// parent scope touches its own tasks or exits.
pub const NESTED_SCOPES_TRACE_DIGEST: &str =
    "051da66037c3a17b8e58fda8f12902dcdc53f198e3c7ed1d464c65d239def03d";

fn sha256_domain(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("{:x}", semaprax::digest_hex::LowerHex(hasher.finalize()))
}

fn hex_of(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn event_kinds(run: &ScopedTaskRun<'_>) -> Vec<&'static str> {
    fn kind(event: &TaskEvent) -> &'static str {
        match event {
            TaskEvent::Started { .. } => "started",
            TaskEvent::Completed { .. } => "completed",
            TaskEvent::Failed { .. } => "failed",
            TaskEvent::Cancelled { .. } => "cancelled",
            TaskEvent::ScopeCancelled { .. } => "scope_cancelled",
            TaskEvent::Finalized { .. } => "finalized",
            TaskEvent::ScopeExited { .. } => "scope_exited",
        }
    }
    run.events().iter().map(kind).collect()
}

fn succeed(id: &str, scope: &str) -> TaskSpec {
    TaskSpec::new(
        id,
        scope,
        SendableMark::Sendable,
        ShareableMark::NotShareable,
        TaskOutcome::Succeed,
    )
}

fn failing(id: &str, scope: &str, failure: FailureKind) -> TaskSpec {
    TaskSpec::new(
        id,
        scope,
        SendableMark::NotSendable,
        ShareableMark::NotShareable,
        TaskOutcome::Fail(failure),
    )
}

fn drive_until(run: &mut ScopedTaskRun<'_>, mut predicate: impl FnMut(&TaskEvent) -> bool) {
    loop {
        match run.step().expect("model steps are valid") {
            Some(event) if predicate(&event) => break,
            Some(_) => continue,
            None => panic!("scheduler finished before the expected event"),
        }
    }
}

fn drain(run: &mut ScopedTaskRun<'_>) -> Vec<TaskEvent> {
    let mut events = Vec::new();
    while let Some(event) = run.step().expect("model steps are valid") {
        events.push(event);
    }
    events
}

#[test]
fn kat_join_all_schedules_in_stable_id_order_and_finalizes_in_reverse() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![succeed("a", "r"), succeed("b", "r"), succeed("c", "r")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "started",
            "completed",
            "started",
            "completed",
            "started",
            "completed",
            "finalized",
            "finalized",
            "finalized",
            "scope_exited",
        ]
    );
    assert!(matches!(
        run.finish().unwrap().root_outcome(),
        ScopeExitOutcome::Success
    ));
    assert_eq!(hex_of(run.trace_digest()), JOIN_ALL_TRACE_DIGEST);
}

#[test]
fn kat_cancellation_mid_scope_cancels_pending_before_new_work() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![succeed("a", "r"), succeed("b", "r"), succeed("c", "r")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    drive_until(
        &mut run,
        |event| matches!(event, TaskEvent::Completed { task } if task.as_str() == "a"),
    );
    assert!(run.cancel_scope("r").unwrap());
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "started",
            "completed",
            "scope_cancelled",
            "cancelled",
            "cancelled",
            "finalized",
            "scope_exited",
        ]
    );
    let summary = run.finish().unwrap();
    assert!(matches!(
        summary.root_outcome(),
        ScopeExitOutcome::Cancelled
    ));
    assert_eq!(
        summary.totals(),
        RunTotals {
            started: 1,
            completed: 1,
            failed: 0,
            cancelled: 2
        }
    );
    assert_eq!(run.first_failure("r"), None);
    assert_eq!(
        hex_of(run.trace_digest()),
        CANCELLATION_MID_SCOPE_TRACE_DIGEST
    );
}

#[test]
fn kat_failure_drain_keeps_first_failure_sticky_while_siblings_finish() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![
            failing("a", "r", FailureKind::Semantic),
            succeed("b", "r"),
            succeed("c", "r"),
            succeed("d", "r"),
        ],
        vec![DependencyEdge::new("a", "d")],
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "started",
            "failed",
            "started",
            "completed",
            "started",
            "completed",
            "cancelled",
            "finalized",
            "finalized",
            "finalized",
            "scope_exited",
        ]
    );
    let summary = run.finish().unwrap();
    match summary.root_outcome() {
        ScopeExitOutcome::Failed { task, failure } => {
            assert_eq!(task.as_str(), "a");
            assert_eq!(*failure, FailureKind::Semantic);
        }
        other => panic!("expected failed root outcome, got {other:?}"),
    }
    assert_eq!(
        run.first_failure("r").map(|(task, _)| task.as_str()),
        Some("a")
    );
    assert_eq!(
        summary.totals(),
        RunTotals {
            started: 3,
            completed: 2,
            failed: 1,
            cancelled: 1
        }
    );
    assert_eq!(hex_of(run.trace_digest()), FAILURE_DRAIN_TRACE_DIGEST);
}

#[test]
fn kat_nested_scopes_finalize_children_before_parents() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r"), ScopeSpec::child("inner", "r")],
        vec![
            succeed("x", "inner"),
            succeed("y", "inner"),
            succeed("z", "r"),
        ],
        Vec::new(),
        vec![ScopeJoin::new("r", "inner")],
    )
    .unwrap();
    let mut run = model.prepare_run();
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "started",
            "completed",
            "started",
            "completed",
            "started",
            "completed",
            "finalized",
            "finalized",
            "scope_exited",
            "finalized",
            "scope_exited",
        ]
    );
    assert_eq!(run.task_phase("z"), Some(TaskPhase::Completed));
    assert_eq!(
        run.finish().unwrap().root_outcome(),
        &ScopeExitOutcome::Success
    );
    assert_eq!(hex_of(run.trace_digest()), NESTED_SCOPES_TRACE_DIGEST);
}

#[test]
fn cancel_during_drain_lets_running_work_finish_and_cancels_pending() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![succeed("a", "r"), succeed("b", "r"), succeed("c", "r")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    drive_until(
        &mut run,
        |event| matches!(event, TaskEvent::Started { task } if task.as_str() == "b"),
    );
    assert!(run.cancel_scope("r").unwrap());
    assert!(
        !run.cancel_scope("r").unwrap(),
        "double cancel is effect-free"
    );
    assert!(run.is_scope_cancelled("r"));
    assert_eq!(run.task_phase("b"), Some(TaskPhase::Started));
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "started",
            "completed",
            "started",
            "completed",
            "scope_cancelled",
            "cancelled",
            "finalized",
            "finalized",
            "scope_exited",
        ]
    );
    assert!(matches!(
        run.finish().unwrap().root_outcome(),
        ScopeExitOutcome::Cancelled
    ));
}

#[test]
fn drained_failure_beats_late_cancellation() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![
            failing("a", "r", FailureKind::Physical(7)),
            succeed("b", "r"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    drive_until(
        &mut run,
        |event| matches!(event, TaskEvent::Failed { task, .. } if task.as_str() == "a"),
    );
    run.cancel_scope("r").unwrap();
    drain(&mut run);
    match run.finish().unwrap().root_outcome() {
        ScopeExitOutcome::Failed { task, failure } => {
            assert_eq!(task.as_str(), "a");
            assert_eq!(*failure, FailureKind::Physical(7));
        }
        other => panic!("expected sticky failed outcome, got {other:?}"),
    }
}

#[test]
fn hostile_structures_are_rejected_at_construction() {
    let escape = ScopedTaskModel::try_new(
        vec![
            ScopeSpec::root("r"),
            ScopeSpec::child("left", "r"),
            ScopeSpec::child("right", "r"),
        ],
        vec![succeed("p", "left"), succeed("q", "right")],
        vec![DependencyEdge::new("p", "q")],
        vec![ScopeJoin::new("r", "left"), ScopeJoin::new("r", "right")],
    );
    assert_eq!(escape, Err(ScopedTasksError::EscapingDependency));

    let double_join = ScopedTaskModel::try_new(
        vec![
            ScopeSpec::root("r"),
            ScopeSpec::child("s", "r"),
            ScopeSpec::child("t", "r"),
        ],
        Vec::new(),
        Vec::new(),
        vec![ScopeJoin::new("r", "s"), ScopeJoin::new("r", "s")],
    );
    assert_eq!(double_join, Err(ScopedTasksError::DoubleJoin));

    let orphan_join = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r"), ScopeSpec::child("s", "r")],
        Vec::new(),
        Vec::new(),
        vec![ScopeJoin::new("s", "s")],
    );
    assert_eq!(orphan_join, Err(ScopedTasksError::OrphanJoin));
}

#[test]
fn determinism_survives_input_permutation() {
    let forward = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r"), ScopeSpec::child("inner", "r")],
        vec![
            succeed("x", "inner"),
            succeed("z", "r"),
            succeed("y", "inner"),
        ],
        vec![DependencyEdge::new("x", "y")],
        vec![ScopeJoin::new("r", "inner")],
    )
    .unwrap();
    let backward = ScopedTaskModel::try_new(
        vec![ScopeSpec::child("inner", "r"), ScopeSpec::root("r")],
        vec![
            succeed("y", "inner"),
            succeed("z", "r"),
            succeed("x", "inner"),
        ],
        vec![DependencyEdge::new("x", "y")],
        vec![ScopeJoin::new("r", "inner")],
    )
    .unwrap();
    assert_eq!(forward.fingerprint(), backward.fingerprint());
    assert_eq!(forward.canonical_json(), backward.canonical_json());

    let mut first = forward.prepare_run();
    let mut second = backward.prepare_run();
    let left = drain(&mut first);
    let right = drain(&mut second);
    assert_eq!(left, right);
    assert_eq!(first.trace_canonical_json(), second.trace_canonical_json());
    assert_eq!(first.trace_digest(), second.trace_digest());
}

#[test]
fn projections_are_valid_json_and_domain_separated() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r"), ScopeSpec::child("s", "r")],
        vec![succeed("t", "s")],
        Vec::new(),
        vec![ScopeJoin::new("r", "s")],
    )
    .unwrap();
    let parsed_model: Value =
        serde_json::from_str(&model.canonical_json()).expect("model JSON parses");
    assert_eq!(parsed_model["schema"], SCOPED_TASKS_MODEL_V1);

    let mut run = model.prepare_run();
    drain(&mut run);
    let parsed_trace: Value =
        serde_json::from_str(&run.trace_canonical_json()).expect("trace JSON parses");
    assert_eq!(parsed_trace["schema"], SCOPED_TASKS_TRACE_V1);
    assert_eq!(
        parsed_trace["model_fingerprint"],
        sha256_domain(MODEL_DIGEST_DOMAIN, model.canonical_json().as_bytes())
    );

    assert_ne!(
        hex_of(run.trace_digest()),
        hex_of(model.fingerprint()),
        "trace and model digests must be domain separated"
    );
    let partial = model.prepare_run();
    assert_ne!(
        hex_of(partial.trace_digest()),
        hex_of(run.trace_digest()),
        "empty and complete traces must differ"
    );
}

#[test]
fn join_all_trace_projection_is_byte_pinned() {
    let model = ScopedTaskModel::try_new(
        vec![ScopeSpec::root("r")],
        vec![succeed("a", "r"), succeed("b", "r"), succeed("c", "r")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut run = model.prepare_run();
    drain(&mut run);
    assert_eq!(
        run.trace_canonical_json(),
        "{\"schema\":\"semaprax.scoped-tasks-trace.v1\",\"model_fingerprint\":\"2ad8541e5310a3202e938de17b2e3e8630080de05b460c1b4228a3cb1c50ef37\",\"events\":[{\"kind\":\"started\",\"task\":\"a\"},{\"kind\":\"completed\",\"task\":\"a\"},{\"kind\":\"started\",\"task\":\"b\"},{\"kind\":\"completed\",\"task\":\"b\"},{\"kind\":\"started\",\"task\":\"c\"},{\"kind\":\"completed\",\"task\":\"c\"},{\"kind\":\"finalized\",\"task\":\"c\"},{\"kind\":\"finalized\",\"task\":\"b\"},{\"kind\":\"finalized\",\"task\":\"a\"},{\"kind\":\"scope_exited\",\"scope\":\"r\",\"outcome\":{\"kind\":\"success\"}}],\"root_outcome\":{\"kind\":\"success\"},\"first_failure\":null}"
    );
}
