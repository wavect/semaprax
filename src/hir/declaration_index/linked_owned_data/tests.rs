//! Complete old/current linker controls; this compatibility route has no payer.
use super::*;
use crate::test_allocation_observer::measure;

type Facts = BTreeMap<DeclarationId, LinkedDeclarationFact>;
fn fixture(shared: bool) -> (Vec<ResolvedTypeDeclaration>, Facts) {
    let ast = crate::check(
        r#"module linked.views;
@id("row.type") record Row { @id("row.x") x:Option<i64>, }
@id("choice.type") variant Choice {
    @id("choice.left") Left { @id("choice.x") x:Row, },
    @id("choice.right") Right,
}
@id("app.main") fn main()->i64 {0}
"#,
        "linked-views.spx",
    )
    .unwrap();
    let mut hir = if shared {
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            crate::hir::identity_resolution::resolve_checked_nominal(
                &ast,
                None,
                None,
                None,
                None,
                |n| {
                    if crate::bounded_output::reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(())
                    }
                },
            )
            .unwrap()
            .0
        })
        .0
    } else {
        crate::hir::resolve(&ast).unwrap()
    };
    hir.types
        .retain(|ty| matches!(ty.id.as_str(), "row.type" | "choice.type"));
    let facts = hir
        .declarations
        .declarations()
        .filter(|d| {
            matches!(
                d.id.as_str(),
                "row.type" | "row.x" | "choice.type" | "choice.left" | "choice.x" | "choice.right"
            )
        })
        .map(|d| {
            (
                d.id.clone(),
                LinkedDeclarationFact {
                    kind: d.kind,
                    origin: d.identity_origin,
                    owner: d.owner.clone(),
                },
            )
        })
        .collect();
    (std::mem::take(&mut hir.types), facts)
}
fn extend(
    types: &[ResolvedTypeDeclaration],
    facts: &Facts,
    old: bool,
    scalar: bool,
) -> (DeclarationIndex, Result<(), Diagnostic>) {
    let mut index = DeclarationIndex::default();
    let result = match (old, scalar) {
        (true, true) => index.extend_linked_scalar_data_original(types, &[], &[], &[], facts),
        (true, false) => index.extend_linked_owned_data_original(types, &[], &[], &[], facts),
        (false, true) => index.extend_linked_scalar_data(types, &[], &[], &[], facts),
        (false, false) => index.extend_linked_owned_data(types, &[], &[], &[], facts),
    };
    (index, result)
}
#[test]
fn linked_nominal_views_preserve_complete_old_maps_wire_and_owned_decoded_facts() {
    for shared in [false, true] {
        let (types, facts) = fixture(shared);
        let wire = crate::cache_codec::encode(&types).unwrap();
        let decoded: Vec<ResolvedTypeDeclaration> = crate::cache_codec::decode(&wire).unwrap();
        for input in [&types, &decoded] {
            for scalar in [false, true] {
                let (old, old_result) = extend(input, &facts, true, scalar);
                let (current, result) = extend(input, &facts, false, scalar);
                assert_eq!(result, old_result);
                assert!(result.is_ok());
                assert_eq!(current, old);
                assert_eq!(
                    crate::cache_codec::encode(&current).unwrap(),
                    crate::cache_codec::encode(&old).unwrap()
                );
                for ty in input {
                    match &ty.kind {
                        ResolvedTypeDeclarationKind::Record { fields } => {
                            let held = current.record_payload(&ty.id).unwrap();
                            assert_eq!(held.as_slice(), fields.as_slice());
                            if fields.has_shared_owner() {
                                assert!(held.same_owner(fields));
                            } else {
                                assert_ne!(held.as_ptr(), fields.as_ptr());
                            }
                        }
                        ResolvedTypeDeclarationKind::Variant { cases } => {
                            for case in cases {
                                let held = current.case_fields(&case.id).unwrap();
                                assert_eq!(held, case.fields.as_slice());
                                if cases.has_shared_owner() {
                                    assert_eq!(held.as_ptr(), case.fields.as_ptr());
                                } else if !case.fields.is_empty() {
                                    assert_ne!(held.as_ptr(), case.fields.as_ptr());
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}
#[test]
fn linked_nominal_views_keep_full_old_errors_partial_maps_and_first_fact_order() {
    for shared in [false, true] {
        let (types, facts) = fixture(shared);
        let mut mutations = Vec::new();
        let mut missing = facts.clone();
        missing.remove(&DeclarationId::new("choice.x"));
        mutations.push(missing);
        let mut later_missing = facts.clone();
        later_missing.remove(&DeclarationId::new("choice.right"));
        mutations.push(later_missing);
        let mut mixed = facts.clone();
        mixed.remove(&DeclarationId::new("choice.x"));
        mixed.get_mut(&DeclarationId::new("row.x")).unwrap().kind = DeclarationKind::Function;
        mutations.push(mixed);
        let mut wrong_kind = facts.clone();
        wrong_kind
            .get_mut(&DeclarationId::new("choice.x"))
            .unwrap()
            .kind = DeclarationKind::Function;
        mutations.push(wrong_kind);
        let mut wrong_owner = facts.clone();
        wrong_owner
            .get_mut(&DeclarationId::new("choice.x"))
            .unwrap()
            .owner = None;
        mutations.push(wrong_owner);
        let mut unrelated = facts.clone();
        unrelated.insert(
            DeclarationId::new("extra"),
            LinkedDeclarationFact {
                kind: DeclarationKind::Field,
                origin: IdentityOrigin::Explicit,
                owner: None,
            },
        );
        mutations.push(unrelated);
        for supplied in mutations {
            for scalar in [false, true] {
                let (old, old_result) = extend(&types, &supplied, true, scalar);
                let (current, result) = extend(&types, &supplied, false, scalar);
                assert!(result.is_err());
                assert_eq!(
                    result, old_result,
                    "all six diagnostic fields and selected order"
                );
                assert_eq!(current, old, "complete partial map after first error");
                assert_eq!(
                    crate::cache_codec::encode(&current).unwrap(),
                    crate::cache_codec::encode(&old).unwrap()
                );
            }
        }
        let duplicate = vec![types[0].clone(), types[0].clone()];
        let (_, old) = extend(&duplicate, &facts, true, false);
        let (_, current) = extend(&duplicate, &facts, false, false);
        assert_eq!(current, old);
        assert!(current.is_err());
    }
}
#[test]
fn linked_nominal_views_remove_actual_discarded_backing_and_retain_owner_until_drop() {
    let (types, facts) = fixture(true);
    let ((), old) = measure(|| {
        let (index, result) = extend(&types, &facts, true, false);
        assert!(result.is_ok());
        drop(index);
    });
    let ((), current) = measure(|| {
        let (index, result) = extend(&types, &facts, false, false);
        assert!(result.is_ok());
        drop(index);
    });
    assert!(!old.invalid && !current.invalid);
    assert_eq!(old.live, 0);
    assert_eq!(old.allocated, old.freed);
    assert_eq!(current.live, 0);
    assert_eq!(current.allocated, current.freed);
    assert!(
        current.allocated < old.allocated,
        "source-specific physical clone removal: old={old:?}, current={current:?}"
    );
    // One observer owns input, linked maps, and every final release. No
    // assertion equates this unfunded compatibility route with paid bytes.
    let ((), complete) = measure(|| {
        let (types, facts) = fixture(true);
        let (old, old_result) = extend(&types, &facts, true, false);
        let (current, result) = extend(&types, &facts, false, false);
        assert!(old_result.is_ok() && result.is_ok());
        drop(types);
        assert_eq!(current, old);
        assert!(!current
            .case_fields(&DeclarationId::new("choice.left"))
            .unwrap()
            .is_empty());
        drop(old);
        drop(current);
        drop(facts);
    });
    assert!(!complete.invalid);
    assert_eq!(complete.live, 0);
    assert_eq!(complete.allocated, complete.freed);
}

#[test]
fn linked_shared_case_views_last_owner_drop_uses_prepaid_iterative_frontier() {
    let mut global_owner_deltas = [0usize; 2];
    for attempt in 0..2 {
        let ((), complete) = measure(|| {
            // Keep potential cold global initialization INSIDE this same observer.
            // Only this exact OnceLock call can contribute the classified delta;
            // every fixture/index/payload allocation remains independently owed.
            let before_global = crate::test_allocation_observer::current_metrics();
            let global = crate::prelude::declarations();
            let after_global = crate::test_allocation_observer::current_metrics();
            let global_delta = after_global.live.checked_sub(before_global.live).unwrap();
            global_owner_deltas[attempt] = global_delta;
            assert_eq!(global.as_ptr(), crate::prelude::declarations().as_ptr());
            let owned = crate::prelude::owned_declarations_for_tests();
            let original = crate::prelude::original_initializer_reference();
            assert_eq!(global, owned.as_slice());
            assert_eq!(global, original.as_slice());
            drop(owned);
            drop(original);
            assert_eq!(
                crate::test_allocation_observer::current_metrics().live,
                after_global.live,
                "independent complete owned initializers must free every allocation"
            );
            let (mut types, facts) = fixture(false);
            let cases = types
                .iter_mut()
                .find_map(|ty| match &mut ty.kind {
                    ResolvedTypeDeclarationKind::Variant { cases } => Some(cases),
                    _ => None,
                })
                .unwrap();
            // Synthetic hostile depth tests disposal storage only, not admission.
            let mut ty = ResolvedType::I64;
            for _ in 0..512 {
                ty = ResolvedType::Nominal {
                    declaration: DeclarationId::new("synthetic.depth"),
                    arguments: vec![ty],
                };
            }
            cases[0].fields[0].ty = ty;
            let owner = NominalPayload::from_funded_vec(&mut **cases, |_| Ok::<_, ()>(())).unwrap();
            *cases = owner;
            let (index, result) = extend(&types, &facts, false, false);
            assert!(result.is_ok());
            drop(types);
            let field = &index
                .case_fields(&DeclarationId::new("choice.left"))
                .unwrap()[0];
            let mut depth = 0;
            let mut cursor = &field.ty;
            while let ResolvedType::Nominal { arguments, .. } = cursor {
                depth += 1;
                cursor = &arguments[0];
            }
            assert_eq!(depth, 512);
            drop(index);
            drop(facts);
        });
        assert!(!complete.invalid);
        // This is intentional process ownership classification, not a production
        // Drop fix or an allowance for any unclassified retained allocation.
        assert_eq!(complete.live, global_owner_deltas[attempt]);
        assert_eq!(
            complete.allocated,
            complete.freed + global_owner_deltas[attempt]
        );
        if attempt == 1 {
            // Preserve the original complete zero-live regression on the genuinely
            // warm pass, with all construction and final Drop still in this scope.
            assert_eq!(global_owner_deltas[attempt], 0);
            assert_eq!(complete.live, 0);
            assert_eq!(complete.allocated, complete.freed);
        }
    }
    eprintln!("linked-last-owner-process-prelude-first-touch-deltas={global_owner_deltas:?}");
}

#[path = "tests/legacy.rs"]
mod legacy;
