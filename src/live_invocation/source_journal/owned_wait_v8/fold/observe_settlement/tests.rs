//! Genuine initializer/Context; Observe rows remain inert proof data here.
//! No Observe evaluation, physical successor or terminal authority is claimed.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::{
    append::SourceOwnedWaitJournalV8, checked_context::CheckedOwnedWaitJournalContextV8,
};
use serde_json::json;
use std::sync::Arc;

fn with_prefix(callback: impl FnOnce(&FoldContextV8, &dyn Fn() -> FoldV8, Body, u32)) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        use crate::interpreter::resumable::owned_frame::{
            OwnedFrameInput, OwnedFrameInputField, OwnedFrameInputValue,
        };
        let (runtime, execution) = context.ready_runtime().unwrap();
        let task = runtime.owned_wait_task_v8(execution).unwrap();
        let metadata = execution.wait().lifecycle().owned_wait_task_v8();
        let input = OwnedFrameInput {
            declaration: metadata.id.clone(),
            fields: metadata
                .fields()
                .map(|(id, _)| OwnedFrameInputField {
                    identity: id.clone(),
                    value: if id == metadata.objective_field {
                        OwnedFrameInputValue::Bytes(task.objective.clone())
                    } else {
                        OwnedFrameInputValue::Scalar(crate::interpreter::ArgumentValue::Int(
                            task.budget,
                        ))
                    },
                })
                .collect(),
        };
        let context = context.with_cumulative_initialization(&lease).unwrap();
        let seed =
            crate::live_invocation::source_journal::owned_wait_v8::inventory::tests::rows_for(
                context.fold(),
                &key,
            );
        let observation = seed
            .iter()
            .find_map(|row| match row {
                EntryV8::Owned(Body::OwnedWaitCreated { copy_arguments, .. }) => {
                    Some(copy_arguments[0]["value"].clone())
                }
                _ => None,
            })
            .unwrap();
        let channel =
            crate::interpreter::resumable::checkpoint::channel_from_json(&observation).unwrap();
        let scope = &context.registration().expected_facts().scope;
        let checked =
            v2::bind_owned_wait_observation_v8(&context.fold().checked_binding, scope, &channel)
                .unwrap();
        let observation_digest = checked.ordinary_digest().to_owned();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let _actual = match crate::live_invocation::source_journal::owned_wait_v8::live_upstream::initialize_live_actor_v8(
            &journal, input, &cancel,
        ) {
            Ok(Ok(actual)) => actual,
            _ => panic!("actual initializer profile"),
        };
        let session = journal.begin_session().unwrap();
        let reservation = session.sequence() as u32;
        let context = journal.context().fold();
        let make = || {
            let mut folded = session.fold_for_live_test();
            super::super::ordinary(
                context,
                &mut folded,
                &SourceJournalEntry::StageReservation {
                    turn: 0,
                    attempt: None,
                    role: SourceStageRole::Observe,
                    fuel: context.ordinary.max_steps_per_stage().unwrap(),
                },
                reservation,
            )
            .unwrap();
            folded
        };
        let state_digest = make().state_digest.unwrap();
        callback(
            context,
            &make,
            Body::OwnedObserveSettled {
                turn: 0,
                reservation,
                state_digest,
                consumed: 7,
                settlement: ObserveSettlementV8::Observed {
                    observation,
                    observation_digest,
                },
            },
            reservation + 1,
        );
    });
}

#[test]
fn owned_cumulative_observe_settlement_records_once_and_requires_immediate_matching_observed() {
    with_prefix(|context, make, body, sequence| {
        let mut folded = make();
        let (r, s, before) = (
            folded.reserved_total,
            folded.stages,
            folded.consumed_recorded,
        );
        assert!(settle(context, &mut folded, &body, sequence).unwrap());
        assert_eq!(
            (
                folded.reserved_total,
                folded.stages,
                folded.consumed_recorded
            ),
            (r, s, before + 7)
        );
        assert!(settle(context, &mut folded, &body, sequence + 1).is_err());
        let settled = folded.observe_settlement.as_ref().unwrap();
        let state = settled.ordinary_state.clone();
        let observation = settled.observation.clone().unwrap();
        assert!(validate_observed(context, &folded, sequence + 2, &state, &observation).is_err());
        assert!(
            validate_observed(context, &folded, sequence + 1, &state, &"f".repeat(64)).is_err()
        );
        assert!(validate_observed(context, &make(), sequence, &state, &observation).is_err());
        super::super::ordinary(
            context,
            &mut folded,
            &SourceJournalEntry::TurnObserved {
                turn: 0,
                state,
                observation,
                feedback: wire::record_argument_digest(&json!(null)),
            },
            sequence + 1,
        )
        .unwrap();
        assert_eq!(folded.tail, TailV8::Observed);
        let row = ValidatedEntryV8 {
            entry: EntryV8::Owned(body),
            observation: None,
        };
        assert!(
            super::super::validate_producer_transition(&make(), &row).is_err(),
            "data cannot mint a physical ACK"
        );
    });
}

#[test]
fn owned_cumulative_observe_settlement_rejects_stale_or_overspent_rows_atomically() {
    with_prefix(|context, make, body, sequence| {
        for mode in 0..6 {
            let mut wrong = body.clone();
            let Body::OwnedObserveSettled {
                turn,
                reservation,
                state_digest,
                consumed,
                settlement,
            } = &mut wrong
            else {
                panic!()
            };
            match mode {
                0 => *turn = 1,
                1 => *reservation += 1,
                2 => *state_digest = format!("sha256:{}", "f".repeat(64)),
                3 => *consumed = context.ordinary.max_steps_per_stage().unwrap() as u64 + 1,
                4 => {
                    let ObserveSettlementV8::Observed {
                        observation_digest, ..
                    } = settlement
                    else {
                        panic!()
                    };
                    *observation_digest = format!("sha256:{}", "f".repeat(64));
                }
                _ => {
                    *settlement = ObserveSettlementV8::Failed {
                        status: json!({"failure":"handler_failed","language_status":null}),
                    }
                }
            }
            let mut folded = make();
            let totals = (
                folded.reserved_total,
                folded.stages,
                folded.consumed_recorded,
            );
            assert!(
                settle(context, &mut folded, &wrong, sequence).is_err(),
                "mode {mode}"
            );
            assert_eq!(folded.tail, TailV8::ObserveReserved);
            assert_eq!(
                (
                    folded.reserved_total,
                    folded.stages,
                    folded.consumed_recorded
                ),
                totals
            );
            assert!(folded.observe_settlement.is_none());
        }
    });
}

#[test]
fn owned_cumulative_observe_failed_consumption_retains_cause_and_requires_successful_state_receipt()
{
    with_prefix(|context, make, body, sequence| {
        for failure in [
            "fuel_exhausted",
            "call_depth_exceeded",
            "evaluation_rejected",
        ] {
            let mut body = body.clone();
            let Body::OwnedObserveSettled { settlement, .. } = &mut body else {
                panic!()
            };
            let status = json!({"failure":failure,"language_status":null});
            *settlement = ObserveSettlementV8::Failed {
                status: status.clone(),
            };
            let mut folded = make();
            let before = folded.consumed_recorded;
            settle(context, &mut folded, &body, sequence).unwrap();
            assert_eq!(folded.tail, TailV8::FailedState);
            assert_eq!(folded.cleanup_terminal.as_ref(), Some(&status));
            assert_eq!(folded.consumed_recorded, before + 7);
            let (stop, reason) = if failure == "evaluation_rejected" {
                (
                    crate::live_invocation::source_journal::SourceStopStatus::Rejected,
                    crate::live_invocation::source_journal::SourceStopReason::StageRefused,
                )
            } else {
                (
                    crate::live_invocation::source_journal::SourceStopStatus::BudgetExhausted,
                    crate::live_invocation::source_journal::SourceStopReason::BudgetExhausted,
                )
            };
            assert!(
                validate_stop(&folded, stop, reason).is_err(),
                "no State receipt"
            );
            folded.tail = TailV8::MetadataOnly;
            folded.state_basis = None;
            folded.cleanup = Some(super::super::Cleanup {
                seq: sequence + 1,
                owner: OwnerV8::State,
                basis: sequence - 1,
                operations: json!([]),
                settled: true,
                host_confirmed: false,
                completed: false,
            });
            assert!(
                validate_stop(&folded, stop, reason).is_err(),
                "failed receipt cannot enter Stop"
            );
            folded.cleanup.as_mut().unwrap().completed = true;
            validate_stop(&folded, stop, reason).unwrap();
            assert!(validate_stop(
                &folded,
                crate::live_invocation::source_journal::SourceStopStatus::EffectFailed,
                crate::live_invocation::source_journal::SourceStopReason::EffectFailed
            )
            .is_err());
        }
    });
}
