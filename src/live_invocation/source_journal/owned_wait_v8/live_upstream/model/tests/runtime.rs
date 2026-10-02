//! Actual initial lifecycle execution with ephemeral caller handles. The
//! physical roots are observed from Initialize, not fabricated registry values.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::runtime::{
    OwnedLifecycleRuntimeV8, OwnedLifecycleStatusV8,
};

#[test]
fn owned_runtime_first_model_owner_outlives_session_and_close_refuses() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_cumulative_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let input = super::super::super::tests::input(journal.context());
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(
            Rc::clone(&counts),
            script(&document(journal.context())),
            Rc::new(|_| {}),
        );
        let mut adapter = source(journal.context(), &mut factory);
        let mut runtime = OwnedLifecycleRuntimeV8::open(&journal, &cancel).unwrap();
        {
            let mut session = runtime.session();
            assert_eq!(
                session.run_first_turn_model(input, &mut adapter, &Clock),
                OwnedLifecycleStatusV8::ModelCompleted
            );
        }
        let weak = runtime.test_backings();
        assert!(!weak.is_empty());
        assert!(weak.iter().all(|root| root.strong_count() == 1));
        assert_eq!(counts.borrow().starts, 1);
        let mut runtime = runtime
            .try_close()
            .err()
            .expect("completed State is still a pending physical obligation");
        let before = journal.begin_session().unwrap().sequence();
        {
            let mut session = runtime.session();
            assert_eq!(
                session.run_first_turn_model(
                    super::super::super::tests::input(journal.context()),
                    &mut adapter,
                    &Clock
                ),
                OwnedLifecycleStatusV8::ModelCompleted
            );
        }
        assert_eq!(
            counts.borrow().starts,
            1,
            "reopening a handle never redispatches"
        );
        assert_eq!(journal.begin_session().unwrap().sequence(), before);
        assert!(weak.iter().all(|root| root.strong_count() == 1));
        drop(runtime);
        assert!(
            journal.hold().is_err(),
            "forced runtime teardown retires authority before backing"
        );
        assert!(weak.iter().all(|root| root.upgrade().is_none()));
    });
}

fn failed_observe_runtime(fault_offset: Option<usize>, observer_panics: bool) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_initial_observe_ensures_store(
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let input = super::super::super::tests::input(journal.context());
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(
                Rc::clone(&counts),
                script(&document(journal.context())),
                Rc::new(|_| {}),
            );
            let mut adapter = source(journal.context(), &mut factory);
            let mut runtime = OwnedLifecycleRuntimeV8::open(&journal, &cancel).unwrap();
            {
                let mut session = runtime.session();
                assert_eq!(
                    session.run_first_turn_model(input, &mut adapter, &Clock),
                    OwnedLifecycleStatusV8::ObserveCleanupPending
                );
            }
            let weak = runtime.test_backings();
            assert!(!weak.is_empty());
            assert!(weak.iter().all(|root| root.strong_count() == 1));
            assert_eq!(counts.borrow().factories, 0);
            let mut runtime = runtime
                .try_close()
                .err()
                .expect("unsettled State survives refused runtime close");
            let before = journal.begin_session().unwrap().sequence();
            if let Some(offset) = fault_offset {
                journal
                    .test_observe_lease()
                    .borrow_mut()
                    .test_fail_before_write(before + offset);
            }
            let mut calls = 0;
            let status = runtime.settle_failed_observe(|_| {
                calls += 1;
                assert!(!observer_panics, "explicit observer failure");
            });
            let called = calls;
            assert_eq!(
                runtime.settle_failed_observe(|_| calls += 1),
                status,
                "settlement is one-use"
            );
            assert_eq!(calls, called);
            assert_eq!(counts.borrow().factories, 0);
            if fault_offset.is_some() || observer_panics {
                assert_eq!(
                    status,
                    OwnedLifecycleStatusV8::Quarantined("observe-cleanup")
                );
                let mut runtime = runtime
                    .try_close()
                    .err()
                    .expect("incomplete cleanup cannot close the runtime");
                assert!(journal.hold().is_err());
                if fault_offset == Some(1) {
                    assert_eq!(calls, 0);
                    assert!(weak.iter().all(|root| root.strong_count() == 1));
                } else {
                    assert!(calls > 0);
                }
                drop(runtime.session());
                assert_eq!(runtime.status(), status);
            } else {
                assert_eq!(status, OwnedLifecycleStatusV8::ObserveStopped);
                assert!(calls > 0);
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
                assert_eq!(journal.begin_session().unwrap().sequence(), before + 3);
                assert!(runtime.try_close().is_ok());
            }
        },
    );
}

#[test]
fn owned_runtime_dropped_observe_session_is_settled_by_runtime_once() {
    failed_observe_runtime(None, false);
}
#[test]
fn owned_runtime_observe_started_fault_retains_owner_after_session_drop() {
    failed_observe_runtime(Some(1), false);
}
#[test]
fn owned_runtime_observe_receipt_fault_cannot_retry_cleanup_or_close() {
    failed_observe_runtime(Some(2), false);
}
#[test]
fn owned_runtime_observe_stop_fault_cannot_retry_cleanup_or_close() {
    failed_observe_runtime(Some(3), false);
}
#[test]
fn owned_runtime_observe_panicking_observer_stays_unsettled() {
    failed_observe_runtime(None, true);
}

mod restart;
mod two_turn;

mod continued_observe;
