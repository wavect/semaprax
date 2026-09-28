use super::super::reduce_inventory::tests::plan;
use super::*;

fn receipt(ops: &Value, completed: bool) -> Value {
    json!({"kind":"observed","settlement":if completed {"completed"}else{"failed"},
        "operations":ops.as_array().unwrap().iter().enumerate().map(|(i,o)|json!({"operation":o,
            "outcome":if !completed && i==0 {"failed"}else{"completed"}})).collect::<Vec<_>>()})
}
#[test]
fn owned_reduce_fold_failure_requires_exact_funding_and_whole_active_receipt() {
    let p = plan();
    let scope = json!({});
    let ops = v2::owned_wait_operations_v8(&p.transfers().initial_disposal).unwrap();
    let basis = ReduceBasisV8::InitialFailure {
        status: json!({"failure":"fuel_exhausted","language_status":null}),
    };
    let payload = json!({"scope":scope,"binding":p.binding(),"plan":p.binding(),"turn":0,"attempt":0,
        "stage_reservation":28,"basis":serde_json::to_value(&basis).unwrap()});
    let digest = recipe_digest(ReduceRecipeV8::Basis, &payload).unwrap();
    let mut f = ReduceFoldV8::after_checked_reservation(&p, &scope, 0, 0, 28, 10, 27).unwrap();
    assert!(f
        .cleanup_started(
            &p,
            "forged-plan",
            &scope,
            29,
            0,
            0,
            28,
            27,
            &basis,
            &digest,
            9,
            &ops
        )
        .is_err());
    assert!(f
        .cleanup_started(
            &p,
            p.binding(),
            &scope,
            29,
            0,
            0,
            27,
            27,
            &basis,
            &digest,
            9,
            &ops
        )
        .is_err());
    assert!(f
        .cleanup_started(
            &p,
            p.binding(),
            &scope,
            29,
            0,
            0,
            28,
            27,
            &basis,
            &digest,
            11,
            &ops
        )
        .is_err());
    assert_eq!(
        f.cleanup_started(
            &p,
            p.binding(),
            &scope,
            29,
            0,
            0,
            28,
            27,
            &basis,
            &digest,
            9,
            &ops
        )
        .unwrap(),
        9
    );
    let active = f.cleanup.as_ref().unwrap().active_operations().clone();
    assert!(active.as_array().unwrap().len() >= 2);
    let good = receipt(&active, true);
    assert!(f.cleanup_settled(30, 0, 0, 28, &good).is_err());
    let mut missing = good.clone();
    missing["operations"].as_array_mut().unwrap().pop();
    assert!(f.cleanup_settled(30, 0, 0, 29, &missing).is_err());
    f.cleanup_settled(30, 0, 0, 29, &good).unwrap();
    assert_eq!(f.tail(), ReduceTailV8::FailureCleaned);
    assert_eq!(f.consumed(), Some(9));
    assert_eq!(
        f.failure().unwrap(),
        &json!({"failure":"fuel_exhausted","language_status":null})
    );
}
#[test]
fn owned_reduce_fold_failed_observation_quarantines_without_replacing_failure() {
    let p = plan();
    let scope = json!({});
    let ops = v2::owned_wait_operations_v8(&p.transfers().initial_disposal).unwrap();
    let basis = ReduceBasisV8::InitialFailure {
        status: json!({"failure":"fuel_exhausted","language_status":null}),
    };
    let digest=recipe_digest(ReduceRecipeV8::Basis,&json!({"scope":scope,"binding":p.binding(),
        "plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":28,"basis":serde_json::to_value(&basis).unwrap()})).unwrap();
    let mut f = ReduceFoldV8::after_checked_reservation(&p, &scope, 0, 0, 28, 10, 27).unwrap();
    f.cleanup_started(
        &p,
        p.binding(),
        &scope,
        29,
        0,
        0,
        28,
        27,
        &basis,
        &digest,
        9,
        &ops,
    )
    .unwrap();
    let failed = receipt(f.cleanup.as_ref().unwrap().active_operations(), false);
    f.cleanup_settled(30, 0, 0, 29, &failed).unwrap();
    assert_eq!(f.tail(), ReduceTailV8::Quarantined);
    assert_eq!(f.failure().unwrap()["failure"], "fuel_exhausted");
    assert!(f.cleanup_settled(31, 0, 0, 29, &failed).is_err());
}

#[test]
fn owned_reduce_fold_success_maps_exact_step_and_counts_consumption_once() {
    let p = plan();
    let scope = json!({});
    let mapping = p.mappings().iter().find(|m| m.role == "Continue").unwrap();
    let fields = p
        .helper()
        .program()
        .declarations
        .case_fields(&mapping.case)
        .unwrap();
    // Compiler-shaped inert values: this test does not attest actual evaluation.
    let step = json!({"declaration":p.function().return_type.nominal_id().unwrap().as_str(),
        "case":mapping.case.as_str(),"fields":fields.iter().map(|field|json!({
            "identity":field.id.as_str(),"value":if field.ty==crate::hir::ResolvedType::Bytes {
                json!({"kind":"bytes","hex":"00"})
            } else {json!({"tag":"i64","value":1})}})).collect::<Vec<_>>()});
    let digest = recipe_digest(
        ReduceRecipeV8::Step,
        &json!({"scope":scope,"binding":p.binding(),
        "plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":28,"step":step}),
    )
    .unwrap();
    let mut f = ReduceFoldV8::after_checked_reservation(&p, &scope, 0, 0, 28, 10, 27).unwrap();
    let foreign_scope = json!({"invocation":"foreign"});
    let foreign_digest = recipe_digest(
        ReduceRecipeV8::Step,
        &json!({"scope":foreign_scope,
        "binding":p.binding(),"plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":28,
        "step":step}),
    )
    .unwrap();
    assert!(f
        .staged(
            &p,
            p.binding(),
            &foreign_scope,
            29,
            0,
            0,
            28,
            27,
            &step,
            &foreign_digest,
            7
        )
        .is_err());
    assert!(f
        .staged(&p, p.binding(), &scope, 29, 0, 1, 28, 27, &step, &digest, 7)
        .is_err());
    assert_eq!(
        f.staged(&p, p.binding(), &scope, 29, 0, 0, 28, 27, &step, &digest, 7)
            .unwrap(),
        7
    );
    let c = p
        .transfers()
        .cases
        .iter()
        .find(|c| c.case == mapping.case)
        .unwrap();
    let basis = ReduceBasisV8::Success {
        staged: 29,
        constructor: c.constructor.as_str().into(),
        case: c.case.as_str().into(),
        active_flags: c.completion_live_flags.iter().map(|f| f.0).collect(),
    };
    let value = serde_json::to_value(&basis).unwrap();
    let ops = v2::owned_wait_operations_v8(&p.transfers().completion_cleanup).unwrap();
    let active = v2::validate_owned_reduce_cleanup_v8(&p, &value, &ops).unwrap();
    let (seq, cleanup) = if active.active_operations().as_array().unwrap().is_empty() {
        (30, ReduceCleanupV8::CompilerEmpty)
    } else {
        let digest = recipe_digest(
            ReduceRecipeV8::Basis,
            &json!({"scope":scope,"binding":p.binding(),
            "plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":28,"basis":value}),
        )
        .unwrap();
        assert!(f
            .cleanup_started(
                &p,
                p.binding(),
                &scope,
                30,
                0,
                0,
                28,
                27,
                &basis,
                &digest,
                8,
                &ops
            )
            .is_err());
        assert_eq!(
            f.cleanup_started(
                &p,
                p.binding(),
                &scope,
                30,
                0,
                0,
                28,
                27,
                &basis,
                &digest,
                7,
                &ops
            )
            .unwrap(),
            0
        );
        let receipt = receipt(active.active_operations(), true);
        f.cleanup_settled(31, 0, 0, 30, &receipt).unwrap();
        (
            32,
            ReduceCleanupV8::Observed {
                started: 30,
                settled: 31,
            },
        )
    };
    assert!(f
        .transfer_reserved(
            &p,
            p.binding(),
            seq,
            0,
            0,
            28,
            28,
            &cleanup,
            mapping.case.as_str()
        )
        .is_err());
    f.transfer_reserved(
        &p,
        p.binding(),
        seq,
        0,
        0,
        28,
        29,
        &cleanup,
        mapping.case.as_str(),
    )
    .unwrap();
    let target = f.step.as_ref().unwrap().target().clone();
    let digest = f
        .step
        .as_ref()
        .unwrap()
        .transfer_digest(&scope, &p, 0, 0, seq)
        .unwrap();
    let mut hostile = target.clone();
    hostile["state"]["fields"][0]["value"] = json!({"kind":"bytes","hex":"ff"});
    assert!(f
        .transfer_completed(&p, &scope, seq + 1, 0, 0, seq, &hostile, &digest)
        .is_err());
    f.transfer_completed(&p, &scope, seq + 1, 0, 0, seq, &target, &digest)
        .unwrap();
    assert_eq!(f.tail(), ReduceTailV8::Mapped);
    let bytes = f.step.as_ref().unwrap().ordinary_carrier_bytes(&p).unwrap();
    let carrier =
        crate::live_invocation::identity::digest(b"semaprax.agent-step.value.v2\0", &bytes);
    assert!(f
        .transition(
            &p,
            seq + 2,
            0,
            0,
            super::super::super::SourceTransitionCase::Complete,
            &carrier
        )
        .is_err());
    assert!(f
        .transition(
            &p,
            seq + 2,
            0,
            0,
            super::super::super::SourceTransitionCase::Continue,
            "wrong-digest"
        )
        .is_err());
    f.transition(
        &p,
        seq + 2,
        0,
        0,
        super::super::super::SourceTransitionCase::Continue,
        &carrier,
    )
    .unwrap();
    assert_eq!(f.tail(), ReduceTailV8::Continued);

    assert_eq!(f.consumed(), Some(7));
    assert!(f
        .transfer_completed(&p, &scope, seq + 2, 0, 0, seq, &target, &digest)
        .is_err());
}
