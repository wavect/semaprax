//! Public second-target failure and the three physical State-closure ACK windows.
use super::*;
use crate::live_invocation::source_journal::{SourceOwnedAgentJournalV1, SourceOwnedAgentStatusV1};
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Copy, Debug)]
enum Case {
    Complete,
    Fault(usize, bool),
    StateObserverPanic,
    CancelAfterStateRelease,
}
struct SecondTargetFails<'j> {
    calls: usize,
    journal: &'j SourceOwnedWaitJournalV8,
    case: Case,
}
impl TargetHostHandler for SecondTargetFails<'_> {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        if self.calls == 1 {
            return Host {
                calls: 0,
                fail: false,
            }
            .dispatch(request, sink);
        }
        assert_eq!(self.calls, 2, "no third target dispatch");
        if let Case::Fault(offset, after_write) = self.case {
            // Effect settlement, recorded evidence, Decision Started and receipt
            // precede the State Started/receipt/Stop append numbers.
            let append = self.journal.begin_session().unwrap().sequence() + 4 + offset;
            let lease = self.journal.test_observe_lease();
            let mut lease = lease.borrow_mut();
            if after_write {
                lease.test_fail_after_write(append);
            } else {
                lease.test_fail_before_write(append);
            }
        }
        Err(TargetHostError::Failed)
    }
}
fn exercise(case: Case) {
    std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(move || {
        CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(|context, _lease, _key, directory| {
            let live_policy = public_restart_policy(&context);
            let (_, execution) = context.test_runtime_execution();
            let path = directory.parent().unwrap().join("public-second-target-failure");
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(Rc::clone(&counts), script(&document(&context)), Rc::new(|_| {}));
            let mut adapter = source(&context, &mut factory);
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let opened = SourceOwnedAgentJournalV1::create_fresh(
                context.test_runtime_arc(), "src/app.spx", "fixture.agent", "fixture.agent.type.step",
                &adapter, &live_policy, &cancel, &Clock, execution.evaluation_fuel(), File::open(&path).unwrap(),
                7, crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]), true, |_| true,
            ).unwrap();
            let journal = opened.test_journal();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let mut host = SecondTargetFails { calls: 0, journal, case };
            let mut releases = 0;
            let run = opened.run(&policy, &cancel, &Clock, &mut adapter, &mut host, |_| {
                releases += 1;
                if releases == 4 {
                    if matches!(case, Case::CancelAfterStateRelease) { cancel.cancel(); }
                    assert!(!matches!(case, Case::StateObserverPanic), "injected State cleanup observer panic");
                }
            }).unwrap();
            assert_eq!((counts.borrow().starts, host.calls), (2, 2));
            assert_eq!(releases, if matches!(case, Case::Fault(1, _)) { 3 } else { 4 });
            assert!(run.delivery_projection().is_none());
            let persisted = journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap();
            if matches!(case, Case::Complete) {
                assert_eq!(run.status(), SourceOwnedAgentStatusV1::FailedEffectStopped);
                let session = journal.begin_session().unwrap();
                let (_, _, turn, attempt, selected) = session.inventory.failed_effect_state_facts().unwrap();
                assert_eq!((turn, attempt), (1, 0));
                assert!(matches!(selected, EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn: Some(1), attempt: Some(0), status: crate::live_invocation::source_journal::SourceStopStatus::EffectFailed,
                    reason: crate::live_invocation::source_journal::SourceStopReason::EffectFailed,
                })));
                drop(session);
                assert!(matches!(run.try_close(), Ok(None)));
                assert!(journal.begin_session().is_err(), "dropping a stopped failure holder retires ordinary append authority");
            } else {
                assert_eq!(run.status(), SourceOwnedAgentStatusV1::Quarantined("continued-failed-effect-cleanup"));
                let run = run.try_close().err().expect("unsettled or uncertain owner cannot close");
                assert!(journal.begin_session().is_err());
                drop(run);
            }
            assert!(opened.run(&policy, &cancel, &Clock, &mut adapter, &mut host, |_| panic!("no cleanup retry")).is_err());
            assert_eq!((counts.borrow().starts, host.calls), (2, 2));
            assert_eq!(journal.test_observe_lease().borrow().test_persisted_snapshot().unwrap(), persisted, "no append or dispatch after reached failure");
            assert_eq!(releases, if matches!(case, Case::Fault(1, _)) { 3 } else { 4 }, "Drop does not finalize twice");
        });
    }).unwrap().join().unwrap();
}
#[test]
fn public_owned_agent_second_target_failure_stops_on_default_stack() {
    exercise(Case::Complete);
}
#[test]
fn public_owned_agent_second_target_failure_started_ack_faults() {
    exercise(Case::Fault(1, false));
    exercise(Case::Fault(1, true));
}
#[test]
fn public_owned_agent_second_target_failure_receipt_ack_faults() {
    exercise(Case::Fault(2, false));
    exercise(Case::Fault(2, true));
}
#[test]
fn public_owned_agent_second_target_failure_stop_ack_faults() {
    exercise(Case::Fault(3, false));
    exercise(Case::Fault(3, true));
}
#[test]
fn public_owned_agent_second_target_failure_state_observer_panic() {
    exercise(Case::StateObserverPanic);
}
#[test]
fn public_owned_agent_second_target_failure_cancelled_stop() {
    exercise(Case::CancelAfterStateRelease);
}
