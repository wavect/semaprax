//! Varying generic censuses avoid intermediate fully charged allocations.
use super::*;
use crate::bounded_output;

fn fixture() -> (Vec<Program>, Vec<hir::ResolvedFunctionInstance>) {
    let mut source = String::from("module generic.preflight;\n");
    for (name, size) in [("small", 1), ("medium", 5), ("large", 17)] {
        source.push_str(&format!("@id(\"{name}\") fn {name}<T>(value:T)->T {{\n"));
        for index in 0..size {
            source.push_str(&format!("let text{index}=\"owned\"; let view{index}=string_as_str(text{index}); let size{index}=string_len(view{index});\n"));
        }
        source.push_str("value }\n");
    }
    source.push_str(
        "@id(\"main\") fn main()->i64 { let a=small<i64>(1); let b=medium<i64>(a); large<i64>(b) }",
    );
    let program = crate::check(&source, "generic-preflight.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    assert_eq!(resolved.function_instances.len(), 3);
    assert!(resolved
        .function_instances
        .iter()
        .all(|i| !i.function.loan_plan.loans.is_empty()));
    (vec![program], resolved.function_instances)
}
#[test]
fn selected_generic_maximum_reuses_one_carrier_and_preserves_wire() {
    let (programs, mut instances) = fixture();
    instances.sort_by_key(|instance| instance.function.loan_plan.endpoints.len());
    let authored = super::super::super::index_authored(&programs).unwrap();
    let wires: Vec<_> = instances
        .iter()
        .map(|i| crate::cache_codec::encode(&i.function.loan_plan).unwrap())
        .collect();
    let run = || {
        let mut scratch = RetentionScratch::default();
        super::super::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances.clone(),
            Some(&mut scratch),
        )
    };
    let (result, overflow, exact) = bounded_output::with_limit_usage(usize::MAX, run);
    assert!(!overflow);
    let (retained, imported) = result.unwrap();
    assert!(imported.is_empty());
    assert_eq!(
        retained
            .iter()
            .map(|i| crate::cache_codec::encode(&i.function.loan_plan).unwrap())
            .collect::<Vec<_>>(),
        wires
    );
    let (progressive, overflow, progressive_bytes) =
        bounded_output::with_limit_usage(usize::MAX, || {
            let mut scratch = RetentionScratch::default();
            for instance in &instances {
                super::super::retain_module_instances(
                    &programs[0],
                    &programs,
                    &authored,
                    vec![instance.clone()],
                    Some(&mut scratch),
                )?;
            }
            Ok::<_, Vec<Diagnostic>>(())
        });
    progressive.unwrap();
    assert!(!overflow);
    assert!(
        exact < progressive_bytes,
        "preflight eliminates intermediate replacements"
    );
    let (result, overflow, used) = bounded_output::with_limit_usage(exact, run);
    assert!(result.is_ok());
    assert!(!overflow);
    assert_eq!(used, exact);
    let (result, overflow, _) = bounded_output::with_limit_usage(exact - 1, run);
    assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
    assert!(overflow);
}
#[test]
fn generic_preflight_validates_selected_backing_before_allocating() {
    let (programs, mut instances) = fixture();
    let authored = super::super::super::index_authored(&programs).unwrap();
    let (missing, overflow, _) =
        bounded_output::with_limit_usage(0, || hir::ExpressionId::from_owned(String::new()));
    assert!(overflow);
    instances.last_mut().unwrap().function.loan_plan.endpoints[0]
        .point
        .expression = missing;
    let (result, overflow, used) = bounded_output::with_limit_usage(usize::MAX, || {
        let mut scratch = RetentionScratch::default();
        super::super::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances,
            Some(&mut scratch),
        )
    });
    assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
    assert!(!overflow);
    assert_eq!(used, 0, "complete census preflight precedes allocation");
}

#[test]
fn unselected_instance_cannot_supply_or_require_census_authority() {
    let (programs, mut instances) = fixture();
    let authored = super::super::super::index_authored(&programs).unwrap();
    let (missing, overflow, _) =
        bounded_output::with_limit_usage(0, || hir::ExpressionId::from_owned(String::new()));
    assert!(overflow);
    for instance in &mut instances {
        instance.template = hir::DeclarationId::new("foreign.unselected");
        instance.function.loan_plan.endpoints[0].point.expression = missing.clone();
        assert!(!selected(&programs[0], &programs, &authored, instance));
    }
    let (result, overflow, used) = bounded_output::with_limit_usage(0, || {
        let mut scratch = RetentionScratch::default();
        super::super::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances,
            Some(&mut scratch),
        )
    });
    let (retained, imported) = result.unwrap();
    assert!(retained.is_empty() && imported.is_empty());
    assert!(!overflow);
    assert_eq!(used, 0);
}

#[test]
fn legacy_generic_retention_preserves_the_existing_accounting_path() {
    let (programs, instances) = fixture();
    let authored = super::super::super::index_authored(&programs).unwrap();
    let (actual, overflow, debit) = bounded_output::with_limit_usage(usize::MAX, || {
        super::super::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances.clone(),
            None,
        )
    });
    let (retained, imported) = actual.unwrap();
    assert!(!overflow);
    assert!(imported.is_empty());
    let (original, overflow, reference) = bounded_output::with_limit_usage(usize::MAX, || {
        super::super::super::filter_owned_vec_accounted(
            instances.clone(),
            GRAPH_ACCOUNTED_RESOLVED_FUNCTION_INSTANCE_BYTES,
            |item| retained_function_loan_bytes(&item.function),
            |item| selected(&programs[0], &programs, &authored, item),
            false,
        )
    });
    assert!(!overflow);
    assert_eq!(debit, reference);
    assert_eq!(
        crate::cache_codec::encode(&retained).unwrap(),
        crate::cache_codec::encode(&original.unwrap()).unwrap()
    );
}
