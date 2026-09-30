//! Genuine live owner producer; selecting Intent alone creates no ACK or host.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
use crate::live_invocation::SourceInvocationClock;
use std::sync::Arc;
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}

fn prepared<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j crate::agent_runtime::AgentCancellation,
    policy: &'j CapabilityPolicy,
) -> (LivePreparedOwnedEffectV8<'j>, Vec<std::sync::Weak<[u8]>>) {
    let (ready, weak) = test_ready_obligation(journal, cancel, &Clock, policy);
    let ready = journal
        .begin_session()
        .unwrap()
        .append_owned_effect(ready)
        .unwrap_or_else(|_| panic!("actual Ready ACK"));
    let consumed = ready
        .advance_ready()
        .unwrap_or_else(|_| panic!("actual source promotion"));
    let consumed = journal
        .begin_session()
        .unwrap()
        .append_owned_authorization_consumed(consumed)
        .unwrap_or_else(|_| panic!("actual Consumed ACK"));
    let held = consumed
        .reserve_owned_reduce()
        .unwrap_or_else(|_| panic!("actual hold"));
    let prepared = held
        .advance_authorization()
        .unwrap_or_else(|_| panic!("actual Prepared"));
    (prepared, weak)
}
#[test]
fn owned_wait_live_effect_intent_selection_matches_frozen_request_without_append_or_work() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (actual, weak) = prepared(&journal, &cancel, &policy);
        let before = journal.begin_session().unwrap();
        let before_facts = before.fold_for_live_test();
        let before_bytes = before.acknowledged_bytes();
        let obligation = actual
            .prepare_intent()
            .unwrap_or_else(|_| panic!("actual Intent selection"));
        obligation.validate_live().unwrap();
        {
            let permit = obligation.fixed_append_permit().unwrap();
            permit.validate_preflight(&journal).unwrap();
            assert_eq!(permit.selected_row(), obligation.selected_row());
            assert_eq!(
                (permit.sequence(), permit.acknowledged_bytes()),
                (22, before_bytes)
            );
        }
        assert!(obligation.belongs_to(&journal));
        assert_eq!(
            (obligation.sequence(), obligation.acknowledged_bytes()),
            (22, before_bytes)
        );
        let (runtime, execution) = journal.context().test_runtime_execution();
        let owner = &obligation.owner;
        let borrowed_store = journal.hold().unwrap();
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &borrowed_store.registration().expected_facts().scope,
            &owner.proposal,
        )
        .unwrap();
        let expected = crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::physical::request_digest_fields(
            owner.commitments.target_grant_digest(), owner.commitments.authorization_binding(),
            plan.operation(), plan.argument(), 0,
        );
        assert_eq!(
            obligation.selected_row(),
            &EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
                turn: 0,
                attempt: 0,
                operation: plan.operation().operation_id().into(),
                request_digest: expected,
            })
        );
        assert_ne!(
            owner.commitments.grant_digest(),
            owner.commitments.target_grant_digest()
        );
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        let after = journal.begin_session().unwrap().fold_for_live_test();
        assert_eq!(
            (after.stages, after.reserved_total, after.consumed_recorded),
            (
                before_facts.stages,
                before_facts.reserved_total,
                before_facts.consumed_recorded
            )
        );
        drop(plan);
        drop(obligation);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        assert!(journal.hold().is_err());
    });
}
#[test]
fn owned_wait_live_effect_intent_selection_guard_loss_retains_same_prepared_and_credit() {
    for before in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (actual, weak) = prepared(&journal, &cancel, &policy);
            if before {
                cancel.cancel();
                let failed = actual.prepare_intent().err().expect("no Intent");
                assert_eq!(failed.error, SourceJournalError::Binding);
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                drop(failed);
            } else {
                let actual = actual
                    .prepare_intent()
                    .unwrap_or_else(|_| panic!("actual selection"));
                cancel.cancel();
                assert_eq!(actual.validate_live(), Err(SourceJournalError::Binding));
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                drop(actual);
            }
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_wait_live_effect_intent_selected_operation_or_request_substitution_quarantines_owner() {
    for operation in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (actual, weak) = prepared(&journal, &cancel, &policy);
            let mut actual = actual
                .prepare_intent()
                .unwrap_or_else(|_| panic!("actual selection"));
            let EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
                operation: selected,
                request_digest,
                ..
            }) = &mut actual.selected
            else {
                panic!("Intent");
            };
            if operation {
                *selected = "substituted-operation".into();
            } else {
                *request_digest = actual.owner.commitments.grant_digest().into();
            }
            assert_eq!(actual.validate_live(), Err(SourceJournalError::Binding));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            drop(actual);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
