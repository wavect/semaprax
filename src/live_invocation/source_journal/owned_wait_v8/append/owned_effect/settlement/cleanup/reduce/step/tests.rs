//! Real evaluated roots and fixed Step ACKs. No test ACK/evaluator/result owner
//! factory is used, and Transition does not authorize a next turn or delivery.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::test_evaluated;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::{LiveMovedStepV8,LiveOwnedStepAppendV8,LiveStepAcknowledgedV8};
use crate::live_invocation::{InvocationClock,SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
use std::cell::Cell;
use std::sync::Arc;
struct Clock {
    calls: Cell<usize>,
}
impl Clock {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
        }
    }
}
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.calls.set(self.calls.get() + 1);
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
    o: LiveOwnedStepAppendV8<'j>,
) -> LiveStepAcknowledgedV8<'j> {
    j.begin_session()
        .unwrap()
        .append_owned_step(o)
        .unwrap_or_else(|_| panic!("fixed Step ACK"))
        .advance_step()
        .unwrap_or_else(|_| panic!("same owner ACK move"))
}
fn full_success<'j>(
    j: &'j SourceOwnedWaitJournalV8,
    e:crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::reduce::LiveEvaluatedOwnedReduceV8<'j>,
    mut observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
) -> LiveMovedStepV8<'j> {
    let LiveStepAcknowledgedV8::Staged(staged) = ack(
        j,
        e.prepare_step().unwrap_or_else(|_| panic!("actual Staged")),
    ) else {
        panic!("staged owner")
    };
    let LiveStepAcknowledgedV8::Staged(staged) = ack(
        j,
        staged
            .prepare_cleanup()
            .unwrap_or_else(|_| panic!("actual nonempty cleanup")),
    ) else {
        panic!("started owner")
    };
    let released = staged
        .release(&mut observe)
        .unwrap_or_else(|_| panic!("actual ordered cleanup"));
    let LiveStepAcknowledgedV8::Released(released) = ack(
        j,
        released
            .prepare_receipt()
            .unwrap_or_else(|_| panic!("actual receipt")),
    ) else {
        panic!("released owner")
    };
    let ready = released
        .into_ready()
        .unwrap_or_else(|_| panic!("guarded ReadyStep"));
    let LiveStepAcknowledgedV8::Ready(ready) = ack(
        j,
        ready
            .prepare_transfer()
            .unwrap_or_else(|_| panic!("transfer reservation")),
    ) else {
        panic!("Ready owner")
    };
    let moved = ready
        .move_fields()
        .unwrap_or_else(|_| panic!("actual compiler field move"));
    let LiveStepAcknowledgedV8::Moved(moved) = ack(
        j,
        moved
            .prepare_completed()
            .unwrap_or_else(|_| panic!("actual mapped target")),
    ) else {
        panic!("mapped owner")
    };
    let LiveStepAcknowledgedV8::Moved(moved) = ack(
        j,
        moved
            .prepare_transition()
            .unwrap_or_else(|_| panic!("ordinary exact carrier")),
    ) else {
        panic!("Transition owner")
    };
    moved
}
#[test]
fn owned_step_append_actual_continue_and_complete_keep_single_mapped_owner_and_spent_funding() {
    for complete in [false, true] {
        let exercise = |c, l, k, _: &std::path::Path| {
            let j = journal(c, l, k);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
                let old = j.begin_session().unwrap();
                let (r, s, _, _, _) = old.inventory.original_reduce_facts().unwrap();
                let seq = old.sequence();
                let ledger = *e.accounting();
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                let mut observations = 0;
                let moved = full_success(&j, e, |_| observations += 1);
                moved.validate_live().unwrap();
                assert_eq!(moved.kind(), if complete { "complete" } else { "continue" });
                assert_eq!(moved.accounting(), &ledger);
                assert_eq!(observations, 1);
                assert_eq!(weak.iter().filter(|w| w.strong_count() == 1).count(), 1);
                assert_eq!(weak.iter().filter(|w| w.strong_count() == 0).count(), 1);
                let current = j.begin_session().unwrap();
                let (nr, ns, _, _, last) = current.inventory.step_reduce_facts().unwrap();
                assert_eq!((nr, ns), (r, s));
                assert_eq!(current.sequence(), seq + 6);
                assert!(
                    matches!(last,EntryV8::Ordinary(SourceJournalEntry::Transition{case,..}) if *case==if complete{crate::live_invocation::source_journal::SourceTransitionCase::Complete}else{crate::live_invocation::source_journal::SourceTransitionCase::Continue})
                );
                drop(moved);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
                assert!(j.hold().is_err());
            });
        };
        if complete {
            CheckedOwnedWaitJournalContextV8::test_with_actual_complete_store(exercise)
        } else {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(true, exercise)
        }
    }
}
#[test]
#[cfg(unix)]
fn owned_step_append_actual_staged_faults_do_not_release_or_advance_spent_hold() {
    for mode in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
            let j = journal(c, l, k);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
                let o = e.prepare_step().unwrap_or_else(|_| panic!("staged"));
                let seq = o.sequence() + 1;
                {
                    let mut lease = j.lease.try_borrow_mut().unwrap();
                    match mode {
                        0 => lease.test_fail_before_write(seq),
                        1 => lease.test_fail_after_write(seq),
                        2 => lease.test_fail_before_sync(seq),
                        _ => lease.test_fail_after_sync(seq),
                    }
                }
                let failed = j
                    .begin_session()
                    .unwrap()
                    .append_owned_step(o)
                    .err()
                    .expect("uncertain persistence");
                assert!(matches!(
                    failed,
                    LiveOwnedStepAppendFailureV8::Append {
                        _failure: AppendFailureV8::InDoubt { .. },
                        ..
                    }
                ));
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                assert!(j.hold().is_err());
                assert!(j.begin_session().is_err());
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        });
    }
}
#[test]
fn owned_step_append_cancel_after_started_records_actual_receipt_but_forbids_transfer() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
            let LiveStepAcknowledgedV8::Staged(o) =
                ack(&j, e.prepare_step().unwrap_or_else(|_| panic!("staged")))
            else {
                panic!()
            };
            let LiveStepAcknowledgedV8::Staged(o) = ack(
                &j,
                o.prepare_cleanup().unwrap_or_else(|_| panic!("started")),
            ) else {
                panic!()
            };
            let before = clock.calls.get();
            cancel.cancel();
            let mut count = 0;
            let o = o
                .release(|_| count += 1)
                .unwrap_or_else(|_| panic!("incurred release"));
            assert_eq!(count, 1);
            assert_eq!(clock.calls.get(), before);
            let LiveStepAcknowledgedV8::Released(o) = ack(
                &j,
                o.prepare_receipt()
                    .unwrap_or_else(|_| panic!("incurred receipt")),
            ) else {
                panic!()
            };
            assert_eq!(clock.calls.get(), before);
            assert_eq!(weak.iter().filter(|w| w.strong_count() == 1).count(), 1);
            let bytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
            let rejected = o.into_ready().err().expect("cancel forbids result work");
            assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), bytes);
            assert!(j.hold().is_err());
            drop(rejected);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_step_append_failed_observer_receipt_is_real_and_never_grants_result_move() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
            let LiveStepAcknowledgedV8::Staged(o) =
                ack(&j, e.prepare_step().unwrap_or_else(|_| panic!()))
            else {
                panic!()
            };
            let LiveStepAcknowledgedV8::Staged(o) =
                ack(&j, o.prepare_cleanup().unwrap_or_else(|_| panic!()))
            else {
                panic!()
            };
            let o = o
                .release(|_| panic!("actual failed observer"))
                .unwrap_or_else(|_| panic!("caught observer continues"));
            let selected = o
                .prepare_receipt()
                .unwrap_or_else(|_| panic!("real failed receipt"));
            let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupSettled{receipt,..})=selected.selected_row()else{panic!()};
            assert_eq!(receipt["settlement"], "failed");
            assert_eq!(receipt["operations"][0]["outcome"], "failed");
            let verified = j
                .begin_session()
                .unwrap()
                .append_owned_step(selected)
                .unwrap_or_else(|_| panic!("failed receipt ACK"));
            let LiveStepAcknowledgedV8::Released(o) = verified
                .advance_step()
                .unwrap_or_else(|_| panic!("actual released owner"))
            else {
                panic!()
            };
            assert_eq!(weak.iter().filter(|w| w.strong_count() == 1).count(), 1);
            let rejected = o
                .into_ready()
                .err()
                .expect("quarantine no successful handoff");
            assert!(j.hold().is_err());
            drop(rejected);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_step_append_cancellation_before_first_ack_has_zero_io_and_retains_actual_roots() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
            let o = e.prepare_step().unwrap_or_else(|_| panic!());
            let session = j.begin_session().unwrap();
            let bytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
            cancel.cancel();
            let failed = session
                .append_owned_step(o)
                .err()
                .expect("cancelled actual owner");
            assert!(matches!(
                failed,
                LiveOwnedStepAppendFailureV8::Before { .. }
            ));
            assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), bytes);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_step_append_actual_ack_witness_tampering_poison_is_sticky() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
            let o = e.prepare_step().unwrap_or_else(|_| panic!());
            let mut verified = j
                .begin_session()
                .unwrap()
                .append_owned_step(o)
                .unwrap_or_else(|_| panic!());
            let original = verified.witness.selected.clone();
            let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceStaged{stage_reservation,..})=&mut verified.witness.selected else{panic!()};
            *stage_reservation += 1;
            let bytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
            assert!(verified.validate_live().is_err());
            verified.witness.selected = original;
            assert!(verified.validate_live().is_err());
            assert!(j.hold().is_err());
            assert!(j.begin_session().is_err());
            assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), bytes);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(verified);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}
#[test]
fn owned_step_append_actual_failed_reduce_releases_both_roots_and_keeps_selected_stop() {
    for mode in 0..3 {
        let exercise = |c, l, k, _: &std::path::Path| {
            let j = journal(c, l, k);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::test_evaluated_failed(&j,&cancel,&policy,&clock,|e,weak|{
                let facts=e.stage_facts().unwrap();let basis=facts.cleanup_basis(None).unwrap();let failure=basis["status"].clone();
                let old=j.begin_session().unwrap();let(r,s,_,_,_)=old.inventory.original_reduce_facts().unwrap();let seq=old.sequence();
                let o=e.prepare_step().unwrap_or_else(|_|panic!("actual failure basis"));
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupStarted{basis:recorded,..})=o.selected_row()else{panic!("no full staged row for failure")};
                assert_eq!(serde_json::to_value(recorded).unwrap()["status"],failure);
                let LiveStepAcknowledgedV8::Staged(o)=ack(&j,o)else{panic!()};let mut count=0;let o=o.release(|_|count+=1).unwrap_or_else(|_|panic!("actual failure cleanup"));
                assert_eq!(count,2);assert!(weak.iter().all(|w|w.upgrade().is_none()));
                let selected=o.prepare_receipt().unwrap_or_else(|_|panic!("real full receipt"));
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupSettled{receipt,..})=selected.selected_row()else{panic!()};
                assert_eq!(receipt["operations"].as_array().unwrap().len(),2);assert_eq!(receipt["settlement"],"completed");
                let LiveStepAcknowledgedV8::Released(o)=ack(&j,selected)else{panic!()};
                let selected=o.prepare_stop().unwrap_or_else(|_|panic!("sticky selected failure Stop"));
                use crate::live_invocation::source_journal::{SourceStopStatus as S,SourceStopReason as R};
                assert!(matches!(selected.selected_row(),EntryV8::Ordinary(SourceJournalEntry::Stop{status,reason,..}) if (*status,*reason)==if mode==0{(S::BudgetExhausted,R::BudgetExhausted)}else{(S::Rejected,R::StageRefused)}));
                let LiveStepAcknowledgedV8::Released(o)=ack(&j,selected)else{panic!()};
                let current=j.begin_session().unwrap();let(nr,ns,_,_,last)=current.inventory.step_reduce_facts().unwrap();assert_eq!((nr,ns),(r,s));assert_eq!(current.sequence(),seq+3);assert!(matches!(last,EntryV8::Ordinary(SourceJournalEntry::Stop{..})));drop(o);
            });
        };
        match mode {
            0 => CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_fuel_store(exercise),
            1 => {
                CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_arithmetic_store(exercise)
            }
            _ => CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_ensures_store(exercise),
        }
    }
}
#[test]
fn owned_step_append_failed_reduce_observers_capture_mixed_order_without_false_terminal() {
    for panic_at in 0..2 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_ensures_store(|c, l, k, _| {
            let j = journal(c, l, k);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::test_evaluated_failed(&j,&cancel,&policy,&clock,|e,weak|{
                let LiveStepAcknowledgedV8::Staged(o)=ack(&j,e.prepare_step().unwrap_or_else(|_|panic!()))else{panic!()};
                let mut count=0;let o=o.release(|_|{let index=count;count+=1;if index==panic_at{panic!("actual mixed observer")}}).unwrap_or_else(|_|panic!("all active operations attempted"));
                assert_eq!(count,2);assert!(weak.iter().all(|w|w.upgrade().is_none()));
                let selected=o.prepare_receipt().unwrap_or_else(|_|panic!("mixed actual receipt"));
                let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupSettled{receipt,..})=selected.selected_row()else{panic!()};
                for index in 0..2{assert_eq!(receipt["operations"][index]["outcome"],if index==panic_at{"failed"}else{"completed"});}assert_eq!(receipt["settlement"],"failed");
                let LiveStepAcknowledgedV8::Released(o)=ack(&j,selected)else{panic!()};let bytes=j.lease.try_borrow_mut().unwrap().read().unwrap();
                let rejected=o.prepare_stop().err().expect("failed receipt forbids Stop");assert!(j.hold().is_err());assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(),bytes);drop(rejected);
            });
        });
    }
}
#[test]
fn owned_step_append_wrong_container_has_zero_io_and_does_not_poison_foreign_container() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
            let o = e.prepare_step().unwrap_or_else(|_| panic!());
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, l, k| {
                let other = journal(c, l, k);
                let ownbytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
                let foreignbytes = other.lease.try_borrow_mut().unwrap().read().unwrap();
                let failed = other
                    .begin_session()
                    .unwrap()
                    .append_owned_step(o)
                    .err()
                    .expect("foreign container");
                assert!(matches!(
                    failed,
                    LiveOwnedStepAppendFailureV8::Before { .. }
                ));
                assert_eq!(j.lease.try_borrow_mut().unwrap().read().unwrap(), ownbytes);
                assert_eq!(
                    other.lease.try_borrow_mut().unwrap().read().unwrap(),
                    foreignbytes
                );
                other.hold().unwrap();
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        });
    });
}
#[test]
fn owned_step_append_compiler_empty_complete_moves_both_original_leaves_without_cleanup_rows() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_empty_complete_store(|c, l, k, _| {
        let j = journal(c, l, k);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_evaluated(&j, &cancel, &policy, &clock, |e, weak| {
            let facts = e.stage_facts().unwrap();
            let old = j.begin_session().unwrap();
            let (r, s, _, _, _) = old.inventory.original_reduce_facts().unwrap();
            let seq = old.sequence();
            let LiveStepAcknowledgedV8::Staged(o) = ack(
                &j,
                e.prepare_step()
                    .unwrap_or_else(|_| panic!("actual empty Staged")),
            ) else {
                panic!()
            };
            let staged = j
                .begin_session()
                .unwrap()
                .sequence()
                .checked_sub(1)
                .unwrap();
            let basis = facts.cleanup_basis(Some(staged as u32)).unwrap();
            let (_, e) = j.context().ready_runtime().unwrap();
            let plan = crate::resumable_effects::owned_frame::v2::compile_owned_reduce_v2(e.wait())
                .unwrap();
            let checked =
                crate::resumable_effects::owned_frame::v2::validate_owned_reduce_cleanup_v8(
                    &plan,
                    &basis,
                    facts.operations(),
                )
                .unwrap();
            assert_eq!(checked.active_operations(), &serde_json::json!([]));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let mut observed = 0;
            let released = o
                .release(|_| observed += 1)
                .unwrap_or_else(|_| panic!("compiler-empty pure cleanup"));
            assert_eq!(observed, 0);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let ready = released
                .into_ready()
                .unwrap_or_else(|_| panic!("actual Ready without fictitious ACK"));
            let selected = ready
                .prepare_transfer()
                .unwrap_or_else(|_| panic!("empty transfer"));
            let EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStepTransferReserved{cleanup,staged:actual,..})=selected.selected_row()else{panic!()};
            assert_eq!(*actual, staged as u32);
            assert!(matches!(cleanup,crate::live_invocation::source_journal::owned_wait_v8::reduce_model::ReduceCleanupV8::CompilerEmpty{}));
            let LiveStepAcknowledgedV8::Ready(ready) = ack(&j, selected) else {
                panic!()
            };
            let moved = ready
                .move_fields()
                .unwrap_or_else(|_| panic!("both actual leaves move to Report"));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let LiveStepAcknowledgedV8::Moved(moved) = ack(
                &j,
                moved
                    .prepare_completed()
                    .unwrap_or_else(|_| panic!("full mapped Report")),
            ) else {
                panic!()
            };
            let LiveStepAcknowledgedV8::Moved(moved) = ack(
                &j,
                moved
                    .prepare_transition()
                    .unwrap_or_else(|_| panic!("actual frozen Report carrier")),
            ) else {
                panic!()
            };
            moved.validate_live().unwrap();
            assert_eq!(moved.kind(), "complete");
            let current = j.begin_session().unwrap();
            let (nr, ns, _, _, _) = current.inventory.step_reduce_facts().unwrap();
            assert_eq!((nr, ns), (r, s));
            assert_eq!(current.sequence(), seq + 4);
            let bytes = j.lease.try_borrow_mut().unwrap().read().unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(!text.contains("owned_reduce_cleanup_started"));
            assert!(!text.contains("owned_reduce_cleanup_settled"));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(moved);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    });
}

/// Genuine successful SDK/target/Reduce/Step pipeline; no synthetic ACKs.
pub(super) fn test_moved<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
    clock: &'j dyn SourceInvocationClock,
    callback: impl FnOnce(LiveMovedStepV8<'j>, Vec<std::sync::Weak<[u8]>>),
) {
    test_evaluated(journal, cancel, policy, clock, |evaluated, weak| {
        let moved = full_success(journal, evaluated, |_| {});
        assert_eq!(moved.kind(), "continue");
        callback(moved, weak);
    });
}
