//! Complete effect/diagnostic receivers and actual allocation-through-Drop controls.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
fn program() -> ResolvedProgram {
    let mut source = crate::hir::iterative_validator_tests::SOURCE.to_owned();
    for n in 0..20 {
        source.push_str(&format!("\n@id(\"lifecycle.row.{n}\") record LifecycleRow{n} {{ @id(\"lifecycle.row.{n}.field\") field: i64, }}\n"));
    }
    source.push_str(
        r#"
@id("lifecycle.make") fn lifecycle_make()->LifecycleRow0 { LifecycleRow0 { field: 1 } }
@id("lifecycle.repeat") fn lifecycle_repeat()->i64 {
 let a=lifecycle_make(); let b=lifecycle_make(); let c=lifecycle_make();
 a.field+b.field+c.field
}
"#,
    );
    crate::hir::resolve(&crate::check(&source, "lifecycle-completed.spx").unwrap()).unwrap()
}
fn root(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}
fn lifetime(
    program: &ResolvedProgram,
    roots: &[ResolvedType],
    count: usize,
    limit: usize,
    shared: bool,
) -> (
    bool,
    crate::test_allocation_observer::Metrics,
    bool,
    usize,
    bool,
) {
    let ((ok, metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut collector = Collector::new(program);
            let mut original = BTreeSet::new();
            let mut ok = true;
            for _ in 0..count {
                for ty in roots {
                    let result = if shared {
                        collector.extend(ty)
                    } else {
                        extend_funded(program, ty, &mut original)
                    };
                    if result.is_err() {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    break;
                }
            }
            drop(original);
            drop(collector);
            ok
        })
    });
    (ok, metrics, refused, used, first.is_some())
}
#[test]
fn completed_roots_reduce_repeated_real_collection_backing_and_pay_receiving_drop() {
    let program = program();
    let roots = (0..20)
        .map(|n| root(&format!("lifecycle.row.{n}")))
        .collect::<Vec<_>>();
    let (ok, current, refused, paid, first) = lifetime(&program, &roots, 32, LIMIT, true);
    let (old_ok, old, old_refused, old_paid, _) = lifetime(&program, &roots, 32, LIMIT, false);
    assert!(ok && old_ok && !refused && !old_refused && !first);
    assert!(!current.invalid && !old.invalid);
    assert_eq!((current.live, old.live), (0, 0));
    assert_eq!(
        (current.allocated, old.allocated),
        (current.freed, old.freed)
    );
    assert!(current.allocated <= paid && current.highwater <= paid);
    assert!(old.allocated <= old_paid && old.highwater <= old_paid);
    assert!(paid < old_paid && current.allocated < old.allocated);
    let (ok, exact, refused, exact_paid, first) = lifetime(&program, &roots, 32, paid, true);
    assert!(ok && !refused && !first && exact_paid == paid && exact.live == 0);
    for limit in [0, paid - 1] {
        let (ok, actual, refused, used, first) = lifetime(&program, &roots, 32, limit, true);
        assert!(!ok && refused && first && used <= limit && !actual.invalid);
        assert_eq!((actual.live, actual.allocated), (0, actual.freed));
    }
}
#[test]
fn complete_effect_sets_and_six_field_diagnostics_match_original_cold_and_warm() {
    let mut program = program();
    let mut roots = program
        .types
        .iter()
        .map(|item| root(item.id.as_str()))
        .collect::<Vec<_>>();
    roots.push(root("missing.lifecycle"));
    let mut collector = Collector::new(&program);
    let mut old = BTreeSet::new();
    for ty in roots.iter().cycle().take(roots.len() * 3) {
        let expected = extend_resolved_lifecycle_effects(&program, ty, &mut old);
        let actual = collector.extend(ty);
        assert_eq!(
            actual.as_ref().err().map(Diagnostic::json),
            expected.as_ref().err().map(Diagnostic::json)
        );
        assert_eq!(&collector.effects, &old);
    }
    drop(collector);
    let item = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "lifecycle.row.0")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut item.kind else {
        panic!("record");
    };
    fields[0].ty = root("missing.descendant");
    let mut collector = Collector::new(&program);
    for _ in 0..2 {
        let mut old = BTreeSet::new();
        let ty = root("lifecycle.row.0");
        let expected = extend_resolved_lifecycle_effects(&program, &ty, &mut old);
        let actual = collector.extend(&ty);
        assert_eq!(actual.unwrap_err().json(), expected.unwrap_err().json());
        assert_eq!(&collector.effects, &old);
        assert!(collector.completed.iter().all(Option::is_none) && collector.spill.is_empty());
    }
}
#[test]
fn complete_generic_arguments_unknown_metadata_and_equal_ids_stay_independent() {
    let program = program();
    let mut other = program.clone();
    let item = other
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "lifecycle.row.0")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut item.kind else {
        panic!("record");
    };
    fields[0].ty = root("missing.other");
    let mut left = Collector::new(&program);
    let mut right = Collector::new(&other);
    let ty = root("lifecycle.row.0");
    left.extend(&ty).unwrap();
    assert!(right.extend(&ty).is_err());
    let mut generic = ty.clone();
    let ResolvedType::Nominal { arguments, .. } = &mut generic else {
        unreachable!()
    };
    arguments.push(ResolvedType::String);
    for ty in [generic, root("missing.root"), ResolvedType::I64] {
        assert!(left.key(&ty).is_none());
        let mut old = BTreeSet::new();
        let expected = extend_resolved_lifecycle_effects(&program, &ty, &mut old);
        let actual = left.extend(&ty);
        assert_eq!(
            actual.as_ref().err().map(Diagnostic::json),
            expected.as_ref().err().map(Diagnostic::json)
        );
    }
}
#[test]
fn warm_hits_authenticate_sticky_refusal_without_allocations_or_work_debits() {
    let program = program();
    let ty = root("lifecycle.row.0");
    let mut collector = Collector::new(&program);
    collector.extend(&ty).unwrap();
    let traversals = collector.traversals;
    let (((), metrics), refused, used) = with_limit_usage(0, || {
        measure(|| {
            for _ in 0..128 {
                collector.extend(&ty).unwrap();
            }
        })
    });
    assert!(!refused && used == 0 && metrics.allocated == 0 && !metrics.invalid);
    assert_eq!(collector.traversals, traversals);
    let ((), refused, used, first) = with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert!(collector.extend(&ty).is_err());
        assert!(collector.extend(&ty).is_err());
    });
    assert!(refused && used == 0 && first.is_some());
    collector.extend(&ty).unwrap();
}
#[test]
fn actual_function_chain_matches_complete_prior_receiver_and_auxiliary_state() {
    let program = program();
    for function in &program.functions {
        for attack in 0..3 {
            let mut function = function.clone();
            if attack == 1 {
                function.return_type = root("missing.function.result");
            }
            if attack == 2 {
                function.return_type = ResolvedType::Unit;
            }
            let execution = FunctionExecutionId::Monomorphic(function.id.clone());
            let mut current = HirValidator::new(&program).unwrap();
            let mut old = current.clone();
            let expected = old.validate_function_lifecycle_saved(&function, &execution);
            let actual = current.validate_function(&function, &execution);
            HirValidator::assert_validation_oracle(
                &actual,
                &expected,
                &current,
                &old,
                &BTreeMap::new(),
                &BTreeMap::new(),
                function.id.as_str(),
            );
            HirValidator::assert_scope_auxiliary_oracle(&current, &old);
        }
    }
}

#[test]
fn nested_cycles_imported_effect_payloads_and_missing_imports_keep_full_original_order() {
    let mut program = program();
    // Forge a resource import binding to exercise full effect payload obligations.
    let token = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "token.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut token.kind else {
        panic!("resource");
    };
    drop.kind = ResolvedResourceDropKind::Imported {
        import: DeclarationId::new("host.echo"),
        import_key: "host.echo".into(),
    };
    let row = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "lifecycle.row.0")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut row.kind else {
        panic!("record");
    };
    fields[0].ty = root("lifecycle.row.0");
    for ty in [
        root("token.type"),
        root("owned.box"),
        root("lifecycle.row.0"),
    ] {
        let mut collector = Collector::new(&program);
        let mut old = BTreeSet::new();
        for _ in 0..3 {
            let expected = extend_resolved_lifecycle_effects(&program, &ty, &mut old);
            let actual = collector.extend(&ty);
            assert_eq!(
                actual.as_ref().err().map(Diagnostic::json),
                expected.as_ref().err().map(Diagnostic::json)
            );
            assert_eq!(&collector.effects, &old);
        }
    }
    let ty = root("token.type");
    let run = || -> Result<(), Diagnostic> {
        let mut collector = Collector::new(&program);
        for _ in 0..32 {
            collector.extend(&ty)?;
        }
        Ok(())
    };
    let ((ok, actual), refused, paid, first) =
        with_limit_usage_refusal(LIMIT, || measure(|| run().is_ok()));
    assert!(ok && !refused && first.is_none() && !actual.invalid && actual.live == 0);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid && actual.allocated == actual.freed
    );
    for limit in [0, paid - 1] {
        let ((ok, actual), refused, used, first) =
            with_limit_usage_refusal(limit, || measure(|| run().is_ok()));
        assert!(
            !ok && refused
                && used <= limit
                && first.is_some()
                && actual.live == 0
                && actual.allocated == actual.freed
        );
    }
    let token = program
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "token.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut token.kind else {
        panic!("resource");
    };
    let ResolvedResourceDropKind::Imported { import, .. } = &mut drop.kind else {
        panic!("import");
    };
    *import = DeclarationId::new("missing.import");
    let mut collector = Collector::new(&program);
    let mut old = BTreeSet::new();
    assert_eq!(
        collector.extend(&ty).unwrap_err().json(),
        extend_resolved_lifecycle_effects(&program, &ty, &mut old)
            .unwrap_err()
            .json()
    );
    assert!(collector.completed.iter().all(Option::is_none));
}

#[test]
fn complete_frozen_function_receiver_is_exact_named_projection_with_original_digest() {
    use sha2::{Digest, Sha256};
    let archived = include_str!("../../../../../../evidence/final-validation-lifecycle-completed/src__hir__validation__function_scope.rs.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(archived.as_bytes()))
        ),
        "d4a6b7e8f4ec7a9a886ec69943ead6fbd57cd91317dc49818c0f0cf5d5a8d88c"
    );
    let body = archived
        .split_once("\n#[cfg(test)]\nmod collection_tests;")
        .unwrap()
        .0;
    let projected = body
        .replace(
            "fn validate_function(",
            "fn validate_function_lifecycle_saved(",
        )
        .replace(
            "validate_expr_persistent(",
            "validate_expr_lifecycle_saved(",
        );
    assert_eq!(
        include_str!("../../lifecycle_receiver_saved.rs"),
        projected.trim_end_matches('\n').to_owned() + "\n"
    );
    let archived_lifecycle = include_str!("../../../../../../evidence/final-validation-lifecycle-completed/src__hir__validation__function_scope__lifecycle.rs.txt");
    let mut legacy = include_str!("../../lifecycle.rs").to_owned();
    // Unregistered compiled source: authenticate the ENTIRE current input,
    // reverse only these exact optional observer adapters, then preserve every
    // original whole-before assertion and old inverse below. No page admission.
    let debit_digest = |text: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
        )
    };
    assert_eq!(legacy.len(), 3938);
    assert_eq!(
        debit_digest(&legacy),
        "d043c680becb6e6385559f4d0d8e86e5eff79f5dbdca72cfbe97b058cc18d8bd"
    );
    {
        let (start, end, current, previous) = (
            396,
            615,
            r########"    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::Lifecycle,
"########,
            r########""########,
        );
        assert_eq!(legacy.get(start..end), Some(current));
        legacy.replace_range(start..end, previous);
    }
    assert_eq!(legacy.len(), 3719);
    assert_eq!(
        debit_digest(&legacy),
        "4359a3991e559f31cbb1ad650886ed3433270cbf17ce535169b3ff04fe64027a"
    );
    for (current, original) in [
        (
            "#[cfg(test)]\npub(super) fn extend_funded(",
            "pub(super) fn extend_funded(",
        ),
        ("#[cfg(test)]\nfn collect<'a>(", "fn collect<'a>("),
    ] {
        assert_eq!(legacy.matches(current).count(), 1);
        legacy = legacy.replacen(current, original, 1);
    }
    // The complete archive predates this exact test-only observation adapter.
    let operation_adapter = "    #[cfg(test)]\n    let _operation = crate::bounded_output::stage_debits::operation_guard(\n        crate::bounded_output::stage_debits::Operation::Lifecycle,\n    );\n";
    assert_eq!(legacy.matches(operation_adapter).count(), 1);
    legacy = legacy.replacen(operation_adapter, "", 1);
    assert_eq!(
        legacy,
        archived_lifecycle.to_owned() + "\npub(super) mod completed;\n"
    );
}

#[test]
fn actual_repeated_call_receiver_is_paid_through_drop_with_exact_short_and_sticky_refusal() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "lifecycle.repeat")
        .unwrap();
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    #[derive(Clone, Copy, Debug)]
    struct Boundary {
        metrics: crate::test_allocation_observer::Metrics,
        remaining: Option<usize>,
    }
    fn boundary() -> Boundary {
        Boundary {
            metrics: crate::test_allocation_observer::current_metrics(),
            remaining: crate::bounded_output::active_remaining(),
        }
    }
    let observe = |limit, shared| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                // Fixed inline snapshots attribute the same complete measured route.
                // They create no nested observation, allocation, debit, or exclusion.
                let mut boundaries = [None; 5];
                boundaries[0] = Some(boundary());
                let result = HirValidator::new(&program).and_then(|mut validator| {
                    boundaries[1] = Some(boundary());
                    let result = if shared {
                        validator.validate_function(function, &execution)
                    } else {
                        validator.validate_function_lifecycle_saved(function, &execution)
                    };
                    boundaries[2] = Some(boundary());
                    result
                });
                // and_then has dropped the validator before handing back its result.
                boundaries[3] = Some(boundary());
                let ok = result.is_ok();
                if !ok {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                boundaries[4] = Some(boundary());
                (ok, boundaries)
            })
        })
    };
    let (((ok, boundaries), actual), refused, paid, first) = observe(LIMIT, true);
    let (((old_ok, old_boundaries), old), old_refused, old_paid, old_first) = observe(LIMIT, false);
    assert!(ok && old_ok && !refused && !old_refused && first.is_none() && old_first.is_none());
    for route in [&boundaries, &old_boundaries] {
        for point in route.iter().flatten() {
            assert!(!point.metrics.invalid && point.remaining.is_some());
        }
    }
    assert!(!actual.invalid && !old.invalid && actual.live == 0 && old.live == 0);
    assert!(actual.allocated == actual.freed && old.allocated == old.freed);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid,
        "complete receiver actual={actual:?} paid={paid} old={old:?} old_paid={old_paid}; boundaries [before constructor, constructed, validated before validator Drop, validator dropped, result dropped]: current={boundaries:?} old={old_boundaries:?}"
    );
    assert!(
        old.allocated <= old_paid && old.highwater <= old_paid,
        "complete saved receiver old={old:?} old_paid={old_paid}; boundaries={old_boundaries:?}"
    );
    assert!(paid < old_paid && actual.allocated < old.allocated);
    let (((ok, _), exact), refused, used, first) = observe(paid, true);
    assert!(ok && !refused && used == paid && first.is_none() && !exact.invalid && exact.live == 0);
    for limit in [0, paid - 1] {
        let (((ok, _), actual), refused, used, first) = observe(limit, true);
        assert!(
            !ok && refused
                && first.is_some()
                && used <= limit
                && !actual.invalid
                && actual.live == 0
                && actual.allocated == actual.freed
        );
    }
}
