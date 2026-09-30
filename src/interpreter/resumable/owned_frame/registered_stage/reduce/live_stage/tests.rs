//! Physical stage metadata only. Existing fixture ACKs bypass pending live
//! Reduce append; these tests cannot establish joined Agent acceptance.
use super::super::super::effect::{with_staged_complete_reduce_v2, with_staged_effect_reduce_v2};
use super::*;

#[test]
fn owned_reduce_live_stage_facts_borrow_full_step_and_exact_compiler_vector_without_release() {
    for complete in [false, true] {
        let exercise = |staged: StagedExecutedOwnedReduceV2<'_>,
                        weak: [std::sync::Weak<[u8]>; 2],
                        outcome: std::sync::Weak<[u8]>,
                        _: &crate::agent_runtime::AgentCancellation,
                        _: &std::path::Path| {
            let binding = staged.inputs.execution.wait();
            let before = (weak[0].strong_count(), outcome.strong_count());
            let facts = staged.live_stage_facts(binding).unwrap();
            assert_eq!(facts.allowance(), 1000);
            assert!(facts.consumed() > 0 && facts.consumed() < facts.allowance());
            assert_eq!(facts.effect_settled(), staged.effect_settled());
            let value = facts.step().expect("passed constructor/ensures full Step");
            v2::validate_owned_reduce_step_v8(&staged.staged.plan, value).unwrap();
            let case = &staged.staged.plan.transfers().cases[staged.staged.case.unwrap()];
            assert_eq!(value["case"], case.case.as_str());
            let expected =
                v2::owned_wait_operations_v8(&staged.staged.plan.transfers().completion_cleanup)
                    .unwrap();
            assert_eq!(facts.operations(), &expected);
            assert_eq!(
                facts.active_flags(),
                case.completion_live_flags
                    .iter()
                    .map(|f| f.0)
                    .collect::<Vec<_>>()
            );
            assert!(facts.cleanup_basis(None).is_err());
            let basis = facts.cleanup_basis(Some(123)).unwrap();
            assert_eq!(basis["kind"], "success");
            assert_eq!(basis["staged"], 123);
            assert_eq!(
                (weak[0].strong_count(), outcome.strong_count()),
                before,
                "read-only facts do not finalize/remint owners"
            );
            assert!(staged.validate_store());
        };
        if complete {
            with_staged_complete_reduce_v2(1000, exercise);
        } else {
            with_staged_effect_reduce_v2(1000, exercise);
        }
    }
}
#[test]
fn owned_reduce_live_stage_facts_retains_initial_and_partial_fuel_obligations() {
    let mut found_partial = false;
    for fuel in 1..32 {
        with_staged_complete_reduce_v2(fuel, |staged, weak, outcome, _, _| {
            if staged.failure().is_none() {
                return;
            }
            let facts = staged
                .live_stage_facts(staged.inputs.execution.wait())
                .unwrap();
            assert!(facts.step().is_none());
            assert_eq!(facts.allowance(), fuel);
            assert!(facts.consumed() <= fuel);
            assert!(
                facts.cleanup_basis(Some(123)).is_err(),
                "failed constructor is never a full Step"
            );
            let basis = facts.cleanup_basis(None).unwrap();
            assert_eq!(basis["status"]["failure"], "fuel_exhausted");
            assert_eq!(
                facts.operations(),
                &v2::owned_wait_operations_v8(&step::actions(&staged.staged)).unwrap()
            );
            assert_eq!(
                facts.active_flags(),
                step::active_flags(&staged.staged)
                    .iter()
                    .map(|f| f.0)
                    .collect::<Vec<_>>()
            );
            assert_eq!(weak[0].strong_count(), 1);
            assert_eq!(outcome.strong_count(), 1);
            if basis["kind"] == "partial_failure" && staged.staged.transferred > 0 {
                found_partial = true;
                assert_eq!(
                    basis["transfer_prefix"].as_array().unwrap().len(),
                    staged.staged.transferred
                );
            }
        });
        if found_partial {
            break;
        }
    }
    assert!(
        found_partial,
        "real charge-after-Bytes-transfer failure required"
    );
}
#[test]
fn owned_reduce_live_stage_facts_refuse_alias_missing_leaf_and_started_settlement() {
    with_staged_complete_reduce_v2(1000, |mut staged, weak, outcome, _, _| {
        let alias = match staged.staged.step.as_ref().unwrap() {
            Value::Variant(root) => root.clone(),
            _ => panic!(),
        };
        assert!(staged
            .live_stage_facts(staged.inputs.execution.wait())
            .is_err());
        drop(alias);
        assert!(staged
            .live_stage_facts(staged.inputs.execution.wait())
            .is_ok());
        let root = match staged.staged.step.as_mut().unwrap() {
            Value::Variant(root) => Arc::get_mut(root).unwrap(),
            _ => panic!(),
        };
        let field = root
            .fields
            .iter()
            .find(|(_, v)| matches!(v, Value::Bytes(_)))
            .map(|(id, _)| id.clone())
            .unwrap();
        let leaf = root.fields.remove(&field).unwrap();
        assert!(staged
            .live_stage_facts(staged.inputs.execution.wait())
            .is_err());
        let Value::Variant(root) = staged.staged.step.as_mut().unwrap() else {
            panic!()
        };
        Arc::get_mut(root).unwrap().fields.insert(field, leaf);
        staged.staged.settlement_started = true;
        assert!(staged
            .live_stage_facts(staged.inputs.execution.wait())
            .is_err());
        assert_eq!(weak[0].strong_count(), 1);
        assert_eq!(outcome.strong_count(), 1);
    });
}
