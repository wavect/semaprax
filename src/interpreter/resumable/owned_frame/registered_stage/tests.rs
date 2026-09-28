use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::compile_owned_frame_helper_v2;
use std::path::Path;
use std::sync::Weak;
const SOURCE: &str = r#"
module owned.v2;
@id("state") record State {
 @id("state.z") first: Bytes,
 @id("state.a") second: Bytes,
 @id("state.m") budget: i64,
 @id("state.ch") ch: char,
 @id("state.f") f: f32,
 @id("state.g") g: f64,
 @id("state.b") b: bool,
 @id("state.n") n: usize,
}
@id("observation") record Observation {
 @id("observation.i") i: i64,
 @id("observation.j") j: i32,
 @id("observation.u") u: u8,
 @id("observation.n") n: usize,
 @id("observation.ch") ch: char,
 @id("observation.f") f: f32,
 @id("observation.g") g: f64,
 @id("observation.b") b: bool,
}
@id("proposal") record Proposal {
 @id("proposal.i") i: i64,
 @id("proposal.j") j: i32,
 @id("proposal.u") u: u8,
 @id("proposal.n") n: usize,
 @id("proposal.ch") ch: char,
 @id("proposal.f") f: f32,
 @id("proposal.g") g: f64,
 @id("proposal.b") b: bool,
}
@id("park") fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
 let proposal = yield observation;
 state
}
@id("main") fn main()->i64 {0}
"#;
fn plan(source: &str) -> CheckedOwnedFrameHelperV2 {
    let p =
        hir::resolve(&crate::parse(source, Path::new("owned-v2-runtime.spx")).unwrap()).unwrap();
    compile_owned_frame_helper_v2(&p, &DeclarationId::new("park")).unwrap()
}
fn input() -> OwnedFrameInput {
    let fields = vec![
        ("state.z", OwnedFrameInputValue::Bytes(vec![])),
        ("state.a", OwnedFrameInputValue::Bytes(vec![0])),
        (
            "state.m",
            OwnedFrameInputValue::Scalar(ArgumentValue::Int(7)),
        ),
        (
            "state.ch",
            OwnedFrameInputValue::Scalar(ArgumentValue::Char(0x2603)),
        ),
        (
            "state.f",
            OwnedFrameInputValue::Scalar(ArgumentValue::Float32(f32::from_bits(0x7fc01234))),
        ),
        (
            "state.g",
            OwnedFrameInputValue::Scalar(ArgumentValue::Float64(f64::from_bits(
                0x8000000000000000,
            ))),
        ),
        (
            "state.b",
            OwnedFrameInputValue::Scalar(ArgumentValue::Bool(true)),
        ),
        (
            "state.n",
            OwnedFrameInputValue::Scalar(ArgumentValue::Usize(0)),
        ),
    ];
    OwnedFrameInput {
        declaration: DeclarationId::new("state"),
        fields: fields
            .into_iter()
            .map(|(id, value)| OwnedFrameInputField {
                identity: DeclarationId::new(id),
                value,
            })
            .collect(),
    }
}
fn carrier(id: &str) -> ResumableChannelValue {
    ResumableChannelValue::Record {
        declaration: DeclarationId::new(id),
        fields: vec![
            ArgumentValue::Int(-1),
            ArgumentValue::Int32(i32::MIN),
            ArgumentValue::Uint8(255),
            ArgumentValue::Usize(u32::MAX as u64),
            ArgumentValue::Char(0x10ffff),
            ArgumentValue::Float32(f32::from_bits(0xffc01234)),
            ArgumentValue::Float64(f64::from_bits(0x8000000000000000)),
            ArgumentValue::Bool(true),
        ],
    }
}
fn argument(p: &CheckedOwnedFrameHelperV2) -> OwnedAgentStateArgument {
    admit_owned_agent_state_input(p, input()).unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
}
fn prepared(p: &CheckedOwnedFrameHelperV2) -> PreparedOwnedCopyWaitV2 {
    prepare_owned_copy_wait_v2(argument(p), carrier("observation"))
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
}
fn parked(step: OwnedCopyWaitStepV2) -> OwnedCopyWaitParkedV2 {
    match step {
        OwnedCopyWaitStepV2::Parked(p) => p,
        OwnedCopyWaitStepV2::Terminal(t) => panic!("unexpected terminal {:?}", t.failure()),
    }
}
fn terminal(step: OwnedCopyWaitStepV2) -> OwnedCopyWaitTerminalV2 {
    match step {
        OwnedCopyWaitStepV2::Terminal(t) => t,
        OwnedCopyWaitStepV2::Parked(_) => panic!("unexpected park"),
    }
}
fn weak(root: &Value) -> Vec<Weak<[u8]>> {
    let Value::Record(r) = root else { panic!() };
    ["state.z", "state.a"]
        .iter()
        .map(|id| {
            let Value::Bytes(b) = &r.fields[&DeclarationId::new(id)] else {
                panic!()
            };
            Arc::downgrade(&b.bytes)
        })
        .collect()
}
fn assert_bits(root: &Value) {
    let Value::Record(r) = root else { panic!() };
    assert!(matches!(
        r.fields[&DeclarationId::new("state.ch")],
        Value::Char(0x2603)
    ));
    let Value::Float32(v) = r.fields[&DeclarationId::new("state.f")] else {
        panic!()
    };
    assert_eq!(v.to_bits(), 0x7fc01234);
    let Value::Float64(v) = r.fields[&DeclarationId::new("state.g")] else {
        panic!()
    };
    assert_eq!(v.to_bits(), 0x8000000000000000);
}
#[test]
fn owned_frame_v2_actual_owner_copy_bits_survive_start_park_resume_and_staged_transfer() {
    let p = plan(SOURCE);
    let arg = argument(&p);
    let backing = weak(arg.root.as_ref().unwrap());
    assert_bits(arg.root.as_ref().unwrap());
    let prep = prepare_owned_copy_wait_v2(arg, carrier("observation"))
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let mut start = OwnedFrameBudget::new(100).unwrap();
    let park = parked(begin_owned_copy_wait_v2(prep, &mut start).unwrap_or_else(|_| panic!("PID")));
    assert!(start.consumed() > 0);
    assert_bits(park.root.as_ref().unwrap());
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    let ResumableChannelValue::Record { fields, .. } = park.request() else {
        panic!()
    };
    let ArgumentValue::Float32(v) = fields[5] else {
        panic!()
    };
    assert_eq!(v.to_bits(), 0xffc01234);
    let mut resume = OwnedFrameBudget::new(100).unwrap();
    let staged = terminal(
        resume_owned_copy_wait_v2(park, carrier("proposal"), &mut resume)
            .unwrap_or_else(|_| panic!("PID")),
    );
    assert!(staged.failure().is_none());
    assert!(resume.consumed() > 0);
    assert_bits(staged.root.as_ref().unwrap());
    let ResumableChannelValue::Record { fields, .. } = staged.proposal().unwrap() else {
        panic!()
    };
    let ArgumentValue::Float32(v) = fields[5] else {
        panic!()
    };
    assert_eq!(v.to_bits(), 0xffc01234);
    let ArgumentValue::Float64(v) = fields[6] else {
        panic!()
    };
    assert_eq!(v.to_bits(), 0x8000000000000000);
    let settled = settle_owned_copy_wait_v2(
        staged,
        || true,
        |_| panic!("successful root cannot be finalized"),
    )
    .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let OwnedCopyWaitSettledV2::Completed(next) = settled else {
        panic!()
    };
    assert_bits(next.root.as_ref().unwrap());
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    assert_eq!(next.creator, std::process::id());
    assert_eq!(next.plan.function().id.as_str(), "park");
    assert!(
        matches!(next.proposal,ResumableChannelValue::Record{ref declaration,..} if declaration.as_str()=="proposal")
    );
    drop(next);
    assert!(backing.iter().all(|w| w.upgrade().is_none()));
}
#[test]
fn owned_frame_v2_preparation_rejects_wrong_shape_preserving_actual_argument() {
    let p = plan(SOURCE);
    let arg = argument(&p);
    let backing = weak(arg.root.as_ref().unwrap());
    let rejected = prepare_owned_copy_wait_v2(arg, carrier("proposal"))
        .err()
        .expect("nominal substitution");
    assert!(
        matches!(rejected.observation,ResumableChannelValue::Record{ref declaration,..} if declaration.as_str()=="proposal")
    );
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    let prep = prepare_owned_copy_wait_v2(rejected.argument, carrier("observation"))
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    assert!(matches!(
        begin_owned_copy_wait_v2(prep, &mut OwnedFrameBudget::new(100).unwrap())
            .unwrap_or_else(|_| panic!("PID")),
        OwnedCopyWaitStepV2::Parked(_)
    ));
    let mut invalid = input();
    invalid.fields[3].value = OwnedFrameInputValue::Scalar(ArgumentValue::Char(0xd800));
    let rejected = admit_owned_agent_state_input(&p, invalid)
        .err()
        .expect("invalid Unicode");
    assert!(matches!(
        rejected.input.fields[3].value,
        OwnedFrameInputValue::Scalar(ArgumentValue::Char(0xd800))
    ));
    assert!(
        matches!(&rejected.input.fields[0].value,OwnedFrameInputValue::Bytes(v) if v.is_empty())
    );
}
#[test]
fn owned_frame_v2_answer_negative_controls_preserve_root_and_select_stable_failure() {
    let p = plan(SOURCE);
    let mut wrong_leaf = carrier("proposal");
    let ResumableChannelValue::Record { fields, .. } = &mut wrong_leaf else {
        panic!()
    };
    fields[0] = ArgumentValue::Bool(false);
    let mut wrong_count = carrier("proposal");
    let ResumableChannelValue::Record { fields, .. } = &mut wrong_count else {
        panic!()
    };
    fields.pop();
    let bad = vec![
        wrong_leaf,
        wrong_count,
        carrier("observation"),
        ResumableChannelValue::Variant {
            declaration: DeclarationId::new("proposal"),
            case: DeclarationId::new("wrong"),
            fields: vec![],
        },
        ResumableChannelValue::RecordBytes {
            declaration: DeclarationId::new("proposal"),
            fields: vec![],
        },
    ];
    for answer in bad {
        let park = parked(
            begin_owned_copy_wait_v2(prepared(&p), &mut OwnedFrameBudget::new(100).unwrap())
                .unwrap_or_else(|_| panic!("PID")),
        );
        let backing = weak(park.root.as_ref().unwrap());
        let mut budget = OwnedFrameBudget::new(100).unwrap();
        let failed = terminal(
            resume_owned_copy_wait_v2(park, answer, &mut budget).unwrap_or_else(|_| panic!("PID")),
        );
        assert_eq!(
            failed.failure(),
            Some(&OwnedFrameFailure::AnswerTypeMismatch)
        );
        assert_eq!(budget.consumed(), 0);
        assert!(backing.iter().all(|w| w.strong_count() == 1));
        let mut order = Vec::new();
        let settled = settle_owned_copy_wait_v2(
            failed,
            || true,
            |a| order.push(a.source.projections[0].as_str().to_owned()),
        )
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        assert!(matches!(
            settled,
            OwnedCopyWaitSettledV2::Failed {
                failure: OwnedFrameFailure::AnswerTypeMismatch,
                observations_succeeded: true,
                ..
            }
        ));
        assert_eq!(order, ["state.a", "state.z"]);
        assert!(backing.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_requires_ensures_fuel_cancel_hold_real_root_until_settlement() {
    for mode in ["requires", "ensures", "start_fuel", "resume_fuel", "cancel"] {
        let source = match mode {
            "requires" => SOURCE.replace(
                "yields Observation -> Proposal {",
                "yields Observation -> Proposal requires false {",
            ),
            "ensures" => SOURCE.replace(
                "yields Observation -> Proposal {",
                "yields Observation -> Proposal ensures false {",
            ),
            _ => SOURCE.to_owned(),
        };
        let p = plan(&source);
        let prep = prepared(&p);
        let backing = weak(prep.argument.root.as_ref().unwrap());
        let mut start = OwnedFrameBudget::new(if mode == "start_fuel" { 1 } else { 100 }).unwrap();
        if mode == "cancel" {
            start.cancel();
        }
        let step = begin_owned_copy_wait_v2(prep, &mut start).unwrap_or_else(|_| panic!("PID"));
        let failed = if matches!(mode, "ensures" | "resume_fuel") {
            let mut resume =
                OwnedFrameBudget::new(if mode == "resume_fuel" { 1 } else { 100 }).unwrap();
            terminal(
                resume_owned_copy_wait_v2(parked(step), carrier("proposal"), &mut resume)
                    .unwrap_or_else(|_| panic!("PID")),
            )
        } else {
            terminal(step)
        };
        match mode {
            "requires" | "ensures" => assert!(matches!(
                failed.failure(),
                Some(OwnedFrameFailure::Language(_))
            )),
            "cancel" => assert_eq!(failed.failure(), Some(&OwnedFrameFailure::HostAbandoned)),
            _ => assert_eq!(failed.failure(), Some(&OwnedFrameFailure::FuelExhausted)),
        }
        assert!(backing.iter().all(|w| w.strong_count() == 1));
        let selected = failed.failure.clone().unwrap();
        let mut observations = Vec::new();
        let result = settle_owned_copy_wait_v2(
            failed,
            || true,
            |action| {
                observations.push((
                    action.source.projections[0].as_str().to_owned(),
                    backing[0].upgrade().is_none(),
                    backing[1].upgrade().is_none(),
                ));
            },
        )
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        let OwnedCopyWaitSettledV2::Failed {
            failure,
            receipt,
            observations_succeeded,
        } = result
        else {
            panic!()
        };
        assert_eq!(failure, selected);
        assert!(observations_succeeded);
        assert_eq!(receipt.operations.len(), 2);
        assert_eq!(
            observations,
            [
                ("state.a".into(), false, true),
                ("state.z".into(), true, true)
            ]
        );
    }
}
#[test]
fn owned_frame_v2_callback_panic_and_guard_loss_do_not_fabricate_cleanup() {
    let p = plan(
        SOURCE
            .replace(
                "yields Observation -> Proposal {",
                "yields Observation -> Proposal requires false {",
            )
            .as_str(),
    );
    let prep = prepared(&p);
    let backing = weak(prep.argument.root.as_ref().unwrap());
    let failed = terminal(
        begin_owned_copy_wait_v2(prep, &mut OwnedFrameBudget::new(100).unwrap())
            .unwrap_or_else(|_| panic!("PID")),
    );
    let mut states = Vec::new();
    let result = settle_owned_copy_wait_v2(
        failed,
        || true,
        |a| {
            states.push((
                a.source.projections[0].as_str().to_owned(),
                backing[0].upgrade().is_none(),
                backing[1].upgrade().is_none(),
            ));
            panic!("observer");
        },
    )
    .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    assert!(matches!(
        result,
        OwnedCopyWaitSettledV2::Failed {
            observations_succeeded: false,
            ..
        }
    ));
    assert_eq!(
        states,
        [
            ("state.a".into(), false, true),
            ("state.z".into(), true, true)
        ]
    );
    let prep = prepared(&p);
    let backing = weak(prep.argument.root.as_ref().unwrap());
    let failed = terminal(
        begin_owned_copy_wait_v2(prep, &mut OwnedFrameBudget::new(100).unwrap())
            .unwrap_or_else(|_| panic!("PID")),
    );
    let authority = std::cell::Cell::new(true);
    let mut count = 0;
    let rejected = settle_owned_copy_wait_v2(
        failed,
        || authority.get(),
        |_| {
            count += 1;
            authority.set(false);
        },
    )
    .err()
    .expect("lost authority");
    assert_eq!(count, 1);
    assert!(backing[1].upgrade().is_none());
    assert!(backing[0].upgrade().is_some());
    let mut retried = 0;
    assert!(settle_owned_copy_wait_v2(rejected.terminal, || true, |_| retried += 1).is_err());
    assert_eq!(retried, 0);
}

#[test]
fn owned_frame_v2_foreign_process_refuses_before_evaluation_or_release() {
    let p = plan(SOURCE);
    let mut prep = prepared(&p);
    let backing = weak(prep.argument.root.as_ref().unwrap());
    prep.argument.creator = std::process::id().wrapping_add(1);
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let prep = begin_owned_copy_wait_v2(prep, &mut budget)
        .err()
        .expect("foreign prepared owner");
    assert_eq!(budget.consumed(), 0);
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    // Foreign abandonment performs backing disposal only, with no semantic
    // release or receipt. Actual inherited-FD behavior has the store gate.
    drop(prep);
    assert!(backing.iter().all(|w| w.upgrade().is_none()));

    let parked = parked(
        begin_owned_copy_wait_v2(prepared(&p), &mut OwnedFrameBudget::new(100).unwrap())
            .unwrap_or_else(|_| panic!("PID")),
    );
    let backing = weak(parked.root.as_ref().unwrap());
    let mut terminal = terminal(
        resume_owned_copy_wait_v2(parked, carrier("proposal"), &mut budget)
            .unwrap_or_else(|_| panic!("PID")),
    );
    terminal.creator = std::process::id().wrapping_add(1);
    let mut observations = 0;
    let rejected = settle_owned_copy_wait_v2(terminal, || true, |_| observations += 1)
        .err()
        .expect("foreign terminal owner");
    assert_eq!(observations, 0);
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    drop(rejected);
    assert!(backing.iter().all(|w| w.upgrade().is_none()));
}
