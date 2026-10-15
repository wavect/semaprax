use super::*;
use crate::bounded_output::{with_limit, with_limit_usage};
use crate::test_allocation_observer::measure;

#[test]
fn held_parameters_match_complete_saved_profile_and_argument_diagnostics_across_clones_and_nested_validators(
) {
    let program = super::super::held_profiles::tests::fixture();
    with_limit(usize::MAX, || {
        let validator = HirValidator::new(&program).unwrap();
        let clone = validator.clone();
        let nested = HirValidator::with_profiles(
            &program,
            None,
            None,
            validator.executable_profiles.clone(),
        )
        .unwrap();
        for function in &program.functions {
            let call = validator
                .executable_profiles
                .call(&function.id, None)
                .unwrap();
            for index in 0..function.params.len() {
                for lookup in [
                    &validator.executable_profiles,
                    &clone.executable_profiles,
                    &nested.executable_profiles,
                ] {
                    let view = ParameterView::held_view(call.parameter(index));
                    let old = super::super::executable_profiles::saved_observations_812(
                        &program.declarations,
                        view.ty,
                    );
                    assert_eq!(lookup.runtime_admitted_param(&view), old.0);
                    assert_eq!(lookup.runtime_admitted_param(&view), old.0);
                    assert_eq!(lookup.validate_use(view.ty), old.1);
                }
                for mode in [
                    OwnershipMode::Value,
                    OwnershipMode::Own,
                    OwnershipMode::Borrow,
                    OwnershipMode::Shared,
                ] {
                    let mut argument = function.body.clone();
                    argument.ty = function.params[index].ty.clone();
                    argument.ownership = mode;
                    let saved = validator.validate_argument_ownership_view_saved_812(
                        &argument,
                        ParameterView::held_view(call.parameter(index)),
                    );
                    let current = validator.validate_argument_ownership_view(
                        &argument,
                        ParameterView::held_view(call.parameter(index)),
                    );
                    assert_eq!(current, saved);
                }
            }
        }
    });
}

#[test]
fn cold_and_warm_held_profile_backing_is_paid_through_drop_exact_short_zero_and_sticky() {
    let program = super::super::held_profiles::tests::fixture();
    // Shared unchanged validator setup is outside this scoped memo-owner
    // observation. All real generic flags acquisitions execute inside it.
    let (validator, _) = with_limit(usize::MAX, || HirValidator::new(&program).unwrap());
    let receive = || {
        let lookup = super::super::executable_profiles::Lookup::for_program(&program)?;
        let call = lookup.call(&DeclarationId::new("held.take"), None).unwrap();
        let param = ParameterView::held_view(call.parameter(0));
        let expected =
            crate::hir::owned_text_record::paid::runtime_admitted(param.ty, &program.declarations)?;
        let copy = validator.borrowed_ownership_copy(param.ty)?;
        assert!(copy.is_some());
        let copy_calls = std::cell::Cell::new(0usize);
        for _ in 0..17 {
            assert_eq!(
                lookup.copy_for_param(&param, |ty| {
                    copy_calls.set(copy_calls.get() + 1);
                    validator.borrowed_ownership_copy(ty)
                })?,
                copy
            );
            assert_eq!(lookup.runtime_admitted_param(&param)?, expected);
        }
        match lookup.held_entries() {
            0 => assert_eq!(copy_calls.get(), 17), // honest optional-growth fallback
            1 => assert_eq!(copy_calls.get(), 1),
            _ => panic!("one parameter published more than one memo key"),
        }
        drop(lookup);
        Ok::<(), Diagnostic>(())
    };
    let ((result, metrics), refused, paid) = with_limit_usage(usize::MAX, || measure(&receive));
    assert!(result.is_ok() && !refused && !metrics.invalid);
    assert!(metrics.allocated <= paid);
    assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
    let ((result, metrics), refused, exact) = with_limit_usage(paid, || measure(&receive));
    assert!(result.is_ok() && !refused && !metrics.invalid);
    assert_eq!(exact, paid);
    for limit in [0, paid - 1] {
        let ((failed, metrics), refused, used) = with_limit_usage(limit, || {
            measure(|| {
                let result = receive();
                if crate::bounded_output::active_refused() {
                    let before = crate::test_allocation_observer::current_metrics();
                    let retry = receive();
                    assert!(retry.is_err());
                    assert_eq!(
                        before.allocated,
                        crate::test_allocation_observer::current_metrics().allocated
                    );
                    drop(retry);
                }
                let failed = result.is_err();
                drop(result);
                failed
            })
        });
        assert!(!metrics.invalid && metrics.allocated <= used);
        assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
        // Optional table growth may decline, without manufacturing a refusal.
        // The complete uncached classifier then keeps its ordinary boundary.
        if refused {
            assert!(failed);
        }
    }
}

#[test]
fn genuine_held_copy_observations_reuse_one_tree_and_foreign_detached_unknown_and_changed_arguments_keep_full_receiver(
) {
    let program = super::super::held_profiles::tests::fixture();
    let foreign = program.clone();
    with_limit(usize::MAX, || {
        let validator = HirValidator::new(&program).unwrap();
        let lookup = &validator.executable_profiles;
        let call = lookup.call(&DeclarationId::new("held.take"), None).unwrap();
        let view = ParameterView::held_view(call.parameter(0));
        let expected = validator.borrowed_ownership_copy(view.ty).unwrap();
        assert!(expected.is_some());
        let calls = std::cell::Cell::new(0usize);
        for _ in 0..17 {
            assert_eq!(
                lookup
                    .copy_for_param(&view, |ty| {
                        calls.set(calls.get() + 1);
                        validator.borrowed_ownership_copy(ty)
                    })
                    .unwrap(),
                expected
            );
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(lookup.held_entries(), 1);
        let temporary = view.ty.clone();
        let mut detached = ParameterView::held_view(call.parameter(0));
        detached.ty = &temporary;
        assert_eq!(
            lookup
                .copy_for_param(&detached, |ty| {
                    calls.set(calls.get() + 1);
                    validator.borrowed_ownership_copy(ty)
                })
                .unwrap(),
            expected
        );
        assert_eq!(calls.get(), 2);
        let other = HirValidator::new(&foreign).unwrap();
        let before = other.executable_profiles.held_entries();
        assert_eq!(
            other
                .executable_profiles
                .copy_for_param(&view, |ty| other.borrowed_ownership_copy(ty))
                .unwrap(),
            expected
        );
        assert_eq!(other.executable_profiles.held_entries(), before);
        let mut argument = call.function().body.clone();
        argument.ty = ResolvedType::Bool;
        let old = validator
            .require_type(&argument.ty, view.ty, "call argument")
            .and_then(|()| {
                validator.validate_argument_ownership_view_saved_812(
                    &argument,
                    ParameterView::held_view(call.parameter(0)),
                )
            });
        let current = validator
            .require_type(&argument.ty, view.ty, "call argument")
            .and_then(|()| {
                validator.validate_argument_ownership_view(
                    &argument,
                    ParameterView::held_view(call.parameter(0)),
                )
            });
        assert!(old.is_err());
        assert_eq!(current, old);
        assert_eq!(lookup.held_entries(), 1);
    });
    let checked_vec_argument = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "held.take")
        .unwrap()
        .params
        .iter()
        .find(|parameter| parameter.name == "rows")
        .unwrap()
        .ty
        .clone();
    for arguments in [
        vec![checked_vec_argument],
        vec![],
        vec![ResolvedType::String],
        vec![ResolvedType::I64, ResolvedType::I64],
        vec![ResolvedType::Nominal {
            declaration: DeclarationId::new("missing.λ"),
            arguments: vec![],
        }],
    ] {
        let mut changed = program.clone();
        changed
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "held.take")
            .unwrap()
            .params[0]
            .ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("held.bag"),
            arguments,
        };
        with_limit(usize::MAX, || {
            let validator = HirValidator::new(&changed).unwrap();
            let call = validator
                .executable_profiles
                .call(&DeclarationId::new("held.take"), None)
                .unwrap();
            let view = ParameterView::held_view(call.parameter(0));
            assert_eq!(
                validator
                    .executable_profiles
                    .copy_for_param(&view, |ty| validator.borrowed_ownership_copy(ty)),
                validator.borrowed_ownership_copy(view.ty)
            );
            let mut argument = call.function().body.clone();
            argument.ty = view.ty.clone();
            argument.ownership = OwnershipMode::Borrow;
            let old = validator.validate_argument_ownership_view_saved_812(
                &argument,
                ParameterView::held_view(call.parameter(0)),
            );
            let current = validator.validate_argument_ownership_view(
                &argument,
                ParameterView::held_view(call.parameter(0)),
            );
            assert_eq!(current, old);
        });
    }
}
