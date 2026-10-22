//! Complete scalar owner plus complete original schema controls, never proof bypass.
use super::*;
use crate::test_allocation_observer::measure;
fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module selected.schema.family;
@id("family.one") fn one(n:i64)->i64 {n+1}
@id("family.main") fn main()->i64 {one(3)}
@id("family.fill") fn fill(data:own Bytes)->Bytes {data}
@id("family.bytes") fn bytes()->i64 {
 let data=fill(bytes_zeroed(1usize));
 if byte_len(bytes_as_slice(data))==1usize {7}else{0}
}
"#,
            "selected-schema-family.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn legacy(error: expression_facts::errors::EngineError, function: &ResolvedFunction) -> Diagnostic {
    match error {
        expression_facts::errors::EngineError::Owned(error) => error,
        expression_facts::errors::EngineError::Structural(receipt) => receipt.legacy(function),
        expression_facts::errors::EngineError::Fact(receipt) => receipt.legacy(function),
        expression_facts::errors::EngineError::Failed => {
            panic!("fresh owner cannot already have failed")
        }
    }
}
#[test]
fn scalar_selected_schema_keeps_complete_original_schema_and_whole_owner() {
    let program = program();
    let source = primitive_fixture::Context::new(&program);
    for function in &program.functions {
        if !function.id.as_str().starts_with("family.")
            || !crate::hir::is_scalar_resolved_type(&function.return_type)
        {
            continue;
        }
        let mut original = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let old_schema = schema::selected_schema_with_index(&source, function, None).unwrap();
        let selected = schema::selected_paid::selected(&source, function, &mut original)
            .map_err(|e| legacy(e, function))
            .unwrap();
        assert_eq!(selected, old_schema);
        for old in [true, false] {
            let ((result, bytes, work), observed) = measure(|| {
                let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
                let initial = budget.remaining;
                let context =
                    selected_context::Context::new_paid(&source, function, &mut budget, false)
                        .unwrap();
                let result = if old {
                    paid_before::validate_paid_before(&context, function, &mut budget)
                } else {
                    validate_paid(&context, function, &mut budget)
                };
                let result = result.map_err(|e| legacy(e, function));
                (
                    result,
                    budget.physical.as_ref().unwrap().charged(),
                    initial - budget.remaining,
                )
            });
            assert_eq!(
                result,
                Ok(()),
                "whole scalar owner {} old={old}",
                function.id
            );
            eprintln!(
                "selected scalar {} old={old} bytes={bytes} work={work}",
                function.id
            );
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn scalar_selected_schema_hostile_plan_keeps_old_refusal_precedence_and_all_fields() {
    for case in 0..2 {
        let mut program = program();
        let index = program
            .functions
            .iter()
            .position(|f| f.id.as_str() == "family.main")
            .unwrap();
        if case == 0 {
            program.functions[index].cleanup_plan.schema = "hostile.schema";
        } else {
            program.functions[index].loan_plan.schema = "hostile.loan";
        }
        // Mutate the actual held member before borrowing, rather than passing
        // an unheld clone that the unchanged owner check must reject first.
        let source = primitive_fixture::Context::new(&program);
        let forged = &program.functions[index];
        let mut before = ReplayBudget::with_physical(forged, usize::MAX).unwrap();
        let mut after = ReplayBudget::with_physical(forged, usize::MAX).unwrap();
        let a = selected_context::Context::new_paid(&source, forged, &mut before, false).unwrap();
        let b = selected_context::Context::new_paid(&source, forged, &mut after, false).unwrap();
        let original = legacy(
            paid_before::validate_paid_before(&a, forged, &mut before).unwrap_err(),
            forged,
        );
        let current = legacy(validate_paid(&b, forged, &mut after).unwrap_err(), forged);
        assert_eq!(current, original, "first complete refusal case {case}");
    }
}
