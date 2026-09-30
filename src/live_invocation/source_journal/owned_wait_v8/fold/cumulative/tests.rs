//! Explicit profile admission; inert grammar does not manufacture owner authority.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::{
    append::SourceOwnedWaitJournalV8, checked_context::CheckedOwnedWaitJournalContextV8,
};
use std::sync::Arc;

fn row(entry: EntryV8) -> ValidatedEntryV8 {
    ValidatedEntryV8 {
        entry,
        observation: None,
    }
}

fn checked_continue(
    context: &FoldContextV8,
) -> (super::super::reduce::ReduceJournalV8, Value, u32) {
    use crate::live_invocation::source_journal::owned_wait_v8::{
        reduce_fold::ReduceFoldV8,
        reduce_inventory::checked_step,
        reduce_model::{ReduceBasisV8, ReduceCleanupV8},
        reduce_wire::{recipe_digest, ReduceRecipeV8},
    };
    let plan = crate::resumable_effects::owned_frame::v2::compile_owned_reduce_v2(
        &context.checked_binding,
    )
    .unwrap();
    let mapping = plan
        .mappings()
        .iter()
        .find(|mapping| mapping.role == "Continue")
        .unwrap();
    let fields = plan
        .helper()
        .program()
        .declarations
        .case_fields(&mapping.case)
        .unwrap();
    let step = serde_json::json!({"declaration":plan.function().return_type.nominal_id().unwrap().as_str(),
        "case":mapping.case.as_str(),"fields":fields.iter().map(|field| serde_json::json!({"identity":field.id.as_str(),
            "value":if field.ty == crate::hir::ResolvedType::Bytes { serde_json::json!({"kind":"bytes","hex":"00"}) }
            else { serde_json::json!({"tag":"i64","value":1}) }})).collect::<Vec<_>>()});
    let Body::OwnedRunCreated { scope, .. } = &context.created else {
        panic!()
    };
    let digest = recipe_digest(
        ReduceRecipeV8::Step,
        &serde_json::json!({"scope":scope,"binding":plan.binding(),
        "plan":plan.binding(),"turn":0,"attempt":0,"stage_reservation":28,"step":step}),
    )
    .unwrap();
    let checked = checked_step(&plan, scope, 0, 0, 28, &step, &digest).unwrap();
    let target = checked.target().clone();
    let mut reduced =
        ReduceFoldV8::after_checked_reservation(&plan, scope, 0, 0, 28, 100, 27).unwrap();
    reduced
        .staged(
            &plan,
            plan.binding(),
            scope,
            29,
            0,
            0,
            28,
            27,
            &step,
            &digest,
            7,
        )
        .unwrap();
    let constructor = plan
        .transfers()
        .cases
        .iter()
        .find(|case| case.case == mapping.case)
        .unwrap();
    let basis = ReduceBasisV8::Success {
        staged: 29,
        constructor: constructor.constructor.as_str().into(),
        case: constructor.case.as_str().into(),
        active_flags: constructor
            .completion_live_flags
            .iter()
            .map(|flag| flag.0)
            .collect(),
    };
    let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
        &plan.transfers().completion_cleanup,
    )
    .unwrap();
    let active = crate::resumable_effects::owned_frame::v2::validate_owned_reduce_cleanup_v8(
        &plan,
        &serde_json::to_value(&basis).unwrap(),
        &operations,
    )
    .unwrap();
    let (reserved, cleanup) = if active.active_operations().as_array().unwrap().is_empty() {
        (30, ReduceCleanupV8::CompilerEmpty {})
    } else {
        let basis_digest = recipe_digest(ReduceRecipeV8::Basis, &serde_json::json!({"scope":scope,"binding":plan.binding(),
            "plan":plan.binding(),"turn":0,"attempt":0,"stage_reservation":28,"basis":serde_json::to_value(&basis).unwrap()})).unwrap();
        reduced
            .cleanup_started(
                &plan,
                plan.binding(),
                scope,
                30,
                0,
                0,
                28,
                27,
                &basis,
                &basis_digest,
                7,
                &operations,
            )
            .unwrap();
        let receipt = serde_json::json!({"kind":"observed","settlement":"completed","operations":active.active_operations()
            .as_array().unwrap().iter().map(|operation| serde_json::json!({"operation":operation,"outcome":"completed"})).collect::<Vec<_>>()});
        reduced.cleanup_settled(31, 0, 0, 30, &receipt).unwrap();
        (
            32,
            ReduceCleanupV8::Observed {
                started: 30,
                settled: 31,
            },
        )
    };
    reduced
        .transfer_reserved(
            &plan,
            plan.binding(),
            reserved,
            0,
            0,
            28,
            29,
            &cleanup,
            mapping.case.as_str(),
        )
        .unwrap();
    let transfer_digest = checked
        .transfer_digest(scope, &plan, 0, 0, reserved)
        .unwrap();
    reduced
        .transfer_completed(
            &plan,
            scope,
            reserved + 1,
            0,
            0,
            reserved,
            &target,
            &transfer_digest,
        )
        .unwrap();
    let carrier = crate::live_invocation::identity::digest(
        b"semaprax.agent-step.value.v2\0",
        &checked.ordinary_carrier_bytes(&plan).unwrap(),
    );
    reduced
        .transition(
            &plan,
            reserved + 2,
            0,
            0,
            super::super::super::super::SourceTransitionCase::Continue,
            &carrier,
        )
        .unwrap();
    (
        super::super::reduce::ReduceJournalV8::inert_test(plan, reduced),
        target["state"].clone(),
        reserved + 3,
    )
}

#[test]
fn owned_cumulative_profile_continue_retires_turn_bases_without_resetting_totals() {
    let mut context = super::super::tests::context();
    context.cumulative_initialization = true;
    context.initialized_task = Some(serde_json::json!({"inert":true}));
    let (reduced, state, sequence) = checked_continue(&context);
    let mut folded = FoldV8::capacity_fresh_turn(0);
    folded.tail = TailV8::Reduce;
    folded.continuation_profile_selected = true;
    folded.reduce = Some(reduced);
    folded.reserved_total = 731;
    folded.consumed_recorded = 107;
    folded.stages = 4;
    folded.wait_fuel = 53;
    folded
        .stage_originals
        .push((28, SourceStageRole::Reduce, 100));
    folded.stage_current = Some((28, SourceStageRole::Reduce, 100));
    let body = Body::OwnedStateCommitted {
        turn: 1,
        argument_digest: wire::record_argument_digest(&state),
        state: state.clone(),
        cleanup_plan_digest: context.cleanup_plan_digest.clone(),
    };
    for mode in 0..6 {
        let mut bad = body.clone();
        let Body::OwnedStateCommitted {
            turn,
            state,
            argument_digest,
            cleanup_plan_digest,
        } = &mut bad
        else {
            panic!()
        };
        match mode {
            0 => *turn = 0,
            1 => *turn = context.ordinary.max_iterations(),
            2 => {
                state["fields"][0]["value"] = serde_json::json!({"kind":"bytes","hex":"ff"});
                *argument_digest = wire::record_argument_digest(state);
            }
            3 => *argument_digest = "0".repeat(64),
            4 => *cleanup_plan_digest = "0".repeat(64),
            _ => context.cumulative_initialization = false,
        }
        assert!(commit_next_state(&context, &mut folded, &bad, sequence).is_err());
        assert!(
            folded.reduce.is_some(),
            "failed joins retain the predecessor"
        );
        assert_eq!(folded.current_turn, 0);
        context.cumulative_initialization = true;
    }
    assert!(commit_next_state(&context, &mut folded, &body, sequence + 1).is_err());
    assert!(commit_next_state(&context, &mut folded, &body, sequence).unwrap());
    assert_eq!(
        (
            folded.reserved_total,
            folded.consumed_recorded,
            folded.stages,
            folded.wait_fuel
        ),
        (731, 107, 4, 53)
    );
    assert_eq!(
        (folded.current_turn, folded.state_basis, folded.tail),
        (1, Some(sequence), TailV8::CommittedState)
    );
    assert_eq!(folded.state.as_ref(), Some(&state));
    assert!(folded.stage_originals.is_empty() && folded.stage_current.is_none());
    assert!(folded.reduce.is_none() && folded.effect.is_none() && folded.wait.is_none());
    assert_eq!(remaining_turns(&context, &folded).unwrap(), Some(0));
}

#[test]
fn owned_cumulative_profile_forecasts_future_closures_without_changing_legacy_room() {
    let mut context = super::super::tests::context();
    let folded = FoldV8::capacity_fresh_turn(0);
    let legacy = super::super::super::capacity::outstanding(&context, &folded).unwrap();
    assert_eq!(remaining_turns(&context, &folded).unwrap(), None);
    context.cumulative_initialization = true;
    context.initialized_task = Some(serde_json::json!({"inert":true}));
    let cumulative = super::super::super::capacity::outstanding(&context, &folded).unwrap();
    assert!(cumulative.rows_for_inert_test() > legacy.rows_for_inert_test());
    assert!(cumulative.bytes_for_inert_test() > legacy.bytes_for_inert_test());
    let mut final_turn = FoldV8::capacity_fresh_turn(context.ordinary.max_iterations() - 1);
    let final_room = super::super::super::capacity::outstanding(&context, &final_turn).unwrap();
    assert!(final_room.rows_for_inert_test() < cumulative.rows_for_inert_test());
    final_turn.tail = TailV8::Terminal;
    let terminal = super::super::super::capacity::outstanding(&context, &final_turn).unwrap();
    assert_eq!(
        (
            terminal.rows_for_inert_test(),
            terminal.bytes_for_inert_test()
        ),
        (0, 0)
    );
}

#[test]
fn owned_cumulative_profile_requires_exact_version_context_and_position() {
    let mut context = super::super::tests::context();
    // This grammar-only context supplies no runtime, store, or physical owner.
    context.cumulative_initialization = true;
    context.initialized_task = Some(serde_json::json!({"inert":true}));
    let created = context.created.clone();
    let profile = profile_row(&context);
    let rows = [
        row(EntryV8::Owned(created.clone())),
        row(EntryV8::Owned(profile.clone())),
        row(EntryV8::Ordinary(SourceJournalEntry::RunOpened)),
    ];
    assert!(fold(&context, &rows)
        .unwrap()
        .continuation_profile_selected());
    for invalid in [
        vec![
            row(EntryV8::Owned(created.clone())),
            row(EntryV8::Ordinary(SourceJournalEntry::RunOpened)),
        ],
        vec![
            row(EntryV8::Owned(profile.clone())),
            row(EntryV8::Owned(created.clone())),
        ],
        vec![
            row(EntryV8::Owned(created.clone())),
            row(EntryV8::Owned(profile.clone())),
            row(EntryV8::Owned(profile.clone())),
        ],
        vec![
            row(EntryV8::Owned(created.clone())),
            row(EntryV8::Owned(Body::OwnedContinuationProfileSelected {
                profile: "unknown".into(),
                max_iterations: context.ordinary.max_iterations(),
            })),
        ],
        vec![
            row(EntryV8::Owned(created.clone())),
            row(EntryV8::Owned(Body::OwnedContinuationProfileSelected {
                profile: PROFILE_V1.into(),
                max_iterations: context.ordinary.max_iterations() + 1,
            })),
        ],
    ] {
        assert!(fold(&context, &invalid).is_err());
    }
    context.cumulative_initialization = false;
    assert!(
        fold(&context, &rows).is_err(),
        "rows cannot opt in the expected Context"
    );
    assert!(
        fold(
            &context,
            &[
                row(EntryV8::Owned(created)),
                row(EntryV8::Ordinary(SourceJournalEntry::RunOpened)),
            ]
        )
        .is_ok(),
        "frozen default initialization grammar remains admitted"
    );
}

fn actual_task(
    context: &CheckedOwnedWaitJournalContextV8,
) -> crate::interpreter::resumable::owned_frame::OwnedFrameInput {
    use crate::interpreter::resumable::owned_frame::{
        OwnedFrameInput, OwnedFrameInputField, OwnedFrameInputValue,
    };
    let (runtime, execution) = context.ready_runtime().expect("actual runtime");
    let task = runtime.owned_wait_task_v8(execution).unwrap();
    let metadata = execution.wait().lifecycle().owned_wait_task_v8();
    OwnedFrameInput {
        declaration: metadata.id.clone(),
        fields: metadata
            .fields()
            .map(|(identity, _)| OwnedFrameInputField {
                identity: identity.clone(),
                value: if identity == metadata.objective_field {
                    OwnedFrameInputValue::Bytes(task.objective.clone())
                } else if identity == metadata.budget_field {
                    OwnedFrameInputValue::Scalar(crate::interpreter::ArgumentValue::Int(
                        task.budget,
                    ))
                } else {
                    panic!("actual checked Task map")
                },
            })
            .collect(),
    }
}

#[test]
fn owned_cumulative_profile_actual_initializer_appends_bound_selection_before_charge() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let input = actual_task(&context);
        let context = context.with_cumulative_initialization(&lease).unwrap();
        let allowance = context.ordinary().max_steps_per_stage().unwrap() as u64;
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let actual = match super::super::super::live_upstream::initialize_live_actor_v8(
            &journal, input, &cancel,
        ) {
            Ok(Ok(actual)) => actual,
            _ => panic!("real profile ACK and initializer"),
        };
        let prefix = journal.begin_session().unwrap();
        assert_eq!(prefix.sequence(), 6);
        let folded = prefix.fold_for_live_test();
        assert!(folded.continuation_profile_selected());
        assert_eq!((folded.reserved_total, folded.stages), (allowance, 1));
        assert!(folded.consumed_recorded > 0);
        drop(prefix);
        drop(actual);
    });
}

#[test]
#[cfg(unix)]
fn owned_cumulative_profile_failed_selection_ack_never_enters_initializer() {
    for after in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, mut lease, key| {
            let input = actual_task(&context);
            let context = context.with_cumulative_initialization(&lease).unwrap();
            if after {
                lease.test_fail_after_write(2);
            } else {
                lease.test_fail_before_write(2);
            }
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            match super::super::super::live_upstream::initialize_live_actor_v8(
                &journal, input, &cancel,
            ) {
                Ok(Err(_)) => (),
                _ => panic!("failed profile ACK cannot continue initialization"),
            }
            assert!(
                journal.begin_session().is_err(),
                "uncertain prefix remains quarantined"
            );
        });
    }
}
