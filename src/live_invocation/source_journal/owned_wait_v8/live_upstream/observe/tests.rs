use super::*;
use std::sync::Arc;
#[test]
fn owned_wait_live_observe_moves_same_state_once_after_actual_reservation_ack() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let input = super::super::tests::input(&context);
        let context = context.with_initialization(&lease).unwrap();
        let allowance = context.ordinary().max_steps_per_stage().unwrap() as u64;
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let initialized = match initialize_live_actor_v8(&journal, input, &cancel) {
            Ok(Ok(x)) => x,
            _ => panic!("actual initialized actor"),
        };
        let original = initialized.owner.test_weak();
        let observed = match observe_live_actor_v8(initialized) {
            Ok(x) => x,
            Err(_) => panic!("actual Observe and ACK"),
        };
        assert_eq!(
            (
                observed.reservation,
                observed.observed,
                observed.session.sequence()
            ),
            (5, 6, 7)
        );
        assert!(observed.owner.consumed() > 0 && observed.owner.consumed() <= allowance);
        let current = observed.owner.test_weak();
        assert_eq!(current.len(), original.len());
        assert!(current.iter().all(
            |w| w.strong_count() == 1 && original.iter().any(|o| std::sync::Weak::ptr_eq(w, o))
        ));
        assert_eq!(
            observed.session.fold_for_live_test().reserved_total,
            2 * allowance
        );
        assert_eq!(observed.session.fold_for_live_test().stages, 2);
        observed.held.validate_guard().unwrap();
        assert_eq!(journal.begin_session().unwrap().sequence(), 7);
        let count = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = std::rc::Rc::clone(&count);
        crate::interpreter::resumable::owned_frame::snapshot::observe_releases(Some(Box::new(
            move |_| seen.set(seen.get() + 1),
        )));
        drop(observed);
        crate::interpreter::resumable::owned_frame::snapshot::observe_releases(None);
        assert_eq!(count.get(), 0);
        assert!(original.iter().all(|w| w.upgrade().is_none()));
        journal.hold().unwrap().validate_guard().unwrap();
    });
}
#[test]
fn owned_wait_live_observe_ack_faults_keep_owner_and_never_repeat_source() {
    for append in [6, 7] {
        for persisted in [false, true] {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
                |context, mut lease, key| {
                    let input = super::super::tests::input(&context);
                    let context = context.with_initialization(&lease).unwrap();
                    if persisted {
                        lease.test_fail_after_write(append)
                    } else {
                        lease.test_fail_before_write(append)
                    }
                    let journal =
                        SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                    let cancel = crate::agent_runtime::AgentCancellation::new();
                    let initialized = match initialize_live_actor_v8(&journal, input, &cancel) {
                        Ok(Ok(x)) => x,
                        _ => panic!("init before Observe fault"),
                    };
                    let weak = initialized.owner.test_weak();
                    let failed = match observe_live_actor_v8(initialized) {
                        Err(x) => x,
                        Ok(_) => panic!("no live continuation after failed ACK"),
                    };
                    let LiveObserveFailureV8::Legacy { owner, held, error } = &failed else {
                        panic!("default legacy failure")
                    };
                    assert_eq!(*error, SourceJournalError::Uncertain);
                    if append == 6 {
                        assert!(
                            matches!(owner, LiveObserveOutcomeV8::Refused(_)),
                            "no Observe evaluation without reservation ACK"
                        );
                    } else {
                        let LiveObserveOutcomeV8::GuardLost(owner) = owner else {
                            panic!("actual observed owner retained")
                        };
                        assert!(owner.consumed() > 0);
                    }
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    assert_eq!(held.validate_guard(), Err(SourceJournalError::Poisoned));
                    assert!(journal.begin_session().is_err());
                    drop(failed);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
            );
        }
    }
}
#[test]
fn owned_wait_live_observe_cancellation_preserves_initialized_owner_and_history() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let input = super::super::tests::input(&context);
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let initialized = match initialize_live_actor_v8(&journal, input, &cancel) {
            Ok(Ok(x)) => x,
            _ => panic!("init"),
        };
        let weak = initialized.owner.test_weak();
        cancel.cancel();
        let failed = match observe_live_actor_v8(initialized) {
            Err(x) => x,
            Ok(_) => panic!("cancelled Observe"),
        };
        assert!(matches!(
            &failed,
            LiveObserveFailureV8::Legacy {
                owner: LiveObserveOutcomeV8::Refused(_),
                ..
            }
        ));
        assert_eq!(journal.begin_session().unwrap().sequence(), 5);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(failed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
