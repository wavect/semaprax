use super::super::observe::observe_live_actor_v8;
use super::*;
use std::sync::Arc;
#[test]
fn owned_wait_live_park_checkpoints_actual_same_owner_and_recorded_start_only() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let input = super::super::tests::input(&context);
        let context = context.with_initialization(&lease).unwrap();
        let allowance = context.ordinary().max_steps_per_stage().unwrap() as u64;
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let initialized = match initialize_live_actor_v8(&journal, input, &cancel) {
            Ok(Ok(x)) => x,
            _ => panic!("actual init"),
        };
        let init_consumed = initialized.owner.consumed();
        let weak = initialized.owner.test_weak();
        let observed = match observe_live_actor_v8(initialized) {
            Ok(x) => x,
            Err(_) => panic!("actual Observe"),
        };
        let observe_consumed = observed.owner.consumed();
        assert!(observe_consumed > 0);
        let parked = match start_live_actor_v8(observed) {
            Ok(x) => x,
            Err(_) => panic!("actual helper park and authenticated checkpoint"),
        };
        assert_eq!(
            (
                parked.reservation,
                parked.prepared,
                parked.session.sequence()
            ),
            (8, 9, 10)
        );
        assert!(parked.owner.consumed() > 0 && parked.owner.consumed() <= allowance);
        let actual = parked.owner.test_weak();
        assert_eq!(actual.len(), weak.len());
        assert!(actual.iter().all(
            |w| w.strong_count() == 1 && weak.iter().any(|old| std::sync::Weak::ptr_eq(w, old))
        ));
        let folded = parked.session.fold_for_live_test();
        assert_eq!(folded.reserved_total, 3 * allowance);
        assert_eq!(
            folded.stages, 2,
            "helper wait is not another ordinary Agent stage"
        );
        assert_eq!(
            folded.consumed_recorded,
            init_consumed + parked.owner.consumed(),
            "Observe has no consumed ACK field; never infer it"
        );
        assert_eq!(
            journal.begin_session().unwrap().sequence(),
            10,
            "authenticate real checkpoint with actual key/B/seq and fold"
        );
        parked.held.validate_guard().unwrap();
        drop(parked);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        journal.hold().unwrap().validate_guard().unwrap();
    });
}
#[test]
fn owned_wait_live_park_created_reservation_prepared_ack_faults_preserve_exact_owner() {
    for append in [8, 9, 10] {
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
                        _ => panic!("init before fault"),
                    };
                    let weak = initialized.owner.test_weak();
                    let observed = match observe_live_actor_v8(initialized) {
                        Ok(x) => x,
                        Err(_) => panic!("Observe before fault"),
                    };
                    let failed = match start_live_actor_v8(observed) {
                        Err(x) => x,
                        Ok(_) => panic!("no continuation on uncertain wait ACK"),
                    };
                    assert_eq!(failed.error, SourceJournalError::Uncertain);
                    if append < 10 {
                        assert!(
                            matches!(&failed.owner, LiveWaitFailureOwnerV8::Observed(_)),
                            "no helper evaluator before Start reservation ACK"
                        );
                    } else {
                        let LiveWaitFailureOwnerV8::Start(LiveWaitStartOutcomeV8::GuardLost(owner)) =
                            &failed.owner
                        else {
                            panic!("actual parked root retained")
                        };
                        assert!(owner.consumed() > 0);
                    }
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    assert_eq!(
                        failed.held.validate_guard(),
                        Err(SourceJournalError::Poisoned)
                    );
                    assert!(journal.begin_session().is_err());
                    drop(failed);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
            );
        }
    }
}
#[test]
fn owned_wait_live_park_cancel_preserves_prepared_owner_and_no_wait_writes() {
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
        let observed = match observe_live_actor_v8(initialized) {
            Ok(x) => x,
            Err(_) => panic!("Observe"),
        };
        cancel.cancel();
        let failed = match start_live_actor_v8(observed) {
            Err(x) => x,
            Ok(_) => panic!("cancelled helper"),
        };
        assert!(matches!(&failed.owner, LiveWaitFailureOwnerV8::Observed(_)));
        assert_eq!(journal.begin_session().unwrap().sequence(), 7);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(failed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
