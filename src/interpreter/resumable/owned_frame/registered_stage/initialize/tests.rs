use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::{
    compile_owned_frame_helper_v2, compile_owned_initialize_v2, compile_owned_observe_v2,
};
use std::sync::Weak;
const SOURCE: &str = r#"
module owned.initialize;
@id("task") record Task {
 @id("task.a") first: Bytes,
 @id("task.z") second: Bytes,
 @id("task.budget") budget: i64,
}
@id("state") record State {
 @id("state.z") first: Bytes,
 @id("state.a") second: Bytes,
 @id("state.budget") budget: i64,
}
@id("observation") record Observation { @id("observation.budget") budget: i64, @id("observation.length") length: usize, }
@id("proposal") record Proposal { @id("proposal.budget") budget: i64, }
@id("park") fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
 let proposal = yield observation;
 state
}
@id("initialize") fn initialize(task: own Task) -> State {
 State { first: task.first, budget: task.budget + 2, second: task.second }
}
@id("observe") fn observe(state: borrow State) -> Observation {
 let length = match borrow state {
  State { first, second: _, budget: _ } => { let view = bytes_as_slice(first); byte_len(view) },
 };
 Observation { budget: state.budget, length: length }
}
@id("main") fn main()->i64 { 0 }
"#;
fn proof(source: &str) -> CheckedOwnedInitializeV2 {
    let p = hir::resolve(&crate::check(source, "owned-initialize.spx").unwrap()).unwrap();
    let helper = compile_owned_frame_helper_v2(&p, &DeclarationId::new("park")).unwrap();
    compile_owned_initialize_v2(&helper, &DeclarationId::new("initialize")).unwrap()
}
fn input(budget: i64) -> OwnedFrameInput {
    OwnedFrameInput {
        declaration: DeclarationId::new("task"),
        fields: vec![
            OwnedFrameInputField {
                identity: DeclarationId::new("task.a"),
                value: OwnedFrameInputValue::Bytes(vec![0]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("task.z"),
                value: OwnedFrameInputValue::Bytes(vec![]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("task.budget"),
                value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(budget)),
            },
        ],
    }
}
fn argument(p: &CheckedOwnedInitializeV2, budget: i64) -> OwnedTaskArgumentV2 {
    admit_owned_task_input_v2(p, input(budget)).unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
}
fn weak(task: &OwnedTaskArgumentV2) -> Vec<Weak<[u8]>> {
    let Value::Record(r) = task.root.as_ref().unwrap() else {
        panic!()
    };
    ["task.a", "task.z"]
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
fn owned_frame_v2_initialize_actual_transfer_identity_charges_and_observe() {
    for increment in [2, 3] {
        let p = proof(&SOURCE.replace("task.budget + 2", &format!("task.budget + {increment}")));
        let task = argument(&p, 10);
        let roots = weak(&task);
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let staged = stage_owned_initialize_v2(task, &p, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        assert_eq!(staged.failure(), None);
        assert!(staged.provisional);
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        let OwnedInitializeSettledV2::Initialized(state) = settle_owned_initialize_v2(
            staged,
            || true,
            |_| panic!("compiler success vector is empty"),
        )
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic)) else {
            panic!()
        };
        let Value::Record(r) = state.root.as_ref().unwrap() else {
            panic!()
        };
        let Value::Bytes(first) = &r.fields[&DeclarationId::new("state.z")] else {
            panic!()
        };
        let Value::Bytes(second) = &r.fields[&DeclarationId::new("state.a")] else {
            panic!()
        };
        assert!(Arc::ptr_eq(&roots[0].upgrade().unwrap(), &first.bytes));
        assert!(Arc::ptr_eq(&roots[1].upgrade().unwrap(), &second.bytes));
        assert_eq!((first.allocation, second.allocation), (1, 2));
        assert_eq!(
            r.fields[&DeclarationId::new("state.budget")],
            Value::Int(10 + increment)
        );
        use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue};
        let args = [RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("task"),
            fields: vec![
                RetainedField {
                    field: DeclarationId::new("task.a"),
                    value: RetainedValue::Bytes(vec![0]),
                },
                RetainedField {
                    field: DeclarationId::new("task.z"),
                    value: RetainedValue::Bytes(vec![]),
                },
                RetainedField {
                    field: DeclarationId::new("task.budget"),
                    value: RetainedValue::I64(10),
                },
            ],
        })];
        let prep = crate::interpreter::retained_call::prepare_retained_call(
            p.helper().program(),
            "initialize",
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
        let observe = compile_owned_observe_v2(p.helper(), &DeclarationId::new("observe")).unwrap();
        let super::super::observe::OwnedObserveStepV2::Observed(observed) =
            super::super::observe::observe_owned_agent_state_v2(
                state,
                &observe,
                &mut OwnedFrameBudget::new(100).unwrap(),
            )
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
        else {
            panic!()
        };
        let ResumableChannelValue::Record { fields, .. } = observed.observation() else {
            panic!()
        };
        assert!(
            matches!(fields.as_slice(), [ArgumentValue::Int(v),ArgumentValue::Usize(1)] if *v == 10 + increment)
        );
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        drop(observed);
        assert!(roots.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn owned_frame_v2_initialize_false_contracts_cancel_and_arithmetic_keep_real_owners() {
    for mode in ["requires", "ensures", "cancel", "arithmetic"] {
        let source = match mode {
            "requires" => SOURCE.replace("-> State {", "-> State requires false {"),
            "ensures" => SOURCE.replace("-> State {", "-> State ensures false {"),
            _ => SOURCE.to_owned(),
        };
        let p = proof(&source);
        let task = argument(&p, if mode == "arithmetic" { i64::MAX } else { 10 });
        let roots = weak(&task);
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        if mode == "cancel" {
            fuel.cancel();
        }
        let staged = stage_owned_initialize_v2(task, &p, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        let selected = staged.failure().cloned().expect("actual failure");
        assert_eq!(staged.provisional, mode == "ensures");
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        if mode == "arithmetic" {
            assert_eq!(staged.transferred, 1);
        }
        assert_eq!(fuel.consumed() == 0, mode == "cancel");
        let expected = if staged.provisional {
            p.transfers().result_disposal.clone()
        } else {
            p.transfers().failure_by_prefix[staged.transferred].clone()
        };
        let mut seen = vec![];
        let mut deaths = vec![];
        let OwnedInitializeSettledV2::Failed {
            failure,
            operations,
            observations_succeeded,
        } = settle_owned_initialize_v2(
            staged,
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
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
        else {
            panic!()
        };
        assert_eq!(failure, selected);
        assert_eq!(operations, expected);
        assert_eq!(seen, expected);
        assert!(observations_succeeded);
        let first = if mode == "arithmetic" {
            vec![true, false]
        } else {
            vec![false, true]
        };
        assert_eq!(deaths, vec![first, vec![true, true]]);
    }
}
#[test]
fn owned_frame_v2_initialize_mid_constructor_fuel_and_guard_loss_never_double_drop() {
    let p = proof(SOURCE);
    let mut found = false;
    for limit in 1..30 {
        let task = argument(&p, 10);
        let roots = weak(&task);
        let staged =
            stage_owned_initialize_v2(task, &p, &mut OwnedFrameBudget::new(limit).unwrap())
                .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        if staged.transferred != 1
            || staged.provisional
            || staged.failure() != Some(&OwnedFrameFailure::FuelExhausted)
        {
            continue;
        }
        found = true;
        assert!(roots.iter().all(|w| w.strong_count() == 1));
        let live = std::cell::Cell::new(true);
        let calls = std::cell::Cell::new(0);
        let rejection = settle_owned_initialize_v2(
            staged,
            || live.get(),
            |_| {
                calls.set(calls.get() + 1);
                live.set(false);
            },
        )
        .err()
        .expect("authority loss");
        assert_eq!(calls.get(), 1);
        assert!(roots[0].upgrade().is_none());
        assert!(roots[1].upgrade().is_some());
        let rejection = settle_owned_initialize_v2(
            rejection.staged,
            || true,
            |_| panic!("no repeated release"),
        )
        .err()
        .expect("started cleanup cannot retry");
        assert_eq!(
            rejection.staged.failure(),
            Some(&OwnedFrameFailure::FuelExhausted)
        );
        drop(rejection);
        assert!(roots.iter().all(|w| w.upgrade().is_none()));
        break;
    }
    assert!(found, "real partial ownership fuel window reached");
}
#[test]
fn owned_frame_v2_initialize_compiler_prefix_graph_roundtrip_and_hostile_flags() {
    let checked = crate::check(SOURCE, "owned-initialize.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let again = crate::check(&canonical, "owned-initialize.spx").unwrap();
    assert_eq!(crate::format::canonical(&again), canonical);
    assert_eq!(
        crate::graph::to_json(&checked).unwrap(),
        crate::graph::to_json(&again).unwrap()
    );
    let p = proof(SOURCE);
    assert_eq!(p.transfers().fields.len(), 2);
    assert_eq!(p.transfers().fields[0].field_index, 0);
    assert_eq!(p.transfers().fields[1].field_index, 2);
    let actual = p
        .function()
        .cleanup_plan
        .exits
        .iter()
        .find(|e| e.finalize_in_order == p.transfers().failure_by_prefix[1])
        .expect("ordinary arithmetic failure has actual partial vector");
    assert!(matches!(
        actual.continuation,
        crate::cleanup_plan::ExitContinuation::ReturnFailure { .. }
    ));
    let graph: serde_json::Value =
        serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "initialize")
        .unwrap();
    assert_eq!(node["params"][0]["ownership_mode"], "own");
    assert_eq!(node["return_type_id"], "nominal:5:state:0:");
    let mut hostile = p.helper().program().clone();
    let f = hostile
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "initialize")
        .unwrap();
    f.cleanup.flags[0].lifecycle = DeclarationId::new("forged.drop");
    assert!(crate::cleanup_plan::owned_record_transfer_plan(
        &hostile.declarations,
        f,
        p.constructor()
    )
    .is_err());
}

#[test]
fn owned_frame_v2_initialize_preflight_preserves_task_input_and_rejects_extra_owner() {
    let p = proof(SOURCE);
    let mut wrong = input(10);
    wrong.fields[2].value = OwnedFrameInputValue::Scalar(ArgumentValue::Bool(true));
    let rejection = admit_owned_task_input_v2(&p, wrong)
        .err()
        .expect("wrong scalar");
    assert!(matches!(
        rejection.input.fields[2].value,
        OwnedFrameInputValue::Scalar(ArgumentValue::Bool(true))
    ));
    assert!(
        matches!(&rejection.input.fields[0].value, OwnedFrameInputValue::Bytes(b) if b == &[0])
    );
    let task = argument(&p, 10);
    let roots = weak(&task);
    let different = proof(&SOURCE.replace("task.budget + 2", "task.budget + 3"));
    let mut fuel = OwnedFrameBudget::new(100).unwrap();
    let rejection = stage_owned_initialize_v2(task, &different, &mut fuel)
        .err()
        .expect("different source proof");
    assert_eq!(fuel.consumed(), 0);
    assert!(roots.iter().all(|w| w.strong_count() == 1));
    drop(rejection);
    assert!(roots.iter().all(|w| w.upgrade().is_none()));
    let extra = SOURCE.replace(
        "State { first: task.first",
        "let seed = [1u8];\n let extra = bytes_copy(array_as_slice(seed));\n State { first: task.first",
    );
    let ordinary = hir::resolve(&crate::check(&extra, "initialize-extra.spx").unwrap()).unwrap();
    let helper = compile_owned_frame_helper_v2(&ordinary, &DeclarationId::new("park")).unwrap();
    assert_eq!(
        compile_owned_initialize_v2(&helper, &DeclarationId::new("initialize"))
            .err()
            .unwrap()
            .code,
        "SPX-T303"
    );
}
