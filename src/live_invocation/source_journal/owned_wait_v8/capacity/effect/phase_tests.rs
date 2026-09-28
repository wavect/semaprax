//! Actual checked context and target exchange; this is inert journal data only.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    owned_wait_v8::settlement::{
        checked_owned_effect_request_v8, test_effect_exchange, OwnedEffectSettlementInputsV8,
    },
    TargetEvidence,
};
use crate::resumable_effects::owned_frame::v2;

fn encode(
    context: &CheckedOwnedWaitJournalContextV8,
    key: &SourceCheckpointKey,
    rows: &[EntryV8],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut previous = "0".repeat(64);
    for (seq, row) in rows.iter().enumerate() {
        let encoded = wire::encode(
            row,
            &ExpectedRowV8 {
                invocation: context.ordinary().invocation(),
                generation: context.generation(),
                seq: seq as u32,
                prev_mac: &previous,
                ordinary: context.ordinary(),
            },
            key,
        )
        .unwrap();
        previous = wire::parse(&encoded[..encoded.len() - 1]).unwrap()["authentication"]
            .as_str()
            .unwrap()
            .into();
        bytes.extend(encoded);
    }
    bytes
}

#[test]
fn effect_actual_authenticated_phase_edges_keep_reserved_room_and_refuse_producer() {
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
        let (runtime, execution) = context.test_runtime_execution().unwrap();
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
        rows.push(EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
            turn: 0,
            attempt: 0,
            operation: request.operation().operation_id().into(),
            request_digest: request.request_digest(),
        }));
        let (settlement, evidence, result_wire) = test_effect_exchange(&inputs);
        rows.push(EntryV8::Ordinary(settlement));
        rows.push(EntryV8::Owned(
            model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                turn: 0,
                attempt: 0,
                intent: 20,
                settlement: 21,
                evidence: crate::live_invocation::identity::hex(&evidence),
                evidence_digest: TargetEvidence::decode(&evidence).unwrap().digest().into(),
                result_wire: result_wire
                    .as_deref()
                    .map(crate::live_invocation::identity::hex),
            },
        ));
        let max = templates::maxima(context.fold()).unwrap();
        let operations = max.effect_operations;
        let operations_digest = wire::recipe_digest(wire::RecipeV8::EffectDecisionOperations, &json!({"turn":0,"attempt":0,"staged":17,"ready":18,"consumed":19,"intent":20,"settlement":21,"recorded":22,"decision_digest":decision_digest,"operations":operations})).unwrap();
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
        let receipt = templates::receipt(&operations).unwrap();
        rows.push(EntryV8::Owned(
            model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                turn: 0,
                attempt: 0,
                started: 23,
                receipt_digest: wire::recipe_digest(wire::RecipeV8::Receipt, &receipt).unwrap(),
                receipt,
            },
        ));
        let check = |count: usize| {
            let checked = inventory::checked_inventory_v8(
                &context,
                &lease,
                &key,
                &encode(&context, &key, &rows[..count]),
            )
            .unwrap();
            let folded = fold::fold(context.fold(), checked.entries()).unwrap();
            let room = super::super::outstanding(context.fold(), &folded).unwrap();
            (folded, room)
        };
        for count in 17..rows.len() {
            let (previous, before) = check(count);
            let (_, after) = check(count + 1);
            let old = encode(&context, &key, &rows[..count]);
            let next = encode(&context, &key, &rows[..count + 1]);
            let width = next.len() - old.len();
            check_edge(before, width, after);
            if count >= 20 {
                assert!(fold::validate_producer_transition(
                    &previous,
                    &ValidatedEntryV8 {
                        entry: rows[count].clone(),
                        observation: None
                    }
                )
                .is_err());
            }
        }
        let (released, room) = check(rows.len());
        assert_eq!(released.tail, fold::TailV8::EffectDecisionReleased);
        assert!(
            room.rows >= 4,
            "successful Decision release still preserves future State failure closure"
        );
        assert!(room.bytes > 0);
        // Failed observation cannot be replaced by an unrelated legacy terminal.
        let mut failed_receipt = rows.clone();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
            receipt,
            receipt_digest,
            ..
        }) = &mut failed_receipt[24]
        else {
            panic!()
        };
        receipt["operations"][0]["outcome"] = json!("failed");
        receipt["settlement"] = json!("failed");
        *receipt_digest = wire::recipe_digest(wire::RecipeV8::Receipt, receipt).unwrap();
        let checked = inventory::checked_inventory_v8(
            &context,
            &lease,
            &key,
            &encode(&context, &key, &failed_receipt),
        )
        .unwrap();
        let folded = fold::fold(context.fold(), checked.entries()).unwrap();
        assert_eq!(folded.tail, fold::TailV8::EffectCleanupFailed);
        assert_eq!(
            super::super::outstanding(context.fold(), &folded).unwrap(),
            room
        );
    });
}
