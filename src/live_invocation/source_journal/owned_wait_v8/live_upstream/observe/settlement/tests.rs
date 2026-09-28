//! Actual initial/continued Observe and physical ACKs, with independent source oracle.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::{InvocationClock, SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
use std::{cell::Cell, sync::Arc};
thread_local! {static INITIAL_ENTRIES:Cell<usize>=const{Cell::new(0)};}
pub(crate) fn test_initial_observe_entry_v8() {
    INITIAL_ENTRIES.with(|n| n.set(n.get() + 1));
}
fn entries() -> usize {
    INITIAL_ENTRIES.with(Cell::get)
}
struct Clock;
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
fn initial<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
) -> InitializedLiveOwnedRunV8<'j> {
    let input = super::super::super::tests::input(journal.context());
    match initialize_live_actor_v8(journal, input, cancel) {
        Ok(Ok(owner)) => owner,
        _ => panic!("actual initialization"),
    }
}
fn ordinary(
    journal: &SourceOwnedWaitJournalV8,
    state: &serde_json::Value,
) -> crate::interpreter::retained_call::RetainedCallEvaluation {
    use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue as R};
    let fields = state["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| {
            let value = &field["value"];
            let retained = if value["kind"] == "bytes" {
                let hex = value["hex"].as_str().unwrap();
                R::Bytes(
                    (0..hex.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                        .collect(),
                )
            } else {
                match value["tag"].as_str().unwrap() {
                    "i64" => R::I64(value["value"].as_i64().unwrap()),
                    "i32" => R::I32(value["value"].as_i64().unwrap() as i32),
                    "bool" => R::Bool(value["value"].as_bool().unwrap()),
                    "u8" => R::U8(value["value"].as_u64().unwrap() as u8),
                    "usize" => R::Usize(value["value"].as_u64().unwrap()),
                    _ => panic!("fixture scalar"),
                }
            };
            RetainedField {
                field: crate::hir::DeclarationId::new(field["identity"].as_str().unwrap()),
                value: retained,
            }
        })
        .collect();
    let arg = R::Record(RetainedRecord {
        record: crate::hir::DeclarationId::new(state["declaration"].as_str().unwrap()),
        fields,
    });
    let proof = journal
        .context()
        .ready_runtime()
        .unwrap()
        .1
        .wait()
        .observe();
    let program = proof.helper().program();
    let prepared = crate::interpreter::retained_call::prepare_retained_call(
        program,
        proof.function().id.as_str(),
    )
    .unwrap();
    crate::interpreter::retained_call::evaluate_retained_call(
        program,
        &prepared,
        &[arg],
        journal.context().ordinary().max_steps_per_stage().unwrap(),
    )
    .unwrap()
}
#[test]
fn owned_observe_settlement_initial_actual_source_oracle_and_once_consumed() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let initialized = initial(&journal, &cancel);
            let weak = initialized.owner.test_weak();
            let count = entries();
            let observed = observe_live_actor_v8(initialized)
                .unwrap_or_else(|_| panic!("cumulative initial Observe"));
            assert_eq!(entries(), count + 1);
            let oracle = ordinary(&journal, observed.owner.facts());
            assert_eq!(observed.owner.consumed(), oracle.steps_used as u64);
            let crate::interpreter::retained_call::RetainedCallOutcome::Returned(result) =
                oracle.outcome
            else {
                panic!("ordinary observation")
            };
            assert_eq!(
                crate::agent_lifecycle::encode_value(&result),
                observed.observation.ordinary_bytes()
            );
            let session = journal.begin_session().unwrap();
            let rows = &session.test_observe_inventory().test_observe_entries();
            let settled = &rows[rows.len() - 2].entry;
            let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedObserveSettled {
                consumed,
                settlement: ObserveSettlementV8::Observed { .. },
                ..
            }) = settled
            else {
                panic!("actual consumed row immediately before observed")
            };
            assert_eq!(*consumed, observed.owner.consumed());
            assert!(matches!(
                rows.last().unwrap().entry,
                EntryV8::Ordinary(SourceJournalEntry::TurnObserved { turn: 0, .. })
            ));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(observed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        },
    );
}
#[test]
fn owned_observe_settlement_initial_ensures_retains_failed_cause_state_and_count() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_initial_observe_ensures_store(
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let initialized = initial(&journal, &cancel);
            let weak = initialized.owner.test_weak();
            let count = entries();
            let failure = observe_live_actor_v8(initialized)
                .err()
                .expect("actual failed Observe");
            let LiveObserveFailureV8::Settlement(LiveObserveSettlementActorFailureV8::Failed(
                failed,
            )) = &failure
            else {
                panic!("actual failed settled owner")
            };
            let data = failed.owner.data().unwrap();
            let oracle = ordinary(&journal, &data.state);
            assert_eq!(data.consumed, oracle.steps_used as u64);
            let crate::interpreter::retained_call::RetainedCallOutcome::LanguageFailure(status) =
                oracle.outcome
            else {
                panic!("ordinary failed Observe")
            };
            assert_eq!(data.failure, Some(OwnedFrameFailure::Language(status)));
            assert!(data.observation.is_none());
            assert_eq!(entries(), count + 1);
            let current = journal.begin_session().unwrap();
            let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedObserveSettled {
                consumed,
                settlement: ObserveSettlementV8::Failed { status },
                ..
            }) = &current
                .test_observe_inventory()
                .test_observe_entries()
                .last()
                .unwrap()
                .entry
            else {
                panic!("failure row")
            };
            assert_eq!(*consumed, data.consumed);
            assert_eq!(status["failure"], "language_failure");
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        },
    );
}
#[test]
#[cfg(unix)]
fn owned_observe_settlement_initial_actual_append_faults_retain_owner_without_reentry() {
    for observed_row in [false, true] {
        for mode in 0..4 {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
                true,
                |context, lease, key, _| {
                    let context = context.with_cumulative_initialization(&lease).unwrap();
                    let journal =
                        SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                    let cancel = AgentCancellation::new();
                    let initialized = initial(&journal, &cancel);
                    let weak = initialized.owner.test_weak();
                    let count = entries();
                    let number = initialized.session.sequence() + if observed_row { 3 } else { 2 };
                    {
                        let mut lease = journal.test_observe_lease().borrow_mut();
                        match mode {
                            0 => lease.test_fail_before_write(number),
                            1 => lease.test_fail_after_write(number),
                            2 => lease.test_fail_before_sync(number),
                            _ => lease.test_fail_after_sync(number),
                        }
                    }
                    let failure = observe_live_actor_v8(initialized)
                        .err()
                        .expect("same-FD failure");
                    assert!(matches!(failure, LiveObserveFailureV8::Settlement(_)));
                    assert_eq!(entries(), count + 1);
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    assert!(journal.hold().is_err());
                    assert!(journal.begin_session().is_err());
                    assert_eq!(entries(), count + 1);
                    drop(failure);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
            );
        }
    }
}

pub(super) fn with_continued(
    failed: bool,
    callback: impl for<'j> FnOnce(
        &'j SourceOwnedWaitJournalV8,
        LiveOwnedObserveSettlementAppendV8<'j>,
        Vec<std::sync::Weak<[u8]>>,
        crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
        Option<ResumableChannelValue>,
        usize,
        &'j AgentCancellation,
    ),
) {
    let run = |context: CheckedOwnedWaitJournalContextV8,
               lease: crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8,
               key: crate::resumable_effects::source_checkpoint::SourceCheckpointKey,
               _: &std::path::Path| {
        let context = context.with_cumulative_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock;
        crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::test_moved(
            &journal,
            &cancel,
            &policy,
            &clock,
            |moved, weak| {
                let ledger = *moved.accounting();
                let (observation, mut consumed) = if failed {
                    (None, 0)
                } else {
                    let (value, used) = moved.test_observe_oracle();
                    (Some(value), used)
                };
                let state_obligation = moved
                    .prepare_continue()
                    .unwrap_or_else(|_| panic!("actual Continue"));
                let state = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continue(state_obligation)
                    .unwrap_or_else(|_| panic!("State ACK"))
                    .advance_continue()
                    .unwrap_or_else(|_| panic!("State owner"));
                let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::LiveContinueAcknowledgedV8::State(state)=state else{panic!("State phase")};
                let observed = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continue(
                        state
                            .prepare_observe()
                            .unwrap_or_else(|_| panic!("Observe F")),
                    )
                    .unwrap_or_else(|_| panic!("Observe original ACK"))
                    .advance_continue()
                    .unwrap_or_else(|_| panic!("actual Observe"));
                let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::LiveContinueAcknowledgedV8::Observed(observed)=observed else{panic!("observed/failed owner")};
                let obligation = observed
                    .prepare_observe_settlement()
                    .unwrap_or_else(|_| panic!("actual settlement producer"));
                if failed {
                    let data = obligation.owner.data().unwrap();
                    let oracle = ordinary(&journal, &data.state);
                    let crate::interpreter::retained_call::RetainedCallOutcome::LanguageFailure(
                        status,
                    ) = oracle.outcome
                    else {
                        panic!("ordinary continued failed Observe")
                    };
                    assert_eq!(data.failure, Some(OwnedFrameFailure::Language(status)));
                    consumed = oracle.steps_used;
                }
                callback(
                    &journal,
                    obligation,
                    weak,
                    ledger,
                    observation,
                    consumed,
                    &cancel,
                );
            },
        );
    };
    if failed {
        CheckedOwnedWaitJournalContextV8::test_with_actual_continued_observe_ensures_store(run)
    } else {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(true, run)
    }
}
pub(super) fn ack<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedObserveSettlementAppendV8<'j>,
) -> LiveSettledObserveV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_observe_settlement(owner)
        .unwrap_or_else(|_| panic!("sameFD settlement/Observed ACK"))
        .advance_observe_settlement()
        .unwrap_or_else(|_| panic!("actual successor"))
}
#[test]
fn owned_observe_settlement_continued_actual_oracle_same_ledger_and_once_funding() {
    with_continued(
        false,
        |journal, owner, weak, ledger, observation, consumed, _| {
            let before = journal.begin_session().unwrap();
            let (r, s, turn, _) = before
                .test_observe_inventory()
                .observe_settlement_facts()
                .unwrap();
            let settled = ack(journal, owner);
            let data = settled.owner.data().unwrap();
            assert_eq!(data.observation, observation);
            assert_eq!(data.consumed, consumed as u64);
            let LiveObserveSettlementOwnerV8::Continued(c) = &settled.owner else {
                panic!()
            };
            assert_eq!(c.test_accounting(), &ledger);
            let final_owner = ack(
                journal,
                settled
                    .prepare_turn_observed()
                    .unwrap_or_else(|_| panic!("immediate observed obligation")),
            );
            let current = journal.begin_session().unwrap();
            let (nr, ns, next, _) = current
                .test_observe_inventory()
                .observe_settlement_facts()
                .unwrap();
            assert_eq!((nr, ns, next), (r, s, turn));
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            drop(final_owner);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        },
    );
}
#[test]
fn owned_observe_settlement_continued_ensures_retains_actual_failed_state_count() {
    with_continued(true, |journal, owner, weak, ledger, _, consumed, _| {
        let settled = ack(journal, owner);
        let data = settled.owner.data().unwrap();
        assert!(matches!(data.failure, Some(OwnedFrameFailure::Language(_))));
        assert!(data.observation.is_none());
        assert!(data.consumed > 0);
        assert_eq!(data.consumed, consumed as u64);
        let LiveObserveSettlementOwnerV8::Continued(c) = &settled.owner else {
            panic!()
        };
        assert_eq!(c.test_accounting(), &ledger);
        let failure = settled
            .prepare_turn_observed()
            .err()
            .expect("no observed/model after failed Observe");
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(failure);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
#[cfg(unix)]
fn owned_observe_settlement_continued_actual_faults_and_postack_guard_loss_never_reenter() {
    for observed_row in [false, true] {
        for mode in 0..4 {
            with_continued(false, |journal, mut owner, weak, _, _, _, _| {
                if observed_row {
                    owner = ack(journal, owner)
                        .prepare_turn_observed()
                        .unwrap_or_else(|_| panic!("Observed obligation"));
                }
                let count=crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
                let number = owner.sequence() + 1;
                {
                    let mut lease = journal.test_observe_lease().borrow_mut();
                    match mode {
                        0 => lease.test_fail_before_write(number),
                        1 => lease.test_fail_after_write(number),
                        2 => lease.test_fail_before_sync(number),
                        _ => lease.test_fail_after_sync(number),
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_observe_settlement(owner)
                    .err()
                    .expect("physical fault");
                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(),count);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}
#[test]
fn owned_observe_settlement_fresh_mac_hostile_rows_cannot_replace_actual_source_facts() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let observed = observe_live_actor_v8(initial(&journal, &cancel))
                .unwrap_or_else(|_| panic!("actual initial Observe"));
            let current = journal.begin_session().unwrap();
            let baseline: Vec<_> = current
                .test_observe_inventory()
                .test_observe_entries()
                .iter()
                .map(|v| v.entry.clone())
                .collect();
            let settled = baseline.len() - 2;
            let key =
                crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]);
            let encode = |rows: &[EntryV8]| {
                let mut previous = "0".repeat(64);
                let mut document = Vec::new();
                for (seq, row) in rows.iter().enumerate() {
                    let expected =
                        crate::live_invocation::source_journal::owned_wait_v8::ExpectedRowV8 {
                            invocation: journal.context().ordinary().invocation(),
                            generation: journal.context().generation(),
                            seq: seq as u32,
                            prev_mac: &previous,
                            ordinary: journal.context().ordinary(),
                        };
                    let bytes = wire::encode(row, &expected, &key).unwrap();
                    previous = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
                        ["authentication"]
                        .as_str()
                        .unwrap()
                        .into();
                    document.extend(bytes)
                }
                document
            };
            let validate = |document: &[u8]| {
                crate::live_invocation::source_journal::owned_wait_v8::inventory::checked_inventory_v8(journal.context(),&journal.test_observe_lease().borrow(),&key,document).is_ok()
            };
            assert!(
                validate(&encode(&baseline)),
                "positive same-context fresh-MAC reconstruction"
            );
            for mode in 0..7 {
                let mut rows = baseline.clone();
                let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedObserveSettled {
                    turn,
                    reservation,
                    state_digest,
                    consumed,
                    settlement,
                }) = &mut rows[settled]
                else {
                    panic!()
                };
                match mode {
                    0 => *turn += 1,
                    1 => *reservation += 1,
                    2 => *state_digest = format!("sha256:{}", "f".repeat(64)),
                    3 => {
                        *consumed =
                            journal.context().ordinary().max_steps_per_stage().unwrap() as u64 + 1
                    }
                    4 => {
                        let ObserveSettlementV8::Observed {
                            observation_digest, ..
                        } = settlement
                        else {
                            panic!()
                        };
                        *observation_digest = format!("sha256:{}", "e".repeat(64));
                    }
                    5 => {
                        let duplicated = rows[settled].clone();
                        rows.insert(settled + 1, duplicated);
                    }
                    _ => {
                        rows.remove(settled);
                    }
                }
                assert!(!validate(&encode(&rows)), "hostile fresh-MAC mode {mode}");
            }
            assert!(journal.hold().is_ok());
            drop(observed);
        },
    );
}

#[test]
fn owned_observe_settlement_continued_postack_cancel_keeps_owner_and_permanent_quarantine() {
    with_continued(false, |journal, owner, weak, _, _, _, cancel| {
        let settled = ack(journal, owner);
        cancel.cancel();
        let failure = settled
            .prepare_turn_observed()
            .err()
            .expect("postACK guard loss");
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        drop(failure);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
