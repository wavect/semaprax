//! Inert authenticated history over genuine E/B, lease and target exchange.
//! No Reduce evaluation, physical cleanup, result claim or public delivery proof.
use super::super::super::{
    candidate, fold, inventory, reduce_inventory, reduce_model, reduce_wire,
};
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    owned_wait_v8::settlement::{
        checked_owned_effect_request_v8, test_effect_exchange, test_failed_effect_exchange,
        OwnedEffectSettlementInputsV8,
    },
    TargetEvidence,
};
use crate::resumable_effects::owned_frame::v2;
use model::OwnedBodyV8 as B;
use serde_json::{json, Value};
fn seq(rows: &[EntryV8]) -> u32 {
    u32::try_from(rows.len()).unwrap()
}
fn observed(operations: &Value) -> Value {
    json!({"kind":"observed","settlement":"completed","operations":operations.as_array().unwrap().iter()
        .map(|operation|json!({"operation":operation,"outcome":"completed"})).collect::<Vec<_>>()})
}
fn effect_history(
    c: &CheckedOwnedWaitJournalContextV8,
    key: &SourceCheckpointKey,
    failed: bool,
) -> (Vec<EntryV8>, Value) {
    let (base, _) = c.test_ready_documents(key);
    let mut rows = wire::decode_inventory(
        &base,
        &ExpectedRowV8 {
            invocation: c.ordinary().invocation(),
            generation: c.generation(),
            seq: 0,
            prev_mac: &"0".repeat(64),
            ordinary: c.ordinary(),
        },
        key,
    )
    .unwrap();
    let (state, staged, decision, decision_digest) = rows
        .iter()
        .enumerate()
        .find_map(|(i, e)| match e {
            EntryV8::Owned(B::OwnedAuthorizationStaged {
                decision,
                decision_digest,
                ..
            }) => {
                let state = rows
                    .iter()
                    .rev()
                    .find_map(|e| match e {
                        EntryV8::Owned(B::OwnedStateTransferCompleted { state, .. }) => {
                            Some(state.clone())
                        }
                        _ => None,
                    })
                    .unwrap();
                Some((
                    state,
                    u32::try_from(i).unwrap(),
                    decision.clone(),
                    decision_digest.clone(),
                ))
            }
            _ => None,
        })
        .unwrap();
    let ready = rows
        .iter()
        .position(|e| matches!(e, EntryV8::Owned(B::OwnedAuthorizationReady { .. })))
        .unwrap() as u32;
    let consumed = rows
        .iter()
        .position(|e| {
            matches!(
                e,
                EntryV8::Ordinary(SourceJournalEntry::AuthorizationConsumed { .. })
            )
        })
        .unwrap() as u32;
    let response = rows
        .iter()
        .find_map(|e| match e {
            EntryV8::Ordinary(SourceJournalEntry::AttemptSettled { response, .. }) => {
                Some(response)
            }
            _ => None,
        })
        .unwrap();
    let (runtime, execution) = c.test_runtime_execution();
    let scope = &c.registration().expected_facts().scope;
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
    let intent = seq(&rows);
    rows.push(EntryV8::Ordinary(SourceJournalEntry::EffectIntent {
        turn: 0,
        attempt: 0,
        operation: request.operation().operation_id().into(),
        request_digest: request.request_digest(),
    }));
    let (settlement_row, evidence, result_wire) = if failed {
        test_failed_effect_exchange(&inputs)
    } else {
        test_effect_exchange(&inputs)
    };
    let settlement = seq(&rows);
    rows.push(EntryV8::Ordinary(settlement_row));
    let recorded = seq(&rows);
    rows.push(EntryV8::Owned(B::OwnedEffectSettlementRecorded {
        turn: 0,
        attempt: 0,
        intent,
        settlement,
        evidence: crate::live_invocation::identity::hex(&evidence),
        evidence_digest: TargetEvidence::decode(&evidence).unwrap().digest().into(),
        result_wire: result_wire
            .as_deref()
            .map(crate::live_invocation::identity::hex),
    }));
    let actions = execution
        .wait()
        .authorize()
        .disposal()
        .iter()
        .filter(|a| {
            a.active_case
                .as_ref()
                .is_some_and(|x| x.case == *execution.wait().authorize().granted())
        })
        .cloned()
        .collect::<Vec<_>>();
    let operations = v2::owned_wait_operations_v8(&actions).unwrap();
    let operations_digest=wire::recipe_digest(wire::RecipeV8::EffectDecisionOperations,&json!({"turn":0,"attempt":0,"staged":staged,"ready":ready,"consumed":consumed,
        "intent":intent,"settlement":settlement,"recorded":recorded,"decision_digest":decision_digest,"operations":operations})).unwrap();
    let started = seq(&rows);
    rows.push(EntryV8::Owned(B::OwnedEffectDecisionCleanupStarted {
        turn: 0,
        attempt: 0,
        staged,
        ready,
        consumed,
        intent,
        settlement,
        recorded,
        decision_digest,
        operations: operations.clone(),
        operations_digest,
    }));
    let receipt = observed(&operations);
    rows.push(EntryV8::Owned(B::OwnedEffectDecisionCleanupSettled {
        turn: 0,
        attempt: 0,
        started,
        receipt_digest: wire::recipe_digest(wire::RecipeV8::Receipt, &receipt).unwrap(),
        receipt,
    }));
    (rows, state)
}
fn scope(c: &CheckedOwnedWaitJournalContextV8) -> &Value {
    let B::OwnedRunCreated { scope, .. } = &c.fold().created else {
        panic!()
    };
    scope
}
fn success(
    c: &CheckedOwnedWaitJournalContextV8,
    key: &SourceCheckpointKey,
) -> (Vec<EntryV8>, usize) {
    let (mut rows, state) = effect_history(c, key, false);
    let start = rows.len();
    let cleanup = seq(&rows) - 1;
    let plan = v2::compile_owned_reduce_v2(c.execution.wait()).unwrap();
    let reservation = seq(&rows);
    rows.push(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: Some(0),
        role: super::super::super::super::SourceStageRole::Reduce,
        fuel: c.ordinary().max_steps_per_stage().unwrap(),
    }));
    let mapping = plan
        .mappings()
        .iter()
        .find(|m| m.role == "Continue")
        .unwrap();
    let fields = plan
        .helper()
        .program()
        .declarations
        .case_fields(&mapping.case)
        .unwrap()
        .iter()
        .map(|field| {
            let (_, destination) = mapping
                .fields
                .iter()
                .find(|(source, _)| *source == field.id)
                .unwrap();
            let value = state["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["identity"] == destination.as_str())
                .unwrap()["value"]
                .clone();
            json!({"identity":field.id.as_str(),"value":value})
        })
        .collect::<Vec<_>>();
    let step = json!({"declaration":plan.function().return_type.nominal_id().unwrap().as_str(),"case":mapping.case.as_str(),"fields":fields});
    let digest=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Step,&json!({"scope":scope(c),"binding":plan.binding(),"plan":plan.binding(),"turn":0,"attempt":0,"stage_reservation":reservation,"step":step})).unwrap();
    let checked =
        reduce_inventory::checked_step(&plan, scope(c), 0, 0, reservation, &step, &digest).unwrap();
    let staged = seq(&rows);
    rows.push(EntryV8::Owned(B::OwnedReduceStaged {
        turn: 0,
        attempt: 0,
        plan: plan.binding().into(),
        stage_reservation: reservation,
        effect_cleanup_settled: cleanup,
        step,
        step_digest: digest,
        consumed: 1,
    }));
    let constructor = plan
        .transfers()
        .cases
        .iter()
        .find(|x| x.case == mapping.case)
        .unwrap();
    let basis = reduce_model::ReduceBasisV8::Success {
        staged,
        constructor: constructor.constructor.as_str().into(),
        case: mapping.case.as_str().into(),
        active_flags: constructor
            .completion_live_flags
            .iter()
            .map(|f| f.0)
            .collect(),
    };
    let operations = v2::owned_wait_operations_v8(&plan.transfers().completion_cleanup).unwrap();
    let facts = v2::validate_owned_reduce_cleanup_v8(
        &plan,
        &serde_json::to_value(&basis).unwrap(),
        &operations,
    )
    .unwrap();
    let cleanup = if facts.active_operations().as_array().unwrap().is_empty() {
        reduce_model::ReduceCleanupV8::CompilerEmpty {}
    } else {
        let basis_digest=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Basis,&json!({"scope":scope(c),"binding":plan.binding(),"plan":plan.binding(),"turn":0,"attempt":0,"stage_reservation":reservation,"basis":serde_json::to_value(&basis).unwrap()})).unwrap();
        let started = seq(&rows);
        rows.push(EntryV8::Owned(B::OwnedReduceCleanupStarted {
            turn: 0,
            attempt: 0,
            plan: plan.binding().into(),
            stage_reservation: reservation,
            effect_cleanup_settled: cleanup,
            basis,
            basis_digest,
            consumed: 1,
            operations,
        }));
        let settled = seq(&rows);
        rows.push(EntryV8::Owned(B::OwnedReduceCleanupSettled {
            turn: 0,
            attempt: 0,
            started,
            receipt: observed(facts.active_operations()),
        }));
        reduce_model::ReduceCleanupV8::Observed { started, settled }
    };
    let reserved = seq(&rows);
    rows.push(EntryV8::Owned(B::OwnedStepTransferReserved {
        turn: 0,
        attempt: 0,
        plan: plan.binding().into(),
        stage_reservation: reservation,
        staged,
        cleanup,
        case: mapping.case.as_str().into(),
    }));
    rows.push(EntryV8::Owned(B::OwnedStepTransferCompleted {
        turn: 0,
        attempt: 0,
        reserved,
        target: serde_json::from_value(checked.target().clone()).unwrap(),
        transfer_digest: checked
            .transfer_digest(scope(c), &plan, 0, 0, reserved)
            .unwrap(),
    }));
    rows.push(EntryV8::Ordinary(SourceJournalEntry::Transition {
        turn: 0,
        attempt: 0,
        case: super::super::super::super::SourceTransitionCase::Continue,
        carrier_digest: crate::live_invocation::identity::digest(
            b"semaprax.agent-step.value.v2\0",
            &checked.ordinary_carrier_bytes(&plan).unwrap(),
        ),
    }));
    (rows, start)
}
fn remint(rows: &mut [EntryV8], index: usize, edit: impl FnOnce(&mut Value)) {
    let EntryV8::Owned(body) = &rows[index] else {
        panic!()
    };
    let mut value = serde_json::to_value(body).unwrap();
    edit(&mut value);
    rows[index] = EntryV8::Owned(serde_json::from_value(value).unwrap());
}
#[test]
fn owned_reduce_authenticated_history_joins_exact_refs_and_rejects_reminted_substitution() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, mut lease, key| {
        let (rows, start) = success(&c, &key);
        let check = |rows: &[EntryV8]| {
            inventory::checked_inventory_v8(&c, &lease, &key, &encode(&c, &key, rows))
        };
        let accepted = check(&rows).unwrap();
        assert_eq!(
            fold::fold(c.fold(), accepted.entries()).unwrap().tail,
            fold::TailV8::Reduce
        );
        for (kind, field) in [
            ("owned_reduce_staged", "plan"),
            ("owned_reduce_staged", "stage_reservation"),
            ("owned_reduce_staged", "effect_cleanup_settled"),
            ("owned_reduce_staged", "consumed"),
            ("owned_reduce_cleanup_started", "basis"),
            ("owned_reduce_cleanup_settled", "receipt"),
            ("owned_step_transfer_reserved", "staged"),
            ("owned_step_transfer_completed", "target"),
        ] {
            let index=rows.iter().position(|e|matches!(e,EntryV8::Owned(b) if serde_json::to_value(b).unwrap()["kind"]==kind)).unwrap();
            let mut bad = rows.clone();
            remint(&mut bad, index, |v| {
                match field {
                    "plan" => v[field] = format!("sha256:{}", "f".repeat(64)).into(),
                    "basis" => v[field]["constructor"] = "foreign.expression".into(),
                    "receipt" => v[field]["operations"] = json!([]),
                    "target" => {
                        v[field]["state"]["fields"][0]["value"]["hex"] = format!(
                            "{}ff",
                            v[field]["state"]["fields"][0]["value"]["hex"]
                                .as_str()
                                .unwrap()
                        )
                        .into()
                    }
                    "consumed" => v[field] = u64::MAX.into(),
                    _ => v[field] = 0.into(),
                }
                // Recompute public commitments as well as the row HMAC, so causal
                // compiler provenance/equality is exercised beyond a stale hash.
                if kind == "owned_reduce_staged" {
                    v["step_digest"]=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Step,&json!({"scope":scope(&c),"binding":c.execution.wait().binding(),"plan":v["plan"],"turn":v["turn"],"attempt":v["attempt"],"stage_reservation":v["stage_reservation"],"step":v["step"]})).unwrap().into();
                } else if kind == "owned_reduce_cleanup_started" {
                    v["basis_digest"]=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Basis,&json!({"scope":scope(&c),"binding":c.execution.wait().binding(),"plan":v["plan"],"turn":v["turn"],"attempt":v["attempt"],"stage_reservation":v["stage_reservation"],"basis":v["basis"]})).unwrap().into();
                } else if kind == "owned_step_transfer_completed" {
                    let p = v2::compile_owned_reduce_v2(c.execution.wait()).unwrap();
                    let m = p.mappings().iter().find(|m| m.role == "Continue").unwrap();
                    let mapping = m
                        .fields
                        .iter()
                        .map(|(a, b)| json!([a.as_str(), b.as_str()]))
                        .collect::<Vec<_>>();
                    v["transfer_digest"]=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Transfer,&json!({"scope":scope(&c),"binding":p.binding(),"plan":p.binding(),"turn":0,"attempt":0,"reserved":v["reserved"],"case":m.case.as_str(),"mapping":mapping,"target":v["target"]})).unwrap().into();
                }
            });
            assert!(check(&bad).is_err(), "reminted {kind}/{field}");
        }
        let mut wrong_scope = rows.clone();
        remint(&mut wrong_scope, start + 1, |v| {
            let mut foreign = scope(&c).clone();
            foreign["policy_epoch"] = "foreign".into();
            v["step_digest"]=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Step,&json!({"scope":foreign,"binding":c.execution.wait().binding(),"plan":c.execution.wait().binding(),"turn":0,"attempt":0,"stage_reservation":start,"step":v["step"]})).unwrap().into();
        });
        assert!(check(&wrong_scope).is_err());
        let mut wrong_transition = rows.clone();
        let EntryV8::Ordinary(SourceJournalEntry::Transition { carrier_digest, .. }) =
            wrong_transition.last_mut().unwrap()
        else {
            panic!()
        };
        *carrier_digest = format!("sha256:{}", "e".repeat(64));
        assert!(check(&wrong_transition).is_err());
        let physical_before = lease.read().unwrap();
        for end in start..rows.len() {
            let bytes = encode(&c, &key, &rows[..end]);
            let before = candidate::InventoryV8::recover(&c, &lease, &key, &bytes).unwrap();
            let rejected = before
                .prepare(&lease, rows[end].clone())
                .err()
                .expect("generic producer remains closed");
            assert!(!rejected.physical);
            assert_eq!(rejected.inventory.sequence(), end);
            assert_eq!(rejected.inventory.acknowledged_bytes(), bytes.len());
        }
        assert_eq!(lease.read().unwrap(), physical_before);
    });
}

#[test]
fn owned_reduce_authenticated_failed_cleanup_allows_only_the_selected_stop_after_observed_receipt()
{
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, mut lease, key| {
        let (mut rows, _) = effect_history(&c, &key, false);
        let effect_cleanup = seq(&rows) - 1;
        let p = v2::compile_owned_reduce_v2(c.execution.wait()).unwrap();
        let reservation = seq(&rows);
        rows.push(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: Some(0),
            role: super::super::super::super::SourceStageRole::Reduce,
            fuel: c.ordinary().max_steps_per_stage().unwrap(),
        }));
        let basis = reduce_model::ReduceBasisV8::InitialFailure {
            status: json!({"failure":"fuel_exhausted","language_status":null}),
        };
        let digest=reduce_wire::recipe_digest(reduce_wire::ReduceRecipeV8::Basis,&json!({"scope":scope(&c),"binding":p.binding(),"plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":reservation,"basis":serde_json::to_value(&basis).unwrap()})).unwrap();
        let operations = v2::owned_wait_operations_v8(&p.transfers().initial_disposal).unwrap();
        let facts = v2::validate_owned_reduce_cleanup_v8(
            &p,
            &serde_json::to_value(&basis).unwrap(),
            &operations,
        )
        .unwrap();
        let started = seq(&rows);
        rows.push(EntryV8::Owned(B::OwnedReduceCleanupStarted {
            turn: 0,
            attempt: 0,
            plan: p.binding().into(),
            stage_reservation: reservation,
            effect_cleanup_settled: effect_cleanup,
            basis,
            basis_digest: digest,
            consumed: 2,
            operations,
        }));
        let settled = seq(&rows);
        rows.push(EntryV8::Owned(B::OwnedReduceCleanupSettled {
            turn: 0,
            attempt: 0,
            started,
            receipt: observed(facts.active_operations()),
        }));
        let stop = EntryV8::Ordinary(SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: super::super::super::super::SourceStopStatus::BudgetExhausted,
            reason: super::super::super::super::SourceStopReason::BudgetExhausted,
        });
        let check = |rows: &[EntryV8]| {
            inventory::checked_inventory_v8(&c, &lease, &key, &encode(&c, &key, rows))
        };
        let mut early = rows[..settled as usize].to_vec();
        early.push(stop.clone());
        assert!(check(&early).is_err());
        rows.push(stop);
        assert!(check(&rows).is_ok());
        let mut bad = rows.clone();
        let EntryV8::Ordinary(SourceJournalEntry::Stop { status, reason, .. }) =
            bad.last_mut().unwrap()
        else {
            panic!()
        };
        *status = super::super::super::super::SourceStopStatus::Rejected;
        *reason = super::super::super::super::SourceStopReason::StageRefused;
        assert!(check(&bad).is_err());
        let mut failed = rows.clone();
        remint(&mut failed, settled as usize, |v| {
            v["receipt"]["operations"][0]["outcome"] = "failed".into();
            v["receipt"]["settlement"] = "failed".into();
        });
        assert!(check(&failed[..failed.len() - 1]).is_ok());
        assert!(
            check(&failed).is_err(),
            "failed observer quarantines, never publishes a Stop"
        );
        let mut reused = rows.clone();
        reused.push(rows[started as usize].clone());
        assert!(check(&reused).is_err());
        let prefix = encode(&c, &key, &rows[..rows.len() - 1]);
        let before = candidate::InventoryV8::recover(&c, &lease, &key, &prefix).unwrap();
        let physical = lease.read().unwrap();
        let rejected = before
            .prepare(&lease, rows.last().unwrap().clone())
            .err()
            .expect("generic failed Stop refused");
        assert!(!rejected.physical);
        assert_eq!(rejected.inventory.acknowledged_bytes(), prefix.len());
        assert_eq!(lease.read().unwrap(), physical);
    });
}

#[test]
fn owned_reduce_authenticated_failed_effect_keeps_selected_status_and_refuses_cleanup_or_stop_substitution(
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|c, mut lease, key| {
        let (mut rows, _) = effect_history(&c, &key, true);
        let prefix = rows.len();
        let (settlement, failure) = rows
            .iter()
            .enumerate()
            .find_map(|(i, e)| match e {
                EntryV8::Ordinary(SourceJournalEntry::EffectFailed { reason, .. }) => {
                    Some((i as u32, *reason))
                }
                _ => None,
            })
            .unwrap();
        let recorded = rows
            .iter()
            .position(|e| matches!(e, EntryV8::Owned(B::OwnedEffectSettlementRecorded { .. })))
            .unwrap() as u32;
        let decision_cleanup_settled = rows
            .iter()
            .position(|e| {
                matches!(
                    e,
                    EntryV8::Owned(B::OwnedEffectDecisionCleanupSettled { .. })
                )
            })
            .unwrap() as u32;
        let state_digest = rows
            .iter()
            .rev()
            .find_map(|e| match e {
                EntryV8::Owned(B::OwnedStateTransferCompleted { state_digest, .. }) => {
                    Some(state_digest.clone())
                }
                _ => None,
            })
            .unwrap();
        let operations =
            v2::owned_wait_operations_v8(&c.execution.wait().helper().liveness().result_disposal)
                .unwrap();
        let started = seq(&rows);
        rows.push(EntryV8::Owned(B::OwnedEffectFailureStateCleanupStarted {
            turn: 0,
            attempt: 0,
            plan: c.execution.wait().binding().into(),
            settlement,
            recorded,
            decision_cleanup_settled,
            effect_failure: failure.as_str().into(),
            state_digest,
            operations: operations.clone(),
        }));
        let settled = seq(&rows);
        rows.push(EntryV8::Owned(B::OwnedEffectFailureStateCleanupSettled {
            turn: 0,
            attempt: 0,
            started,
            receipt: observed(&operations),
        }));
        rows.push(EntryV8::Ordinary(SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: super::super::super::super::SourceStopStatus::EffectFailed,
            reason: super::super::super::super::SourceStopReason::EffectFailed,
        }));
        let check = |rows: &[EntryV8]| {
            inventory::checked_inventory_v8(&c, &lease, &key, &encode(&c, &key, rows))
        };
        assert!(check(&rows).is_ok());
        for field in [
            "plan",
            "settlement",
            "recorded",
            "decision_cleanup_settled",
            "effect_failure",
            "state_digest",
            "operations",
        ] {
            let mut bad = rows.clone();
            remint(&mut bad, started as usize, |v| match field {
                "plan" | "state_digest" => v[field] = format!("sha256:{}", "e".repeat(64)).into(),
                "operations" => v[field] = json!([]),
                "effect_failure" => v[field] = "result_limit".into(),
                _ => v[field] = 0.into(),
            });
            assert!(check(&bad).is_err(), "reminted failed State {field}");
        }
        let mut bad = rows.clone();
        remint(&mut bad, settled as usize, |v| {
            v["started"] = recorded.into()
        });
        assert!(check(&bad).is_err());
        let mut observer_failed = rows.clone();
        remint(&mut observer_failed, settled as usize, |v| {
            v["receipt"]["operations"][0]["outcome"] = "failed".into();
            v["receipt"]["settlement"] = "failed".into();
        });
        assert!(check(&observer_failed[..observer_failed.len() - 1]).is_ok());
        assert!(check(&observer_failed).is_err());
        let mut wrong_stop = rows.clone();
        let EntryV8::Ordinary(SourceJournalEntry::Stop { status, reason, .. }) =
            wrong_stop.last_mut().unwrap()
        else {
            panic!()
        };
        *status = super::super::super::super::SourceStopStatus::BudgetExhausted;
        *reason = super::super::super::super::SourceStopReason::BudgetExhausted;
        assert!(check(&wrong_stop).is_err());
        let physical = lease.read().unwrap();
        for end in prefix..rows.len() {
            let bytes = encode(&c, &key, &rows[..end]);
            let before = candidate::InventoryV8::recover(&c, &lease, &key, &bytes).unwrap();
            let rejected = before
                .prepare(&lease, rows[end].clone())
                .err()
                .expect("generic failed effect successor refused");
            assert!(!rejected.physical);
            assert_eq!(rejected.inventory.sequence(), end);
            assert_eq!(rejected.inventory.acknowledged_bytes(), bytes.len());
        }
        assert_eq!(lease.read().unwrap(), physical);
    });
}
