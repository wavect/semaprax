//! Actual runtime/lease plus real target exchange, but journal rows remain inert
//! data. These tests do not prove source authorize or physical cleanup ran.
use super::super::super::{fold, inventory};
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    owned_wait_v8::settlement::{
        checked_owned_effect_request_v8, test_effect_exchange, OwnedEffectSettlementInputsV8,
    },
    TargetEvidence,
};
use crate::resumable_effects::owned_frame::v2;
use serde_json::json;

#[test]
fn owned_wait_effect_inventory_replays_real_exchange_and_keeps_cleanup_data_inert() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, mut lease, key| {
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
        let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) =
            &rows[15]
        else {
            panic!()
        };
        let state = state.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged {
            decision,
            decision_digest,
            ..
        }) = &rows[17]
        else {
            panic!()
        };
        let decision = decision.clone();
        let decision_digest = decision_digest.clone();
        let EntryV8::Ordinary(SourceJournalEntry::AttemptSettled { response, .. }) = &rows[9]
        else {
            panic!()
        };
        let execution = &context.execution;
        let decoded = execution
            .wait()
            .lifecycle()
            .proposal_schema()
            .decode(std::str::from_utf8(response).unwrap())
            .unwrap();
        let scope = &context.registration().expected_facts().scope;
        let proposal = v2::bind_owned_wait_proposal_v8(execution.wait(), scope, &decoded).unwrap();
        let (runtime, execution) = context.test_runtime_execution().unwrap();
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
        rows.push(EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
            turn: 0,
            attempt: 0,
            operation: request.operation().operation_id().into(),
            request_digest: request.request_digest(),
        }));
        let (settlement, evidence, result_wire) = test_effect_exchange(&inputs);
        rows.push(EntryV8::Ordinary(settlement));
        let evidence_digest = TargetEvidence::decode(&evidence)
            .unwrap()
            .digest()
            .to_owned();
        rows.push(EntryV8::Owned(
            model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                turn: 0,
                attempt: 0,
                intent: 20,
                settlement: 21,
                evidence: crate::live_invocation::identity::hex(&evidence),
                evidence_digest,
                result_wire: result_wire
                    .as_deref()
                    .map(crate::live_invocation::identity::hex),
            },
        ));
        let authorize = execution.wait().authorize();
        let actions = authorize
            .disposal()
            .iter()
            .filter(|a| {
                a.active_case
                    .as_ref()
                    .is_some_and(|c| c.case == *authorize.granted())
            })
            .cloned()
            .collect::<Vec<_>>();
        let operations = v2::owned_wait_operations_v8(&actions).unwrap();
        let operations_digest = wire::recipe_digest(
            wire::RecipeV8::EffectDecisionOperations,
            &json!({"turn":0,"attempt":0,"staged":17,"ready":18,"consumed":19,
                "intent":20,"settlement":21,"recorded":22,"decision_digest":decision_digest,
                "operations":operations}),
        )
        .unwrap();
        rows.push(EntryV8::Owned(
            model::OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                turn: 0,
                attempt: 0,
                staged: 17,
                ready: 18,
                consumed: 19,
                intent: 20,
                settlement: 21,
                recorded: 22,
                decision_digest,
                operations: operations.clone(),
                operations_digest,
            },
        ));
        let receipt = json!({"kind":"observed","settlement":"completed",
            "operations":operations.as_array().unwrap().iter().map(|operation|
                json!({"operation":operation,"outcome":"completed"})).collect::<Vec<_>>()});
        rows.push(EntryV8::Owned(
            model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                turn: 0,
                attempt: 0,
                started: 23,
                receipt_digest: wire::recipe_digest(wire::RecipeV8::Receipt, &receipt).unwrap(),
                receipt,
            },
        ));
        let check = |rows: &[EntryV8]| {
            inventory::checked_inventory_v8(&context, &lease, &key, &encode(&context, &key, rows))
        };
        let checked = check(&rows).unwrap();
        assert_eq!(checked.entries().len(), 25);
        assert_eq!(
            fold::fold(context.fold(), checked.entries()).unwrap().tail,
            fold::TailV8::EffectDecisionReleased
        );
        // Even matching authenticated data cannot authorize generic production
        // extension, cleanup or owner restoration.
        for end in 20..25 {
            let before = check(&rows[..end]).unwrap();
            let previous = fold::fold(context.fold(), before.entries()).unwrap();
            assert!(fold::validate_producer_transition(
                &previous,
                &ValidatedEntryV8 {
                    entry: rows[end].clone(),
                    observation: None
                }
            )
            .is_err());
        }
        for variant in 0..10 {
            let mut bad = rows.clone();
            match variant {
                0 => {
                    if let EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
                        request_digest,
                        ..
                    }) = &mut bad[20]
                    {
                        *request_digest = format!("sha256:{}", "3".repeat(64));
                    }
                }
                1 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                        settlement,
                        ..
                    }) = &mut bad[22]
                    {
                        *settlement = 20;
                    }
                }
                2 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                        result_wire,
                        ..
                    }) = &mut bad[22]
                    {
                        *result_wire = None;
                    }
                }
                3 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                        evidence,
                        ..
                    }) = &mut bad[22]
                    {
                        *evidence = "00".into();
                    }
                }
                4 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                        evidence_digest,
                        ..
                    }) = &mut bad[22]
                    {
                        *evidence_digest = format!("sha256:{}", "4".repeat(64));
                    }
                }
                5 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                        ready,
                        ..
                    }) = &mut bad[23]
                    {
                        *ready = 17;
                    }
                }
                6 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                        consumed,
                        ..
                    }) = &mut bad[23]
                    {
                        *consumed = 18;
                    }
                }
                7 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                        operations,
                        ..
                    }) = &mut bad[23]
                    {
                        *operations = json!([]);
                    }
                }
                8 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                        started,
                        ..
                    }) = &mut bad[24]
                    {
                        *started = 22;
                    }
                }
                9 => {
                    if let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                        receipt,
                        receipt_digest,
                        ..
                    }) = &mut bad[24]
                    {
                        receipt["operations"] = json!([]);
                        *receipt_digest =
                            wire::recipe_digest(wire::RecipeV8::Receipt, receipt).unwrap();
                    }
                }
                _ => unreachable!(),
            }
            assert!(check(&bad).is_err(), "re-MACed variant {variant}");
        }
        let mut failed = rows.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
            receipt,
            receipt_digest,
            ..
        }) = &mut failed[24]
        else {
            panic!()
        };
        receipt["operations"][0]["outcome"] = json!("failed");
        receipt["settlement"] = json!("failed");
        *receipt_digest = wire::recipe_digest(wire::RecipeV8::Receipt, receipt).unwrap();
        let checked = check(&failed).unwrap();
        assert_eq!(
            fold::fold(context.fold(), checked.entries()).unwrap().tail,
            fold::TailV8::EffectCleanupFailed
        );
        assert!(
            lease.read().unwrap().is_empty(),
            "pure replay must not append physical bytes"
        );
    });
}
