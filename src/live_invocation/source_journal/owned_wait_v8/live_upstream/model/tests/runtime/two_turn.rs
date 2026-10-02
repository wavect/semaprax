//! The same runtime owns Initialize through both turns and terminal Report.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    TargetHostError, TargetHostHandler, TargetHostRequest, TargetResponseSink, TypedCarrier,
};
use crate::resumable_effects::CapabilityPolicy;

#[derive(Clone, Copy)]
enum Scenario {
    Complete,
    Prewrite(usize),
    Cancel,
    DeniedPolicy,
    TargetFailure,
    TargetFailureStopFault(usize),
    ObserverPanic,
    ObserverTargetFailure,
    ObserverStopFault(usize, bool),
    ObserverStatePanic,
    ObserverCancelled,
}
pub(super) struct Host {
    pub(super) calls: usize,
    pub(super) fail: bool,
}
impl TargetHostHandler for Host {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        if self.fail {
            return Err(TargetHostError::Failed);
        }
        let payload =
            b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n"
                .to_vec();
        let wire = TypedCarrier::new(request.operation().result_type(), payload)
            .unwrap()
            .encode();
        sink.write(&wire).map_err(|_| TargetHostError::Failed)
    }
}
fn exercise(scenario: Scenario) {
    let observer_failure = matches!(
        scenario,
        Scenario::ObserverPanic
            | Scenario::ObserverTargetFailure
            | Scenario::ObserverStopFault(..)
            | Scenario::ObserverStatePanic
            | Scenario::ObserverCancelled
    );
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let context = Arc::new(context.with_cumulative_initialization(&lease).unwrap());
            let retained = Arc::clone(&context);
            let journal = SourceOwnedWaitJournalV8::open(context, key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let policy = CapabilityPolicy::new(if matches!(scenario, Scenario::DeniedPolicy) {
                Vec::new()
            } else {
                vec!["read".into()]
            })
            .unwrap();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(
                Rc::clone(&counts),
                script(&document(journal.context())),
                Rc::new(|_| {}),
            );
            let mut adapter = source(journal.context(), &mut factory);
            let mut runtime = OwnedLifecycleRuntimeV8::open(&journal, &cancel).unwrap();
            let input = super::super::super::super::tests::input(journal.context());
            assert_eq!(
                runtime
                    .session()
                    .run_first_turn_model(input, &mut adapter, &Clock),
                OwnedLifecycleStatusV8::ModelCompleted
            );
            let weak = runtime.test_backings();
            assert!(weak.iter().all(|root| root.strong_count() == 1));
            let before = journal.begin_session().unwrap().sequence();
            let prefix = journal.test_observe_lease().borrow_mut().read().unwrap();
            if let Scenario::Prewrite(offset) = scenario {
                journal
                    .test_observe_lease()
                    .borrow_mut()
                    .test_fail_before_write(before + offset);
            }
            if matches!(scenario, Scenario::Cancel) {
                cancel.cancel();
            }
            let mut host = Host {
                calls: 0,
                fail: matches!(
                    scenario,
                    Scenario::TargetFailure
                        | Scenario::TargetFailureStopFault(_)
                        | Scenario::ObserverTargetFailure
                ),
            };
            let mut releases = 0;
            let status =
                runtime
                    .session()
                    .finish_two_turn_run(&policy, &mut adapter, &mut host, |_| {
                        releases += 1;
                        assert!(!observer_failure, "injected cleanup observer panic");
                    });
            let actions = (counts.borrow().starts, host.calls, releases);
            let persisted = journal
                .test_observe_lease()
                .borrow()
                .test_persisted_snapshot()
                .unwrap();
            // Discarding and reborrowing the session never retries a source stage,
            // dispatch, cleanup or uncertain append, even when runtime close fails.
            assert_eq!(
                runtime
                    .session()
                    .finish_two_turn_run(&policy, &mut adapter, &mut host, |_| releases += 1),
                status
            );
            assert_eq!((counts.borrow().starts, host.calls, releases), actions);
            assert_eq!(
                journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap(),
                persisted
            );
            if observer_failure {
                assert_eq!(
                    status,
                    OwnedLifecycleStatusV8::ObserverFailureCleanupPending
                );
                assert_eq!(actions, (1, 1, 1));
                assert!(weak.iter().all(|root| root.strong_count() == 1));
                assert!(runtime.delivery_projection().is_none());
                assert!(
                    journal.begin_session().is_err(),
                    "ordinary authority stays poisoned"
                );
                let mut runtime = runtime
                    .try_close()
                    .err()
                    .expect("same State stays in custody");
                let cleanup_start = persisted.iter().filter(|b| **b == b'\n').count();
                if let Scenario::ObserverStopFault(offset, after_write) = scenario {
                    let lease = journal.test_observe_lease();
                    let mut lease = lease.borrow_mut();
                    if after_write {
                        lease.test_fail_after_write(cleanup_start + offset);
                    } else {
                        lease.test_fail_before_write(cleanup_start + offset);
                    }
                }
                if matches!(scenario, Scenario::ObserverCancelled) {
                    cancel.cancel();
                }
                let mut state_releases = 0;
                let stopped = runtime.settle_failed_observer(|_| {
                    state_releases += 1;
                    assert!(
                        !matches!(scenario, Scenario::ObserverStatePanic),
                        "injected State observer panic"
                    );
                });
                let after = journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap();
                assert_eq!(
                    state_releases,
                    if matches!(
                        scenario,
                        Scenario::ObserverStopFault(1, _) | Scenario::ObserverCancelled
                    ) {
                        0
                    } else {
                        1
                    }
                );
                assert_eq!(
                    runtime.settle_failed_observer(|_| panic!("State cleanup cannot retry")),
                    stopped
                );
                assert_eq!(
                    runtime.settle_failed_effect(|_| panic!("wrong failure tail")),
                    stopped
                );
                assert_eq!(
                    runtime.session().finish_two_turn_run(
                        &policy,
                        &mut adapter,
                        &mut host,
                        |_| panic!("no redispatch")
                    ),
                    stopped
                );
                assert_eq!(
                    journal
                        .test_observe_lease()
                        .borrow()
                        .test_persisted_snapshot()
                        .unwrap(),
                    after
                );
                assert_eq!((counts.borrow().starts, host.calls, releases), actions);
                assert!(journal.hold().is_err());
                if matches!(
                    scenario,
                    Scenario::ObserverPanic | Scenario::ObserverTargetFailure
                ) {
                    assert_eq!(stopped, OwnedLifecycleStatusV8::ObserverFailureStopped);
                    assert_eq!(
                        after.iter().filter(|b| **b == b'\n').count(),
                        cleanup_start + 3
                    );
                    let row: serde_json::Value = serde_json::from_slice(
                        after
                            .split(|b| *b == b'\n')
                            .filter(|row| !row.is_empty())
                            .last()
                            .unwrap(),
                    )
                    .unwrap();
                    assert_eq!(row["kind"], "stop");
                    let effect_failed = matches!(scenario, Scenario::ObserverTargetFailure);
                    assert_eq!(
                        row["status"],
                        if effect_failed {
                            "effect_failed"
                        } else {
                            "rejected"
                        }
                    );
                    assert_eq!(
                        row["reason"],
                        if effect_failed {
                            "effect_failed"
                        } else {
                            "stage_refused"
                        }
                    );
                    assert!(weak.iter().all(|root| root.upgrade().is_none()));
                    assert!(runtime.try_close().is_ok());
                } else {
                    assert_eq!(
                        stopped,
                        OwnedLifecycleStatusV8::Quarantined("observer-failure-cleanup")
                    );
                    let runtime = runtime
                        .try_close()
                        .err()
                        .expect("unsettled boundary refuses close");
                    assert_eq!(
                        weak.iter().any(|root| root.strong_count() == 1),
                        state_releases == 0
                    );
                    drop(runtime);
                    assert!(weak.iter().all(|root| root.upgrade().is_none()));
                }
            } else if matches!(
                scenario,
                Scenario::TargetFailure | Scenario::TargetFailureStopFault(_)
            ) {
                assert_eq!(status, OwnedLifecycleStatusV8::FailedEffectCleanupPending);
                assert_eq!(actions, (1, 1, 1));
                assert!(runtime.delivery_projection().is_none());
                let mut runtime = runtime
                    .try_close()
                    .err()
                    .expect("failed target retains State owner");
                let cleanup_start = journal.begin_session().unwrap().sequence();
                if let Scenario::TargetFailureStopFault(offset) = scenario {
                    journal
                        .test_observe_lease()
                        .borrow_mut()
                        .test_fail_before_write(cleanup_start + offset);
                }
                let mut state_releases = 0;
                let stopped = runtime.settle_failed_effect(|_| state_releases += 1);
                let after = journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap();
                assert_eq!(
                    state_releases,
                    if matches!(scenario, Scenario::TargetFailureStopFault(1)) {
                        0
                    } else {
                        1
                    }
                );
                assert_eq!(
                    runtime.settle_failed_effect(|_| panic!("State cleanup cannot retry")),
                    stopped
                );
                assert_eq!(
                    journal
                        .test_observe_lease()
                        .borrow()
                        .test_persisted_snapshot()
                        .unwrap(),
                    after
                );
                assert_eq!((counts.borrow().starts, host.calls, releases), actions);
                if matches!(scenario, Scenario::TargetFailure) {
                    assert_eq!(stopped, OwnedLifecycleStatusV8::FailedEffectStopped);
                    let current = journal.begin_session().unwrap();
                    assert_eq!(current.sequence(), cleanup_start + 3);
                    assert!(matches!(
                        current.inventory.failed_effect_state_facts().unwrap().4,
                        EntryV8::Ordinary(SourceJournalEntry::Stop {
                            status: crate::live_invocation::source_journal::SourceStopStatus::EffectFailed,
                            reason: crate::live_invocation::source_journal::SourceStopReason::EffectFailed,
                            ..
                        })
                    ));
                    assert!(weak.iter().all(|root| root.upgrade().is_none()));
                    assert!(runtime.try_close().is_ok());
                } else {
                    assert_eq!(
                        stopped,
                        OwnedLifecycleStatusV8::Quarantined("failed-effect-cleanup")
                    );
                    assert!(runtime.try_close().is_err());
                }
            } else if matches!(scenario, Scenario::Complete) {
                assert_eq!(status, OwnedLifecycleStatusV8::Complete);
                assert_eq!(actions, (2, 2, 4));
                assert_eq!(journal.begin_session().unwrap().sequence(), before + 51);
                let projection = runtime.delivery_projection().unwrap();
                assert_eq!(projection["kind"], "complete");
                assert!(projection["report"]["fields"].as_array().is_some());
                let terminal = journal.terminal_evidence().unwrap();
                assert_eq!(
                    projection["terminal_evidence"].as_str().map(str::as_bytes),
                    Some(terminal.evidence())
                );
                let evidence = terminal.evidence().to_vec();
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
                assert!(
                    journal.begin_fresh_session().is_err(),
                    "settled terminal history cannot initialize another run"
                );
                assert!(runtime.try_close().is_ok());
                journal
                    .hold()
                    .expect("successful runtime close preserves settled store");
                drop(adapter);
                drop(journal);
                let registration = retained.registration().clone();
                let lease = crate::resumable_effects::owned_frame::recover_source_owned_wait_v8(
                std::fs::File::open(directory).unwrap(), &registration, registration.expected_facts().clone(),
                crate::resumable_effects::owned_frame::ExplicitStoreRegistrationGrant::for_trusted_host(true).unwrap(),
            ).unwrap();
                let recovered = SourceOwnedWaitJournalV8::open(
                    retained,
                    crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                    lease,
                )
                .unwrap();
                assert_eq!(recovered.terminal_evidence().unwrap().evidence(), evidence);
                assert!(
                    OwnedLifecycleRuntimeV8::open(&recovered, &cancel).is_err(),
                    "recovered evidence grants no fresh run"
                );
            } else {
                assert!(
                    matches!(status, OwnedLifecycleStatusV8::Quarantined(_)),
                    "{status:?}"
                );
                assert!(runtime.delivery_projection().is_none());
                let runtime = runtime
                    .try_close()
                    .err()
                    .expect("pending owner remains in runtime custody");
                assert_eq!(runtime.status(), status);
                assert!(journal.hold().is_err());
                assert!(journal.terminal_evidence().is_err());
                if let Scenario::Prewrite(offset) = scenario {
                    assert!(persisted.starts_with(&prefix));
                    assert_eq!(
                        persisted.iter().filter(|b| **b == b'\n').count(),
                        before + offset - 1
                    );
                    assert!(
                        weak.iter().any(|root| root.strong_count() == 1),
                        "owner retained at boundary {offset}"
                    );
                    assert_eq!(actions.0, if offset <= 27 { 1 } else { 2 });
                    assert_eq!(
                        actions.1,
                        if offset <= 8 {
                            0
                        } else if offset <= 39 {
                            1
                        } else {
                            2
                        }
                    );
                }
                if matches!(scenario, Scenario::Cancel) {
                    assert_eq!(
                        status,
                        OwnedLifecycleStatusV8::Quarantined("first-authorize")
                    );
                    assert_eq!(persisted, prefix);
                    assert_eq!(actions, (1, 0, 0));
                }
                if matches!(scenario, Scenario::DeniedPolicy) {
                    assert_eq!(status, OwnedLifecycleStatusV8::Quarantined("first-ready"));
                    assert_eq!(actions, (1, 0, 0));
                }
                drop(runtime);
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
                assert_eq!((counts.borrow().starts, host.calls, releases), actions);
                assert!(journal.hold().is_err());
            }
        },
    );
}
#[test]
fn owned_runtime_two_turn_complete_projects_once_and_reopens_only_as_evidence() {
    exercise(Scenario::Complete);
}
#[test]
fn owned_runtime_two_turn_every_first_bridge_prewrite_retains_custody() {
    for offset in 1..=19 {
        exercise(Scenario::Prewrite(offset));
    }
}
#[test]
fn owned_runtime_two_turn_later_model_and_terminal_prewrite_retain_custody() {
    for offset in [28, 51] {
        exercise(Scenario::Prewrite(offset));
    }
}
#[test]
fn owned_runtime_two_turn_cancelled_and_denied_policy_do_not_dispatch_targets() {
    exercise(Scenario::Cancel);
    exercise(Scenario::DeniedPolicy);
}
#[test]
fn owned_runtime_two_turn_target_and_cleanup_failures_cannot_retry_or_close() {
    exercise(Scenario::TargetFailure);
    exercise(Scenario::ObserverPanic);
}
#[test]
fn owned_runtime_two_turn_failed_target_checked_stop_and_faults() {
    for offset in 1..=3 {
        exercise(Scenario::TargetFailureStopFault(offset));
    }
}

#[test]
fn owned_runtime_observer_failure_preserves_target_failure_and_closes_once() {
    exercise(Scenario::ObserverPanic);
    exercise(Scenario::ObserverTargetFailure);
}
#[test]
fn owned_runtime_observer_failure_every_append_fault_retains_custody() {
    for offset in 1..=3 {
        for after_write in [false, true] {
            exercise(Scenario::ObserverStopFault(offset, after_write));
        }
    }
}
#[test]
fn owned_runtime_observer_failure_state_panic_and_cancellation_never_retry() {
    exercise(Scenario::ObserverStatePanic);
    exercise(Scenario::ObserverCancelled);
}

#[test]
fn public_owned_agent_fresh_entry_runs_two_real_turns_and_projects_report() {
    std::thread::Builder::new()
        .name("public-owned-agent-default-stack".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(public_owned_agent_fresh_entry_on_default_stack)
        .unwrap()
        .join()
        .unwrap();
}

fn public_owned_agent_fresh_entry_on_default_stack() {
    use crate::agent_lifecycle::iterative::source_live::SourceLivePolicy;
    use crate::live_invocation::source_journal::{
        SourceOwnedAgentJournalV1, SourceOwnedAgentOpenErrorV1, SourceOwnedAgentStatusV1,
    };
    use std::os::unix::fs::PermissionsExt;
    CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(
        |context, _lease, _key, directory| {
            let (_, execution) = context.test_runtime_execution();
            let ordinary = execution.ordinary();
            let live_policy = SourceLivePolicy {
                deployment_binding: execution.model().digest().into(),
                response_limit: ordinary.response_limit(),
                ceiling: ordinary.ceiling(),
                reservation_units: ordinary.reservation_units(),
                unit: ordinary.unit().into(),
                clock_domain: ordinary.clock_domain().into(),
                initial_millis: ordinary.initial_millis(),
                deadline_millis: ordinary.deadline_millis(),
                max_total_steps: ordinary.max_total_steps().unwrap(),
                program_root: None,
            };
            let path = directory.parent().unwrap().join("public-owned-agent");
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut first_factory = factory(
                Rc::clone(&counts),
                script(&document(&context)),
                Rc::new(|_| {}),
            );
            let mut adapter = source(&context, &mut first_factory);
            let mut retention_calls = 0;
            let cancel = crate::agent_runtime::AgentCancellation::new();
            assert!(matches!(
                SourceOwnedAgentJournalV1::create_fresh(
                    context.test_runtime_arc(),
                    "src/missing.spx",
                    "fixture.agent",
                    "fixture.agent.type.step",
                    &adapter,
                    &live_policy,
                    &cancel,
                    &Clock,
                    execution.evaluation_fuel(),
                    File::open(&path).unwrap(),
                    7,
                    crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                    true,
                    |_| {
                        retention_calls += 1;
                        true
                    },
                ),
                Err(SourceOwnedAgentOpenErrorV1::Compiler(_))
            ));
            assert_eq!(retention_calls, 0);
            let opened = SourceOwnedAgentJournalV1::create_fresh(
                context.test_runtime_arc(),
                "src/app.spx",
                "fixture.agent",
                "fixture.agent.type.step",
                &adapter,
                &live_policy,
                &cancel,
                &Clock,
                execution.evaluation_fuel(),
                File::open(&path).unwrap(),
                7,
                crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                true,
                |facts| {
                    retention_calls += 1;
                    assert_eq!(facts["scope"]["policy_epoch"], 7);
                    assert!(facts["store_identity"]["file_inode"].as_u64().is_some());
                    assert!(facts["generation"].as_str().is_some());
                    true
                },
            )
            .unwrap();
            assert_eq!(retention_calls, 1);
            assert!(matches!(
                SourceOwnedAgentJournalV1::create_fresh(
                    context.test_runtime_arc(),
                    "src/app.spx",
                    "fixture.agent",
                    "fixture.agent.type.step",
                    &adapter,
                    &live_policy,
                    &cancel,
                    &Clock,
                    execution.evaluation_fuel(),
                    File::open(&path).unwrap(),
                    7,
                    crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                    true,
                    |_| panic!("nonempty store cannot obtain a new registration ACK"),
                ),
                Err(SourceOwnedAgentOpenErrorV1::Store(_))
            ));
            assert_eq!(counts.borrow().starts, 0);
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = Host {
                calls: 0,
                fail: false,
            };
            let mut releases = 0;
            let run = opened
                .run(&policy, &cancel, &Clock, &mut adapter, &mut host, |_| {
                    releases += 1;
                })
                .unwrap();
            assert_eq!(run.status(), SourceOwnedAgentStatusV1::Complete);
            assert_eq!(run.delivery_projection().unwrap()["kind"], "complete");
            assert_eq!((counts.borrow().starts, host.calls, releases), (2, 2, 4));
            let projection = match run.try_close() {
                Ok(projection) => projection,
                Err(_) => panic!("terminal run must close"),
            };
            assert!(projection.unwrap()["report"]["fields"].as_array().is_some());
            assert!(opened
                .run(
                    &policy,
                    &cancel,
                    &Clock,
                    &mut adapter,
                    &mut host,
                    |_| panic!("no replay")
                )
                .is_err());
            assert_eq!((counts.borrow().starts, host.calls, releases), (2, 2, 4));
            drop(adapter);
            let failed_path = directory
                .parent()
                .unwrap()
                .join("public-owned-agent-failed");
            std::fs::create_dir(&failed_path).unwrap();
            std::fs::set_permissions(&failed_path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let failed_counts = Rc::new(RefCell::new(Counts::default()));
            let mut failed_factory = factory(
                Rc::clone(&failed_counts),
                script(&document(&context)),
                Rc::new(|_| {}),
            );
            let mut failed_adapter = source(&context, &mut failed_factory);
            let failed_journal = SourceOwnedAgentJournalV1::create_fresh(
                context.test_runtime_arc(),
                "src/app.spx",
                "fixture.agent",
                "fixture.agent.type.step",
                &failed_adapter,
                &live_policy,
                &cancel,
                &Clock,
                execution.evaluation_fuel(),
                File::open(&failed_path).unwrap(),
                7,
                crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                true,
                |_| true,
            )
            .unwrap();
            let mut failed_host = Host {
                calls: 0,
                fail: true,
            };
            let mut failed_releases = 0;
            let failed = failed_journal
                .run(
                    &policy,
                    &cancel,
                    &Clock,
                    &mut failed_adapter,
                    &mut failed_host,
                    |_| failed_releases += 1,
                )
                .unwrap();
            assert_eq!(
                failed.status(),
                SourceOwnedAgentStatusV1::FailedEffectStopped
            );
            assert!(failed.delivery_projection().is_none());
            assert_eq!(
                (
                    failed_counts.borrow().starts,
                    failed_host.calls,
                    failed_releases
                ),
                (1, 1, 2)
            );
            assert!(matches!(failed.try_close(), Ok(None)));
        },
    );
}

const PUBLIC_RESTART_MODE: &str = "SEMAPRAX_PUBLIC_OWNED_RESTART_MODE";
const PUBLIC_RESTART_META: &str = "SEMAPRAX_PUBLIC_OWNED_RESTART_META";
#[derive(serde::Serialize, serde::Deserialize)]
struct PublicRestartMeta {
    directory: std::path::PathBuf,
    journal_file: std::path::PathBuf,
    retained_registration: serde_json::Value,
    preparer_pid: u32,
    resumed_pid_marker: std::path::PathBuf,
}

fn public_restart_policy(
    context: &CheckedOwnedWaitJournalContextV8,
) -> crate::agent_lifecycle::iterative::source_live::SourceLivePolicy {
    use crate::agent_lifecycle::iterative::source_live::SourceLivePolicy;
    let (_, execution) = context.test_runtime_execution();
    let ordinary = execution.ordinary();
    SourceLivePolicy {
        deployment_binding: execution.model().digest().into(),
        response_limit: ordinary.response_limit(),
        ceiling: ordinary.ceiling(),
        reservation_units: ordinary.reservation_units(),
        unit: ordinary.unit().into(),
        clock_domain: ordinary.clock_domain().into(),
        initial_millis: ordinary.initial_millis(),
        deadline_millis: ordinary.deadline_millis(),
        max_total_steps: ordinary.max_total_steps().unwrap(),
        program_root: None,
    }
}

#[test]
fn public_owned_agent_prepared_relaunch_child() {
    use crate::live_invocation::source_journal::{
        SourceOwnedAgentJournalV1, SourceOwnedAgentStatusV1,
    };
    use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;
    use std::os::unix::fs::PermissionsExt;
    let Some(mode) = std::env::var_os(PUBLIC_RESTART_MODE) else {
        return;
    };
    let metadata_path = std::path::PathBuf::from(std::env::var_os(PUBLIC_RESTART_META).unwrap());
    match mode.to_str() {
        Some("prepare") => {
            CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(
                |context, _lease, _key, fixture_directory| {
                    let directory = fixture_directory.parent().unwrap().join("public-restart");
                    std::fs::create_dir(&directory).unwrap();
                    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                        .unwrap();
                    let counts = Rc::new(RefCell::new(Counts::default()));
                    let mut factory = factory(
                        Rc::clone(&counts),
                        script(&document(&context)),
                        Rc::new(|_| {}),
                    );
                    let mut adapter = source(&context, &mut factory);
                    let cancel = crate::agent_runtime::AgentCancellation::new();
                    let mut retained_registration = None;
                    let opened = SourceOwnedAgentJournalV1::create_fresh(
                        context.test_runtime_arc(),
                        "src/app.spx",
                        "fixture.agent",
                        "fixture.agent.type.step",
                        &adapter,
                        &public_restart_policy(&context),
                        &cancel,
                        &Clock,
                        context.test_runtime_execution().1.evaluation_fuel(),
                        File::open(&directory).unwrap(),
                        7,
                        SourceCheckpointKey::new([73; 32]),
                        true,
                        |facts| {
                            retained_registration = Some(facts.clone());
                            true
                        },
                    )
                    .unwrap();
                    let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                    let mut host = Host {
                        calls: 0,
                        fail: false,
                    };
                    assert!(
                        opened
                            .restart_first_prepared(
                                &policy,
                                &cancel,
                                &Clock,
                                &mut adapter,
                                &mut host,
                                |_| {},
                                true,
                                true,
                            )
                            .is_err(),
                        "fresh history is not Prepared recovery"
                    );
                    assert_eq!((counts.borrow().starts, host.calls), (0, 0));
                    let parked = super::super::park(opened.test_journal(), &cancel);
                    drop(parked);
                    let journal_file = std::fs::read_dir(&directory)
                        .unwrap()
                        .next()
                        .unwrap()
                        .unwrap()
                        .path();
                    let meta = PublicRestartMeta {
                        directory,
                        journal_file,
                        retained_registration: retained_registration.unwrap(),
                        preparer_pid: std::process::id(),
                        resumed_pid_marker: metadata_path.with_extension("resumed-pid"),
                    };
                    drop(adapter);
                    drop(opened);
                    std::fs::write(&metadata_path, serde_json::to_vec(&meta).unwrap()).unwrap();
                },
            );
        }
        Some("resume") | Some("hostile") => {
            let meta: PublicRestartMeta =
                serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
            CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(
                |context, _lease, _key, _fixture_directory| {
                    let counts = Rc::new(RefCell::new(Counts::default()));
                    let mut factory = factory(
                        Rc::clone(&counts),
                        script(&document(&context)),
                        Rc::new(|_| {}),
                    );
                    let mut adapter = source(&context, &mut factory);
                    let cancel = crate::agent_runtime::AgentCancellation::new();
                    let open = || {
                        SourceOwnedAgentJournalV1::recover(
                            context.test_runtime_arc(),
                            "src/app.spx",
                            "fixture.agent",
                            "fixture.agent.type.step",
                            &adapter,
                            &public_restart_policy(&context),
                            &cancel,
                            &Clock,
                            context.test_runtime_execution().1.evaluation_fuel(),
                            File::open(&meta.directory).unwrap(),
                            7,
                            SourceCheckpointKey::new([73; 32]),
                            true,
                            &meta.retained_registration,
                        )
                    };
                    if mode.to_str() == Some("hostile") {
                        if let Ok(opened) = open() {
                            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                            let mut host = Host {
                                calls: 0,
                                fail: false,
                            };
                            assert!(
                                opened
                                    .restart_first_prepared(
                                        &policy,
                                        &cancel,
                                        &Clock,
                                        &mut adapter,
                                        &mut host,
                                        |_| {},
                                        true,
                                        true,
                                    )
                                    .is_err(),
                                "hostile tail must refuse before Model"
                            );
                            assert_eq!(host.calls, 0);
                        }
                        assert_eq!(counts.borrow().starts, 0);
                        return;
                    }
                    let mut wrong = meta.retained_registration.clone();
                    wrong["generation"] = serde_json::Value::String("forged".into());
                    assert!(
                        SourceOwnedAgentJournalV1::recover(
                            context.test_runtime_arc(),
                            "src/app.spx",
                            "fixture.agent",
                            "fixture.agent.type.step",
                            &adapter,
                            &public_restart_policy(&context),
                            &cancel,
                            &Clock,
                            context.test_runtime_execution().1.evaluation_fuel(),
                            File::open(&meta.directory).unwrap(),
                            7,
                            SourceCheckpointKey::new([73; 32]),
                            true,
                            &wrong,
                        )
                        .is_err(),
                        "foreign retained generation must refuse"
                    );
                    let opened = open().unwrap();
                    let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                    let mut host = Host {
                        calls: 0,
                        fail: false,
                    };
                    let mut releases = 0;
                    let run = opened
                        .restart_first_prepared(
                            &policy,
                            &cancel,
                            &Clock,
                            &mut adapter,
                            &mut host,
                            |_| releases += 1,
                            true,
                            true,
                        )
                        .unwrap();
                    assert_eq!(run.status(), SourceOwnedAgentStatusV1::Complete);
                    assert_eq!(run.delivery_projection().unwrap()["kind"], "complete");
                    assert_eq!((counts.borrow().starts, host.calls, releases), (2, 2, 4));
                    match run.try_close() {
                        Ok(Some(_)) => {}
                        _ => panic!("recovered Complete must close with Report projection"),
                    }
                    std::fs::write(meta.resumed_pid_marker, std::process::id().to_string())
                        .unwrap();
                },
            );
        }
        _ => panic!("unknown public restart child mode"),
    }
}

#[test]
fn public_owned_agent_prepared_relaunch_completes_and_hostile_tail_refuses() {
    for mode in ["resume", "hostile"] {
        let root = std::env::temp_dir().join(format!(
            "spx-public-owned-restart-{}-{mode}",
            std::process::id(),
        ));
        std::fs::create_dir(&root).unwrap();
        let metadata = root.join("retained.json");
        for (child_mode, keep_fixture) in [("prepare", true), (mode, false)] {
            if child_mode == "hostile" {
                let meta: PublicRestartMeta =
                    serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&meta.journal_file)
                    .unwrap()
                    .write_all(b"hostile")
                    .unwrap();
            }
            let module = module_path!();
            let child = module
                .strip_prefix(concat!(env!("CARGO_PKG_NAME"), "::"))
                .unwrap_or(module);
            let test_name = format!("{child}::public_owned_agent_prepared_relaunch_child");
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", test_name.as_str(), "--nocapture"])
                .env(PUBLIC_RESTART_MODE, child_mode)
                .env(PUBLIC_RESTART_META, &metadata)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            if keep_fixture {
                command.env("SEMAPRAX_KEEP_OWNED_WAIT_CONTEXT_FIXTURE", "1");
            } else {
                command.env_remove("SEMAPRAX_KEEP_OWNED_WAIT_CONTEXT_FIXTURE");
            }
            assert!(
                command.status().unwrap().success(),
                "public restart child {child_mode} failed"
            );
        }
        let meta: PublicRestartMeta =
            serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
        assert_ne!(meta.preparer_pid, std::process::id());
        if mode == "resume" {
            let resumed = std::fs::read_to_string(&meta.resumed_pid_marker)
                .unwrap()
                .parse::<u32>()
                .unwrap();
            assert_ne!(meta.preparer_pid, resumed);
        }
        std::fs::remove_dir_all(meta.directory.parent().unwrap()).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}

mod later_target;

mod refusal;

mod transferred;
