use super::*;
fn plan() -> v2::CheckedOwnedReduceV2 {
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx")
        .replace(
            "    runtime_v1 {",
            "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
        );
    let source = format!(
        "{source}\n{}",
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    );
    let binding = v2::compile_owned_agent_wait_v8(
        &source,
        std::path::Path::new("reduce-inventory.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    v2::compile_owned_reduce_v2(&binding).unwrap()
}
#[test]
fn owned_reduce_inventory_maps_actual_nominal_cases_without_target_substitution() {
    let p = plan();
    for mapping in p.mappings() {
        let fields = p
            .helper()
            .program()
            .declarations
            .case_fields(&mapping.case)
            .unwrap();
        let step = json!({"declaration":p.function().return_type.nominal_id().unwrap().as_str(),
            "case":mapping.case.as_str(),"fields":fields.iter().enumerate().map(|(i,f)|
                json!({"identity":f.id.as_str(),"value":if f.ty==crate::hir::ResolvedType::Bytes {
                    json!({"kind":"bytes","hex":"00ff"})
                } else {json!({"tag":"i64","value":i as i64 - 3})}})).collect::<Vec<_>>()});
        let scope =
            json!({"program_root":"source","invocation":"invocation","policy_epoch":"epoch"});
        let digest = recipe_digest(
            ReduceRecipeV8::Step,
            &json!({"scope":scope,"binding":p.binding(),
            "plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":29,"step":step}),
        )
        .unwrap();
        let checked = checked_step(&p, &scope, 0, 0, 29, &step, &digest).unwrap();
        checked.matches_target(checked.target()).unwrap();
        assert_eq!(checked.step(), &step);
        assert_eq!(checked.case(), mapping.case.as_str());
        let mut hostile = checked.target().clone();
        if mapping.role == "Fail" {
            hostile["code"] = 4.into();
        } else {
            let key = if mapping.role == "Complete" {
                "report"
            } else {
                "state"
            };
            hostile[key]["fields"][0]["value"]["hex"] = "01".into();
        }
        assert!(checked.matches_target(&hostile).is_err());
        assert!(checked_step(&p, &scope, 0, 0, 30, &step, &digest).is_err());
        assert_ne!(
            checked.transfer_digest(&scope, &p, 0, 0, 31).unwrap(),
            checked.transfer_digest(&scope, &p, 0, 0, 32).unwrap()
        );
    }
}
