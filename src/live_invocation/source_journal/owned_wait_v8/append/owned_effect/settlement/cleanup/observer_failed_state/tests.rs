//! Actual failed Decision observer ACK -> same State cleanup -> sticky Stop.
//! No normal poison reopening, Outcome, Reduce, recovery owner, or result claim.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::{test_observer_failed_receipt_mode,TestObserverTargetV8};
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveOutcomeV8;
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
fn room(
    session: &AppendSessionV8<'_>,
) -> crate::live_invocation::source_journal::owned_wait_v8::capacity::RoomV8 {
    let room = crate::live_invocation::source_journal::owned_wait_v8::capacity::outstanding(
        session.journal.context.fold(),
        &session.inventory.fold_for_live_test(),
    )
    .unwrap();
    let bytes = crate::live_invocation::source_journal::MAX_SOURCE_DOCUMENT_BYTES
        - room.bytes_for_inert_test();
    let rows =
        crate::live_invocation::source_journal::MAX_SOURCE_ENTRIES - room.rows_for_inert_test();
    room.check(bytes, rows).unwrap();
    assert!(room.check(bytes + 1, rows).is_err());
    assert!(room.check(bytes, rows + 1).is_err());
    room
}
fn ack<'j>(o: LiveObserverStateAppendV8<'j>) -> LiveObserverStateAcknowledgedV8<'j> {
    let session = o.begin_session().unwrap();
    let old_bytes = session.acknowledged_bytes();
    let before = room(&session);
    let envelope = session
        .append_observer_state(o)
        .unwrap_or_else(|_| panic!("actual State append ACK"));
    let after = room(&envelope.session);
    assert!(
        before.bytes_for_inert_test()
            >= envelope.session.acknowledged_bytes() - old_bytes + after.bytes_for_inert_test()
    );
    assert!(before.rows_for_inert_test() >= 1 + after.rows_for_inert_test());
    envelope
        .advance_observer_state()
        .unwrap_or_else(|_| panic!("actual same State successor"))
}

fn failed<'j>(o:crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveCleanedOwnedEffectV8<'j>)->crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveFailedOwnedEffectV8<'j>{
    let LiveOutcomeV8::Failed(o) = o
        .advance_outcome()
        .unwrap_or_else(|_| panic!("actual failed receipt holder"))
    else {
        panic!("no Outcome after failed observation")
    };
    o
}
#[test]
fn owned_observer_state_actual_cleanup_preserves_success_and_two_target_causes() {
    for mode in [
        TestObserverTargetV8::Observed,
        TestObserverTargetV8::HandlerFailed,
        TestObserverTargetV8::ResultLimit,
    ] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
            let j = journal(c, l, k);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock(Cell::new(0));
            test_observer_failed_receipt_mode(&j, &cancel, &policy, &clock, mode, |o, weak| {
                let o = failed(o);
                let before = *o.accounting();
                let selected = o
                    .prepare_observer_state()
                    .unwrap_or_else(|_| panic!("actual State intent"));
                let expected = match mode {
                    TestObserverTargetV8::Observed => None,
                    TestObserverTargetV8::HandlerFailed => Some("handler_failed"),
                    TestObserverTargetV8::ResultLimit => Some("result_limit"),
                };
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted{selected_effect_failure,operations,..})=selected.selected_row()else{panic!()};
                assert_eq!(selected_effect_failure.as_deref(), expected);
                let operations = operations.clone();
                assert_eq!(operations.as_array().unwrap().len(), 1);
                assert_eq!(weak[0].strong_count(), 1);
                let LiveObserverStateAcknowledgedV8::Started(o) = ack(selected) else {
                    panic!()
                };
                let calls = Cell::new(0);
                let o = o
                    .release(|_| {
                        calls.set(calls.get() + 1);
                        assert!(weak[0].upgrade().is_none());
                    })
                    .unwrap_or_else(|_| panic!("actual State physical release"));
                assert_eq!(calls.get(), 1);
                let selected = o
                    .prepare_receipt()
                    .unwrap_or_else(|_| panic!("actual State receipt"));
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{receipt,..})=selected.selected_row()else{panic!()};
                assert_eq!(receipt["settlement"], "completed");
                assert_eq!(receipt["operations"][0]["operation"], operations[0]);
                assert_eq!(receipt["operations"][0]["outcome"], "completed");
                let LiveObserverStateAcknowledgedV8::Released(o) = ack(selected) else {
                    panic!()
                };
                let selected = o.prepare_stop().unwrap_or_else(|_| panic!("sticky Stop"));
                let EntryV8::Ordinary(SourceJournalEntry::Stop { status, reason, .. }) =
                    selected.selected_row()
                else {
                    panic!()
                };
                use crate::live_invocation::source_journal::{
                    SourceStopReason as R, SourceStopStatus as S,
                };
                assert_eq!(
                    (*status, *reason),
                    if expected.is_some() {
                        (S::EffectFailed, R::EffectFailed)
                    } else {
                        (S::Rejected, R::StageRefused)
                    }
                );
                let session = selected.begin_session().unwrap();
                let (reserved, stages, _, _, _) = session.inventory.observer_state_facts().unwrap();
                let envelope = session
                    .append_observer_state(selected)
                    .unwrap_or_else(|_| panic!("actual sticky Stop ACK"));
                envelope.validate_live().unwrap();
                let (r, s, _, _, _) = envelope.session.inventory.observer_state_facts().unwrap();
                assert_eq!((r, s), (reserved, stages));
                assert_eq!(before, *envelope.obligation.owner_accounting_for_test());
                assert!(j.poisoned.get());
                assert!(!j.poisoned.retired_for_test());
                assert!(j.hold().is_err());
                assert!(j.begin_session().is_err());
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
                drop(envelope);
            });
        });
    }
}
#[test]
fn owned_observer_state_cancellation_after_started_allows_release_receipt_but_no_stop() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_observer_failed_receipt_mode(
            &j,
            &cancel,
            &policy,
            &clock,
            TestObserverTargetV8::Observed,
            |o, weak| {
                let selected = failed(o)
                    .prepare_observer_state()
                    .unwrap_or_else(|_| panic!());
                let LiveObserverStateAcknowledgedV8::Started(o) = ack(selected) else {
                    panic!()
                };
                let calls = clock.0.get();
                cancel.cancel();
                let o = o
                    .release(|_| assert!(weak[0].upgrade().is_none()))
                    .unwrap_or_else(|_| panic!("incurred cleanup ignores cancellation"));
                let selected = o.prepare_receipt().unwrap_or_else(|_| panic!());
                let LiveObserverStateAcknowledgedV8::Released(o) = ack(selected) else {
                    panic!()
                };
                assert_eq!(clock.0.get(), calls);
                let bytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
                assert!(o.prepare_stop().is_err());
                assert!(j.poisoned.retired_for_test());
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), bytes);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    });
}
#[test]
fn owned_observer_state_failed_state_observation_records_real_failure_and_quarantines() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_observer_failed_receipt_mode(
            &j,
            &cancel,
            &policy,
            &clock,
            TestObserverTargetV8::Observed,
            |o, weak| {
                let LiveObserverStateAcknowledgedV8::Started(o) = ack(failed(o)
                    .prepare_observer_state()
                    .unwrap_or_else(|_| panic!()))
                else {
                    panic!()
                };
                let o = o
                    .release(|_| panic!("actual failed State observer"))
                    .unwrap_or_else(|_| panic!("retain actual failed receipt"));
                let selected = o.prepare_receipt().unwrap_or_else(|_| panic!());
                assert!(
                    matches!(selected.selected_row(),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled{receipt,..})if receipt["settlement"]=="failed"&&receipt["operations"][0]["outcome"]=="failed")
                );
                let LiveObserverStateAcknowledgedV8::Released(o) = ack(selected) else {
                    panic!()
                };
                let bytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
                assert!(j.poisoned.retired_for_test());
                assert!(o.prepare_stop().is_err());
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), bytes);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    });
}
#[cfg(unix)]
#[test]
fn owned_observer_state_pin_loss_restore_after_started_permanently_forbids_state_work() {
    use std::os::unix::fs::MetadataExt;
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(true, |c, l, k, directory| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_observer_failed_receipt_mode(
            &j,
            &cancel,
            &policy,
            &clock,
            TestObserverTargetV8::Observed,
            |o, weak| {
                let LiveObserverStateAcknowledgedV8::Started(o) = ack(failed(o)
                    .prepare_observer_state()
                    .unwrap_or_else(|_| panic!()))
                else {
                    panic!()
                };
                let pin = j.context.registration().identity();
                let entries: Vec<_> = std::fs::read_dir(directory)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .filter(|p| {
                        std::fs::metadata(p).is_ok_and(|m| {
                            m.is_file() && m.dev() == pin.file_device && m.ino() == pin.file_inode
                        })
                    })
                    .collect();
                assert_eq!(entries.len(), 1);
                let entry = &entries[0];
                let moved = directory.join("original-observer-state-journal");
                let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
                std::fs::rename(entry, &moved).unwrap();
                std::fs::write(entry, b"replacement").unwrap();
                assert!(o.lineage_guard_for_test().is_err());
                std::fs::remove_file(entry).unwrap();
                std::fs::rename(&moved, entry).unwrap();
                let calls = Cell::new(0);
                let result = o.release(|_| calls.set(calls.get() + 1));
                assert!(result.is_err());
                assert_eq!(calls.get(), 0);
                assert_eq!(weak[0].strong_count(), 1);
                assert!(j.poisoned.retired_for_test());
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), before);
                drop(result);
            },
        );
    });
}
fn encode_row(j: &SourceOwnedWaitJournalV8, row: &EntryV8, seq: usize, prefix: &[u8]) -> Vec<u8> {
    let line = prefix.split_inclusive(|b| *b == b'\n').last().unwrap();
    let header = wire::parse(&line[..line.len() - 1]).unwrap();
    wire::encode(
        row,
        &ExpectedRowV8 {
            invocation: j.context.ordinary().invocation(),
            generation: j.context.generation(),
            seq: u32::try_from(seq).unwrap(),
            prev_mac: header["authentication"].as_str().unwrap(),
            ordinary: j.context.ordinary(),
        },
        &j.key,
    )
    .unwrap()
}
#[test]
fn owned_observer_state_reminted_cause_refs_binding_vector_and_generic_route_never_grant_work() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_observer_failed_receipt_mode(
            &j,
            &cancel,
            &policy,
            &clock,
            TestObserverTargetV8::Observed,
            |o, weak| {
                let o = failed(o)
                    .prepare_observer_state()
                    .unwrap_or_else(|_| panic!());
                let prefix = j.lease.try_borrow_mut().unwrap().read().unwrap();
                for mutation in 0..8 {
                    let mut row = o.selected_row().clone();
                    let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted{plan,settlement,recorded,decision_cleanup_settled,decision_receipt_digest,cause,selected_effect_failure,operations,..})=&mut row else{panic!()};
                    match mutation {
                        0 => *plan = format!("sha256:{}", "0".repeat(64)),
                        1 => *settlement -= 1,
                        2 => *recorded -= 1,
                        3 => *decision_cleanup_settled -= 1,
                        4 => *decision_receipt_digest = format!("sha256:{}", "0".repeat(64)),
                        5 => *cause = "target_failed".into(),
                        6 => *selected_effect_failure = Some("handler_failed".into()),
                        _ => *operations = serde_json::json!([]),
                    };
                    let encoded = encode_row(&j, &row, o.sequence(), &prefix);
                    let lease = j.lease.try_borrow().unwrap();
                    assert!(crate::live_invocation::source_journal::owned_wait_v8::inventory::checked_candidate_inventory_v8(&j.context,&lease,&j.key,&prefix,&encoded).is_err());
                    drop(lease);
                    assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), prefix);
                    assert_eq!(weak[0].strong_count(), 1);
                }
                // A former generic session is already ordinarily poisoned; no new authority
                // is inferred from that refusal. The owning seal remains valid after it.
                assert!(j.begin_session().is_err());
                o.validate_live().unwrap();
                assert!(!j.poisoned.retired_for_test());
                drop(o);
            },
        );
    });
}
#[test]
fn owned_observer_state_entry_cancellation_never_writes_or_releases_state() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_observer_failed_receipt_mode(
            &j,
            &cancel,
            &policy,
            &clock,
            TestObserverTargetV8::Observed,
            |o, weak| {
                let o = failed(o);
                let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
                cancel.cancel();
                let result = o.prepare_observer_state();
                assert!(result.is_err());
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), before);
                assert_eq!(weak[0].strong_count(), 1);
                assert!(j.poisoned.retired_for_test());
                drop(result);
            },
        );
    });
}
#[cfg(unix)]
#[test]
fn owned_observer_state_four_persistence_faults_never_release_without_actual_started_ack() {
    for fault in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |c, l, k, directory| {
                let j = journal(c, l, k);
                let cancel = AgentCancellation::new();
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let clock = Clock(Cell::new(0));
                test_observer_failed_receipt_mode(
                    &j,
                    &cancel,
                    &policy,
                    &clock,
                    TestObserverTargetV8::Observed,
                    |o, weak| {
                        let o = failed(o)
                            .prepare_observer_state()
                            .unwrap_or_else(|_| panic!());
                        let session = o.begin_session().unwrap();
                        let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
                        let encoded = encode_row(&j, o.selected_row(), o.sequence(), &before);
                        let n = o.sequence() + 1;
                        {
                            let mut lease = j.lease.try_borrow_mut().unwrap();
                            match fault {
                                0 => lease.test_fail_before_write(n),
                                1 => lease.test_fail_after_write(n),
                                2 => lease.test_fail_before_sync(n),
                                _ => lease.test_fail_after_sync(n),
                            }
                        }
                        let result = session.append_observer_state(o);
                        assert!(result.is_err());
                        assert_eq!(weak[0].strong_count(), 1);
                        assert!(j.poisoned.retired_for_test());
                        let files: Vec<_> = std::fs::read_dir(directory)
                            .unwrap()
                            .map(|e| e.unwrap().path())
                            .collect();
                        assert_eq!(files.len(), 1);
                        let mut expected = before;
                        if fault != 0 {
                            expected.extend(encoded)
                        }
                        assert_eq!(std::fs::read(&files[0]).unwrap(), expected);
                        drop(result);
                    },
                );
            },
        );
    }
}
#[test]
fn owned_observer_state_wrong_container_has_zero_io_and_retains_original_state() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock(Cell::new(0));
        test_observer_failed_receipt_mode(
            &j,
            &cancel,
            &policy,
            &clock,
            TestObserverTargetV8::Observed,
            |o, weak| {
                let selected = failed(o)
                    .prepare_observer_state()
                    .unwrap_or_else(|_| panic!());
                let before = j.lease.try_borrow_mut().unwrap().read().unwrap();
                CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
                    let other = journal(c, l, k);
                    let other_before = other.lease.try_borrow_mut().unwrap().read().unwrap();
                    let result = other
                        .begin_session()
                        .unwrap()
                        .append_observer_state(selected);
                    assert!(result.is_err());
                    assert_eq!(weak[0].strong_count(), 1);
                    assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), before);
                    assert_eq!(
                        other.lease.try_borrow_mut().unwrap().read().unwrap(),
                        other_before
                    );
                    assert!(!other.poisoned.get());
                    assert!(!j.poisoned.retired_for_test());
                    drop(result);
                });
            },
        );
    });
}
