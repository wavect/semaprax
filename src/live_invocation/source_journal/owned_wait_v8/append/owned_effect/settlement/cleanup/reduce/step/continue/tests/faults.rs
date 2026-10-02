use super::*;

#[test]
#[cfg(unix)]
fn owned_continue_state_and_observe_real_append_faults_never_evaluate_or_remint() {
    for observe in [false, true] {
        for after in [false, true] {
            with_moved(|journal, moved, weak, _, _| {
                let entries_before = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
                let mut obligation = moved
                    .prepare_continue()
                    .unwrap_or_else(|_| panic!("State selection"));
                if observe {
                    let LiveContinueAcknowledgedV8::State(state) = ack(journal, obligation)
                        .advance_continue()
                        .unwrap_or_else(|_| panic!("State ACK"))
                    else {
                        panic!()
                    };
                    obligation = state
                        .prepare_observe()
                        .unwrap_or_else(|_| panic!("Observe original selection"));
                }
                {
                    let number = obligation.sequence() + 1;
                    let mut lease = journal.lease.borrow_mut();
                    if after {
                        lease.test_fail_after_sync(number)
                    } else {
                        lease.test_fail_before_write(number)
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continue(obligation)
                    .err()
                    .expect("actual persistence failure");
                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries_before);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
                false
            });
        }
    }
}

#[test]
fn owned_continue_failed_observe_retains_actual_state_and_observed_consumption() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_continued_observe_ensures_store(
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedRunCreated { execution, .. } = &context.fold().created else {
                panic!("actual Created execution");
            };
            assert_eq!(
                execution,
                context.ready_runtime().unwrap().1.ordinary().invocation()
            );
            assert_ne!(
                execution,
                context.ordinary().invocation(),
                "E and derived I8 are distinct"
            );
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock { now: Cell::new(1) };
            super::super::tests::test_moved(&journal, &cancel, &policy, &clock, |moved, weak| {
                let ledger = *moved.accounting();
                let LiveContinueAcknowledgedV8::State(state) = ack(
                    &journal,
                    moved
                        .prepare_continue()
                        .unwrap_or_else(|_| panic!("Continue")),
                )
                .advance_continue()
                .unwrap_or_else(|_| panic!("State ACK")) else {
                    panic!("State")
                };
                let LiveContinueAcknowledgedV8::Observed(failed) = ack(
                    &journal,
                    state
                        .prepare_observe()
                        .unwrap_or_else(|_| panic!("Observe reservation")),
                )
                .advance_continue()
                .unwrap_or_else(|_| panic!("actual failed Observe retained")) else {
                    panic!("Observe outcome")
                };
                assert!(failed.is_failed());
                assert!(!failed.is_observed());
                assert_eq!(failed.turn(), 1);
                assert!(failed.consumed() > 0);
                assert_eq!(failed.accounting(), &ledger);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        },
    );
}
