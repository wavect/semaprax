//! Genuine A→Ready promotion→Consumed same-token renewal, no target entry.
use super::super::tests::test_staged;
use super::*;
fn promotions() -> usize {
    crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_ready_promotions_v8()
}
fn ack<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    obligation: LiveOwnedContinuedEffectAppendV8<'j>,
) -> LiveContinuedEffectV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_continued_effect(obligation)
        .unwrap_or_else(|_| panic!("true same-FD C ACK"))
        .advance_continued_effect()
        .unwrap_or_else(|_| panic!("actual promotion/renewal"))
}
#[test]
fn owned_continued_effect_actual_ready_consumed_renew_same_token_and_ledger() {
    test_staged(
        |journal, staged, weak, ledger| {
            let old = journal.test_continued_effect_registry();
            let original = staged.current().sequence();
            let leaves = staged.actual().unwrap().owner.test_authorize_weak();
            let used = staged
                .actual()
                .unwrap()
                .owner
                .staged_authorize_facts(staged.binding().unwrap())
                .unwrap()
                .2;
            let n = promotions();
            let selected = staged
                .prepare_effect()
                .unwrap_or_else(|_| panic!("actual Granted"));
            assert!(
                matches!(selected.selected(),EntryV8::Owned(journal_model::OwnedBodyV8::OwnedAuthorizationReady{turn:1,attempt:0,staged,..}) if *staged as usize+1==original)
            );
            assert_eq!(promotions(), n);
            let ready = ack(journal, selected);
            assert_eq!(promotions(), n + 1);
            let r = journal.test_continued_effect_registry();
            assert_eq!(
                (&r.0, &r.1, &r.2, &r.3, &r.4),
                (&old.0, &old.1, &old.2, &old.3, &old.4)
            );
            assert_ne!(r.5, old.5);
            assert_eq!(*ready.authorization.completed.owner.accounting(), ledger);
            assert_eq!(
                ready
                    .authorization
                    .actual()
                    .unwrap()
                    .owner
                    .effect_facts(ready.authorization.binding().unwrap())
                    .unwrap()
                    .2,
                used
            );
            let actual = ready
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_effect_weak();
            assert_eq!(actual.len(), leaves.len());
            assert!(leaves
                .iter()
                .all(|old| actual.iter().any(|new| old.ptr_eq(new))));
            let consumed = ack(
                journal,
                ready.prepare_next().unwrap_or_else(|_| panic!("Consumed")),
            );
            consumed.validate_live().unwrap();
            let c = journal.test_continued_effect_registry();
            assert_eq!(
                (&c.0, &c.1, &c.2, &c.3, &c.4),
                (&old.0, &old.1, &old.2, &old.3, &old.4)
            );
            assert_ne!(c.5, r.5);
            assert_eq!(consumed.current().sequence(), original + 2);
            assert_eq!(*consumed.authorization.completed.owner.accounting(), ledger);
            assert_eq!(promotions(), n + 1);
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            assert!(
                consumed.prepare_next().is_err(),
                "no Intent or further permission"
            );
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        },
        true,
    );
}
#[cfg(unix)]
#[test]
fn owned_continued_effect_all_ack_faults_are_actual_in_doubt_no_promotion_repeat() {
    for row in 0..2 {
        for mode in 0..4 {
            test_staged(
                |journal, staged, weak, ledger| {
                    let mut selected = staged
                        .prepare_effect()
                        .unwrap_or_else(|_| panic!("Granted"));
                    if row == 1 {
                        selected = ack(journal, selected)
                            .prepare_next()
                            .unwrap_or_else(|_| panic!("Consumed"));
                    }
                    let n = promotions();
                    let leaves = selected
                        .owner
                        .authorization
                        .actual()
                        .unwrap()
                        .owner
                        .test_effect_weak();
                    assert!(!leaves.is_empty());
                    let number = selected.sequence() + 1;
                    {
                        let mut lease = journal.test_observe_lease().borrow_mut();
                        match mode {
                            0 => lease.test_fail_before_write(number),
                            1 => lease.test_fail_after_write(number),
                            2 => lease.test_fail_before_sync(number),
                            _ => lease.test_fail_after_sync(number),
                        }
                    }
                    assert_eq!(
                        *selected.owner.authorization.completed.owner.accounting(),
                        ledger
                    );
                    let failed = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_continued_effect(selected)
                        .err()
                        .expect("actual physical fault");
                    assert!(failed.test_is_in_doubt(), "row {row} mode {mode}");
                    assert_eq!(promotions(), n);
                    assert!(weak.iter().any(|w| w.strong_count() == 1));
                    assert!(leaves.iter().all(|w| w.strong_count() == 1));
                    assert!(journal.hold().is_err());
                    assert!(journal.begin_session().is_err());
                    drop(failed);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                    assert!(leaves.iter().all(|w| w.upgrade().is_none()));
                },
                true,
            );
        }
    }
}
#[test]
fn owned_continued_effect_refused_is_retained_without_ready_or_renewal() {
    test_staged(
        |journal, staged, weak, ledger| {
            let old = journal.test_continued_effect_registry();
            let seq = staged.current().sequence();
            let n = promotions();
            let failed = staged
                .prepare_effect()
                .err()
                .expect("Refused is not C Granted");
            assert_eq!(failed.error, SourceJournalError::Binding);
            assert_eq!(*failed.owner.completed.owner.accounting(), ledger);
            assert_eq!(journal.begin_session().unwrap().sequence(), seq);
            assert_eq!(journal.test_continued_effect_registry(), old);
            assert_eq!(promotions(), n);
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        },
        false,
    );
}
thread_local! {static AFTER:std::cell::Cell<usize>=const{std::cell::Cell::new(usize::MAX)};}
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        if promotions() > AFTER.with(std::cell::Cell::get) {
            panic!("post-promotion clock callback");
        }
        1
    }
}
impl crate::live_invocation::SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_continued_effect_post_promotion_guard_loss_retains_actual_ready() {
    test_staged(
        |journal, mut staged, weak, ledger| {
            let n = promotions();
            AFTER.with(|x| x.set(n));
            let ModelOwnerV8::Resumed(actual) = &mut staged.completed.owner else {
                panic!("actual staged")
            };
            actual.owner.test_authorize_clock(&Clock);
            let selected = staged
                .prepare_effect()
                .unwrap_or_else(|_| panic!("before promotion"));
            let failed = journal
                .begin_session()
                .unwrap()
                .append_owned_continued_effect(selected)
                .unwrap_or_else(|_| panic!("Ready ACK"))
                .advance_continued_effect()
                .err()
                .expect("actual postpromotion guard loss");
            let LiveContinuedEffectAcknowledgmentFailureV8::Entered(failed) = &failed else {
                panic!("retained Ready boundary")
            };
            assert_eq!(promotions(), n + 1);
            assert_eq!(
                *failed.owner.authorization.completed.owner.accounting(),
                ledger
            );
            let leaves = failed
                .owner
                .authorization
                .actual()
                .unwrap()
                .owner
                .test_effect_weak();
            assert!(!leaves.is_empty());
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            AFTER.with(|x| x.set(usize::MAX));
        },
        true,
    );
}
