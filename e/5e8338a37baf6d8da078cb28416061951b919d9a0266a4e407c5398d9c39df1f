//! Actual Legacy allocation/COW and immutable funded negative controls.
use super::*;
use crate::ast::Span;
use crate::hir::{DeclarationId, ResolvedFieldDeclaration};
use crate::test_allocation_observer::{current_metrics, measure};
fn field(ty: ResolvedType) -> ResolvedFieldDeclaration {
    ResolvedFieldDeclaration {
        id: DeclarationId::new("field"),
        name: "field".to_owned(),
        index: 0,
        ty,
        span: Span::default(),
    }
}
fn deep(mut ty: ResolvedType) -> ResolvedType {
    for _ in 0..512 {
        ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("nested"),
            arguments: vec![ty],
        };
    }
    ty
}
#[test]
fn legacy_owner_inside_zero_active_scope_is_unfunded_and_mutations_isolate_both_drop_orders() {
    for first in [false, true] {
        let ((), observed) = measure(|| {
            let mut values = vec![field(ResolvedType::I64)];
            let original = values.clone();
            let before = current_metrics();
            let ((), refused, used, first_refusal) =
                crate::bounded_output::with_limit_usage_refusal(0, || {
                    let mut payload = NominalPayload::from_legacy_vec_unfunded(&mut values);
                    assert!(!payload.is_shared());
                    assert!(payload.is_legacy_shared());
                    let sibling = payload.clone();
                    assert!(payload.same_owner(&sibling));
                    let before_clone = current_metrics();
                    let copy = sibling.clone();
                    drop(copy);
                    assert_eq!(current_metrics().allocated, before_clone.allocated);
                    assert_eq!(payload.as_slice(), original.as_slice());
                    payload[0].ty = ResolvedType::Bool;
                    assert!(!payload.has_shared_owner());
                    assert_eq!(sibling[0].ty, ResolvedType::I64);
                    for item in &mut payload {
                        item.name.push_str(".changed");
                    }
                    assert_eq!(sibling[0].name, "field");
                    assert_eq!(payload[0].name, "field.changed");
                    if first {
                        drop(payload);
                        drop(sibling);
                    } else {
                        drop(sibling);
                        drop(payload);
                    }
                });
            assert!(!refused && first_refusal.is_none());
            assert_eq!(used, 0);
            assert!(
                current_metrics().allocated > before.allocated,
                "real explicitly unfunded owner/COW allocations"
            );
            drop(original);
            drop(values);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn legacy_and_funded_explicit_mutation_refusal_never_falls_through_to_implicit_cow() {
    for legacy in [false, true] {
        let mut values = vec![field(ResolvedType::I64)];
        let mut payload = if legacy {
            NominalPayload::from_legacy_vec_unfunded(&mut values)
        } else {
            NominalPayload::from_funded_vec(&mut values, |_| Ok::<_, ()>(())).unwrap()
        };
        let sibling = payload.clone();
        let pointer = payload.as_ptr();
        let ((error, observed), refused, _, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                measure(|| {
                    payload
                        .try_make_mut(|n| {
                            crate::bounded_output::reserve_active_required(n)
                                .then_some(())
                                .ok_or(())
                        })
                        .map(|_| ())
                })
            });
        assert!(error.is_err() && refused && first.is_some());
        assert_eq!(observed.allocated, 0);
        assert!(!observed.invalid);
        assert!(payload.same_owner(&sibling));
        assert_eq!(pointer, payload.as_ptr());
        if !legacy {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                payload[0].ty = ResolvedType::Bool
            }));
            assert!(
                result.is_err(),
                "funded implicit mutation remains forbidden"
            );
            assert!(payload.is_shared());
            assert_eq!(sibling[0].ty, ResolvedType::I64);
        }
        payload.try_make_mut(|_| Ok::<_, ()>(())).unwrap()[0].ty = ResolvedType::Bool;
        assert_eq!(sibling[0].ty, ResolvedType::I64);
        assert!(!payload.has_shared_owner());
    }
    use crate::hir::nominal_payload::map::CaseFields;
    use crate::hir::ResolvedVariantCaseDeclaration;
    let case = ResolvedVariantCaseDeclaration {
        id: DeclarationId::new("case"),
        name: "Case".to_owned(),
        index: 0,
        fields: vec![field(ResolvedType::I64)],
        span: Span::default(),
    };
    let mut values = vec![case.clone()];
    let legacy = NominalPayload::from_legacy_vec_unfunded(&mut values);
    assert!(!legacy.is_shared());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| CaseFields::canonical(
            legacy.clone(),
            0
        )))
        .is_err(),
        "legacy storage must not pass the paid coordinate constructor"
    );
    let mut values = vec![case];
    let funded = NominalPayload::from_funded_vec(&mut values, |_| Ok::<_, ()>(())).unwrap();
    let mut view = CaseFields::canonical(funded, 0);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || view[0].ty = ResolvedType::Bool
        ))
        .is_err(),
        "funded case coordinate mutation remains forbidden"
    );
    assert_eq!(view[0].ty, ResolvedType::I64);
}
#[test]
fn legacy_512_owner_and_host_detached_payloads_keep_iterative_last_drop() {
    for first in [false, true] {
        let ((), observed) = measure(|| {
            let mut fields = vec![field(deep(ResolvedType::I64))];
            let payload = NominalPayload::from_legacy_vec_unfunded(&mut fields);
            let sibling = payload.clone();
            let before = current_metrics();
            if first {
                drop(payload);
                drop(sibling);
            } else {
                drop(sibling);
                drop(payload);
            }
            assert_eq!(
                current_metrics().allocated,
                before.allocated,
                "last legacy owner uses preacquired scratch"
            );
            let mut fields = vec![field(ResolvedType::I64)];
            let mut payload = NominalPayload::from_legacy_vec_unfunded(&mut fields);
            let sibling = payload.clone();
            payload[0].ty = deep(ResolvedType::Function {
                parameters: vec![ResolvedType::I64],
                result: Box::new(ResolvedType::I64),
            });
            assert!(!payload.has_shared_owner());
            assert_eq!(sibling[0].ty, ResolvedType::I64);
            drop(sibling);
            drop(payload);
            let mut fields = vec![field(deep(ResolvedType::Function {
                parameters: vec![ResolvedType::I64, ResolvedType::Bool],
                result: Box::new(ResolvedType::U8),
            }))];
            let mut payload = NominalPayload::from_legacy_vec_unfunded(&mut fields);
            let sibling = payload.clone();
            payload[0].name.push_str(".deep");
            assert_eq!(sibling[0].name, "field");
            assert_eq!(payload[0].ty, sibling[0].ty);
            assert!(
                !super::clone::fields_equal(payload.as_slice(), sibling.as_slice()),
                "field spelling remains significant"
            );
            payload[0].name.truncate("field".len());
            assert!(
                super::clone::fields_equal(payload.as_slice(), sibling.as_slice()),
                "independent deep COW tree is complete and equal"
            );
            let mut cursor = &mut payload[0].ty;
            for _ in 0..512 {
                let ResolvedType::Nominal { arguments, .. } = cursor else {
                    panic!("deep nominal contour")
                };
                cursor = &mut arguments[0];
            }
            let ResolvedType::Function { parameters, .. } = cursor else {
                panic!("deep function tail")
            };
            parameters.swap(0, 1);
            assert!(
                !super::clone::fields_equal(payload.as_slice(), sibling.as_slice()),
                "deep function parameter order remains significant"
            );
            drop(payload);
            drop(sibling);
            let mut fields = vec![field(deep(ResolvedType::I64))];
            let mut unique = NominalPayload::from_legacy_vec_unfunded(&mut fields);
            let pointer = unique.as_ptr();
            let before = current_metrics();
            unique[0].index = 3;
            assert!(!unique.has_shared_owner());
            assert_eq!(unique.as_ptr(), pointer);
            assert_eq!(
                current_metrics().allocated,
                before.allocated,
                "unique legacy mutation moves the original Vec"
            );
            drop(unique);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
