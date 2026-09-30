//! Real source lineage, with no test ACK producer or physical effect dispatch.
use super::super::authorize::authorize_live_actor_v8;
use super::super::model::tests::completed_test_actor;
use super::*;
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

/// Runs the real fixed source/SDK actor; it never fabricates a staged root or ACK.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_ready_obligation<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn SourceInvocationClock,
    policy: &'j CapabilityPolicy,
) -> (LiveOwnedEffectAppendV8<'j>, Vec<std::sync::Weak<[u8]>>) {
    let completed = completed_test_actor(journal, cancellation, clock, 3);
    let staged = authorize_live_actor_v8(completed).unwrap_or_else(|_| panic!("actual authorize"));
    let leaves = staged.owner.test_weak();
    let obligation = prepare_live_effect_ready_v8(staged, policy)
        .unwrap_or_else(|failed| panic!("actual Ready {:?}", failed.error));
    (obligation, leaves)
}

#[test]
fn owned_wait_live_effect_ready_requires_current_policy_and_retains_actual_roots() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let completed = completed_test_actor(&journal, &cancel, &Clock, 3);
        let state = completed.owner.test_weak();
        let staged =
            authorize_live_actor_v8(completed).unwrap_or_else(|_| panic!("actual authorize"));
        let leaves = staged.owner.test_weak();
        let prior = staged.session.fold_for_live_test();
        let denied = CapabilityPolicy::new(vec![]).unwrap();
        let allowed = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let rejected = prepare_live_effect_ready_v8(staged, &denied)
            .err()
            .expect("policy");
        assert_eq!(rejected.error, SourceJournalError::Binding);
        assert_eq!(rejected.owner.session.sequence(), 20);
        assert!(leaves.iter().all(|w| w.strong_count() == 1));
        let obligation = prepare_live_effect_ready_v8(rejected.owner, &allowed)
            .unwrap_or_else(|failed| panic!("Ready {:?}", failed.error));
        obligation.validate_live().unwrap();
        assert!(obligation.belongs_to(&journal));
        assert_eq!(obligation.sequence(), 20);
        assert_eq!(leaves.len(), state.len() + 1);
        assert!(state
            .iter()
            .all(|old| leaves.iter().any(|new| std::sync::Weak::ptr_eq(old, new))));
        assert!(leaves.iter().all(|w| w.strong_count() == 1));
        let EntryV8::Owned(journal_model::OwnedBodyV8::OwnedAuthorizationReady {
            turn,
            attempt,
            staged,
            grant_digest,
            ..
        }) = obligation.selected_row()
        else {
            panic!("exact Ready");
        };
        assert_eq!((*turn, *attempt, *staged), (0, 0, 19));
        assert_eq!(grant_digest, obligation.commitments.grant_digest());
        assert_ne!(grant_digest, obligation.commitments.target_grant_digest());
        let after = obligation.owner.session.fold_for_live_test();
        assert_eq!(after.reserved_total, prior.reserved_total);
        assert_eq!(after.consumed_recorded, prior.consumed_recorded);
        assert_eq!(after.stages, prior.stages);
        assert_eq!(
            obligation.acknowledged_bytes(),
            obligation.owner.session.acknowledged_bytes()
        );
        drop(obligation);
        assert!(leaves.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_wait_live_effect_ready_refused_and_cancelled_keep_staged_owner_without_append() {
    for refused in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let (runtime, execution) = journal.context().test_runtime_execution();
            let budget = if refused {
                runtime.owned_wait_task_v8(execution).unwrap().budget + 1
            } else {
                3
            };
            let completed = completed_test_actor(&journal, &cancel, &Clock, budget);
            let staged =
                authorize_live_actor_v8(completed).unwrap_or_else(|_| panic!("actual authorize"));
            let leaves = staged.owner.test_weak();
            let before = staged.session.acknowledged_bytes();
            if !refused {
                cancel.cancel();
            }
            let allowed = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let failed = prepare_live_effect_ready_v8(staged, &allowed)
                .err()
                .expect("no Ready");
            assert_eq!(failed.error, SourceJournalError::Binding);
            assert_eq!(failed.owner.session.sequence(), 20);
            assert_eq!(failed.owner.session.acknowledged_bytes(), before);
            assert!(leaves.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_wait_live_effect_ready_inert_generic_append_cannot_refresh_live_prefix() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let completed = completed_test_actor(&journal, &cancel, &Clock, 3);
        let staged =
            authorize_live_actor_v8(completed).unwrap_or_else(|_| panic!("actual authorize"));
        let leaves = staged.owner.test_weak();
        let allowed = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let obligation = prepare_live_effect_ready_v8(staged, &allowed)
            .unwrap_or_else(|failed| panic!("Ready {:?}", failed.error));
        // Existing generic §20 metadata is legal; this ACK carries no owner.
        let inert = journal.begin_session().unwrap();
        let inert = match inert.append(obligation.selected_row().clone()) {
            Ok(session) => session,
            Err(_) => panic!("existing generic inert Ready metadata"),
        };
        assert_eq!(inert.sequence(), 21);
        assert_eq!(obligation.sequence(), 20);
        assert_eq!(obligation.validate_live(), Err(SourceJournalError::Order));
        assert!(leaves.iter().all(|w| w.strong_count() == 1));
        drop(obligation);
        assert!(leaves.iter().all(|w| w.upgrade().is_none()));
    });
}
