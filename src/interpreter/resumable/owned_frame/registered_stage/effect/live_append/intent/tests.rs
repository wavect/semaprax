//! Trusted test ACKs exercise shared old activation checks, not a live producer.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::authorize::effect_tests::{
    proposal, ready,
};
use crate::live_invocation::source_journal::{
    CheckedOwnedWaitJournalContextV8, SourceOwnedWaitJournalV8,
};

#[test]
fn owned_wait_effect_activation_shared_ack_boundary_has_no_target_or_release() {
    for wrong in 0..6 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = Arc::new(context);
            let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
            let (runtime, execution) = context.test_runtime_execution();
            let store = journal.hold().unwrap();
            let k = proposal(
                execution.wait(),
                &store.registration().expected_facts().scope,
            );
            let (ready, weak) = ready(execution.wait());
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let cancellation = AgentCancellation::new();
            let inputs = OwnedEffectInputsV8 {
                runtime,
                execution,
                proposal: k,
                store,
                policy: &policy,
                cancellation: &cancellation,
                turn: 0,
                attempt: 0,
            };
            let (basis, _) = checked_basis_facts(&inputs, ready.effect_facts().unwrap()).unwrap();
            let auth = OwnedEffectAuthorizationAckV8 {
                basis,
                staged: 20,
                ready: 21,
                consumed: 22,
            };
            let prepared = prepare_owned_effect_v8(inputs, ready, auth, |_| true)
                .unwrap_or_else(|_| panic!("trusted test authorization"));
            let row = prepared.live_intent_row().unwrap();
            let SourceJournalEntry::EffectIntent {
                operation,
                request_digest,
                ..
            } = row
            else {
                panic!("Intent");
            };
            let mut intent = OwnedEffectIntentAckV8 {
                basis: prepared.basis.clone(),
                authorization: 22,
                intent: 23,
                request: request_digest,
                operation,
            };
            match wrong {
                1 => intent.authorization = 21,
                2 => intent.intent = 22,
                3 => intent.request = "sha256:".to_owned() + &"0".repeat(64),
                4 => intent.operation = "substituted-operation".into(),
                5 => intent.basis.grant = intent.basis.target_grant.clone(),
                _ => (),
            }
            match activate_ack_owned_effect_v8(prepared, intent) {
                Ok(activated) => {
                    assert_eq!(wrong, 0);
                    assert_eq!(activated.staged.intent, 23);
                    assert!(activated.staged.dispatch.is_none());
                    assert!(activated.staged.accepted.is_none());
                    assert!(activated.staged.failure.is_none());
                    assert!(!activated.staged.cleanup_started);
                    assert!(!activated.staged.authority_lost);
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    drop(activated);
                }
                Err(rejected) => {
                    assert_ne!(wrong, 0);
                    assert!(rejected
                        .diagnostic
                        .message
                        .contains("effect intent ACK differs"));
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    drop(rejected);
                }
            }
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
