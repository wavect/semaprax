use super::*;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord};
use crate::resumable_effects::owned_frame::compile_owned_frame_plan;
use std::path::Path;
use std::sync::Weak;

const SOURCE: &str = r#"
module fixture.owned_frame;
@id("fixture.state") record State {
    @id("fixture.state.z") objective: Bytes,
    @id("fixture.state.a") second: Bytes,
    @id("fixture.state.m") budget: i64,
}
@id("fixture.park") fn park_state(state: own State) -> State yields i64 -> i64 {
    let prefix = state.budget + 1;
    let answer = yield prefix;
    let suffix = answer + 1;
    state
}
@id("fixture.main") fn main() -> i64 { 0 }
"#;
fn checked_plan(source: &str) -> CheckedOwnedFramePlan {
    let program =
        hir::resolve(&crate::parse(source, Path::new("owned-frame-owner.spx")).unwrap()).unwrap();
    compile_owned_frame_plan(&program, &hir::DeclarationId::new("fixture.park")).unwrap()
}
fn input() -> RetainedValue {
    RetainedValue::Record(RetainedRecord {
        record: hir::DeclarationId::new("fixture.state"),
        fields: vec![
            RetainedField {
                field: hir::DeclarationId::new("fixture.state.z"),
                value: RetainedValue::Bytes(vec![0, 9, 0]),
            },
            RetainedField {
                field: hir::DeclarationId::new("fixture.state.a"),
                value: RetainedValue::Bytes(Vec::new()),
            },
            RetainedField {
                field: hir::DeclarationId::new("fixture.state.m"),
                value: RetainedValue::I64(4),
            },
        ],
    })
}
fn admitted_argument(plan: &CheckedOwnedFramePlan) -> OwnedFrameArgument {
    match admit_owned_frame_argument(plan, input()) {
        Ok(a) => a,
        Err(e) => panic!("{:?}", e.diagnostic),
    }
}
fn weak(root: &Value) -> Vec<Weak<[u8]>> {
    let Value::Record(record) = root else {
        panic!()
    };
    ["fixture.state.z", "fixture.state.a"]
        .iter()
        .map(|field| {
            let Value::Bytes(bytes) = &record.fields[&hir::DeclarationId::new(*field)] else {
                panic!()
            };
            assert_eq!(Arc::strong_count(&bytes.bytes), 1);
            Arc::downgrade(&bytes.bytes)
        })
        .collect()
}
fn parked(step: OwnedFrameFoundationStep) -> OwnedFrameParked {
    match step {
        OwnedFrameFoundationStep::Parked(p) => p,
        OwnedFrameFoundationStep::Terminal(t) => panic!("{:?}", t.failure()),
    }
}
fn staged(step: OwnedFrameFoundationStep) -> OwnedFrameStagedTerminal {
    match step {
        OwnedFrameFoundationStep::Terminal(t) => t,
        OwnedFrameFoundationStep::Parked(_) => panic!("expected terminal"),
    }
}
fn completed(terminal: OwnedFrameStagedTerminal) -> OwnedFrameResult {
    match settle_owned_frame(terminal) {
        Ok(OwnedFrameSettledOutcome::Completed(result, receipt)) => {
            assert!(receipt.operations.is_empty());
            result
        }
        Ok(_) => panic!("failed"),
        Err(e) => panic!("{:?}", e.diagnostic),
    }
}
fn observe_order(weak: &[Weak<[u8]>]) -> std::rc::Rc<std::cell::RefCell<Vec<String>>> {
    let observed = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let recorded = observed.clone();
    let weak = weak.to_vec();
    RELEASE_OBSERVER.with(|observer| {
        *observer.borrow_mut() = Some(Box::new(move |field| {
            let index = if field.as_str() == "fixture.state.a" {
                1
            } else {
                0
            };
            assert!(
                weak[index].upgrade().is_none(),
                "the real leaf last owner was released before receipt"
            );
            if recorded.borrow().is_empty() {
                assert!(
                    weak[0].upgrade().is_some(),
                    "second physical leaf must remain"
                );
            }
            recorded.borrow_mut().push(field.as_str().to_owned());
        }))
    });
    observed
}
fn clear_observer() {
    RELEASE_OBSERVER.with(|observer| *observer.borrow_mut() = None);
}

#[test]
fn real_owner_survives_park_resume_staged_terminal_and_result_transfer() {
    let plan = checked_plan(SOURCE);
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let park = parked(start_owned_frame(&plan, argument, &mut budget));
    assert_eq!(park.request(), &ArgumentValue::Int(5));
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    let terminal = staged(resume_owned_frame(park, ArgumentValue::Int(8), &mut budget));
    assert!(terminal.failure().is_none());
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    let result = completed(terminal);
    let Value::Record(record) = result.root.as_ref().unwrap() else {
        panic!()
    };
    let Value::Bytes(bytes) = &record.fields[&hir::DeclarationId::new("fixture.state.z")] else {
        panic!()
    };
    assert_eq!(&*bytes.bytes, [0, 9, 0]);
    let result = result
        .into_argument(&plan)
        .ok()
        .expect("consuming same nominal handoff");
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    let terminal = staged(resume_owned_frame(
        parked(start_owned_frame(&plan, result, &mut budget)),
        ArgumentValue::Int(2),
        &mut budget,
    ));
    let result = completed(terminal);
    let observed = observe_order(&weak);
    let receipt = result.dispose().ok().expect("exclusive disposal");
    clear_observer();
    assert_eq!(&*observed.borrow(), &["fixture.state.a", "fixture.state.z"]);
    assert_eq!(receipt.operations, plan.liveness().result_disposal);
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
    assert!(budget.consumed() > 0);
}

#[test]
fn all_failure_classes_keep_pending_root_until_explicit_ordered_settlement() {
    for case in ["requires", "ensures", "fuel", "cancel", "answer"] {
        let source = match case {
            "requires" => {
                SOURCE.replace("yields i64 -> i64 {", "yields i64 -> i64 requires false {")
            }
            "ensures" => SOURCE.replace("yields i64 -> i64 {", "yields i64 -> i64 ensures false {"),
            _ => SOURCE.to_owned(),
        };
        let plan = checked_plan(&source);
        let argument = admitted_argument(&plan);
        let weak = weak(argument.root.as_ref().unwrap());
        let mut budget = OwnedFrameBudget::new(if case == "fuel" { 1 } else { 100 }).unwrap();
        if case == "cancel" {
            budget.cancel();
        }
        let step = start_owned_frame(&plan, argument, &mut budget);
        let terminal = match step {
            OwnedFrameFoundationStep::Terminal(t) => t,
            OwnedFrameFoundationStep::Parked(p) => staged(resume_owned_frame(
                p,
                if case == "answer" {
                    ArgumentValue::Bool(true)
                } else {
                    ArgumentValue::Int(7)
                },
                &mut budget,
            )),
        };
        match (case, terminal.failure()) {
            ("requires" | "ensures", Some(OwnedFrameFailure::Language(_)))
            | ("fuel", Some(OwnedFrameFailure::FuelExhausted))
            | ("cancel", Some(OwnedFrameFailure::HostAbandoned))
            | ("answer", Some(OwnedFrameFailure::AnswerTypeMismatch)) => {}
            _ => panic!("{case}: {:?}", terminal.failure()),
        }
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        let observed = observe_order(&weak);
        match settle_owned_frame(terminal) {
            Ok(OwnedFrameSettledOutcome::Failed(_, receipt)) => {
                assert_eq!(receipt.operations.len(), 2)
            }
            Ok(_) => panic!("unexpected result"),
            Err(e) => panic!("{:?}", e.diagnostic),
        }
        clear_observer();
        assert_eq!(&*observed.borrow(), &["fixture.state.a", "fixture.state.z"]);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
}

#[test]
fn hostile_alias_refuses_before_first_leaf_and_retains_unsettled_root() {
    let plan = checked_plan(SOURCE);
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let Value::Record(record) = argument.root.as_ref().unwrap() else {
        panic!()
    };
    let Value::Bytes(bytes) = &record.fields[&hir::DeclarationId::new("fixture.state.z")] else {
        panic!()
    };
    let alias = bytes.bytes.clone();
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    budget.cancel();
    let terminal = staged(start_owned_frame(&plan, argument, &mut budget));
    let rejection = match settle_owned_frame(terminal) {
        Err(e) => e,
        Ok(_) => panic!("alias cannot settle"),
    };
    assert!(rejection.diagnostic.message.contains("alias"));
    assert!(weak.iter().all(|w| w.upgrade().is_some()));
    drop(alias);
    assert!(matches!(
        settle_owned_frame(rejection.terminal),
        Ok(OwnedFrameSettledOutcome::Failed(_, _))
    ));
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}

#[test]
fn drop_unwind_disposes_backing_without_issuing_semantic_receipt_and_result_drop_is_ordered() {
    let plan = checked_plan(SOURCE);
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let park = parked(start_owned_frame(&plan, argument, &mut budget));
    let observed = observe_order(&weak);
    drop(park);
    clear_observer();
    assert!(observed.borrow().is_empty());
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let terminal = staged(resume_owned_frame(
        parked(start_owned_frame(&plan, argument, &mut budget)),
        ArgumentValue::Int(1),
        &mut budget,
    ));
    let observed = observe_order(&weak);
    drop(completed(terminal));
    clear_observer();
    assert_eq!(&*observed.borrow(), &["fixture.state.a", "fixture.state.z"]);
}

#[test]
fn invalid_carrier_returns_original_inert_input_and_wrong_plan_never_evaluates() {
    let plan = checked_plan(SOURCE);
    let mut carrier = input();
    let RetainedValue::Record(record) = &mut carrier else {
        panic!()
    };
    record.fields[1].field = record.fields[0].field.clone();
    let original = carrier.clone();
    let rejected = match admit_owned_frame_argument(&plan, carrier) {
        Err(e) => e,
        Ok(_) => panic!(),
    };
    assert_eq!(rejected.input, original);
    let other = checked_plan(&SOURCE.replace("state.budget + 1", "state.budget + 2"));
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let terminal = staged(start_owned_frame(&other, argument, &mut budget));
    assert_eq!(
        terminal.failure(),
        Some(&OwnedFrameFailure::EvaluationRejected)
    );
    assert_eq!(budget.consumed(), 0);
    assert!(matches!(
        settle_owned_frame(terminal),
        Ok(OwnedFrameSettledOutcome::Failed(_, _))
    ));
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}

#[test]
fn borrowed_copy_contracts_and_unwind_keep_owner_lifetime_distinct_from_settlement() {
    let source = SOURCE.replace(
        "yields i64 -> i64 {",
        "yields i64 -> i64 requires state.budget > 0 ensures result.budget == 4 {",
    );
    let plan = checked_plan(&source);
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let terminal = staged(resume_owned_frame(
        parked(start_owned_frame(&plan, argument, &mut budget)),
        ArgumentValue::Int(1),
        &mut budget,
    ));
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    drop(completed(terminal));
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
    for phase in ["park", "terminal"] {
        let argument = admitted_argument(&plan);
        let weak = weak(argument.root.as_ref().unwrap());
        let park = parked(start_owned_frame(&plan, argument, &mut budget));
        let observed = observe_order(&weak);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if phase == "park" {
                let _owner = park;
                panic!("park unwind");
            } else {
                let _owner = staged(resume_owned_frame(
                    park,
                    ArgumentValue::Bool(true),
                    &mut budget,
                ));
                panic!("terminal unwind");
            }
        }));
        assert!(result.is_err());
        clear_observer();
        assert!(
            observed.borrow().is_empty(),
            "backing Drop issues no semantic cleanup receipt"
        );
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
}

#[test]
fn resume_fuel_and_cancel_keep_root_and_failure_selection() {
    for cancel in [false, true] {
        let plan = checked_plan(SOURCE);
        let argument = admitted_argument(&plan);
        let weak = weak(argument.root.as_ref().unwrap());
        let mut budget = OwnedFrameBudget::new(100).unwrap();
        let park = parked(start_owned_frame(&plan, argument, &mut budget));
        if cancel {
            budget.cancel();
        } else {
            budget.remaining = 0;
        }
        let terminal = staged(resume_owned_frame(park, ArgumentValue::Int(3), &mut budget));
        assert_eq!(
            terminal.failure(),
            Some(&if cancel {
                OwnedFrameFailure::HostAbandoned
            } else {
                OwnedFrameFailure::FuelExhausted
            })
        );
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        assert!(matches!(
            settle_owned_frame(terminal),
            Ok(OwnedFrameSettledOutcome::Failed(_, _))
        ));
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
}

#[test]
fn full_inert_input_all_eight_scalars_preserve_unicode_nan_payload_and_negative_zero() {
    for (last_ty, last_value) in [
        ("bool", ArgumentValue::Bool(true)),
        (
            "f64",
            ArgumentValue::Float64(f64::from_bits(0x7ff8_0000_0000_0042)),
        ),
        (
            "f64",
            ArgumentValue::Float64(f64::from_bits(0x8000_0000_0000_0000)),
        ),
    ] {
        let source = format!(
            r#"
module fixture.full_input;
@id("full.state") record State {{
    @id("full.bytes") bytes: Bytes,
    @id("full.i64") wide: i64,
    @id("full.i32") narrow: i32,
    @id("full.u8") octet: u8,
    @id("full.usize") size: usize,
    @id("full.char") rune: char,
    @id("full.f32") single: f32,
    @id("full.last") last: {last_ty},
}}
@id("fixture.park") fn park_state(state: own State) -> State yields i64 -> i64 {{
    let answer = yield state.wide;
    state
}}
@id("fixture.main") fn main() -> i64 {{ 0 }}
"#
        );
        let plan = checked_plan(&source);
        for single in [f32::from_bits(0x7fc0_0042), f32::from_bits(0x8000_0000)] {
            let scalar_values = [
                ArgumentValue::Int(-7),
                ArgumentValue::Int32(-3),
                ArgumentValue::Uint8(255),
                ArgumentValue::Usize(u32::MAX as u64),
                ArgumentValue::Char(0x1f642),
                ArgumentValue::Float32(single),
                last_value.clone(),
            ];
            let mut fields = vec![OwnedFrameInputField {
                identity: hir::DeclarationId::new("full.bytes"),
                value: OwnedFrameInputValue::Bytes(vec![0, 2, 0]),
            }];
            for (identity, value) in [
                "full.i64",
                "full.i32",
                "full.u8",
                "full.usize",
                "full.char",
                "full.f32",
                "full.last",
            ]
            .into_iter()
            .zip(scalar_values)
            {
                fields.push(OwnedFrameInputField {
                    identity: hir::DeclarationId::new(identity),
                    value: OwnedFrameInputValue::Scalar(value),
                });
            }
            let carrier = OwnedFrameInput {
                declaration: hir::DeclarationId::new("full.state"),
                fields,
            };
            let argument = match admit_owned_frame_input(&plan, carrier.clone()) {
                Ok(a) => a,
                Err(e) => panic!("{:?}", e.diagnostic),
            };
            let mut budget = OwnedFrameBudget::new(100).unwrap();
            let result = completed(staged(resume_owned_frame(
                parked(start_owned_frame(&plan, argument, &mut budget)),
                ArgumentValue::Int(0),
                &mut budget,
            )));
            let Value::Record(record) = result.root.as_ref().unwrap() else {
                panic!()
            };
            assert!(matches!(
                record.fields[&hir::DeclarationId::new("full.char")],
                Value::Char(0x1f642)
            ));
            let Value::Float32(value) = record.fields[&hir::DeclarationId::new("full.f32")] else {
                panic!()
            };
            assert_eq!(value.to_bits(), single.to_bits());
            match (
                &last_value,
                &record.fields[&hir::DeclarationId::new("full.last")],
            ) {
                (ArgumentValue::Float64(expected), Value::Float64(actual)) => {
                    assert_eq!(actual.to_bits(), expected.to_bits())
                }
                (ArgumentValue::Bool(expected), Value::Bool(actual)) => {
                    assert_eq!(actual, expected)
                }
                _ => panic!("scalar type changed"),
            }
            drop(result);
            for mutation in ["unicode", "borrow", "field", "order", "usize"] {
                let mut bad = carrier.clone();
                match mutation {
                    "unicode" => {
                        bad.fields[5].value =
                            OwnedFrameInputValue::Scalar(ArgumentValue::Char(0xd800))
                    }
                    "borrow" => {
                        bad.fields[5].value =
                            OwnedFrameInputValue::Scalar(ArgumentValue::BorrowedStr("x".to_owned()))
                    }
                    "field" => bad.fields[1].identity = hir::DeclarationId::new("full.other"),
                    "order" => bad.fields.swap(1, 2),
                    "usize" => {
                        bad.fields[4].value =
                            OwnedFrameInputValue::Scalar(ArgumentValue::Usize(u32::MAX as u64 + 1))
                    }
                    _ => unreachable!(),
                }
                let expected_debug = format!("{bad:?}");
                let rejection = match admit_owned_frame_input(&plan, bad) {
                    Err(e) => e,
                    Ok(_) => panic!("{mutation} admitted"),
                };
                assert_eq!(
                    format!("{:?}", rejection.input),
                    expected_debug,
                    "untouched inert input returned"
                );
            }
        }
    }
}

#[test]
fn inclusive_byte_capacity_and_precommit_disposal_use_real_backing() {
    let plan = checked_plan(SOURCE);
    let mut carrier = input();
    let RetainedValue::Record(record) = &mut carrier else {
        panic!()
    };
    record.fields[0].value = RetainedValue::Bytes(vec![0; 1024]);
    record.fields[1].value = RetainedValue::Bytes(vec![7; 1024]);
    let argument = match admit_owned_frame_argument(&plan, carrier.clone()) {
        Ok(a) => a,
        Err(e) => panic!("{:?}", e.diagnostic),
    };
    let weak = weak(argument.root.as_ref().unwrap());
    let observed = observe_order(&weak);
    drop(argument); // precommit argument disposal is compiler ordered
    clear_observer();
    assert_eq!(&*observed.borrow(), &["fixture.state.a", "fixture.state.z"]);
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
    let RetainedValue::Record(record) = &mut carrier else {
        panic!()
    };
    record.fields[0].value = RetainedValue::Bytes(vec![0; 1025]);
    let original = carrier.clone();
    let rejection = match admit_owned_frame_argument(&plan, carrier) {
        Err(e) => e,
        Ok(_) => panic!("capacity +1 admitted"),
    };
    assert_eq!(rejection.input, original);
}

#[test]
fn rejected_result_handoff_preserves_original_owner_and_disposal_plan() {
    let plan = checked_plan(SOURCE);
    let changed = checked_plan(&SOURCE.replace("second: Bytes", "second: i64"));
    let argument = admitted_argument(&plan);
    let weak = weak(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let result = completed(staged(resume_owned_frame(
        parked(start_owned_frame(&plan, argument, &mut budget)),
        ArgumentValue::Int(3),
        &mut budget,
    )));
    let rejection = match result.into_argument(&changed) {
        Err(e) => e,
        Ok(_) => panic!("same nominal ID with different actual field shape must refuse"),
    };
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    let observed = observe_order(&weak);
    assert!(rejection.result.dispose().is_ok());
    clear_observer();
    assert_eq!(&*observed.borrow(), &["fixture.state.a", "fixture.state.z"]);
}
