use super::*;
use crate::interpreter::resumable::owned_frame::{OwnedFrameInputField, OwnedFrameInputValue};
use crate::interpreter::ArgumentValue;
use std::sync::Arc;
pub(super) fn input(context: &CheckedOwnedWaitJournalContextV8) -> OwnedFrameInput {
    let (runtime, execution) = context.ready_runtime().expect("actual runtime");
    let task = runtime.owned_wait_task_v8(execution).unwrap();
    let metadata = execution.wait().lifecycle().owned_wait_task_v8();
    OwnedFrameInput {
        declaration: metadata.id.clone(),
        fields: metadata
            .fields()
            .map(|(id, _)| {
                let value = if id == metadata.objective_field {
                    OwnedFrameInputValue::Bytes(task.objective.clone())
                } else if id == metadata.budget_field {
                    OwnedFrameInputValue::Scalar(ArgumentValue::Int(task.budget))
                } else {
                    panic!("actual compiler Task map")
                };
                OwnedFrameInputField {
                    identity: id.clone(),
                    value,
                }
            })
            .collect(),
    }
}
#[test]
fn owned_wait_live_initialize_has_real_task_owner_charge_and_acknowledged_state() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let input = input(&context);
        let context = context.with_initialization(&lease).unwrap();
        let allowance = context.ordinary().max_steps_per_stage().unwrap() as u64;
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancellation = crate::agent_runtime::AgentCancellation::new();
        let live = match initialize_live_actor_v8(&journal, input, &cancellation) {
            Ok(Ok(live)) => live,
            _ => panic!("real initialization and physical ACKs complete"),
        };
        assert_eq!(
            (
                live.initialization,
                live.state_commit,
                live.session.sequence()
            ),
            (3, 4, 5)
        );
        assert!(live.owner.consumed() > 0 && live.owner.consumed() <= allowance);
        let weak = live.owner.test_weak();
        assert!(!weak.is_empty());
        assert!(
            live.owner.test_same_task_backings(),
            "actual Task backing moved into State, never reminted"
        );
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        live.held.validate_guard().unwrap();
        let recovered = journal.begin_session().unwrap();
        assert_eq!(recovered.sequence(), 5);
        assert_eq!(live.session.fold_for_live_test().reserved_total, allowance);
        assert_eq!(live.session.fold_for_live_test().stages, 1);
        drop(recovered);
        let semantic_releases = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = std::rc::Rc::clone(&semantic_releases);
        crate::interpreter::resumable::owned_frame::snapshot::observe_releases(Some(Box::new(
            move |_| seen.set(seen.get() + 1),
        )));
        drop(live);
        crate::interpreter::resumable::owned_frame::snapshot::observe_releases(None);
        assert_eq!(
            semantic_releases.get(),
            0,
            "abandonment disarms foundation semantic disposer"
        );
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
        journal.hold().unwrap().validate_guard().unwrap();
    });
}
#[test]
fn owned_wait_live_initialize_refuses_task_substitution_and_observe_only_prewrite() {
    for mode in 0..5 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let mut task = input(&context);
            if mode == 0 {
                let field = task
                    .fields
                    .iter_mut()
                    .find(|f| matches!(f.value, OwnedFrameInputValue::Bytes(_)))
                    .unwrap();
                let OwnedFrameInputValue::Bytes(bytes) = &mut field.value else {
                    unreachable!()
                };
                bytes.push(0x7f);
            }
            if mode == 1 {
                task.fields[0].identity = crate::hir::DeclarationId::new("substituted.field");
            }
            if mode == 4 {
                task.declaration = crate::hir::DeclarationId::new("substituted.task");
            }
            let expected = format!("{task:?}");
            let context = if mode == 2 {
                context
            } else {
                context.with_initialization(&lease).unwrap()
            };
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancellation = crate::agent_runtime::AgentCancellation::new();
            if mode == 3 {
                cancellation.cancel();
            }
            let rejected = initialize_live_actor_v8(&journal, task, &cancellation)
                .err()
                .expect("exact typed Task and expected mode required");
            assert_eq!(format!("{:?}", rejected.input), expected);
            assert_eq!(journal.begin_session().unwrap().sequence(), 0);
            journal.hold().unwrap().validate_guard().unwrap();
        });
    }
}
#[test]
fn owned_wait_live_initialize_before_after_persistence_faults_never_repeat_evaluation() {
    for append in 1..=5 {
        for after in [false, true] {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
                |context, mut lease, key| {
                    let task = input(&context);
                    let context = context.with_initialization(&lease).unwrap();
                    if after {
                        lease.test_fail_after_write(append);
                    } else {
                        lease.test_fail_before_write(append);
                    }
                    let journal =
                        SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                    let cancellation = crate::agent_runtime::AgentCancellation::new();
                    let failure = match initialize_live_actor_v8(&journal, task, &cancellation) {
                        Ok(Err(failure)) => failure,
                        _ => panic!("uncertain physical ACK cannot advance live root"),
                    };
                    assert_eq!(
                        failure.held.validate_guard(),
                        Err(SourceJournalError::Poisoned)
                    );
                    assert!(journal.begin_session().is_err());
                    if append <= 3 {
                        assert!(
                            failure.owner.is_none(),
                            "no initializer after failed reservation ACK"
                        );
                    } else {
                        let Some(LiveInitializeOutcomeV8::Initialized(owner)) = &failure.owner
                        else {
                            panic!("actual initialized owner retained")
                        };
                        assert!(owner.consumed() > 0);
                        assert!(owner.test_weak().iter().all(|w| w.strong_count() == 1));
                    }
                },
            );
        }
    }
}

#[test]
fn owned_wait_live_initialize_reservation_requires_exact_retained_stage_allowance() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let allowance = context.ordinary().max_steps_per_stage().unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let session = journal.begin_session().unwrap();
        let session = match session.append(EntryV8::Owned(journal.context().fold().created.clone()))
        {
            Ok(session) => session,
            Err(_) => panic!("Created physical ACK"),
        };
        let mut session = match session.append(EntryV8::Ordinary(SourceJournalEntry::RunOpened)) {
            Ok(session) => session,
            Err(_) => panic!("Opened physical ACK"),
        };
        for fuel in [allowance - 1, allowance + 1] {
            let bytes = session.acknowledged_bytes();
            session =
                match session.append(EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    turn: 0,
                    attempt: None,
                    role: crate::live_invocation::source_journal::SourceStageRole::Initialize,
                    fuel,
                })) {
                    Err(super::super::append::AppendFailureV8::CandidateRefused {
                        session,
                        ..
                    }) => session,
                    _ => panic!("different allowance must refuse before commit/evaluator"),
                };
            assert_eq!(session.sequence(), 2);
            assert_eq!(session.acknowledged_bytes(), bytes);
            assert_eq!(session.fold_for_live_test().reserved_total, 0);
            journal.hold().unwrap().validate_guard().unwrap();
        }
    });
}
