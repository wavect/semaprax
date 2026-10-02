//! Actual public source Refused has no target dispatch and settles State once.
use super::*;
use crate::live_invocation::source_journal::{SourceOwnedAgentJournalV1, SourceOwnedAgentStatusV1};
use std::os::unix::fs::PermissionsExt;

fn refused_document(context: &CheckedOwnedWaitJournalContextV8, budget: i64) -> Vec<u8> {
    let original = String::from_utf8(document(context)).unwrap();
    let field = "\"fixture.agent.type.proposal.budget\":\"3\"";
    assert!(original.contains(field));
    original
        .replacen(
            field,
            &format!("\"fixture.agent.type.proposal.budget\":\"{budget}\""),
            1,
        )
        .into_bytes()
}

fn public_refused(observer_panics: bool) {
    std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(move || {
        CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(|context, _lease, _key, directory| {
            let live_policy = public_restart_policy(&context);
            let (runtime, execution) = context.test_runtime_execution();
            let refused_budget = runtime.owned_wait_task_v8(execution).unwrap().budget + 1;
            let response = refused_document(&context, refused_budget);
            let path = directory.parent().unwrap().join("public-refused-state");
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
            let mut adapter = source(&context, &mut factory);
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let opened = SourceOwnedAgentJournalV1::create_fresh(
                context.test_runtime_arc(), "src/app.spx", "fixture.agent", "fixture.agent.type.step",
                &adapter, &live_policy, &cancel, &Clock, execution.evaluation_fuel(), File::open(&path).unwrap(),
                7, crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([74; 32]), true, |_| true,
            ).unwrap();
            let journal = opened.test_journal();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = Host { calls:0, fail:false };
            let mut releases = 0;
            let run = opened.run(&policy, &cancel, &Clock, &mut adapter, &mut host, |_| { releases += 1; assert!(!observer_panics, "injected Refused State observer panic"); }).unwrap();
            if observer_panics {
                assert_eq!(run.status(),SourceOwnedAgentStatusV1::Quarantined("refused-state-cleanup"));
                assert_eq!((counts.borrow().starts,host.calls,releases),(1,0,1));
                let persisted = journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap();
                let lease = journal.test_observe_lease().borrow();
                let key = crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([74;32]);
                let inventory = crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8::recover(journal.context(),&lease,&key,&persisted).unwrap();
                assert!(inventory.prepare(&lease, EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn:Some(0),attempt:Some(0),status:crate::live_invocation::source_journal::SourceStopStatus::Rejected,
                    reason:crate::live_invocation::source_journal::SourceStopReason::StageRefused,
                })).is_err(), "failed receipt cannot be reminted into completed Stop");
                drop(lease);
                let run = run.try_close().err().expect("observer failure retains custody");
                assert!(opened.run(&policy,&cancel,&Clock,&mut adapter,&mut host, |_|panic!("no cleanup retry")).is_err());
                drop(run);
                assert_eq!(journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap(),persisted);
                return;
            }
            assert_eq!(run.status(), SourceOwnedAgentStatusV1::AuthorizationRefusedStopped);
            assert_eq!((counts.borrow().starts, host.calls, releases), (1,0,1));
            assert!(run.delivery_projection().is_none());
            let session = journal.begin_session().unwrap();
            let entries = session.inventory.test_observe_entries();
            assert!(matches!(entries.last().map(|r|&r.entry), Some(EntryV8::Ordinary(SourceJournalEntry::Stop {
                turn:Some(0), attempt:Some(0), status:crate::live_invocation::source_journal::SourceStopStatus::Rejected,
                reason:crate::live_invocation::source_journal::SourceStopReason::StageRefused,
            }))));
            assert_eq!(entries.iter().filter(|r| matches!(r.entry, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedCleanupStarted { .. }))).count(),1);
            drop(session);
            let persisted = journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap();
            assert!(matches!(run.try_close(), Ok(None)));
            assert!(opened.run(&policy, &cancel, &Clock, &mut adapter, &mut host, |_| panic!("no second cleanup")).is_err());
            assert_eq!(journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap(),persisted);
            assert_eq!((counts.borrow().starts, host.calls, releases),(1,0,1));
        });
    }).unwrap().join().unwrap();
}

#[test]
fn public_owned_agent_authorization_refused_cleans_state_then_stops() {
    public_refused(false);
}
#[test]
fn public_owned_agent_refused_observer_failure_cannot_publish_stop() {
    public_refused(true);
}

#[test]
fn owned_runtime_refused_started_ack_faults_preserve_owner_without_release() {
    for after_write in [false, true] {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
                    true,
                    |context, lease, key, _directory| {
                        let context =
                            Arc::new(context.with_cumulative_initialization(&lease).unwrap());
                        let journal = SourceOwnedWaitJournalV8::open(context, key, lease).unwrap();
                        let (rt, execution) = journal.context().test_runtime_execution();
                        let budget = rt.owned_wait_task_v8(execution).unwrap().budget + 1;
                        let response = refused_document(journal.context(), budget);
                        let counts = Rc::new(RefCell::new(Counts::default()));
                        let mut factory =
                            factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
                        let mut adapter = source(journal.context(), &mut factory);
                        let cancel = crate::agent_runtime::AgentCancellation::new();
                        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                        let mut runtime = OwnedLifecycleRuntimeV8::open(&journal, &cancel).unwrap();
                        let input =
                            super::super::super::super::super::tests::input(journal.context());
                        assert_eq!(
                            runtime
                                .session()
                                .run_first_turn_model(input, &mut adapter, &Clock),
                            OwnedLifecycleStatusV8::ModelCompleted
                        );
                        let weak = runtime.test_backings();
                        let before = journal.begin_session().unwrap().sequence();
                        // Admitted, Transfer Reserved/Completed, Authorize reservation,
                        // Staged, refusal, then State Started.
                        if after_write {
                            journal
                                .test_observe_lease()
                                .borrow_mut()
                                .test_fail_after_write(before + 7);
                        } else {
                            journal
                                .test_observe_lease()
                                .borrow_mut()
                                .test_fail_before_write(before + 7);
                        }
                        let mut host = Host {
                            calls: 0,
                            fail: false,
                        };
                        assert_eq!(
                            runtime.session().finish_two_turn_run(
                                &policy,
                                &mut adapter,
                                &mut host,
                                |_| panic!("no release without Started ACK")
                            ),
                            OwnedLifecycleStatusV8::Quarantined("refused-state-cleanup")
                        );
                        assert_eq!((counts.borrow().starts, host.calls), (1, 0));
                        assert!(weak.iter().all(|root| root.strong_count() == 1));
                        let bytes = journal
                            .test_observe_lease()
                            .borrow()
                            .test_persisted_snapshot()
                            .unwrap();
                        let runtime = runtime
                            .try_close()
                            .err()
                            .expect("retain uncertain actual owner");
                        assert!(journal.begin_session().is_err());
                        drop(runtime);
                        assert!(weak.iter().all(|root| root.upgrade().is_none()));
                        assert_eq!(
                            journal
                                .test_observe_lease()
                                .borrow()
                                .test_persisted_snapshot()
                                .unwrap(),
                            bytes
                        );
                    },
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
