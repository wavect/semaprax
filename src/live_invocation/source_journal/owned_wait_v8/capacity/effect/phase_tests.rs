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
        assert_eq!(
            room,
            RoomV8 {
                bytes: 275709,
                rows: 8
            },
            "this checked fixture retains its full successful Reduce closure"
        );
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
        check_observer_failure_closure(
            &context,
            &lease,
            &key,
            failed_receipt,
            &max.result_operations,
        );
    });
}

fn check_observer_failure_closure(
    context: &CheckedOwnedWaitJournalContextV8,
    lease: &crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8,
    key: &SourceCheckpointKey,
    mut rows: Vec<EntryV8>,
    operations: &Value,
) {
    use crate::live_invocation::source_journal::{SourceStopReason, SourceStopStatus};
    let terminal = RoomV8 {
        bytes: super::super::super::super::execution::TERMINAL_ROOM_BYTES,
        rows: 2,
    };
    // Spec sections 23.6/43: a failed Decision observer has its own State
    // Started + Settled + sticky terminal closure. The historical equality
    // with successful Reduce room predates that separate grammar.
    let max_started = super::tests::checked_width(
        context,
        json!({
            "kind":"owned_effect_observer_failure_state_cleanup_started",
            "turn":u32::MAX,"attempt":u32::MAX,"plan":hash(),
            "settlement":u32::MAX,"recorded":u32::MAX,
            "decision_cleanup_settled":u32::MAX,"decision_receipt_digest":hash(),
            "cause":"decision_observation_failed","selected_effect_failure":"handler_failed",
            "state_digest":hash(),"operations":operations
        }),
    );
    let max_settled = super::tests::checked_width(
        context,
        json!({
            "kind":"owned_effect_observer_failure_state_cleanup_settled",
            "turn":u32::MAX,"attempt":u32::MAX,"started":u32::MAX,
            "receipt":templates::receipt(operations).unwrap()
        }),
    );
    let expected = max_started.add(max_settled).unwrap().add(terminal).unwrap();
    assert_eq!(
        expected,
        RoomV8 {
            bytes: 268245,
            rows: 4
        }
    );
    let check = |rows: &[EntryV8]| {
        let checked =
            inventory::checked_inventory_v8(context, lease, key, &encode(context, key, rows))
                .unwrap();
        let folded = fold::fold(context.fold(), checked.entries()).unwrap();
        let room = super::super::outstanding(context.fold(), &folded).unwrap();
        (folded, room)
    };
    let (failed, before) = check(&rows);
    assert_eq!(before, expected);
    let used_rows = super::super::super::super::MAX_SOURCE_ENTRIES - expected.rows;
    expected.check(0, used_rows).unwrap();
    assert_eq!(
        expected.check(0, used_rows + 1),
        Err(SourceJournalError::Capacity)
    );
    let stop = EntryV8::Ordinary(SourceJournalEntry::Stop {
        turn: Some(0),
        attempt: Some(0),
        status: SourceStopStatus::Rejected,
        reason: SourceStopReason::StageRefused,
    });
    let refuse = |prefix: &[EntryV8], row: &EntryV8| {
        let mut candidate = prefix.to_vec();
        candidate.push(row.clone());
        assert_eq!(
            inventory::checked_inventory_v8(context, lease, key, &encode(context, key, &candidate))
                .err(),
            Some(SourceJournalError::Order)
        );
    };
    refuse(&rows, &stop);
    assert!(fold::validate_producer_transition(
        &failed,
        &ValidatedEntryV8 {
            entry: stop.clone(),
            observation: None,
        }
    )
    .is_err());
    let EntryV8::Owned(model::OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
        receipt_digest,
        ..
    }) = &rows[24]
    else {
        panic!()
    };
    let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) = &rows[15]
    else {
        panic!()
    };
    let started = EntryV8::Owned(
        model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupStarted {
            turn: 0,
            attempt: 0,
            plan: context.fold().checked_binding.binding().into(),
            settlement: 21,
            recorded: 22,
            decision_cleanup_settled: 24,
            decision_receipt_digest: receipt_digest.clone(),
            cause: "decision_observation_failed".into(),
            selected_effect_failure: None,
            state_digest: wire::record_argument_digest(state),
            operations: operations.clone(),
        },
    );
    assert!(fold::validate_producer_transition(
        &failed,
        &ValidatedEntryV8 {
            entry: started.clone(),
            observation: None,
        }
    )
    .is_err());
    let old_len = encode(context, key, &rows).len();
    rows.push(started);
    let (_, after_started) = check(&rows);
    assert_eq!(after_started, max_settled.add(terminal).unwrap());
    check_edge(
        before,
        encode(context, key, &rows).len() - old_len,
        after_started,
    );
    refuse(&rows, &stop);
    let state_receipt = templates::receipt(operations).unwrap();
    let settled = EntryV8::Owned(
        model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled {
            turn: 0,
            attempt: 0,
            started: 25,
            receipt: state_receipt.clone(),
        },
    );
    // A second failed observer quarantines without granting Stop or Reduce.
    let mut failed_state = rows.clone();
    let mut failed_state_receipt = state_receipt;
    failed_state_receipt["operations"][0]["outcome"] = json!("failed");
    failed_state_receipt["settlement"] = json!("failed");
    failed_state.push(EntryV8::Owned(
        model::OwnedBodyV8::OwnedEffectObserverFailureStateCleanupSettled {
            turn: 0,
            attempt: 0,
            started: 25,
            receipt: failed_state_receipt,
        },
    ));
    assert_eq!(check(&failed_state).1, RoomV8::default());
    refuse(&failed_state, &stop);
    let old_len = encode(context, key, &rows).len();
    rows.push(settled);
    let (_, after_settled) = check(&rows);
    assert_eq!(after_settled, terminal);
    check_edge(
        after_started,
        encode(context, key, &rows).len() - old_len,
        after_settled,
    );
    let wrong_stop = EntryV8::Ordinary(SourceJournalEntry::Stop {
        turn: Some(0),
        attempt: Some(0),
        status: SourceStopStatus::EffectFailed,
        reason: SourceStopReason::EffectFailed,
    });
    refuse(&rows, &wrong_stop);
    rows.push(stop);
    let (stopped, after_stop) = check(&rows);
    assert_eq!(stopped.tail, fold::TailV8::Stopped);
    assert_eq!(
        after_stop,
        RoomV8 {
            rows: 1,
            ..terminal
        }
    );
}
