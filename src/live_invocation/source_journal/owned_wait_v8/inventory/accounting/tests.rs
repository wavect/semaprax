//! Actual Context/E/B, pinned lease, authenticated prefix and a real target
//! exchange. This gate makes no two-turn source or live dispatch-ACK claim.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    checked_owned_effect_request_v8, test_effect_exchange, OwnedEffectSettlementInputsV8,
};
use crate::agent_lifecycle::authorization::target_protocol::TargetEvidence;
use crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8;

fn encode(
    context: &CheckedOwnedWaitJournalContextV8,
    key: &SourceCheckpointKey,
    rows: &[EntryV8],
) -> Vec<u8> {
    let mut document = Vec::new();
    let mut prev = "0".repeat(64);
    for (seq, row) in rows.iter().enumerate() {
        let bytes = wire::encode(
            row,
            &ExpectedRowV8 {
                invocation: context.ordinary().invocation(),
                generation: context.generation(),
                seq: seq as u32,
                prev_mac: &prev,
                ordinary: context.ordinary(),
            },
            key,
        )
        .unwrap();
        prev = wire::parse(&bytes[..bytes.len() - 1]).unwrap()["authentication"]
            .as_str()
            .unwrap()
            .into();
        document.extend(bytes);
    }
    document
}
#[test]
fn owned_wait_accounting_authenticated_first_exchange_binds_exact_context_generation_and_prefix() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let (base, _) = context.test_ready_documents(&key);
        let mut rows = wire::decode_inventory(
            &base,
            &ExpectedRowV8 {
                invocation: context.ordinary().invocation(),
                generation: context.generation(),
                seq: 0,
                prev_mac: &"0".repeat(64),
                ordinary: context.ordinary(),
            },
            &key,
        )
        .unwrap();
        let EntryV8::Owned(OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) = &rows[15]
        else {
            panic!()
        };
        let state = state.clone();
        let EntryV8::Owned(OwnedBodyV8::OwnedAuthorizationStaged { decision, .. }) = &rows[17]
        else {
            panic!()
        };
        let decision = decision.clone();
        let EntryV8::Ordinary(Ordinary::AttemptSettled { response, .. }) = &rows[9] else {
            panic!()
        };
        let (runtime, execution) = context.test_runtime_execution();
        let scope = &context.registration().expected_facts().scope;
        let decoded = execution
            .wait()
            .lifecycle()
            .proposal_schema()
            .decode(std::str::from_utf8(response).unwrap())
            .unwrap();
        let proposal = v2::bind_owned_wait_proposal_v8(execution.wait(), scope, &decoded).unwrap();
        let inputs = OwnedEffectSettlementInputsV8 {
            runtime,
            execution,
            scope,
            turn: 0,
            attempt: 0,
            state: &state,
            decision: &decision,
            proposal: &proposal,
        };
        let request = checked_owned_effect_request_v8(&inputs).unwrap();
        rows.push(EntryV8::Ordinary(Ordinary::EffectIntent {
            turn: 0,
            attempt: 0,
            operation: request.operation().operation_id().into(),
            request_digest: request.request_digest(),
        }));
        let (ordinary, evidence, result) = test_effect_exchange(&inputs);
        let target = TargetEvidence::decode(&evidence).unwrap();
        rows.push(EntryV8::Ordinary(ordinary));
        rows.push(EntryV8::Owned(OwnedBodyV8::OwnedEffectSettlementRecorded {
            turn: 0,
            attempt: 0,
            intent: 20,
            settlement: 21,
            evidence: crate::live_invocation::identity::hex(&evidence),
            evidence_digest: target.digest().into(),
            result_wire: result.as_deref().map(crate::live_invocation::identity::hex),
        }));
        let bytes = encode(&context, &key, &rows);
        let checked = super::super::checked_inventory_v8(&context, &lease, &key, &bytes).unwrap();
        let proof = checked
            .accounting
            .as_ref()
            .expect("authenticated history proof");
        assert!(proof.matches(&context, bytes.len(), 23, &checked.last_mac));
        assert_eq!(*proof.previous().unwrap().total(), target.accounting());
        assert_eq!(proof.exchanges[0].0, 20);
        assert_eq!(proof.exchanges[0].1, 21);
        assert_eq!(proof.exchanges[0].2, 22);
        assert!(!proof.matches(&context, bytes.len(), 23, &"f".repeat(64)));
        assert!(!proof.matches(
            &context,
            base.len(),
            20,
            &super::super::document_mac(&base).unwrap()
        ));
        let zero = super::super::checked_inventory_v8(&context, &lease, &key, &base).unwrap();
        let zero = zero.accounting.as_ref().unwrap();
        assert!(zero.previous().is_none());
        assert!(zero.matches(
            &context,
            base.len(),
            20,
            &super::super::document_mac(&base).unwrap()
        ));
        // Pure checked rows are deliberately unable to export this provenance.
        assert!(super::super::check_entries_with_runtime(
            context.fold(),
            &key,
            rows.clone(),
            context.ready_runtime(),
            None
        )
        .unwrap()
        .accounting
        .is_none());
        for mode in 0..5 {
            let mut bad = rows.clone();
            match mode {
                0 => bad.swap(20, 21),
                1 => {
                    bad.remove(21);
                }
                2 => {
                    let EntryV8::Owned(OwnedBodyV8::OwnedEffectSettlementRecorded {
                        intent, ..
                    }) = &mut bad[22]
                    else {
                        panic!()
                    };
                    *intent = 19;
                }
                3 => bad.push(bad[22].clone()),
                _ => {
                    let EntryV8::Owned(OwnedBodyV8::OwnedRunCreated { scope, .. }) = &mut bad[0]
                    else {
                        panic!()
                    };
                    scope["policy_epoch"] = serde_json::json!(999);
                }
            }
            assert!(super::super::checked_inventory_v8(
                &context,
                &lease,
                &key,
                &encode(&context, &key, &bad)
            )
            .is_err());
        }
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
            |other, other_lease, other_key| {
                assert_ne!(context.generation(), other.generation());
                assert!(!proof.matches(&other, bytes.len(), 23, &checked.last_mac));
                assert!(
                    super::super::checked_inventory_v8(&other, &other_lease, &other_key, &bytes)
                        .is_err(),
                    "other registration/context cannot authenticate this history"
                );
            },
        );
    });
}
