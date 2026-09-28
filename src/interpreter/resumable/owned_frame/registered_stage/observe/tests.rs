use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::{
    compile_owned_frame_helper_v2, compile_owned_observe_v2,
};
use std::sync::Weak;
const SOURCE: &str = r#"
module owned.observe;
@id("state") record State {
 @id("state.z") first: Bytes,
 @id("state.a") second: Bytes,
 @id("state.budget") budget: i64,
}
@id("observation") record Observation {
 @id("observation.budget") budget: i64,
 @id("observation.length") length: usize,
}
@id("proposal") record Proposal { @id("proposal.budget") budget: i64, }
@id("park") fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
 let proposal = yield observation;
 state
}
@id("observe") fn observe(state: borrow State) -> Observation {
 let view = bytes_as_slice(state.second);
 Observation { budget: state.budget, length: byte_len(view) }
}
@id("main") fn main() -> i64 { 0 }
"#;
fn proof(source: &str) -> CheckedOwnedObserveV2 {
    let p = hir::resolve(&crate::check(source, "owned-observe.spx").unwrap()).unwrap();
    let h = compile_owned_frame_helper_v2(&p, &DeclarationId::new("park")).unwrap();
    compile_owned_observe_v2(&h, &DeclarationId::new("observe")).unwrap()
}
fn input() -> OwnedFrameInput {
    OwnedFrameInput {
        declaration: DeclarationId::new("state"),
        fields: vec![
            OwnedFrameInputField {
                identity: DeclarationId::new("state.z"),
                value: OwnedFrameInputValue::Bytes(vec![]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("state.a"),
                value: OwnedFrameInputValue::Bytes(vec![0]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("state.budget"),
                value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(10)),
            },
        ],
    }
}
fn argument(p: &CheckedOwnedObserveV2) -> OwnedAgentStateArgument {
    admit_owned_agent_state_input(p.helper(), input())
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
}
fn weak(root: &Value) -> Vec<Weak<[u8]>> {
    let Value::Record(r) = root else { panic!() };
    ["state.z", "state.a"]
        .iter()
        .map(|id| {
            let Value::Bytes(b) = &r.fields[&DeclarationId::new(*id)] else {
                panic!()
            };
            Arc::downgrade(&b.bytes)
        })
        .collect()
}
#[test]
fn owned_frame_v2_observe_actual_body_charges_and_same_root_enter_helper() {
    for (expression, expected) in [("state.budget", 10), ("state.budget + 2", 12)] {
        let p = proof(&SOURCE.replace("budget: state.budget,", &format!("budget: {expression},")));
        let a = argument(&p);
        let roots = weak(a.root.as_ref().unwrap());
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let OwnedObserveStepV2::Observed(o) = observe_owned_agent_state_v2(a, &p, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
        else {
            panic!()
        };
        assert!(root_valid(p.helper(), o.root.as_ref().unwrap()));
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        let ResumableChannelValue::Record {
            declaration,
            fields,
        } = o.observation()
        else {
            panic!()
        };
        assert_eq!(declaration.as_str(), "observation");
        assert!(
            matches!(fields.as_slice(), [ArgumentValue::Int(v), ArgumentValue::Usize(1)] if *v == expected)
        );
        // Independent ordinary call oracle; production never remints State.
        use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue};
        let args = [RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("state"),
            fields: vec![
                RetainedField {
                    field: DeclarationId::new("state.z"),
                    value: RetainedValue::Bytes(vec![]),
                },
                RetainedField {
                    field: DeclarationId::new("state.a"),
                    value: RetainedValue::Bytes(vec![0]),
                },
                RetainedField {
                    field: DeclarationId::new("state.budget"),
                    value: RetainedValue::I64(10),
                },
            ],
        })];
        let prep = crate::interpreter::retained_call::prepare_retained_call(
            p.helper().program(),
            "observe",
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
        let crate::interpreter::retained_call::RetainedCallOutcome::Returned(
            RetainedValue::Record(r),
        ) = ordinary.outcome
        else {
            panic!()
        };
        assert!(matches!(r.fields[0].value, RetainedValue::I64(v) if v == expected));
        assert!(matches!(r.fields[1].value, RetainedValue::Usize(1)));
        let expected_request = o.observation().clone(); // inert Copy only
        let prepared =
            prepare_observed_owned_copy_wait_v2(o).unwrap_or_else(|_| panic!("same root transfer"));
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        let OwnedCopyWaitStepV2::Parked(parked) =
            begin_owned_copy_wait_v2(prepared, &mut OwnedFrameBudget::new(100).unwrap())
                .unwrap_or_else(|_| panic!())
        else {
            panic!()
        };
        assert_eq!(parked.request(), &expected_request);
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        drop(parked);
        assert!(roots.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_observe_failure_retains_root_until_real_ordered_settlement() {
    for mode in ["requires", "ensures", "fuel", "cancel"] {
        let source = match mode {
            "requires" => SOURCE.replace("-> Observation {", "-> Observation requires false {"),
            "ensures" => SOURCE.replace("-> Observation {", "-> Observation ensures false {"),
            _ => SOURCE.to_owned(),
        };
        let p = proof(&source);
        let a = argument(&p);
        let roots = weak(a.root.as_ref().unwrap());
        let mut fuel = OwnedFrameBudget::new(if mode == "fuel" { 1 } else { 100 }).unwrap();
        if mode == "cancel" {
            fuel.cancel();
        }
        let OwnedObserveStepV2::Failed(f) = observe_owned_agent_state_v2(a, &p, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
        else {
            panic!("{mode}")
        };
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        assert_eq!(fuel.consumed() == 0, mode == "cancel");
        let selected = f.failure.clone();
        let expected = p.helper().liveness().failure_cleanup.clone();
        let mut seen = vec![];
        let mut deaths = vec![];
        let settled = settle_failed_owned_observe_v2(
            f,
            || true,
            |a| {
                seen.push(a.clone());
                deaths.push(
                    roots
                        .iter()
                        .map(|w| w.upgrade().is_none())
                        .collect::<Vec<_>>(),
                );
            },
        )
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        assert_eq!(settled.failure, selected);
        assert!(settled.observations_succeeded);
        assert_eq!(seen, expected);
        assert_eq!(settled.receipt.operations, expected);
        assert_eq!(deaths, vec![vec![false, true], vec![true, true]]);
        assert!(roots.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_observe_preflight_preserves_argument_and_source_profile_refuses_owners() {
    let p = proof(SOURCE);
    let different = proof(&SOURCE.replace("budget: state.budget,", "budget: state.budget + 1,"));
    let a = argument(&p);
    let roots = weak(a.root.as_ref().unwrap());
    let mut fuel = OwnedFrameBudget::new(100).unwrap();
    let rejected = observe_owned_agent_state_v2(a, &different, &mut fuel)
        .err()
        .expect("different retained program");
    assert_eq!(fuel.consumed(), 0);
    assert!(roots.iter().all(|w| w.strong_count() == 1));
    let mut a = rejected.argument;
    a.creator = std::process::id().wrapping_add(1);
    let rejected = observe_owned_agent_state_v2(a, &p, &mut fuel)
        .err()
        .expect("foreign process");
    assert_eq!(fuel.consumed(), 0);
    assert!(roots.iter().all(|w| w.strong_count() == 1));
    drop(rejected);
    assert!(roots.iter().all(|w| w.upgrade().is_none()));
    let source = SOURCE.replace(
        "let view =",
        "let extra = bytes_copy(str_as_bytes(\"x\"));\n let view =",
    );
    let ordinary = hir::resolve(&crate::check(&source, "observe-extra.spx").unwrap()).unwrap();
    let h = compile_owned_frame_helper_v2(&ordinary, &DeclarationId::new("park")).unwrap();
    assert_eq!(
        compile_owned_observe_v2(&h, &DeclarationId::new("observe"))
            .err()
            .unwrap()
            .code,
        "SPX-T303"
    );
}
#[test]
fn owned_frame_v2_observe_canonical_graph_borrow_loans_and_empty_cleanup() {
    let checked = crate::check(SOURCE, "owned-observe.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let again = crate::check(&canonical, "owned-observe.spx").unwrap();
    assert_eq!(crate::format::canonical(&again), canonical);
    assert_eq!(
        crate::graph::to_json(&checked).unwrap(),
        crate::graph::to_json(&again).unwrap()
    );
    let p = proof(SOURCE);
    let f = p.function();
    assert_eq!(f.params[0].ownership, hir::OwnershipMode::Borrow);
    assert_eq!(f.return_type, p.helper().function().params[1].ty);
    assert!(!f.loan_plan.loans.is_empty());
    assert!(f
        .loan_plan
        .loans
        .iter()
        .all(|l| l.origin.root == f.params[0].id
            && l.origin.projections
                == vec![hir::PlaceProjection::Field(DeclarationId::new("state.a"))]));
    assert!(f
        .cleanup_plan
        .exits
        .iter()
        .all(|e| e.finalize_in_order.is_empty()));
    let graph: serde_json::Value =
        serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "observe")
        .unwrap();
    assert_eq!(node["params"][0]["ownership_mode"], "borrow");
    assert_eq!(node["return_type_id"], "observation");
}

#[test]
fn owned_frame_v2_observe_alias_refusal_and_interrupted_cleanup_never_retry() {
    let p = proof(&SOURCE.replace("-> Observation {", "-> Observation ensures false {"));
    let a = argument(&p);
    let roots = weak(a.root.as_ref().unwrap());
    let Value::Record(record) = a.root.as_ref().unwrap() else {
        panic!()
    };
    let alias = Arc::clone(record);
    let mut fuel = OwnedFrameBudget::new(100).unwrap();
    let rejection = observe_owned_agent_state_v2(a, &p, &mut fuel)
        .err()
        .expect("aliased root");
    assert_eq!(fuel.consumed(), 0);
    drop(alias);
    let OwnedObserveStepV2::Failed(f) =
        observe_owned_agent_state_v2(rejection.argument, &p, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
    else {
        panic!()
    };
    let live = std::cell::Cell::new(true);
    let calls = std::cell::Cell::new(0);
    let rejection = settle_failed_owned_observe_v2(
        f,
        || live.get(),
        |_| {
            calls.set(calls.get() + 1);
            live.set(false);
        },
    )
    .err()
    .expect("guard loss");
    assert_eq!(calls.get(), 1);
    assert!(roots[0].upgrade().is_some());
    assert!(roots[1].upgrade().is_none());
    assert!(!root_valid(
        p.helper(),
        rejection.failed.root.as_ref().unwrap()
    ));
    let selected = rejection.failed.failure.clone();
    let rejection = settle_failed_owned_observe_v2(
        rejection.failed,
        || true,
        |_| panic!("no retry observation"),
    )
    .err()
    .expect("partial root cannot retry");
    assert_eq!(rejection.failed.failure, selected);
    drop(rejection);
    assert!(roots.iter().all(|w| w.upgrade().is_none()));
}

#[test]
fn owned_frame_v2_observe_invalid_allocation_namespace_refuses_before_evaluation() {
    let p = proof(SOURCE);
    for allocation in [0, 3, 1] {
        let mut a = argument(&p);
        let roots = weak(a.root.as_ref().unwrap());
        let Value::Record(r) = a.root.as_mut().unwrap() else {
            panic!()
        };
        let r = Arc::get_mut(r).unwrap();
        let Value::Bytes(b) = r.fields.get_mut(&DeclarationId::new("state.a")).unwrap() else {
            panic!()
        };
        b.allocation = allocation; // zero, outside retained namespace, duplicate
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let rejection = observe_owned_agent_state_v2(a, &p, &mut fuel)
            .err()
            .expect("invalid logical allocation");
        assert_eq!(fuel.consumed(), 0);
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        let Value::Record(r) = rejection.argument.root.as_ref().unwrap() else {
            panic!()
        };
        let Value::Bytes(b) = &r.fields[&DeclarationId::new("state.a")] else {
            panic!()
        };
        assert_eq!(
            b.allocation, allocation,
            "rejection preserves original owner facts"
        );
        drop(rejection);
        assert!(roots.iter().all(|w| w.upgrade().is_none()));
    }
}
