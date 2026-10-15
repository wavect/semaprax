//! Final constructor, real Legacy case coordinates, and complete old receivers.
use super::{extend, fixture, Facts};
use crate::hir::{
    DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, LinkedDeclarationFact,
    LinkedOwnedDataParts, LinkedScalarFunction, NominalPayload, ResolvedFunction, ResolvedProgram,
    ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
};
use crate::test_allocation_observer::{current_metrics, measure};

fn main_function() -> ResolvedFunction {
    let ast = crate::check(
        "module final.legacy; @id(\"app.main\") fn main()->i64 {0}",
        "final-legacy.spx",
    )
    .unwrap();
    crate::hir::resolve(&ast).unwrap().functions.remove(0)
}
fn final_link(
    types: Vec<ResolvedTypeDeclaration>,
    mut facts: Facts,
    function: ResolvedFunction,
    old: bool,
) -> Result<ResolvedProgram, crate::diagnostic::Diagnostic> {
    facts.insert(
        function.id.clone(),
        LinkedDeclarationFact {
            kind: DeclarationKind::Function,
            origin: IdentityOrigin::Explicit,
            owner: None,
        },
    );
    let entry = function.id.clone();
    let functions = vec![LinkedScalarFunction {
        function,
        origin: IdentityOrigin::Explicit,
    }];
    let parts = LinkedOwnedDataParts {
        permits: Vec::new(),
        types,
        interfaces: Vec::new(),
        declaration_facts: facts,
        function_templates: Vec::new(),
        function_instances: Vec::new(),
    };
    if old {
        crate::hir::workspace_link::legacy_nominal_before::
            link_owned_data_api_workspace_before_legacy_coordinates(
                "final.legacy".to_owned(), entry, functions, parts,
            )
    } else {
        crate::hir::workspace_link::link_owned_data_api_workspace(
            "final.legacy".to_owned(),
            entry,
            functions,
            parts,
        )
    }
}
fn left_case(types: &[ResolvedTypeDeclaration]) -> &crate::hir::ResolvedVariantCaseDeclaration {
    types
        .iter()
        .find_map(|ty| match &ty.kind {
            ResolvedTypeDeclarationKind::Variant { cases } if ty.id.as_str() == "choice.type" => {
                assert!(cases.is_legacy_shared() && !cases.is_shared());
                Some(&cases[0])
            }
            _ => None,
        })
        .unwrap()
}
#[test]
fn legacy_final_constructor_preserves_complete_old_hir_wire_and_mutation_isolation() {
    let (types, facts) = fixture(false);
    let function = main_function();
    let old = final_link(types.clone(), facts.clone(), function.clone(), true).unwrap();
    let mut current = final_link(types.clone(), facts, function, false).unwrap();
    assert_eq!(current, old);
    assert_eq!(
        crate::cache_codec::encode(&current).unwrap(),
        crate::cache_codec::encode(&old).unwrap()
    );
    let decoded: ResolvedProgram =
        crate::cache_codec::decode(&crate::cache_codec::encode(&current).unwrap()).unwrap();
    assert_eq!(decoded, old);
    assert_eq!(
        crate::graph::to_hir_json(&current, "final-legacy-revision").unwrap(),
        crate::graph::to_hir_json(&old, "final-legacy-revision").unwrap()
    );
    let id = DeclarationId::new("choice.left");
    let ptr = left_case(&types).fields.as_ptr();
    assert_eq!(ptr, current.declarations.case_fields(&id).unwrap().as_ptr());
    assert_eq!(ptr, left_case(&current.types).fields.as_ptr());
    assert_ne!(ptr, old.declarations.case_fields(&id).unwrap().as_ptr());
    let sibling = current.clone();
    current.declarations.case_fields.get_mut(&id).unwrap()[0]
        .name
        .push_str(".index");
    assert_eq!(left_case(&types).fields[0].name, "x");
    assert_eq!(left_case(&current.types).fields[0].name, "x");
    assert_eq!(sibling.declarations.case_fields(&id).unwrap()[0].name, "x");
    let choice = current
        .types
        .iter_mut()
        .find(|ty| ty.id.as_str() == "choice.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut choice.kind else {
        panic!("variant")
    };
    for case in cases {
        for field in &mut case.fields {
            field.name.push_str(".hir");
        }
    }
    assert_eq!(
        current.declarations.case_fields(&id).unwrap()[0].name,
        "x.index"
    );
    assert_eq!(left_case(&sibling.types).fields[0].name, "x");
    assert_eq!(old.declarations.case_fields(&id).unwrap()[0].name, "x");
}
#[test]
fn legacy_final_constructor_retains_all_original_fact_errors_and_partial_map_order() {
    let (types, facts) = fixture(false);
    for scalar in [false, true] {
        for mutation in 0..5 {
            let mut supplied = facts.clone();
            match mutation {
                0 => {
                    supplied.remove(&DeclarationId::new("choice.x"));
                }
                1 => {
                    supplied
                        .get_mut(&DeclarationId::new("choice.x"))
                        .unwrap()
                        .kind = DeclarationKind::Function;
                }
                2 => {
                    supplied
                        .get_mut(&DeclarationId::new("choice.x"))
                        .unwrap()
                        .owner = None;
                }
                3 => {
                    supplied.remove(&DeclarationId::new("choice.right"));
                    supplied.remove(&DeclarationId::new("choice.x"));
                }
                _ => {
                    supplied.get_mut(&DeclarationId::new("row.x")).unwrap().kind =
                        DeclarationKind::Function;
                    supplied.remove(&DeclarationId::new("choice.x"));
                }
            }
            let (old, old_error) = extend(&types, &supplied, true, scalar);
            let (current, error) = extend(&types, &supplied, false, scalar);
            let mut before = DeclarationIndex::default();
            let before_error = if scalar {
                before.extend_linked_scalar_data_before_legacy_coordinates(
                    &types,
                    &[],
                    &[],
                    &[],
                    &supplied,
                )
            } else {
                before.extend_linked_owned_data_before_legacy_coordinates(
                    &types,
                    &[],
                    &[],
                    &[],
                    &supplied,
                )
            };
            assert_eq!(
                error, before_error,
                "complete immediate predecessor diagnostics"
            );
            assert_eq!(
                current, before,
                "complete immediate predecessor partial maps"
            );
            assert!(error.is_err());
            assert_eq!(
                error, old_error,
                "complete six fields and competing first failure"
            );
            assert_eq!(current, old, "all partial facts after exact first failure");
            assert_eq!(
                crate::cache_codec::encode(&current).unwrap(),
                crate::cache_codec::encode(&old).unwrap()
            );
            if !scalar {
                let old = final_link(types.clone(), supplied.clone(), main_function(), true);
                let current = final_link(types.clone(), supplied, main_function(), false);
                assert_eq!(
                    current, old,
                    "actual complete finalizer error, not only index helper"
                );
            }
        }
    }
}
#[test]
fn legacy_final_constructor_observes_real_owned_baseline_and_whole_drop_both_orders() {
    // The process prelude is warm before each symmetric constructor-only window.
    crate::prelude::declarations();
    let (types, facts) = fixture(false);
    let wire = crate::cache_codec::encode(&types).unwrap();
    let owned: Vec<ResolvedTypeDeclaration> = crate::cache_codec::decode(&wire).unwrap();
    let function = main_function();
    let mut costs = Vec::new();
    for input in [&owned, &types] {
        for old in [true, false] {
            let ((), observed) = measure(|| {
                let linked = final_link((*input).clone(), facts.clone(), function.clone(), old);
                assert!(linked.is_ok());
                drop(linked);
            });
            // Isolate this no-payer index receiver from the finalizer's other
            // independently metered validation/replay operations.
            let ((), refused, used, first) =
                crate::bounded_output::with_limit_usage_refusal(0, || {
                    let (index, result) = extend(input, &facts, old, false);
                    assert!(result.is_ok());
                    drop(index);
                });
            assert!(!refused && first.is_none());
            assert_eq!(
                used, 0,
                "explicit unfunded receiver, never ambient paid authority"
            );
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            costs.push(observed);
        }
    }
    assert_eq!(
        costs[0].allocated, costs[1].allocated,
        "genuine decoded Owned branch is unchanged"
    );
    assert!(
        costs[3].allocated < costs[2].allocated,
        "same genuine Legacy input, whole original receiver observed: {costs:?}"
    );
    eprintln!("final-link-owned-before/current-legacy-before/current={costs:?}");
    for input_first in [false, true] {
        let ((), whole) = measure(|| {
            let (types, facts) = fixture(false);
            let mut current = final_link(types.clone(), facts, main_function(), false).unwrap();
            let sibling = current.clone();
            let id = DeclarationId::new("choice.left");
            current.declarations.case_fields.get_mut(&id).unwrap()[0].ty = ResolvedType::Bool;
            if input_first {
                drop(types);
                drop(current);
                drop(sibling);
            } else {
                drop(sibling);
                drop(current);
                drop(types);
            }
        });
        assert!(!whole.invalid);
        assert_eq!(whole.live, 0);
        assert_eq!(whole.allocated, whole.freed);
    }
}
#[test]
fn legacy_final_case_coordinates_and_detached_children_drop_512_without_new_backing() {
    crate::prelude::declarations();
    for input_first in [false, true] {
        let ((), whole) = measure(|| {
            let (mut types, facts) = fixture(false);
            let cases = types
                .iter_mut()
                .find_map(|ty| match &mut ty.kind {
                    ResolvedTypeDeclarationKind::Variant { cases } => Some(cases),
                    _ => None,
                })
                .unwrap();
            let mut ty = ResolvedType::I64;
            for _ in 0..512 {
                ty = ResolvedType::Nominal {
                    declaration: DeclarationId::new("synthetic.depth"),
                    arguments: vec![ty],
                };
            }
            cases[0].fields[0].ty = ty;
            let replacement = NominalPayload::from_legacy_vec_unfunded(&mut **cases);
            *cases = replacement;
            let (mut current, result) = extend(&types, &facts, false, false);
            assert!(result.is_ok());
            let sibling = current.clone();
            let id = DeclarationId::new("choice.left");
            assert_eq!(
                current.case_fields(&id).unwrap().as_ptr(),
                left_case(&types).fields.as_ptr()
            );
            // This real CaseFields mutation detaches a deep independent Owned child.
            current.case_fields.get_mut(&id).unwrap()[0]
                .name
                .push_str(".detached");
            assert_eq!(sibling.case_fields(&id).unwrap()[0].name, "x");
            let mut cursor = &sibling.case_fields(&id).unwrap()[0].ty;
            let mut depth = 0;
            while let ResolvedType::Nominal { arguments, .. } = cursor {
                depth += 1;
                cursor = &arguments[0];
            }
            assert_eq!(depth, 512);
            let before = current_metrics();
            if input_first {
                drop(types);
                drop(current);
                drop(sibling);
            } else {
                drop(sibling);
                drop(current);
                drop(types);
            }
            drop(facts);
            assert_eq!(
                current_metrics().allocated,
                before.allocated,
                "complete last-owner Drop allocates nothing"
            );
        });
        assert!(!whole.invalid);
        assert_eq!(whole.live, 0);
        assert_eq!(whole.allocated, whole.freed);
    }
}
