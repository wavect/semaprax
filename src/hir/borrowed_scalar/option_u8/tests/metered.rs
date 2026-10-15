//! Independent saved pre-meter metadata oracle and allocation-free ledger edge.
use super::*;
pub(super) fn original(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration: owner, ..
    } = ty
    else {
        return false;
    };
    if !is_type(ty) {
        return false;
    }
    let Some(declaration) = index.declaration(owner) else {
        return false;
    };
    if declaration.kind != DeclarationKind::Variant
        || declaration.name != "Option"
        || declaration.identity_origin != IdentityOrigin::CompilerOwned
        || declaration.owner.is_some()
    {
        return false;
    }
    if !index.type_parameters(owner).is_some_and(|parameters|
        matches!(parameters, [parameter] if parameter.name == "T" && parameter.index == 0)) { return false; }
    let Some(cases) = index.variant_cases(owner) else {
        return false;
    };
    if cases.len() != 2
        || cases[0].id.as_str() != crate::prelude::OPTION_NONE_ID
        || cases[0].name != "None"
        || cases[0].index != 0
        || !cases[0].fields.is_empty()
        || cases[1].id.as_str() != crate::prelude::OPTION_SOME_ID
        || cases[1].name != "Some"
        || cases[1].index != 1
        || cases[1].fields.len() != 1
    {
        return false;
    }
    for case in cases {
        let Some(declaration) = index.declaration(&case.id) else {
            return false;
        };
        if declaration.kind != DeclarationKind::VariantCase
            || declaration.name != case.name
            || declaration.identity_origin != IdentityOrigin::CompilerOwned
            || declaration.owner.as_ref() != Some(owner)
        {
            return false;
        }
    }
    let field = &cases[1].fields[0];
    let Some(declaration) = index.declaration(&field.id) else {
        return false;
    };
    declaration.kind == DeclarationKind::CaseField
        && declaration.name == "value"
        && declaration.identity_origin == IdentityOrigin::CompilerOwned
        && declaration.owner.as_ref() == Some(&cases[1].id)
        && field.id.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
        && field.name == "value"
        && field.index == 0
        && matches!(&field.ty, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }
            if parameter_owner == owner)
        && index.case_fields(&cases[1].id) == Some(cases[1].fields.as_slice())
        && index
            .case_fields(&cases[0].id)
            .is_some_and(|fields| fields.is_empty())
}

#[test]
fn actual_metadata_probes_preserve_exact_one_short_cumulative_failure_without_allocating() {
    // Independent byte census for the cheap filter; declaration metadata is
    // deliberately outside this shape-only predicate.
    for (value, accepted, expected_work) in [
        (ResolvedType::U8, false, 1),
        (ty(), true, crate::prelude::OPTION_ID.len() + 4),
        (
            ResolvedType::Nominal {
                declaration: DeclarationId::new(crate::prelude::OPTION_ID),
                arguments: vec![ResolvedType::Bytes],
            },
            false,
            crate::prelude::OPTION_ID.len() + 4,
        ),
        (
            ResolvedType::Nominal {
                declaration: DeclarationId::new(crate::prelude::OPTION_ID),
                arguments: vec![],
            },
            false,
            crate::prelude::OPTION_ID.len() + 3,
        ),
    ] {
        for allowance in [expected_work, expected_work - 1] {
            let mut remaining = allowance;
            let received = is_type_metered(&value, &mut |units| {
                remaining = remaining.checked_sub(units).ok_or(())?;
                Ok::<_, ()>(())
            });
            assert_eq!(
                received,
                if allowance == expected_work {
                    Ok(accepted)
                } else {
                    Err(())
                }
            );
            assert_eq!(remaining, 0);
            assert_eq!(is_type(&value), accepted);
        }
    }
    let current = index();
    let value = ty();
    let mut spent = 0usize;
    let (found, allocation) = crate::test_allocation_observer::measure(|| {
        authenticated_metered(&current, &value, &mut |units| {
            spent = spent.checked_add(units).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap()
    });
    assert!(found && original(&current, &value));
    assert_eq!(allocation.allocated, 0);
    assert_eq!(allocation.live, 0);
    assert!(!allocation.invalid);
    // A single private receipt is shared across two actual receivers. No fresh
    // allowance is introduced at the second provider/receiving call.
    for total in [spent * 2, spent * 2 - 1] {
        let mut remaining = total;
        let mut failed = false;
        let mut debit = |units| {
            if failed {
                return Err(());
            }
            match remaining.checked_sub(units) {
                Some(next) => {
                    remaining = next;
                    Ok(())
                }
                None => {
                    failed = true;
                    Err(())
                }
            }
        };
        assert_eq!(
            authenticated_metered(&current, &value, &mut debit),
            Ok(true)
        );
        let second = authenticated_metered(&current, &value, &mut debit);
        if total == spent * 2 {
            assert_eq!(second, Ok(true));
        } else {
            assert_eq!(second, Err(()));
            assert_eq!(authenticated_metered(&current, &value, &mut debit), Err(()));
        }
        #[allow(
            clippy::drop_non_drop,
            reason = "End this borrowed proof at the measured lifetime boundary."
        )]
        drop(debit);
        assert_eq!(remaining, 0);
        assert_eq!(failed, total != spent * 2);
    }
}

#[test]
fn borrowed_option_inventory_matches_independent_ordinary_inventory_and_whole_drop_budget() {
    let source = r#"module option.helper; @id("option.read") fn read(view:borrow Slice<u8>,at:usize)->i64 {match byte_get(view,at) {Option::Some {value}=>i64_from_u8(value),Option::None {}=>0,}} @id("option.main") fn main()->i64 { 0 }"#;
    let program =
        crate::hir::resolve(&crate::check(source, "option-inventory.spx").unwrap()).unwrap();
    let function = &program.functions[0];
    let expected = crate::cleanup::build_inventory(&program, function).unwrap();
    fn measured(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        expected: &crate::cleanup::CleanupInventory,
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), ()>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let mut remaining = work;
            let mut physical = crate::cleanup_plan::ReplayPhysicalReservation::new(bytes);
            let mut failed = false;
            let result = crate::cleanup::build_borrowed_helper_inventory_with_budget(
                program,
                function,
                &mut physical,
                &mut |units| {
                    if failed {
                        return Err(crate::diagnostic::Diagnostic::io(
                            "SPX-H006",
                            "sticky inventory refusal",
                        ));
                    }
                    match remaining.checked_sub(units) {
                        Some(next) => {
                            remaining = next;
                            Ok(())
                        }
                        None => {
                            failed = true;
                            Err(crate::diagnostic::Diagnostic::io(
                                "SPX-H006",
                                "inventory work exhausted",
                            ))
                        }
                    }
                },
            )
            .map_err(|_| ())?;
            assert_eq!(&result, expected);
            let spent = (work - remaining, physical.charged());
            drop(result);
            Ok(spent)
        })
    }
    let (wide, allocation) = measured(&program, function, &expected, 1 << 24, 1 << 20);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    let (exact, allocation) = measured(&program, function, &expected, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    assert!(measured(&program, function, &expected, work - 1, bytes)
        .0
        .is_err());
    assert!(measured(&program, function, &expected, work, bytes - 1)
        .0
        .is_err());
    let mut hostile = program.clone();
    let owner = DeclarationId::new(crate::prelude::OPTION_ID);
    hostile
        .declarations
        .type_parameters
        .get_mut(&owner)
        .unwrap()[0]
        .index = 1;
    // The supplied cleanup inventory is unchanged and cannot authenticate metadata.
    assert_eq!(hostile.functions[0].cleanup, function.cleanup);
    assert!(
        measured(&hostile, &hostile.functions[0], &expected, 1 << 24, 1 << 20)
            .0
            .is_err()
    );
}

#[test]
fn wide_copy_block_work_includes_each_independently_counted_prefix_probe() {
    for bindings in [0usize, 1, 2, 32, 128] {
        let mut source = String::from("module inventory.wide; @id(\"wide.main\") fn main()->i64 {");
        for index in 0..bindings {
            source.push_str(&format!("let value_{index}={index};"));
        }
        source.push_str("42}");
        let program =
            crate::hir::resolve(&crate::check(&source, "wide-inventory.spx").unwrap()).unwrap();
        let function = &program.functions[0];
        let expected = crate::cleanup::build_inventory(&program, function).unwrap();
        // Each let has two traversal ordinals. Both scan 1..N preceding
        // statements; tail and completed child census each scan all N again.
        // There are 6N+7 Enter/Children/Finish/AddBinding frames, N+2
        // entered type probes and one independently evaluated return probe.
        let expected_prefix = bindings * (bindings + 1) + 2 * bindings;
        let exact = expected_prefix + (6 * bindings + 7) + (bindings + 3);
        for allowance in [exact, exact - 1] {
            let mut remaining = allowance;
            let mut physical = crate::cleanup_plan::ReplayPhysicalReservation::new(1 << 20);
            let result = crate::cleanup::build_borrowed_helper_inventory_with_budget(
                &program,
                function,
                &mut physical,
                &mut |units| {
                    assert_eq!(units, 1);
                    remaining = remaining.checked_sub(units).ok_or_else(|| {
                        crate::diagnostic::Diagnostic::io("SPX-H006", "wide callback exhausted")
                    })?;
                    Ok(())
                },
            );
            if allowance == exact {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert_eq!(result.unwrap_err().code, "SPX-H006");
            }
            assert_eq!(remaining, 0);
        }
    }
}
