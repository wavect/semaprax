use super::*;
use crate::resumable_effects::owned_frame::v2::{
    compile_owned_agent_wait_v8, compile_owned_reduce_v2,
};
use std::path::Path;
use std::sync::Weak;
fn source(terminal: &str) -> String {
    let source = include_str!("../../../../../../examples/offline-repair-project/src/app.spx");
    let source = source.replace(
        "    runtime_v1 {",
        "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
    );
    let source = source.replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        terminal,
    );
    format!(
        "{source}\n{}",
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal { let proposal = yield observation; state }
@id("fixture.agent.fn.trusted_outcome_test")
fn trusted_outcome_test(status:i64)->Outcome {
    let seed=[1u8];
    Outcome { value:bytes_copy(array_as_slice(seed)), status:status }
}
"#
    )
}
fn plan(source: &str) -> CheckedOwnedReduceV2 {
    let b = compile_owned_agent_wait_v8(
        source,
        Path::new("owned-reduce-runtime.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap_or_else(|d| panic!("{d:?}"));
    compile_owned_reduce_v2(&b).unwrap_or_else(|d| panic!("{d:?}"))
}
fn record_weak(value: &Value) -> Weak<[u8]> {
    let Value::Record(r) = value else { panic!() };
    let Value::Bytes(b) = r
        .fields
        .values()
        .find(|v| matches!(v, Value::Bytes(_)))
        .unwrap()
    else {
        panic!()
    };
    Arc::downgrade(&b.bytes)
}
/// Test-only producer uses an actual evaluator seeded with the same namespace.
/// It bypasses the external effect layer and grants no dispatch/effect evidence.
fn prepared_input(
    p: &CheckedOwnedReduceV2,
    epoch: i64,
    budget: i64,
) -> (PreparedOwnedReduceV2, [Weak<[u8]>; 2]) {
    let state = admit_owned_agent_state_input(
        p.helper(),
        OwnedFrameInput {
            declaration: DeclarationId::new("fixture.agent.type.state"),
            fields: vec![
                OwnedFrameInputField {
                    identity: DeclarationId::new("fixture.agent.type.state.objective"),
                    value: OwnedFrameInputValue::Bytes(vec![]),
                },
                OwnedFrameInputField {
                    identity: DeclarationId::new("fixture.agent.type.state.budget"),
                    value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(budget)),
                },
                OwnedFrameInputField {
                    identity: DeclarationId::new("fixture.agent.type.state.epoch"),
                    value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(epoch)),
                },
            ],
        },
    )
    .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    prepared_from_state(p, state)
}
fn prepared_from_state(
    p: &CheckedOwnedReduceV2,
    mut state: OwnedAgentStateArgument,
) -> (PreparedOwnedReduceV2, [Weak<[u8]>; 2]) {
    assert!(state.plan.same_helper(p.helper()));
    let root = state.root.take().unwrap();
    let mut allocations = state.allocations.take().unwrap();
    let old = allocations.seed(&[&root]).unwrap();
    let f = p
        .helper()
        .program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "fixture.agent.fn.trusted_outcome_test")
        .unwrap();
    let functions = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&functions),
        BTreeMap::new(),
        &p.helper().program().declarations,
        100,
        0,
        PreparedCancellation::Never,
    );
    evaluator.next_byte_allocation = old;
    let mut env = Environment::from(vec![(f.params[0].id.clone(), Value::Int(9))]);
    let outcome = evaluator
        .evaluate(&f.body, &mut env, 0)
        .unwrap_or_else(|_| panic!("actual Outcome source"));
    let next = evaluator.next_byte_allocation;
    drop(env);
    drop(evaluator);
    allocations.record_frame(&[&root, &outcome], next).unwrap();
    let weak = [record_weak(&root), record_weak(&outcome)];
    (
        PreparedOwnedReduceV2 {
            plan: p.clone(),
            state: Some(root),
            outcome: Some(outcome),
            proposal: ResumableChannelValue::Record {
                declaration: DeclarationId::new("fixture.agent.type.proposal"),
                fields: vec![
                    ArgumentValue::Int(7),
                    ArgumentValue::Bool(false),
                    ArgumentValue::Usize(1),
                ],
            },
            allocations,
            creator: std::process::id(),
        },
        weak,
    )
}
fn ready(staged: StagedOwnedReduceV2) -> ReadyOwnedStepV2 {
    let result = settle_owned_reduce_v2(staged, || true, |_| {})
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let OwnedReduceSettledV2::Ready(ready) = result else {
        panic!("expected Ready")
    };
    ready
}
#[test]
fn owned_frame_v2_reduce_moves_state_outcome_step_without_remint_and_matches_source_charges() {
    let p = plan(&source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    ));
    for epoch in [1, 2] {
        let (input, weak) = prepared_input(&p, epoch, 10);
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let staged = stage_owned_reduce_v2(input, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        assert_eq!(staged.failure(), None);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        use crate::interpreter::retained_call::{
            RetainedField, RetainedRecord, RetainedValue as R,
        };
        let record = |id: &str, fields: Vec<(&str, R)>| {
            R::Record(RetainedRecord {
                record: DeclarationId::new(id),
                fields: fields
                    .into_iter()
                    .map(|(id, value)| RetainedField {
                        field: DeclarationId::new(id),
                        value,
                    })
                    .collect(),
            })
        };
        let args = [
            record(
                "fixture.agent.type.state",
                vec![
                    ("fixture.agent.type.state.objective", R::Bytes(vec![])),
                    ("fixture.agent.type.state.budget", R::I64(10)),
                    ("fixture.agent.type.state.epoch", R::I64(epoch)),
                ],
            ),
            R::I64(7),
            R::Bool(false),
            R::Usize(1),
            record(
                "fixture.agent.type.outcome",
                vec![
                    ("fixture.agent.type.outcome.value", R::Bytes(vec![1])),
                    ("fixture.agent.type.outcome.status", R::I64(9)),
                ],
            ),
        ];
        let prep = crate::interpreter::retained_call::prepare_retained_call(
            p.helper().program(),
            "fixture.agent.fn.reduce",
        )
        .unwrap();
        let ordinary = crate::interpreter::retained_call::evaluate_retained_call(
            p.helper().program(),
            &prep,
            &args,
            100,
        )
        .unwrap();
        assert_eq!(fuel.consumed(), ordinary.steps_used);
        let ready = ready(staged);
        let transfer = consume_owned_step_v2(ready).unwrap_or_else(|_| panic!("checked Step"));
        match transfer {
            OwnedStepTransferV2::Continue(state) => {
                assert_eq!(epoch, 1);
                assert_eq!(weak[1].strong_count(), 0);
                assert_eq!(weak[0].strong_count(), 1);
                let Value::Record(r) = state.root.as_ref().unwrap() else {
                    panic!()
                };
                let Value::Bytes(b) =
                    &r.fields[&DeclarationId::new("fixture.agent.type.state.objective")]
                else {
                    panic!()
                };
                assert!(Weak::ptr_eq(&weak[0], &Arc::downgrade(&b.bytes)));
                assert_eq!(b.allocation, 1);
                assert_eq!(
                    r.fields[&DeclarationId::new("fixture.agent.type.state.epoch")],
                    Value::Int(2)
                );
                assert_eq!(
                    state
                        .allocations
                        .as_ref()
                        .unwrap()
                        .seed(&[state.root.as_ref().unwrap()])
                        .unwrap(),
                    2
                );
                drop(state);
            }
            OwnedStepTransferV2::Complete(report) => {
                assert_eq!(epoch, 2);
                assert_eq!(weak[0].strong_count(), 0);
                assert_eq!(weak[1].strong_count(), 1);
                let Value::Record(r) = report.root.as_ref().unwrap() else {
                    panic!()
                };
                let Value::Bytes(b) =
                    &r.fields[&DeclarationId::new("fixture.agent.type.result.summary")]
                else {
                    panic!()
                };
                assert!(Weak::ptr_eq(&weak[1], &Arc::downgrade(&b.bytes)));
                assert_eq!(b.allocation, 2);
                drop(report);
            }
            _ => panic!("wrong checked case"),
        }
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_reduce_suspend_and_fail_do_real_nonresult_cleanup() {
    for (terminal,fail) in [("Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch + 1 }",false),("Step::Fail { code: outcome.status }",true)] {
        let p=plan(&source(terminal)); let (input,weak)=input(&p,2,10); let mut fuel=OwnedFrameBudget::new(100).unwrap();
        let staged=stage_owned_reduce_v2(input,&mut fuel).unwrap_or_else(|r|panic!("{:?}",r.diagnostic));
        let mut observations=Vec::new();
        let settled=settle_owned_reduce_v2(staged,||true,|a|observations.push((a.clone(),weak[0].strong_count(),weak[1].strong_count()))).unwrap_or_else(|r|panic!("{:?}",r.diagnostic));
        let OwnedReduceSettledV2::Ready(ready)=settled else {panic!()};
        if fail { assert_eq!(observations.len(),2); assert_eq!((observations[0].1,observations[0].2),(1,0)); assert_eq!((observations[1].1,observations[1].2),(0,0)); }
        else { assert_eq!(observations.len(),1); assert_eq!((observations[0].1,observations[0].2),(1,0)); }
        match consume_owned_step_v2(ready).unwrap_or_else(|_|panic!("Step")) { OwnedStepTransferV2::Suspend(state) if !fail =>drop(state),OwnedStepTransferV2::Fail(9) if fail =>{},_=>panic!() }
        assert!(weak.iter().all(|w|w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_reduce_sticky_contract_cancel_and_partial_arithmetic_cleanup() {
    let base = source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    );
    for (source, cancel, overflow) in [
        (
            base.replace("-> Step\n{", "-> Step\nrequires false\n{"),
            false,
            false,
        ),
        (
            base.replace("-> Step\n{", "-> Step\nensures false\n{"),
            false,
            false,
        ),
        (base.clone(), true, false),
        (
            base.replace(
                "budget: state.budget, epoch: state.epoch + 1",
                "budget: state.budget + 1, epoch: state.epoch + 1",
            ),
            false,
            true,
        ),
    ] {
        let p = plan(&source);
        let (input, weak) = prepared_input(&p, 1, if overflow { i64::MAX } else { 10 });
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        if cancel {
            fuel.cancel();
        }
        let staged = stage_owned_reduce_v2(input, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        let failure = staged.failure().cloned().expect("selected failure");
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        if overflow {
            assert_eq!(staged.transferred, 1);
            assert!(!staged.provisional);
        }
        let mut observations = Vec::new();
        let settled = settle_owned_reduce_v2(
            staged,
            || true,
            |a| {
                observations.push((a.clone(), weak[0].strong_count(), weak[1].strong_count()));
                if observations.len() == 1 {
                    panic!("observer failure")
                }
            },
        )
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        let OwnedReduceSettledV2::Failed {
            failure: actual,
            operations,
            observations_succeeded,
        } = settled
        else {
            panic!()
        };
        assert_eq!(actual, failure);
        assert!(!observations_succeeded);
        assert_eq!(operations.len(), 2);
        assert_eq!(observations.len(), 2);
        if overflow {
            assert_eq!((observations[0].1, observations[0].2), (0, 1));
        } else {
            assert_eq!((observations[0].1, observations[0].2), (1, 0));
        }
        assert_eq!((observations[1].1, observations[1].2), (0, 0));
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_reduce_partial_fuel_authority_loss_retains_remaining_owner_no_retry() {
    let p = plan(&source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    ));
    let mut found = false;
    for limit in 1..40 {
        let (input, weak) = prepared_input(&p, 1, 10);
        let mut fuel = OwnedFrameBudget::new(limit).unwrap();
        let staged = stage_owned_reduce_v2(input, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        if staged.failure() == Some(&OwnedFrameFailure::FuelExhausted)
            && staged.transferred == 1
            && !staged.provisional
        {
            found = true;
            let mut observed = 0;
            let current = std::cell::Cell::new(true);
            let rejection = settle_owned_reduce_v2(
                staged,
                || current.get(),
                |_| {
                    observed += 1;
                    current.set(false)
                },
            )
            .err()
            .expect("lost authority");
            assert_eq!(observed, 1);
            assert_eq!(weak[0].strong_count(), 0);
            assert_eq!(weak[1].strong_count(), 1);
            let mut retried = 0;
            let rejection = settle_owned_reduce_v2(rejection.staged, || true, |_| retried += 1)
                .err()
                .expect("no retry");
            assert_eq!(retried, 0);
            drop(rejection);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
            break;
        }
        drop(staged);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
    assert!(
        found,
        "real charge boundary must expose transferred partial owner"
    );
}
#[test]
fn owned_frame_v2_reduce_input_schema_provenance_and_step_tamper_refuse_before_transfer() {
    let p = plan(&source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    ));
    for kind in 0..4 {
        let (mut input, weak) = prepared_input(&p, 1, 10);
        if kind == 2 {
            let ResumableChannelValue::Record { fields, .. } = &mut input.proposal else {
                panic!()
            };
            fields[0] = ArgumentValue::Bool(true);
        } else if kind == 3 {
            input.creator = input.creator.wrapping_add(1);
        } else {
            let Value::Record(r) = input.outcome.as_mut().unwrap() else {
                panic!()
            };
            let r = Arc::get_mut(r).unwrap();
            if kind == 1 {
                let Value::Bytes(b) = r
                    .fields
                    .get_mut(&DeclarationId::new("fixture.agent.type.outcome.value"))
                    .unwrap()
                else {
                    panic!()
                };
                b.allocation = 1;
            } else {
                r.record = DeclarationId::new("wrong.outcome");
            }
        }
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let rejection = stage_owned_reduce_v2(input, &mut fuel)
            .err()
            .expect("preflight");
        assert_eq!(fuel.consumed(), 0);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(rejection);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
    let (input, weak) = prepared_input(&p, 1, 10);
    let staged = stage_owned_reduce_v2(input, &mut OwnedFrameBudget::new(100).unwrap())
        .unwrap_or_else(|_| panic!());
    let mut ready = ready(staged);
    let Some(Value::Variant(r)) = ready.root.as_mut() else {
        panic!()
    };
    Arc::get_mut(r).unwrap().fields.insert(
        DeclarationId::new("fixture.agent.step.continue.budget"),
        Value::Bool(true),
    );
    let ready = consume_owned_step_v2(ready).err().expect("wrong scalar");
    assert_eq!(weak[0].strong_count(), 1);
    drop(ready);
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}

#[test]
fn owned_frame_v2_reduce_three_actual_source_turns_keep_state_backing_and_sparse_namespace() {
    let source = source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    )
    .replace("state.epoch < 2", "state.epoch < 3");
    let p = plan(&source);
    let (mut input, weak) = prepared_input(&p, 1, 10);
    let state_witness = weak[0].clone();
    let mut outcomes = vec![weak[1].clone()];
    for turn in 1..=3 {
        let Value::Record(outcome) = input.outcome.as_ref().unwrap() else {
            panic!()
        };
        let Value::Bytes(value) =
            &outcome.fields[&DeclarationId::new("fixture.agent.type.outcome.value")]
        else {
            panic!()
        };
        assert_eq!(value.allocation, turn + 1);
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let staged = stage_owned_reduce_v2(input, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        assert!(staged.failure().is_none());
        let transfer =
            consume_owned_step_v2(ready(staged)).unwrap_or_else(|_| panic!("checked Step"));
        if turn < 3 {
            let OwnedStepTransferV2::Continue(state) = transfer else {
                panic!()
            };
            assert!(Weak::ptr_eq(
                &state_witness,
                &record_weak(state.root.as_ref().unwrap())
            ));
            assert!(outcomes.iter().all(|w| w.upgrade().is_none()));
            let (next, witnesses) = prepared_from_state(&p, state);
            outcomes.push(witnesses[1].clone());
            input = next;
        } else {
            let OwnedStepTransferV2::Complete(report) = transfer else {
                panic!()
            };
            assert!(state_witness.upgrade().is_none());
            assert_eq!(outcomes[2].strong_count(), 1);
            assert!(outcomes[..2].iter().all(|w| w.upgrade().is_none()));
            assert!(Weak::ptr_eq(
                &outcomes[2],
                &record_weak(report.root.as_ref().unwrap())
            ));
            drop(report);
            assert!(outcomes.iter().all(|w| w.upgrade().is_none()));
            return;
        }
    }
    panic!("third source turn must Complete");
}

#[test]
fn owned_frame_v2_reduce_failed_success_cleanup_observation_blocks_step_publication() {
    let p = plan(&source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    ));
    let (input, weak) = prepared_input(&p, 1, 10);
    let staged = stage_owned_reduce_v2(input, &mut OwnedFrameBudget::new(100).unwrap())
        .unwrap_or_else(|_| panic!());
    let mut observed = Vec::new();
    let settled = settle_owned_reduce_v2(
        staged,
        || true,
        |a| {
            observed.push((a.clone(), weak[0].strong_count(), weak[1].strong_count()));
            panic!("observation failed after actual drop")
        },
    )
    .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let OwnedReduceSettledV2::Ready(ready) = settled else {
        panic!()
    };
    assert!(!ready.observations_succeeded());
    assert_eq!(ready.nonresult_operations().len(), 1);
    assert_eq!(observed.len(), 1);
    assert_eq!((observed[0].1, observed[0].2), (1, 0));
    let ready = consume_owned_step_v2(ready)
        .err()
        .expect("failed observation cannot publish State");
    assert_eq!(weak[0].strong_count(), 1);
    drop(ready);
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}
