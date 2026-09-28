use super::super::compile_owned_agent_wait_v8;
use super::*;
use serde_json::json;

fn plan() -> CheckedOwnedReduceV2 {
    plan_for(false)
}
fn plan_for(language: bool) -> CheckedOwnedReduceV2 {
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx");
    let source = source.replace(
        "    runtime_v1 {",
        "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
    );
    let source = if language {
        source
            .replace(
                "-> Step\n{",
                "-> Step\nrequires budget * 2 > 0\nensures budget / 2 > 0\n{",
            )
            .replace("state.epoch < 2", "state.epoch - 1 < 2")
    } else {
        source
    };
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
    let binding = compile_owned_agent_wait_v8(
        &source,
        std::path::Path::new("reduce-wire.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    super::super::reduce_plan::compile_owned_reduce_v2(&binding).unwrap()
}
fn scalar(ty: &ResolvedType) -> Value {
    if *ty == ResolvedType::Bytes {
        json!({"kind":"bytes","hex":"00"})
    } else {
        assert_eq!(*ty, ResolvedType::I64);
        codec::scalar(&ArgumentValue::Int(1)).unwrap()
    }
}
fn fields<'a>(declared: impl Iterator<Item = (&'a DeclarationId, &'a ResolvedType)>) -> Value {
    Value::Array(
        declared
            .map(|(id, ty)| json!({"identity":id.as_str(),"value":scalar(ty)}))
            .collect(),
    )
}
fn receipt(active: &Value) -> Value {
    json!({"kind":"observed","settlement":"completed","operations":active.as_array().unwrap().iter().map(|v|json!({"operation":v,"outcome":"completed"})).collect::<Vec<_>>()})
}
#[test]
fn owned_reduce_wire_checks_nominal_step_and_all_mapped_target_shapes() {
    let p = plan();
    for mapping in p.mappings() {
        let decl = p
            .helper()
            .program()
            .declarations
            .case_fields(&mapping.case)
            .unwrap();
        let step = json!({"declaration":p.function().return_type.nominal_id().unwrap().as_str(),"case":mapping.case.as_str(),"fields":fields(decl.iter().map(|f|(&f.id,&f.ty)))});
        validate_owned_reduce_step_v8(&p, &step).unwrap();
        let mut hostile = step.clone();
        hostile["fields"][0]["identity"] = json!("wrong.field");
        assert!(validate_owned_reduce_step_v8(&p, &hostile).is_err());
        let target = if mapping.role == "Fail" {
            json!({"kind":"fail","code":i64::MIN})
        } else {
            let declared = p
                .helper()
                .program()
                .declarations
                .record_fields(&mapping.target)
                .unwrap();
            let value = json!({"declaration":mapping.target.as_str(),"fields":fields(declared.iter().map(|f|(&f.id,&f.ty)))});
            match mapping.role {
                "Continue" => json!({"kind":"continue","state":value}),
                "Suspend" => json!({"kind":"suspend","state":value}),
                "Complete" => json!({"kind":"complete","report":value}),
                _ => panic!(),
            }
        };
        validate_owned_reduce_target_v8(&p, mapping.case.as_str(), &target).unwrap();
        let mut hostile = target.clone();
        hostile["extra"] = json!(0);
        assert!(validate_owned_reduce_target_v8(&p, mapping.case.as_str(), &hostile).is_err());
    }
}
#[test]
fn owned_reduce_wire_proves_each_partial_prefix_without_host_count_or_flags() {
    let p = plan();
    for c in &p.transfers().cases {
        for count in 0..=c.fields.len() {
            let actions = &c.failure_by_prefix[count];
            let operations = owned_wait_operations_v8(actions).unwrap();
            let basis = json!({"kind":"partial_failure","status":{"failure":"fuel_exhausted","language_status":null},"constructor":c.constructor.as_str(),"case":c.case.as_str(),"transfer_prefix":c.fields[..count].iter().map(|f|f.at.as_str()).collect::<Vec<_>>(),"active_flags":actions.iter().map(|a|a.guard_flag.0).collect::<Vec<_>>()});
            let facts = validate_owned_reduce_cleanup_v8(&p, &basis, &operations).unwrap();
            assert!(facts.failure().is_some());
            facts
                .validate_receipt(&receipt(facts.active_operations()))
                .unwrap();
            let mut hostile = basis.clone();
            hostile["active_flags"] = json!([u32::MAX]);
            assert!(validate_owned_reduce_cleanup_v8(&p, &hostile, &operations).is_err());
            let mut hostile = basis.clone();
            hostile["constructor"] = json!("other.expression");
            assert!(validate_owned_reduce_cleanup_v8(&p, &hostile, &operations).is_err());
        }
    }
}
#[test]
fn owned_reduce_wire_retains_false_guard_vector_but_observes_only_active_order() {
    let p = plan();
    let mut false_guard = false;
    for c in &p.transfers().cases {
        let basis = json!({"kind":"success","staged":42,"constructor":c.constructor.as_str(),"case":c.case.as_str(),"active_flags":c.completion_live_flags.iter().map(|f|f.0).collect::<Vec<_>>()});
        let operations = owned_wait_operations_v8(&p.transfers().completion_cleanup).unwrap();
        let facts = validate_owned_reduce_cleanup_v8(&p, &basis, &operations).unwrap();
        assert_eq!(facts.operations(), &operations);
        assert_eq!(
            facts.flags(),
            c.completion_live_flags
                .iter()
                .map(|f| f.0)
                .collect::<Vec<_>>()
        );
        facts
            .validate_receipt(&receipt(facts.active_operations()))
            .unwrap();
        if operations.as_array().unwrap().len()
            > facts.active_operations().as_array().unwrap().len()
        {
            false_guard = true;
            assert!(
                facts.validate_receipt(&receipt(&operations)).is_err(),
                "no invented skipped observers"
            );
        }
        let mut wrong = operations.clone();
        wrong.as_array_mut().unwrap().push(json!({}));
        assert!(validate_owned_reduce_cleanup_v8(&p, &basis, &wrong).is_err());
    }
    assert!(
        false_guard,
        "fixture contains an actual compiler false guard"
    );
}
#[test]
fn owned_reduce_wire_observed_receipt_keeps_mixed_first_and_last_failures() {
    let p = plan();
    let ops = owned_wait_operations_v8(&p.transfers().initial_disposal).unwrap();
    let facts=validate_owned_reduce_cleanup_v8(&p,&json!({"kind":"initial_failure","status":{"failure":"fuel_exhausted","language_status":null}}),&ops).unwrap();
    let len = facts.active_operations().as_array().unwrap().len();
    assert!(len >= 2);
    for index in [0, len - 1] {
        let mut observed = receipt(facts.active_operations());
        observed["operations"][index]["outcome"] = json!("failed");
        assert!(facts.validate_receipt(&observed).is_err());
        observed["settlement"] = json!("failed");
        facts.validate_receipt(&observed).unwrap();
        assert_eq!(observed["operations"][index]["outcome"], "failed");
        let mut omitted = observed.clone();
        omitted["operations"].as_array_mut().unwrap().pop();
        assert!(facts.validate_receipt(&omitted).is_err());
    }
}

fn failed_basis(p: &CheckedOwnedReduceV2, kind: &str, status: Value) -> (Value, Value) {
    if kind == "initial_failure" {
        return (
            json!({"kind":kind,"status":status}),
            owned_wait_operations_v8(&p.transfers().initial_disposal).unwrap(),
        );
    }
    let case = p
        .transfers()
        .cases
        .iter()
        .find(|c| {
            p.mappings()
                .iter()
                .any(|m| m.case == c.case && m.role == "Continue")
        })
        .unwrap();
    let (actions, flags, prefix) = if kind == "partial_failure" {
        assert!(!case.fields.is_empty());
        let prefix = case
            .fields
            .iter()
            .take(1)
            .map(|f| f.at.as_str())
            .collect::<Vec<_>>();
        let actions = &case.failure_by_prefix[1];
        (
            actions,
            actions.iter().map(|a| a.guard_flag.0).collect::<Vec<_>>(),
            Some(prefix),
        )
    } else {
        let mut flags = case
            .completion_live_flags
            .iter()
            .map(|f| f.0)
            .collect::<Vec<_>>();
        flags.extend(
            p.transfers()
                .result_disposal
                .iter()
                .filter(|a| a.active_case.as_ref().is_some_and(|c| c.case == case.case))
                .map(|a| a.guard_flag.0),
        );
        (&p.transfers().provisional_failure, flags, None)
    };
    let mut basis = json!({"kind":kind,"status":status,"constructor":case.constructor.as_str(),"case":case.case.as_str(),"active_flags":flags});
    if let Some(prefix) = prefix {
        basis["transfer_prefix"] = json!(prefix);
    }
    (basis, owned_wait_operations_v8(actions).unwrap())
}
fn language(status: crate::conformance::NormalizedStatus) -> Value {
    json!({"failure":"language_failure","language_status":codec::parse(status.to_json().as_bytes(),codec::MAX_CARRIER).unwrap()})
}
#[test]
fn owned_reduce_wire_language_contract_status_matches_actual_phase_and_frozen_fields() {
    use crate::cleanup_plan::ContractPhase;
    let p = plan_for(true);
    for (phase, kind, wrong) in [
        (
            ContractPhase::Requires,
            "initial_failure",
            "provisional_failure",
        ),
        (
            ContractPhase::Ensures,
            "provisional_failure",
            "initial_failure",
        ),
    ] {
        let status = language(crate::conformance::NormalizedStatus::contract(phase));
        let (basis, ops) = failed_basis(&p, kind, status.clone());
        validate_owned_reduce_cleanup_v8(&p, &basis, &ops).unwrap();
        let (basis, ops) = failed_basis(&p, wrong, status.clone());
        assert!(validate_owned_reduce_cleanup_v8(&p, &basis, &ops).is_err());
        let (basis, ops) = failed_basis(&p, "partial_failure", status.clone());
        assert!(validate_owned_reduce_cleanup_v8(&p, &basis, &ops).is_err());
        for field in ["schema", "domain", "code", "class", "retryable"] {
            let mut status = status.clone();
            status["language_status"][field] = json!("reminted");
            let (basis, ops) = failed_basis(&p, kind, status);
            assert!(
                validate_owned_reduce_cleanup_v8(&p, &basis, &ops).is_err(),
                "{field}"
            );
        }
    }
}
#[test]
fn owned_reduce_wire_arithmetic_status_reaches_only_actual_expression_segment() {
    use crate::cleanup_plan::StatusProducer;
    let p = plan_for(true);
    let mut phases = [false; 3];
    for source in &p.function().cleanup_plan.status_sources {
        let StatusProducer::CheckedArithmetic {
            normalized_cases, ..
        } = &source.producer
        else {
            continue;
        };
        for code in normalized_cases {
            for (index, kind) in ["initial_failure", "partial_failure", "provisional_failure"]
                .iter()
                .enumerate()
            {
                let (basis, ops) = failed_basis(
                    &p,
                    kind,
                    language(crate::conformance::NormalizedStatus::arithmetic(*code)),
                );
                let reachable = arithmetic_reachable(&p, &basis, &source.id.expression);
                let accepted = validate_owned_reduce_cleanup_v8(&p, &basis, &ops).is_ok();
                // Each phase uses a distinct opcode in this fixture: multiply /
                // subtract before the constructor, add after Bytes transfer,
                // divide only in Ensures. No equivalent-code alternative site.
                assert_eq!(accepted, reachable, "{kind} {:?}", source.id.expression);
                if reachable {
                    phases[index] = true;
                }
            }
        }
    }
    assert_eq!(
        phases, [true; 3],
        "nonvacuous initial/partial/provisional arithmetic"
    );
}
