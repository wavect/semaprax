//! Genuine Consumed owner acquisition; inert Intent shape is not a producer grant.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::execution_revision::typed::TestProspectiveReduceLimitV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::tests::test_ready_obligation;
use crate::live_invocation::SourceInvocationClock;
use crate::resumable_effects::CapabilityPolicy;

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
fn bytes(journal: &SourceOwnedWaitJournalV8) -> Vec<u8> {
    journal.lease.try_borrow_mut().unwrap().read().unwrap()
}
fn consumed<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j AgentCancellation,
    policy: &'j CapabilityPolicy,
) -> (
    VerifiedOwnedAuthorizationConsumedV8<'j>,
    Vec<std::sync::Weak<[u8]>>,
) {
    let (ready, weak) = test_ready_obligation(journal, cancel, &Clock, policy);
    let ready = journal
        .begin_session()
        .unwrap()
        .append_owned_effect(ready)
        .unwrap_or_else(|_| panic!("actual Ready ACK"));
    let actual = ready
        .advance_ready()
        .unwrap_or_else(|_| panic!("actual same-owner Ready promotion"));
    let consumed = journal
        .begin_session()
        .unwrap()
        .append_owned_authorization_consumed(actual)
        .unwrap_or_else(|_| panic!("actual Consumed ACK"));
    consumed.validate_live().unwrap();
    (consumed, weak)
}
#[test]
fn owned_reduce_hold_acquisition_preserves_actual_owner_prefix_and_drop_retires_without_refund() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (owner, weak) = consumed(&journal, &cancel, &policy);
        let before = bytes(&journal);
        let sequence = owner.session.sequence();
        let mac = owner.session.inventory.authentication_tail().to_owned();
        let (reserved, stages, turn, attempt) =
            owner.session.inventory.prospective_reduce_facts().unwrap();
        assert_eq!(
            (reserved, stages),
            (5000, 3),
            "actual acknowledged evaluator reservations"
        );
        let held = owner
            .reserve_owned_reduce()
            .unwrap_or_else(|_| panic!("prospective acquisition"));
        held.validate_live().unwrap();
        assert_eq!(bytes(&journal), before);
        assert_eq!(held.owner.session.sequence(), sequence);
        assert_eq!(held.owner.session.inventory.authentication_tail(), mac);
        assert_eq!(
            held.owner
                .session
                .inventory
                .prospective_reduce_facts()
                .unwrap(),
            (reserved, stages, turn, attempt)
        );
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        {
            let registry = journal.prospective_reduce.borrow();
            let record = registry.as_ref().unwrap();
            assert_eq!(
                (record.turn, record.attempt, record.fuel),
                (turn, attempt, 1000)
            );
            assert_eq!((record.sequence, record.bytes), (sequence, before.len()));
            assert_eq!(record.authentication, mac);
        }
        let old = journal.hold().unwrap();
        // White-box move only: retry this SAME actual owner while its existing
        // hold remains retained. No cloned owner/token or production extractor.
        let HeldOwnedAuthorizationConsumedV8 { owner, hold } = held;
        let refused = owner
            .reserve_owned_reduce()
            .err()
            .expect("second credit refused");
        assert_eq!(refused.error, SourceJournalError::Order);
        assert_eq!(journal.prospective_reduce_identity.get(), 1);
        assert_eq!(bytes(&journal), before);
        old.validate_guard().unwrap();
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(refused);
        drop(hold);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        assert!(
            journal.prospective_reduce.borrow().is_some(),
            "no refund/replacement on Drop"
        );
        assert_eq!(old.validate_guard(), Err(SourceJournalError::Poisoned));
        assert!(journal.begin_session().is_err());
    });
}
fn history_legal_intent(journal: &SourceOwnedWaitJournalV8) -> EntryV8 {
    use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
        checked_owned_effect_request_v8, OwnedEffectSettlementInputsV8,
    };
    use crate::resumable_effects::owned_frame::v2;
    let document = bytes(journal);
    let rows = wire::decode_inventory(
        &document,
        &ExpectedRowV8 {
            invocation: journal.context.ordinary().invocation(),
            generation: journal.context.generation(),
            seq: 0,
            prev_mac: &"0".repeat(64),
            ordinary: journal.context.ordinary(),
        },
        &journal.key,
    )
    .unwrap();
    let state = rows
        .iter()
        .rev()
        .find_map(|e| match e {
            EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) => {
                Some(state)
            }
            _ => None,
        })
        .unwrap();
    let decision = rows
        .iter()
        .rev()
        .find_map(|e| match e {
            EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged { decision, .. }) => {
                Some(decision)
            }
            _ => None,
        })
        .unwrap();
    let response = rows
        .iter()
        .rev()
        .find_map(|e| match e {
            EntryV8::Ordinary(SourceJournalEntry::AttemptSettled { response, .. }) => {
                Some(response)
            }
            _ => None,
        })
        .unwrap();
    let (runtime, execution) = journal.context.test_runtime_execution();
    let decoded = execution
        .wait()
        .lifecycle()
        .proposal_schema()
        .decode(std::str::from_utf8(response).unwrap())
        .unwrap();
    let scope = &journal.context.registration().expected_facts().scope;
    let proposal = v2::bind_owned_wait_proposal_v8(execution.wait(), scope, &decoded).unwrap();
    let (turn, attempt) = match rows.last().unwrap() {
        EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { turn, attempt, .. }) => {
            (*turn, *attempt)
        }
        _ => panic!(),
    };
    let request = checked_owned_effect_request_v8(&OwnedEffectSettlementInputsV8 {
        runtime,
        execution,
        scope,
        turn,
        attempt,
        state,
        decision,
        proposal: &proposal,
    })
    .unwrap();
    EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
        turn,
        attempt,
        operation: request.operation().operation_id().into(),
        request_digest: request.request_digest(),
    })
}
#[test]
fn owned_reduce_hold_blocks_all_generic_sessions_before_candidate_without_spending_credit() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let (owner, weak) = consumed(&journal, &cancel, &policy);
        let before = bytes(&journal);
        let intent = history_legal_intent(&journal);
        // Pure authenticated inventory admits this history. Generic production
        // EffectIntent remains separately refused even without the hold.
        let encoded = {
            let expected = ExpectedRowV8 {
                invocation: journal.context.ordinary().invocation(),
                generation: journal.context.generation(),
                seq: owner.session.sequence().try_into().unwrap(),
                prev_mac: owner.session.inventory.authentication_tail(),
                ordinary: journal.context.ordinary(),
            };
            wire::encode(&intent, &expected, &journal.key).unwrap()
        };
        {
            let lease = journal.lease.borrow();
            inventory::checked_candidate_inventory_v8(
                &journal.context,
                &lease,
                &journal.key,
                &before,
                &encoded,
            )
            .unwrap();
        }
        let first = journal.begin_session().unwrap();
        let second = journal.begin_session().unwrap();
        let held = owner
            .reserve_owned_reduce()
            .unwrap_or_else(|_| panic!("acquire"));
        for session in [first, second, journal.begin_session().unwrap()] {
            let rejected = session
                .append(intent.clone())
                .err()
                .expect("registry-first generic refusal");
            assert!(matches!(
                rejected,
                AppendFailureV8::CandidateRefused {
                    error: SourceJournalError::Order,
                    ..
                }
            ));
            assert_eq!(bytes(&journal), before);
            held.validate_live().unwrap();
        }
        assert_eq!(journal.prospective_reduce_identity.get(), 1);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(held);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_reduce_hold_uses_actual_rebuilt_policy_exact_and_one_short_fuel_and_stage_limits() {
    use TestProspectiveReduceLimitV8::*;
    for (limits, succeeds) in [
        (ExactFuel, true),
        (FuelOneShort, false),
        (ExactStages, true),
        (StagesOneShort, false),
    ] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_hold_limits_store(
            limits,
            |context, lease, key, _| {
                let context = context.with_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = AgentCancellation::new();
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let (owner, weak) = consumed(&journal, &cancel, &policy);
                let before = bytes(&journal);
                let (r, s, _, _) = owner.session.inventory.prospective_reduce_facts().unwrap();
                assert_eq!((r, s), (5000, 3));
                match limits {
                    ExactFuel => {
                        assert_eq!(journal.context.ordinary().max_total_steps(), Some(6000))
                    }
                    FuelOneShort => {
                        assert_eq!(journal.context.ordinary().max_total_steps(), Some(5999))
                    }
                    ExactStages => assert_eq!(journal.context.ordinary().max_stages(), 4),
                    StagesOneShort => assert_eq!(journal.context.ordinary().max_stages(), 3),
                }
                match owner.reserve_owned_reduce() {
                    Ok(held) => {
                        assert!(succeeds);
                        held.validate_live().unwrap();
                        assert_eq!(bytes(&journal), before);
                        drop(held);
                    }
                    Err(rejected) => {
                        assert!(!succeeds);
                        assert_eq!(rejected.error, SourceJournalError::Capacity);
                        assert!(journal.prospective_reduce.borrow().is_none());
                        rejected._owner.validate_live().unwrap();
                        assert_eq!(bytes(&journal), before);
                        assert!(weak.iter().all(|w| w.strong_count() == 1));
                        drop(rejected);
                    }
                }
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    }
}
#[test]
fn owned_reduce_hold_checked_arithmetic_never_refunds_or_overflows() {
    assert_eq!(funding(5000, 3, 1000, 6000, 4), Ok(()));
    assert_eq!(
        funding(5000, 3, 1000, 5999, 4),
        Err(SourceJournalError::Capacity)
    );
    assert_eq!(
        funding(5000, 3, 1000, 6000, 3),
        Err(SourceJournalError::Capacity)
    );
    assert_eq!(
        funding(u64::MAX, 0, 1, u64::MAX, 1),
        Err(SourceJournalError::Capacity)
    );
    assert_eq!(
        funding(0, u32::MAX, 1, 1, u32::MAX),
        Err(SourceJournalError::Capacity)
    );
}

#[test]
fn owned_reduce_hold_cancelled_acquisition_keeps_empty_registry_and_permanently_retires_lineage() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (owner, weak) = consumed(&journal, &cancel, &policy);
            let before = bytes(&journal);
            cancel.cancel();
            let refused = owner
                .reserve_owned_reduce()
                .err()
                .expect("cancelled acquisition");
            assert!(journal.prospective_reduce.borrow().is_none());
            assert_eq!(journal.prospective_reduce_identity.get(), 0);
            let paths = std::fs::read_dir(directory)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 1);
            assert_eq!(std::fs::read(&paths[0]).unwrap(), before);
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(refused);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        },
    );
}
