use super::*;
use crate::hir::TypeFacts;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
fn run(
    bytes: usize,
    work: usize,
    index: &DeclarationIndex,
    function: &ResolvedFunction,
    root: &ResolvedType,
    expected: Option<TypeFlags>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "full flag receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        flags(
                            index,
                            root,
                            &mut super::super::tests::payer(function, &mut budget),
                        )
                    });
                let matched = result.as_ref().is_ok_and(|actual| *actual == expected);
                let failed = result.is_err();
                let used = work.saturating_sub(budget.remaining);
                if failed {
                    let before = current_metrics();
                    let retry = flags(
                        index,
                        root,
                        &mut super::super::tests::payer(function, &mut budget),
                    );
                    assert!(retry.is_err());
                    assert_eq!(before.allocated, current_metrics().allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used)
            })
        });
    (outcome, observed, refused, paid)
}
fn control(index: &DeclarationIndex, function: &ResolvedFunction, root: &ResolvedType) {
    let expected = index.type_flags(root);
    let (ample, observed, refused, paid) = run(
        usize::MAX,
        MAX_REPLAY_WORK_UNITS,
        index,
        function,
        root,
        expected,
    );
    assert!(ample.0 && !ample.1 && !refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let (exact, observed, refused, used) = run(paid, ample.2, index, function, root, expected);
    assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
    assert_eq!(used, paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    for (bytes, work) in [
        (0, MAX_REPLAY_WORK_UNITS),
        (paid.saturating_sub(1), MAX_REPLAY_WORK_UNITS),
        (paid, 0),
        (paid, ample.2.saturating_sub(1)),
    ] {
        if bytes == paid && work == ample.2 {
            continue;
        }
        let (short, observed, _, used) = run(bytes, work, index, function, root, expected);
        assert!(short.1 && !observed.invalid && observed.allocated <= used);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
fn nominal(id: DeclarationId, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: id,
        arguments,
    }
}
#[test]
fn complete_flag_only_fold_matches_independent_owned_facts_all_fields_cases_and_receiving_drop() {
    for program in super::super::tests::fixtures() {
        let index = program.declarations.classifier_uncached_for_test();
        let function = &program.functions[0];
        let mut roots = vec![
            ResolvedType::Never,
            ResolvedType::Unit,
            ResolvedType::String,
            ResolvedType::Bytes,
            ResolvedType::MutFunctionI64,
            ResolvedType::ArrayU8(u32::MAX),
        ];
        for item in &program.types {
            roots.push(nominal(item.id.clone(), Vec::new()));
        }
        for f in &program.functions {
            roots.push(f.return_type.clone());
            roots.extend(f.params.iter().map(|p| p.ty.clone()));
        }
        if let Some(id) = index.type_id("GenericChoice") {
            for args in [
                vec![ResolvedType::Bytes],
                vec![ResolvedType::I64],
                vec![ResolvedType::String],
                vec![ResolvedType::Bytes, ResolvedType::Bytes],
            ] {
                roots.push(nominal(id.clone(), args));
            }
        }
        for root in roots {
            let full = index.classifier_old_facts_for_test(&root);
            assert_eq!(index.type_flags(&root), full.as_ref().map(TypeFlags::from));
            control(&index, function, &root);
        }
    }
}
#[test]
fn full_generic_zero_one_seventeen_arguments_functions_and_forged_cache_precedence_keep_old_flags()
{
    let program = super::super::tests::fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let base = program.declarations.classifier_uncached_for_test();
    for width in [0, 1, 17] {
        for root in [
            nominal(
                DeclarationId::new("missing.é\0suffix"),
                vec![
                    ResolvedType::Function {
                        parameters: vec![ResolvedType::I64],
                        result: Box::new(ResolvedType::Bytes)
                    };
                    width
                ],
            ),
            ResolvedType::Function {
                parameters: vec![ResolvedType::I64; width],
                result: Box::new(ResolvedType::I64),
            },
        ] {
            control(&base, function, &root);
            let mut index = base.clone();
            let facts = TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "forged complete ignored layout".to_owned(),
            };
            index.inventory_insert_fact_for_test(&root, facts);
            control(&index, function, &root);
        }
    }
}
#[test]
fn full_cycle_forgery_and_missing_metadata_keep_original_none_and_success_priority() {
    let ast = crate::parse(
        include_str!("../../../../../tests/fixtures/owned-nested-outcome.spx"),
        "full-flags.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = &program.functions[0];
    for index in program.declarations.inventory_nested_forgeries_for_test() {
        let index = index.classifier_uncached_for_test();
        for root in [
            nominal(DeclarationId::new("n.Outcome"), Vec::new()),
            nominal(DeclarationId::new("n.Payload"), Vec::new()),
            nominal(DeclarationId::new("n.Payload"), vec![ResolvedType::Bytes]),
        ] {
            control(&index, function, &root);
        }
    }
}
#[test]
fn complete_original_owned_facts_compute_and_builtin_bodies_remain_independent_byte_exact() {
    assert_eq!(
        include_str!("../../../../hir/declaration_index/flags/paid/compute.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/flags-paid-compute.rs.before")
    );
    assert_eq!(
        include_str!("../../../../hir/declaration_index/flags/paid/builtin.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/flags-paid-builtin.rs.before")
    );
}
