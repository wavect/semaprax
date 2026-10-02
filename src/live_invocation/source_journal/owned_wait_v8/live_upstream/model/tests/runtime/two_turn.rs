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
                    Scenario::TargetFailure | Scenario::TargetFailureStopFault(_)
                ),
            };
            let mut releases = 0;
            let status =
                runtime
                    .session()
                    .finish_two_turn_run(&policy, &mut adapter, &mut host, |_| {
                        releases += 1;
                        assert!(
                            !matches!(scenario, Scenario::ObserverPanic),
                            "injected cleanup observer panic"
                        );
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
            if matches!(
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
                if matches!(scenario, Scenario::ObserverPanic) {
                    assert_eq!((actions.0, actions.1), (1, 1));
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
