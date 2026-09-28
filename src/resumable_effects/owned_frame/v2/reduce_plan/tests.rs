use super::*;
use crate::resumable_effects::owned_frame::v2::compile_owned_agent_wait_v8;
use std::path::Path;

fn source(terminal: &str) -> String {
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx");
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
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    )
}
fn binding(source: &str) -> CheckedOwnedAgentWaitBindingV8 {
    compile_owned_agent_wait_v8(
        source,
        Path::new("owned-reduce.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap_or_else(|d| panic!("{d:?}"))
}
#[test]
fn owned_frame_v2_reduce_plan_uses_actual_agent_role_maps_and_guarded_vectors() {
    for terminal in [
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch + 1 }",
        "Step::Fail { code: outcome.status }",
    ] {
        let source = source(terminal);
        let b = binding(&source);
        let p = compile_owned_reduce_v2(&b).unwrap_or_else(|d| panic!("terminal={terminal}: {d:?}"));
        assert_eq!(p.binding(), b.binding());
        assert!(p.helper().same_helper(b.helper()));
        assert_eq!(p.function().id.as_str(), "fixture.agent.fn.reduce");
        assert_eq!(p.mappings().len(), 4);
        let step = b.lifecycle().owned_wait_step_v8();
        for (actual, original) in p.mappings().iter().zip(step.cases()) {
            assert_eq!(actual.case, *original.id);
            assert_eq!(actual.role, original.role);
            assert_eq!(actual.fields, original.fields);
            assert_eq!(actual.target, if actual.role == "Complete" { step.result.clone() } else { step.state.clone() });
        }
        let commit = p.function().cleanup_plan.exits.iter().find(|e| matches!(e.continuation, crate::cleanup_plan::ExitContinuation::CommitResult { .. })).unwrap();
        assert_eq!(p.transfers().completion_cleanup, commit.finalize_in_order);
        assert!(p.transfers().result_disposal.iter().all(|a| a.active_case.is_some() && a.source.storage == crate::cleanup_plan::StorageId::ProvisionalResult));
        for case in &p.transfers().cases {
            assert_eq!(case.failure_by_prefix.len(), case.fields.len()+1);
            for transfer in &case.fields {
                assert_eq!(transfer.destination.projections[0], case.case);
                assert!(p.function().cleanup_plan.blocks.iter().flat_map(|b| &b.transitions).any(|t| matches!(t, crate::cleanup_plan::CleanupTransition::Transfer { at, source, destination } if *at == transfer.at && *source == transfer.source && *destination == transfer.destination)));
            }
        }
    }
}
#[test]
fn owned_frame_v2_reduce_plan_canonical_graph_and_compiler_flag_substitution() {
    let source = source(
        "Step::Complete { summary: outcome.value, budget: state.budget, status: outcome.status }",
    );
    let checked = crate::check(&source, "owned-reduce.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let again = crate::check(&canonical, "owned-reduce.spx").unwrap();
    assert_eq!(crate::format::canonical(&again), canonical);
    assert_eq!(
        crate::graph::to_json(&checked).unwrap(),
        crate::graph::to_json(&again).unwrap()
    );
    let b = binding(&source);
    let p = compile_owned_reduce_v2(&b).unwrap();
    let graph: serde_json::Value =
        serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "fixture.agent.fn.reduce")
        .unwrap();
    assert_eq!(node["params"][0]["ownership_mode"], "own");
    assert_eq!(node["params"][4]["ownership_mode"], "own");
    assert_eq!(
        node["return_type_id"],
        p.function().return_type.identity_key()
    );
    for wrong_edge in [false, true] {
        let mut hostile = b.helper().program().clone();
        let f = hostile
            .functions
            .iter_mut()
            .find(|f| f.id == p.function().id)
            .unwrap();
        if wrong_edge {
            let edge = f
                .cleanup_plan
                .blocks
                .iter_mut()
                .flat_map(|b| &mut b.transitions)
                .find(|t| {
                    matches!(
                        t,
                        crate::cleanup_plan::CleanupTransition::TransferVariant { .. }
                    )
                })
                .unwrap();
            let crate::cleanup_plan::CleanupTransition::TransferVariant { variant, .. } = edge
            else {
                unreachable!()
            };
            *variant = DeclarationId::new("forged.step");
        } else {
            f.cleanup.flags[0].lifecycle = DeclarationId::new("forged.drop");
        }
        let mut constructors = Vec::new();
        body(&f.body, &mut constructors).unwrap();
        assert!(owned_step_transfer_plan(&hostile.declarations, f, &constructors).is_err());
    }
}
#[test]
fn owned_frame_v2_reduce_plan_refuses_valid_extra_owner_and_owner_constructor() {
    let source = source(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
    );
    for source in [
        source.replace(
            "    if state.epoch < 2",
            "    let extra = bytes_zeroed(1usize);\n    if state.epoch < 2",
        ),
        source.replace(
            "objective: state.objective",
            "objective: bytes_zeroed(1usize)",
        ),
    ] {
        let b = binding(&source); // ordinary source + existing Agent ABI positive control
        assert_eq!(compile_owned_reduce_v2(&b).err().unwrap().code, "SPX-T303");
    }
}
