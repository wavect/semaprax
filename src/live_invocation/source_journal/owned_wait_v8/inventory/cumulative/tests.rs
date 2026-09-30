//! Genuine Consumed history binds the retained execution, independently of I8.
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::execution_revision::typed::TestProspectiveReduceLimitV8;
use crate::live_invocation::source_journal::owned_wait_v8::{
    append::SourceOwnedWaitJournalV8, checked_context::CheckedOwnedWaitJournalContextV8,
    live_upstream::effect::tests::test_ready_obligation,
};
use crate::live_invocation::{InvocationClock, SourceInvocationClock};
use crate::resumable_effects::CapabilityPolicy;
use std::sync::Arc;
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
#[test]
fn cumulative_effect_prefix_binds_actual_execution_and_refuses_foreign_same_source_execution() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, _| {
            let context = context.with_cumulative_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let (ready, weak) = test_ready_obligation(&journal, &cancel, &Clock, &policy);
            let ready = journal
                .begin_session()
                .unwrap()
                .append_owned_effect(ready)
                .unwrap_or_else(|_| panic!("actual Ready ACK"));
            let consumed = ready
                .advance_ready()
                .unwrap_or_else(|_| panic!("actual Ready owner"));
            let owner = journal
                .begin_session()
                .unwrap()
                .append_owned_authorization_consumed(consumed)
                .unwrap_or_else(|_| panic!("actual Consumed ACK"));
            owner.validate_live().unwrap();
            let document = journal.test_observe_lease().borrow_mut().read().unwrap();
            let session = journal.begin_session().unwrap();
            let rows = session.test_observe_inventory().test_observe_entries();
            let context = journal.context().fold();
            assert_eq!(
                fold::fold(context, rows).unwrap().tail,
                fold::TailV8::ReadyPair
            );
            let scope = scope(context).unwrap();
            let state = rows
                .iter()
                .rev()
                .find_map(|row| match &row.entry {
                    EntryV8::Owned(Body::OwnedStateTransferCompleted { state, .. }) => Some(state),
                    _ => None,
                })
                .unwrap();
            let decision = rows
                .iter()
                .rev()
                .find_map(|row| match &row.entry {
                    EntryV8::Owned(Body::OwnedAuthorizationStaged { decision, .. }) => {
                        Some(decision)
                    }
                    _ => None,
                })
                .unwrap();
            let response = rows
                .iter()
                .rev()
                .find_map(|row| match &row.entry {
                    EntryV8::Ordinary(Ordinary::AttemptSettled { response, .. }) => Some(response),
                    _ => None,
                })
                .unwrap();
            let (runtime, execution) = journal.context().test_runtime_execution();
            let decoded = execution
                .wait()
                .lifecycle()
                .proposal_schema()
                .decode(std::str::from_utf8(response).unwrap())
                .unwrap();
            let proposal =
                v2::bind_owned_wait_proposal_v8(execution.wait(), &scope, &decoded).unwrap();
            let inputs = OwnedEffectSettlementInputsV8 {
                runtime,
                execution,
                scope: &scope,
                turn: 0,
                attempt: 0,
                state,
                decision,
                proposal: &proposal,
            };
            assert_ne!(
                execution.ordinary().invocation(),
                context.ordinary.invocation()
            );
            checked_prefix(context, rows, &inputs, None).unwrap();
            CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_hold_limits_store(
                TestProspectiveReduceLimitV8::ExactFuel,
                |foreign, _, _, _| {
                    let (_, foreign_execution) = foreign.test_runtime_execution();
                    assert_eq!(
                        execution.wait().binding(),
                        foreign_execution.wait().binding()
                    );
                    assert_ne!(
                        execution.ordinary().invocation(),
                        foreign_execution.ordinary().invocation()
                    );
                    let foreign_inputs = OwnedEffectSettlementInputsV8 {
                        execution: foreign_execution,
                        ..inputs
                    };
                    assert!(matches!(
                        checked_prefix(context, rows, &foreign_inputs, None),
                        Err(Error::Binding)
                    ));
                },
            );
            drop(session);
            assert_eq!(
                journal.test_observe_lease().borrow_mut().read().unwrap(),
                document
            );
            assert!(weak.iter().all(|root| root.strong_count() == 1));
            drop(owner);
            assert!(weak.iter().all(|root| root.upgrade().is_none()));
        },
    );
}
