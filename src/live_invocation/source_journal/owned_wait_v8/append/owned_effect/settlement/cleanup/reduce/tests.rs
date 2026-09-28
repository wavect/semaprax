//! Real SDK/host/Decision finalizer/Outcome followed by the original full-F ACK.
//! No raw ACK, lower caller budget, owner reconstruction or extra host call.
use super::super::tests::test_executed;
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::{InvocationClock, SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
use std::cell::Cell;
use std::sync::Arc;
struct Clock {
    now: Cell<i64>,
    reads: Cell<usize>,
    expire_at: Cell<usize>,
    expired: Cell<i64>,
}
impl Clock {
    fn new() -> Self {
        Self {
            now: Cell::new(1),
            reads: Cell::new(0),
            expire_at: Cell::new(usize::MAX),
            expired: Cell::new(0),
        }
    }
}
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        let n = self.reads.get() + 1;
        self.reads.set(n);
        if n == self.expire_at.get() {
            self.now.set(self.expired.get());
        }
        self.now.get()
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
fn journal(
    context: CheckedOwnedWaitJournalContextV8,
    lease: crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8,
    key: SourceCheckpointKey,
) -> SourceOwnedWaitJournalV8 {
    let context = context.with_initialization(&lease).unwrap();
    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap()
}
fn ordinary_step(
    journal: &SourceOwnedWaitJournalV8,
    args: &[crate::interpreter::retained_call::RetainedValue],
) -> (serde_json::Value, usize) {
    use crate::interpreter::retained_call::{self, RetainedCallOutcome, RetainedValue as R};
    let (_, execution) = journal.context().ready_runtime().unwrap();
    let plan = crate::resumable_effects::owned_frame::v2::compile_owned_reduce_v2(execution.wait())
        .unwrap();
    let program = plan.helper().program();
    let prepared =
        retained_call::prepare_retained_call(program, plan.function().id.as_str()).unwrap();
    let evaluated = retained_call::evaluate_retained_call(
        program,
        &prepared,
        args,
        execution.evaluation_fuel(),
    )
    .unwrap();
    let RetainedCallOutcome::Returned(R::Variant(step)) = evaluated.outcome else {
        panic!("ordinary full Step")
    };
    let fields = step.fields.iter().map(|field| {
        let value = match &field.value {
            R::Bytes(bytes) => serde_json::json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(bytes)}),
            R::I64(value) => serde_json::json!({"tag":"i64","value":value}),
            other => panic!("unexpected actual fixture Step leaf: {other:?}"),
        };
        serde_json::json!({"identity":field.field.as_str(),"value":value})
    }).collect::<Vec<_>>();
    (
        serde_json::json!({"declaration":step.variant.as_str(),"case":step.case.as_str(),"fields":fields}),
        evaluated.steps_used,
    )
}
#[test]
fn owned_original_reduce_actual_ack_spends_once_and_evaluates_same_roots() {
    let exercise = |context, lease, key, _: &std::path::Path| {
        let journal = journal(context, lease, key);
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock::new();
        test_executed(&journal, &cancel, &policy, &clock, |executed, weak| {
            let args = executed.test_reduce_arguments_v8();
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let (expected_step, expected_consumed) = ordinary_step(&journal, &args);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            let accounting = *executed.accounting();
            let old = journal.begin_session().unwrap();
            let (r, s, _, _, _) = old.inventory.released_reduce_facts().unwrap();
            let oldseq = old.sequence();
            let selected = executed
                .prepare_reduce()
                .unwrap_or_else(|_| panic!("select"));
            let verified = old
                .append_owned_reduce_reservation(selected)
                .unwrap_or_else(|_| panic!("real original ACK"));
            let (nr, ns, _, _, row) = verified.session.inventory.original_reduce_facts().unwrap();
            let row = row.clone();
            let EntryV8::Ordinary(SourceJournalEntry::StageReservation { fuel, .. }) = &row else {
                panic!()
            };
            assert_eq!(nr, r + *fuel as u64);
            assert_eq!(ns, s + 1);
            assert_eq!(verified.session.sequence(), oldseq + 1);
            let staged = verified
                .advance_reduce()
                .unwrap_or_else(|_| panic!("actual source entry"));
            staged.validate_live().unwrap();
            let facts = staged.stage_facts().unwrap();
            assert_eq!(facts.allowance(), 1000);
            assert!(facts.consumed() > 0 && facts.consumed() < 1000);
            assert_eq!(facts.step(), Some(&expected_step));
            assert_eq!(facts.consumed(), expected_consumed);
            assert_eq!(staged.accounting(), &accounting);
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert_eq!(journal.begin_session().unwrap().sequence(), oldseq + 1);
            // An acknowledged original charge cannot be repeated or refunded.
            let competitor = journal.begin_session().unwrap().append(row.clone());
            assert!(competitor.is_err());
            drop(staged);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    };
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(true, exercise);
    CheckedOwnedWaitJournalContextV8::test_with_actual_complete_store(exercise);
}
#[cfg(unix)]
#[test]
fn owned_original_reduce_real_ack_faults_never_enter_evaluator_or_release_roots() {
    for mode in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let journal = journal(context, lease, key);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            test_executed(&journal, &cancel, &policy, &clock, |executed, weak| {
                let selected = executed
                    .prepare_reduce()
                    .unwrap_or_else(|_| panic!("select"));
                let seq = selected.sequence() + 1;
                {
                    let mut lease = journal.lease.try_borrow_mut().unwrap();
                    match mode {
                        0 => lease.test_fail_before_write(seq),
                        1 => lease.test_fail_after_write(seq),
                        2 => lease.test_fail_before_sync(seq),
                        _ => lease.test_fail_after_sync(seq),
                    }
                }
                let failed = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_reduce_reservation(selected)
                    .err()
                    .expect("uncertain physical ACK");
                assert!(matches!(
                    &failed,
                    LiveOriginalReduceAppendFailureV8::Append {
                        _failure: AppendFailureV8::InDoubt { .. },
                        ..
                    }
                ));
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        });
    }
}
#[test]
fn owned_original_reduce_actual_source_failure_retains_partial_and_provisional_roots() {
    for mode in 0..3 {
        let exercise = |context, lease, key, _: &std::path::Path| {
            let journal = journal(context, lease, key);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            test_executed(&journal, &cancel, &policy, &clock, |executed, weak| {
                let selected = executed
                    .prepare_reduce()
                    .unwrap_or_else(|_| panic!("select"));
                let verified = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_reduce_reservation(selected)
                    .unwrap_or_else(|_| panic!("original ACK"));
                let staged = verified
                    .advance_reduce()
                    .unwrap_or_else(|_| panic!("actual source failure is staged"));
                let facts = staged.stage_facts().unwrap();
                assert!(facts.step().is_none());
                let basis = facts.cleanup_basis(None).unwrap();
                assert_eq!(
                    basis["kind"],
                    if mode == 2 {
                        "provisional_failure"
                    } else {
                        "partial_failure"
                    }
                );
                assert_eq!(
                    basis["status"]["failure"],
                    if mode == 0 {
                        "fuel_exhausted"
                    } else {
                        "language_failure"
                    }
                );
                if mode == 0 {
                    assert_eq!(facts.consumed(), 1000);
                } else {
                    assert!(facts.consumed() > 0 && facts.consumed() < 1000);
                }
                if mode != 2 {
                    assert_eq!(basis["transfer_prefix"].as_array().unwrap().len(), 1);
                }
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(staged);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        };
        match mode {
            0 => CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_fuel_store(exercise),
            1 => {
                CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_arithmetic_store(exercise)
            }
            _ => CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_ensures_store(exercise),
        };
    }
}

#[test]
fn owned_original_reduce_postack_cancel_and_guard_losses_keep_exact_boundary_owner() {
    use crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveReduceEvaluationFailureV8;
    use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::reduce::LiveReduceAdvanceFailureV8;
    for mode in 0..3 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let journal = journal(context, lease, key);
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock::new();
            test_executed(&journal, &cancel, &policy, &clock, |executed, weak| {
                let selected = executed
                    .prepare_reduce()
                    .unwrap_or_else(|_| panic!("select"));
                let verified = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_reduce_reservation(selected)
                    .unwrap_or_else(|_| panic!("full-F ACK"));
                clock.reads.set(0);
                clock.expired.set(
                    journal
                        .context()
                        .ordinary()
                        .deadline_millis()
                        .checked_add(1)
                        .unwrap(),
                );
                match mode {
                    0 => cancel.cancel(),
                    1 => clock.expire_at.set(3),
                    _ => clock.expire_at.set(6),
                }
                let failure = verified
                    .advance_reduce()
                    .err()
                    .expect("actual entry/handoff/post-eval failure");
                match &failure {
                    LiveReduceAdvanceFailureV8::Evaluation {
                        _owner: LiveReduceEvaluationFailureV8::Executed { error, .. },
                        ..
                    } if mode == 0 => assert_eq!(*error, SourceJournalError::Binding),
                    LiveReduceAdvanceFailureV8::Evaluation {
                        _owner: LiveReduceEvaluationFailureV8::Prepared { error, .. },
                        ..
                    } if mode == 1 => assert_eq!(*error, SourceJournalError::Time),
                    LiveReduceAdvanceFailureV8::Evaluation {
                        _owner: LiveReduceEvaluationFailureV8::Staged { owner, error },
                        ..
                    } if mode == 2 => {
                        assert_eq!(*error, SourceJournalError::Time);
                        assert!(
                            owner.failure().is_none(),
                            "source success retained separately from guard error"
                        );
                    }
                    _ => panic!("failure must preserve the actual boundary holder"),
                }
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        });
    }
}

/// Genuine first-turn physical pipeline. The caller selects only an existing
/// closed checked Context fixture; this helper creates no ACK or owner from data.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_evaluated<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
    clock: &'j dyn SourceInvocationClock,
    callback: impl FnOnce(LiveEvaluatedOwnedReduceV8<'j>, Vec<std::sync::Weak<[u8]>>),
) {
    test_executed(journal, cancel, policy, clock, |executed, weak| {
        let selected = executed
            .prepare_reduce()
            .unwrap_or_else(|_| panic!("actual original Reduce selection"));
        let verified = journal
            .begin_session()
            .unwrap()
            .append_owned_reduce_reservation(selected)
            .unwrap_or_else(|_| panic!("actual original full-F ACK"));
        let evaluated = verified
            .advance_reduce()
            .unwrap_or_else(|_| panic!("actual consuming reducer"));
        evaluated.validate_live().unwrap();
        assert!(evaluated.stage_facts().unwrap().step().is_some());
        assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
        callback(evaluated, weak);
    });
}
