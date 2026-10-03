//! Public completed-State custody has an explicit, checked shutdown boundary.
use super::*;
use crate::live_invocation::source_journal::{SourceOwnedAgentJournalV1, SourceOwnedAgentStatusV1};
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Copy)]
enum ShutdownCase {
    Explicit,
    Cancelled,
    Fault { offset: usize, after: bool },
    ObserverPanic,
}
fn exercise_shutdown(case: ShutdownCase) {
    std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(move || {
        CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(|context, _lease, _key, directory| {
            let live_policy = public_restart_policy(&context);
            let (_, execution) = context.test_runtime_execution();
            let response = document(&context);
            let path = directory.parent().unwrap().join("public-completed-shutdown");
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
            let mut host = Host { calls: 0, fail: false };
            let run = opened.prepare_first_model(&cancel, &Clock, &mut adapter, |_| panic!("first Model retains State")).unwrap();
            assert_eq!(run.status(), SourceOwnedAgentStatusV1::ModelCompleted);
            let weak = run.test_backings();
            assert!(weak.iter().all(|root| root.strong_count() == 1));
            let mut run = run.try_close().err().expect("completed State stays in opaque custody");
            let before = journal.begin_session().unwrap().sequence();
            if let ShutdownCase::Fault {offset, after} = case {
                if after { journal.test_observe_lease().borrow_mut().test_fail_after_write(before + offset); }
                else { journal.test_observe_lease().borrow_mut().test_fail_before_write(before + offset); }
            }
            let mut releases = 0;
            let mut observe = |_: &crate::cleanup_plan::FinalizeAction| { releases += 1; assert!(!matches!(case, ShutdownCase::ObserverPanic), "injected completed State observer panic"); };
            let status = if matches!(case, ShutdownCase::Cancelled) {
                cancel.cancel();
                run.finish(&policy, &mut adapter, &mut host, &mut observe)
            } else { run.shutdown(&mut observe) };
            let succeeded = matches!(case, ShutdownCase::Explicit | ShutdownCase::Cancelled);
            assert_eq!(status, if succeeded { SourceOwnedAgentStatusV1::ShutdownStopped } else { SourceOwnedAgentStatusV1::Quarantined("completed-state-shutdown") });
            let expected_releases = usize::from(!matches!(case, ShutdownCase::Fault {offset: 1 | 2, ..}));
            assert_eq!((counts.borrow().starts, host.calls, releases), (1, 0, expected_releases));
            assert!(run.delivery_projection().is_none());
            let bytes = journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap();
            if matches!(case, ShutdownCase::ObserverPanic) {
                let lease = journal.test_observe_lease().borrow();
                let key = crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([74;32]);
                let inventory = crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8::recover(journal.context(),&lease,&key,&bytes).unwrap();
                assert!(inventory.prepare(&lease, EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn:Some(0),attempt:Some(0),status:crate::live_invocation::source_journal::SourceStopStatus::Cancelled,
                    reason:crate::live_invocation::source_journal::SourceStopReason::Cancelled,
                })).is_err(), "failed receipt cannot be reminted into completed Stop");
            }

            assert_eq!(run.shutdown(|_|panic!("no repeated shutdown release")),status);
            assert_eq!(run.finish(&policy,&mut adapter,&mut host, |_|panic!("no repeated finish release")),status);
            assert_eq!(journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap(),bytes);
            if succeeded {
                let session = journal.begin_session().unwrap();
                assert_eq!(session.sequence(),before+4);
                let rows = session.inventory.test_observe_entries();
                assert!(matches!(rows.last().map(|r| &r.entry),Some(EntryV8::Ordinary(SourceJournalEntry::Stop { status:crate::live_invocation::source_journal::SourceStopStatus::Cancelled, reason:crate::live_invocation::source_journal::SourceStopReason::Cancelled, .. }))));
                assert!(!rows.iter().any(|r| matches!(&r.entry,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedAuthorizationStaged { .. }))));
                assert!(weak.iter().all(|root|root.upgrade().is_none()));
                drop(session);
                assert!(matches!(run.try_close(),Ok(None)));
            } else {
                if expected_releases == 0 { assert!(weak.iter().all(|root|root.strong_count()==1)); }
                else { assert!(weak.iter().all(|root|root.upgrade().is_none())); }
                let run = run.try_close().err().expect("uncertain custody cannot close");
                assert!(journal.begin_session().is_err());
                drop(run);
                assert!(weak.iter().all(|root|root.upgrade().is_none()));
            }
            assert!(opened.prepare_first_model(&cancel,&Clock,&mut adapter, |_|panic!("no fresh retry")).is_err());
            assert_eq!(journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap(),bytes);
            assert_eq!((counts.borrow().starts,host.calls,releases),(1,0,expected_releases));
        });
    }).unwrap().join().unwrap();
}
#[test]
fn public_owned_agent_completed_state_explicit_shutdown_and_cancellation_stop_once() {
    for case in [ShutdownCase::Explicit, ShutdownCase::Cancelled] {
        exercise_shutdown(case);
    }
}
#[test]
fn public_owned_agent_completed_shutdown_failure_and_started_ack_faults_retain_actual_owner() {
    for offset in [1, 2] {
        for after in [false, true] {
            exercise_shutdown(ShutdownCase::Fault { offset, after });
        }
    }
}
#[test]
fn public_owned_agent_completed_shutdown_receipt_and_stop_ack_faults_never_retry() {
    for offset in [3, 4] {
        for after in [false, true] {
            exercise_shutdown(ShutdownCase::Fault { offset, after });
        }
    }
}
#[test]
fn public_owned_agent_completed_shutdown_observer_failure_cannot_publish_stop() {
    exercise_shutdown(ShutdownCase::ObserverPanic);
}
