//! A real second-turn Observe refusal remains cleanup-capable in runtime custody.
use super::*;
use crate::resumable_effects::CapabilityPolicy;

fn exercise(fault: Option<(usize, bool)>, observer_panics: bool) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_continued_observe_ensures_store(
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
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
            let mut host = super::two_turn::Host {
                calls: 0,
                fail: false,
            };
            let mut releases = 0;
            assert_eq!(
                runtime
                    .session()
                    .finish_two_turn_run(&policy, &mut adapter, &mut host, |_| releases += 1),
                OwnedLifecycleStatusV8::ObserveCleanupPending
            );
            assert_eq!((counts.borrow().starts, host.calls, releases), (1, 1, 2));
            assert!(weak.iter().all(|root| root.strong_count() == 1));
            let mut runtime = runtime
                .try_close()
                .err()
                .expect("actual continued State remains live");
            let before = journal.begin_session().unwrap().sequence();
            if let Some((offset, after_write)) = fault {
                let lease = journal.test_observe_lease();
                let mut lease = lease.borrow_mut();
                if after_write {
                    lease.test_fail_after_write(before + offset);
                } else {
                    lease.test_fail_before_write(before + offset);
                }
            }
            let mut state_releases = 0;
            let status = runtime.settle_failed_observe(|_| {
                state_releases += 1;
                assert!(!observer_panics, "injected continued State observer panic");
            });
            assert_eq!(
                state_releases,
                if matches!(fault, Some((1, _))) { 0 } else { 1 }
            );
            let persisted = journal
                .test_observe_lease()
                .borrow()
                .test_persisted_snapshot()
                .unwrap();
            assert_eq!(
                runtime.settle_failed_observe(|_| panic!("cannot retry State cleanup")),
                status
            );
            assert_eq!(
                runtime.session().finish_two_turn_run(
                    &policy,
                    &mut adapter,
                    &mut host,
                    |_| panic!("cannot retry turn")
                ),
                status
            );
            assert_eq!(
                journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap(),
                persisted
            );
            assert_eq!((counts.borrow().starts, host.calls, releases), (1, 1, 2));
            assert!(runtime.delivery_projection().is_none());
            if fault.is_none() && !observer_panics {
                assert_eq!(status, OwnedLifecycleStatusV8::ObserveStopped);
                assert_eq!(journal.begin_session().unwrap().sequence(), before + 3);
                let row: serde_json::Value = serde_json::from_slice(
                    persisted
                        .split(|b| *b == b'\n')
                        .filter(|row| !row.is_empty())
                        .last()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(row["kind"], "stop");
                assert_eq!(row["turn"], 1);
                assert_eq!(row["reason"], "stage_refused");
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
                assert!(runtime.try_close().is_ok());
            } else {
                assert_eq!(
                    status,
                    OwnedLifecycleStatusV8::Quarantined("observe-cleanup")
                );
                assert!(journal.hold().is_err());
                let runtime = runtime
                    .try_close()
                    .err()
                    .expect("incomplete cleanup retains runtime");
                assert_eq!(
                    weak.iter().any(|root| root.strong_count() == 1),
                    state_releases == 0
                );
                drop(runtime);
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
            }
        },
    );
}

#[test]
fn owned_runtime_continued_observe_failure_cleans_same_state_and_stops() {
    exercise(None, false);
}
#[test]
fn owned_runtime_continued_observe_failure_append_faults_never_retry() {
    for offset in 1..=3 {
        for after_write in [false, true] {
            exercise(Some((offset, after_write)), false);
        }
    }
}
#[test]
fn owned_runtime_continued_observe_failure_state_observer_panic_never_retries() {
    exercise(None, true);
}
