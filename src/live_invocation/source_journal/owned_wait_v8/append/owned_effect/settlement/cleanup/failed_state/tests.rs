//! Genuine failed host -> real Decision receipt -> actual State leaf cleanup.
//! Stop is journal evidence only; no terminal delivery or recovered owner mint.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::{test_failed_target,TestFailedTargetV8};
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::failed_state::{LiveFailedEffectStateAppendV8,LiveFailedEffectStateAcknowledgedV8};
use crate::live_invocation::{InvocationClock,SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
use std::{cell::Cell,sync::Arc};
struct Clock(Cell<usize>);
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.0.set(self.0.get() + 1);
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
fn journal(
    c: CheckedOwnedWaitJournalContextV8,
    l: crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8,
    k: SourceCheckpointKey,
) -> SourceOwnedWaitJournalV8 {
    let c = c.with_initialization(&l).unwrap();
    SourceOwnedWaitJournalV8::open(Arc::new(c), k, l).unwrap()
}
fn ack<'j>(
    j: &'j SourceOwnedWaitJournalV8,
    o: LiveFailedEffectStateAppendV8<'j>,
) -> LiveFailedEffectStateAcknowledgedV8<'j> {
    j.begin_session()
        .unwrap()
        .append_failed_effect_state(o)
        .unwrap_or_else(|_| panic!("actual fixed failure-State ACK"))
        .advance_failed_state()
        .unwrap_or_else(|_| panic!("same failed owner"))
}
fn encode_row(
    journal: &SourceOwnedWaitJournalV8,
    row: &EntryV8,
    seq: usize,
    prefix: &[u8],
) -> Vec<u8> {
    let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
    let header = wire::parse(&line[..line.len() - 1]).unwrap();
    wire::encode(
        row,
        &ExpectedRowV8 {
            invocation: journal.context.ordinary().invocation(),
            generation: journal.context.generation(),
            seq: u32::try_from(seq).unwrap(),
            prev_mac: header["authentication"].as_str().unwrap(),
            ordinary: journal.context.ordinary(),
        },
        &journal.key,
    )
    .unwrap()
}
#[test]
fn owned_failed_state_append_actual_handler_and_result_failure_preserve_sticky_stop_and_ledger() {
    for result_limit in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
            let j = journal(c, l, k);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock(Cell::new(0));
            test_failed_target(
                &j,
                &cancel,
                &policy,
                &clock,
                if result_limit {
                    TestFailedTargetV8::ResultLimit
                } else {
                    TestFailedTargetV8::HandlerFailed
                },
                |o, weak| {
                    let old = j.begin_session().unwrap();
                    let (r, s, _, _, last) = old.inventory.failed_effect_state_facts().unwrap();
                    let seq = old.sequence();
                    assert!(matches!(last,EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled{..})));
                    let selected = o
                        .prepare_failed_state()
                        .unwrap_or_else(|_| panic!("actual failed-State start"));
                    let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupStarted{effect_failure,operations,decision_cleanup_settled,..})=selected.selected_row()else{panic!()};
                    assert_eq!(
                        effect_failure,
                        if result_limit {
                            "result_limit"
                        } else {
                            "handler_failed"
                        }
                    );
                    assert_eq!(*decision_cleanup_settled as usize + 1, seq);
                    assert_eq!(operations.as_array().unwrap().len(), 1);
                    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                    let LiveFailedEffectStateAcknowledgedV8::Started(o) = ack(&j, selected) else {
                        panic!()
                    };
                    let mut observations = 0;
                    let o = o
                        .release(|_| {
                            observations += 1;
                            assert_eq!(weak[0].strong_count(), 0)
                        })
                        .unwrap_or_else(|_| panic!("actual State physical drop"));
                    assert_eq!(observations, 1);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                    let selected = o
                        .prepare_receipt()
                        .unwrap_or_else(|_| panic!("real State receipt"));
                    let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{receipt,..})=selected.selected_row()else{panic!()};
                    assert_eq!(receipt["kind"], "observed");
                    assert_eq!(receipt["operations"].as_array().unwrap().len(), 1);
                    assert_eq!(receipt["operations"][0]["outcome"], "completed");
                    assert_eq!(receipt["settlement"], "completed");
                    let LiveFailedEffectStateAcknowledgedV8::Released(o) = ack(&j, selected) else {
                        panic!()
                    };
                    let selected = o
                        .prepare_stop()
                        .unwrap_or_else(|_| panic!("sticky target Stop"));
                    assert!(matches!(selected.selected_row(),EntryV8::Ordinary(SourceJournalEntry::Stop{status:crate::live_invocation::source_journal::SourceStopStatus::EffectFailed,reason:crate::live_invocation::source_journal::SourceStopReason::EffectFailed,..})));
                    let LiveFailedEffectStateAcknowledgedV8::Released(o) = ack(&j, selected) else {
                        panic!()
                    };
                    let current = j.begin_session().unwrap();
                    let (nr, ns, _, _, _) = current.inventory.failed_effect_state_facts().unwrap();
                    assert_eq!((nr, ns), (r, s));
                    assert_eq!(current.sequence(), seq + 3);
                    drop(o);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
            );
        });
    }
}
#[test]
fn owned_failed_state_append_cancel_after_started_allows_real_receipt_but_no_stop() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_failed_target(
            &j,
            &cancel,
            &policy,
            &clock,
            TestFailedTargetV8::HandlerFailed,
            |o, weak| {
                let selected = o.prepare_failed_state().unwrap_or_else(|_| panic!());
                let LiveFailedEffectStateAcknowledgedV8::Started(o) = ack(&j, selected) else {
                    panic!()
                };
                cancel.cancel();
                let clocks = clock.0.get();
                let mut observed = 0;
                let o = o
                    .release(|_| observed += 1)
                    .unwrap_or_else(|_| panic!("incurred physical cleanup"));
                let selected = o
                    .prepare_receipt()
                    .unwrap_or_else(|_| panic!("incurred real receipt"));
                let LiveFailedEffectStateAcknowledgedV8::Released(o) = ack(&j, selected) else {
                    panic!()
                };
                assert_eq!(observed, 1);
                assert_eq!(clock.0.get(), clocks);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
                let rejected = o
                    .prepare_stop()
                    .err()
                    .expect("full guard prohibits new terminal work");
                assert!(j.hold().is_err());
                drop(rejected);
            },
        );
    });
}
#[test]
fn owned_failed_state_append_observer_failure_records_actual_failure_and_quarantines_stop() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_failed_target(
            &j,
            &cancel,
            &policy,
            &clock,
            TestFailedTargetV8::HandlerFailed,
            |o, weak| {
                let LiveFailedEffectStateAcknowledgedV8::Started(o) =
                    ack(&j, o.prepare_failed_state().unwrap_or_else(|_| panic!()))
                else {
                    panic!()
                };
                let o = o
                    .release(|_| panic!("real failed State observer"))
                    .unwrap_or_else(|_| panic!("observer caught after actual release"));
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
                let selected = o.prepare_receipt().unwrap_or_else(|_| panic!());
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupSettled{receipt,..})=selected.selected_row()else{panic!()};
                assert_eq!(receipt["settlement"], "failed");
                assert_eq!(receipt["operations"][0]["outcome"], "failed");
                let LiveFailedEffectStateAcknowledgedV8::Released(o) = ack(&j, selected) else {
                    panic!()
                };
                let failure = o
                    .prepare_stop()
                    .err()
                    .expect("no fabricated successful cleanup");
                assert!(j.hold().is_err());
                assert!(j.begin_session().is_err());
                drop(failure);
            },
        );
    });
}
#[test]
fn owned_failed_state_append_entry_cancellation_retains_original_state_with_zero_io() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_failed_target(
            &j,
            &cancel,
            &policy,
            &clock,
            TestFailedTargetV8::HandlerFailed,
            |o, weak| {
                let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
                cancel.cancel();
                let rejected = o.prepare_failed_state().err().expect("no start authority");
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), before);
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                assert!(j.hold().is_err());
                drop(rejected);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    });
}
#[cfg(unix)]
#[test]
fn owned_failed_state_append_actual_persistence_faults_do_not_release_without_started_ack() {
    for fault in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |c, l, k, directory| {
                let j = journal(c, l, k);
                let cancel = AgentCancellation::new();
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let clock = Clock(Cell::new(0));
                test_failed_target(
                    &j,
                    &cancel,
                    &policy,
                    &clock,
                    TestFailedTargetV8::HandlerFailed,
                    |o, weak| {
                        let selected = o.prepare_failed_state().unwrap_or_else(|_| panic!());
                        let n = selected.sequence() + 1;
                        let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
                        let encoded =
                            encode_row(&j, selected.selected_row(), selected.sequence(), &before);
                        {
                            let mut lease = j.lease.try_borrow_mut().unwrap();
                            match fault {
                                0 => lease.test_fail_before_write(n),
                                1 => lease.test_fail_after_write(n),
                                2 => lease.test_fail_before_sync(n),
                                _ => lease.test_fail_after_sync(n),
                            }
                        }
                        let failed = j
                            .begin_session()
                            .unwrap()
                            .append_failed_effect_state(selected)
                            .err()
                            .expect("attempted-write uncertainty");
                        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                        assert!(j.hold().is_err());
                        assert!(j.begin_session().is_err());
                        let files = std::fs::read_dir(directory)
                            .unwrap()
                            .map(|v| v.unwrap().path())
                            .collect::<Vec<_>>();
                        assert_eq!(files.len(), 1);
                        let after = std::fs::read(&files[0]).unwrap();
                        let mut expected = before;
                        if fault != 0 {
                            expected.extend(encoded);
                        }
                        assert_eq!(after, expected);
                        drop(failed);
                        assert!(weak.iter().all(|w| w.upgrade().is_none()));
                    },
                );
            },
        );
    }
}
#[test]
fn owned_failed_state_append_reminted_binding_refs_vector_and_state_do_not_supply_cleanup_authority(
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_failed_target(
            &j,
            &cancel,
            &policy,
            &clock,
            TestFailedTargetV8::HandlerFailed,
            |o, weak| {
                let selected = o.prepare_failed_state().unwrap_or_else(|_| panic!());
                let prefix = j.lease.try_borrow_mut().unwrap().read().unwrap();
                for mutation in 0..5 {
                    let mut row = selected.selected_row().clone();
                    let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectFailureStateCleanupStarted{plan,settlement,recorded,state_digest,operations,..})=&mut row else{panic!()};
                    match mutation{0=>*plan="sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),1=>*settlement=settlement.checked_sub(1).unwrap(),2=>*recorded=recorded.checked_sub(1).unwrap(),3=>*state_digest="sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),_=>*operations=serde_json::json!([])}
                    let encoded = encode_row(&j, &row, selected.sequence(), &prefix);
                    let lease = j.lease.try_borrow().unwrap();
                    assert!(crate::live_invocation::source_journal::owned_wait_v8::inventory::checked_candidate_inventory_v8(&j.context,&lease,&j.key,&prefix,&encoded).is_err());
                    drop(lease);
                    assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), prefix);
                    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                }
                let row = selected.selected_row().clone();
                assert!(j.begin_session().unwrap().append(row).is_err());
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), prefix);
                assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                drop(selected);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    });
}
#[test]
fn owned_failed_state_append_wrong_container_preserves_other_journal_and_original_owner() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_failed_target(
            &j,
            &cancel,
            &policy,
            &clock,
            TestFailedTargetV8::HandlerFailed,
            |o, weak| {
                let selected = o.prepare_failed_state().unwrap_or_else(|_| panic!());
                CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
                    let other = journal(c, l, k);
                    let before = other.lease.try_borrow_mut().unwrap().read().unwrap();
                    let failed = other
                        .begin_session()
                        .unwrap()
                        .append_failed_effect_state(selected)
                        .err()
                        .expect("wrong container zero IO");
                    assert!(matches!(
                        failed,
                        LiveFailedEffectStateAppendFailureV8::Before { .. }
                    ));
                    assert_eq!(
                        other.lease.try_borrow_mut().unwrap().read().unwrap(),
                        before
                    );
                    assert!(other.hold().is_ok());
                    assert!(j.hold().is_ok());
                    assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                    drop(failed);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                });
            },
        );
    });
}
