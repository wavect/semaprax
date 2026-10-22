//! Independent whole-driver/function comparisons and actual backing through Drop.
use super::*;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;

fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(source, std::path::Path::new("frame-reuse.spx")).unwrap())
        .unwrap()
}
fn scalar_program() -> ResolvedProgram {
    let mut source = String::from("module validation.reuse;\n");
    for index in 0..32 {
        source.push_str(&format!(
            "@id(\"reuse.{index}\") fn f{index}(v:i64)->i64 requires v>=0 ensures result>=0 {{if v>0 {{v+1}} else {{0}}}}\n"
        ));
    }
    source.push_str("@id(\"reuse.main\") fn main()->i64 {f31(1)}\n");
    program(&source)
}
fn decimal_program() -> ResolvedProgram {
    // Preserve the entire library; executable resolution also needs an entry.
    let source = format!(
        "{}\n@id(\"reuse.decimal.main\") fn main()->i64 {{let value=add(\"1\",\"2\");string_len(value)}}\n",
        include_str!("../../../../std/int-decimal/src/decimal.spx")
    );
    program(&source)
}

#[test]
fn invocation_reuse_matches_complete_immediate_previous_function_chain_and_decimal() {
    for program in [
        scalar_program(),
        program(crate::hir::iterative_validator_tests::SOURCE),
        decimal_program(),
    ] {
        let mut current = HirValidator::new(&program).unwrap();
        let mut old = current.clone();
        let mut scratch = Scratch::new().unwrap();
        for function in &program.functions {
            let execution = FunctionExecutionId::Monomorphic(function.id.clone());
            let expected = old.validate_function_before_frame_reuse(function, &execution);
            argument().unwrap();
            let actual =
                current.validate_function_with_frames(function, &execution, Some(&mut scratch));
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
            actual.unwrap();
            assert_eq!((scratch.frames.len(), scratch.scopes.len()), (0, 0));
        }
    }
}

#[test]
fn late_errors_preserve_complete_diagnostics_partial_publication_and_auxiliary_state() {
    let program = scalar_program();
    let original = &program.functions[0];
    for phase in ["requires", "body", "ensures", "identity", "type"] {
        let mut function = original.clone();
        let root = match phase {
            "requires" => &mut function.requires[0],
            "ensures" => &mut function.ensures[0],
            _ => &mut function.body,
        };
        if phase == "identity" {
            root.id = ExpressionId::from_owned("forged.trailing.identity".to_owned());
        } else if phase == "type" {
            root.ty = ResolvedType::Bool;
        } else {
            root.ownership = OwnershipMode::Borrow;
        }
        let mut current = HirValidator::new(&program).unwrap();
        let mut old = current.clone();
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let mut scratch = Scratch::new().unwrap();
        // A preceding successful root fills backing without contributing state
        // to the validator under comparison.
        let mut warm = HirValidator::new(&program).unwrap();
        argument().unwrap();
        warm.validate_function_with_frames(
            &program.functions[1],
            &FunctionExecutionId::Monomorphic(program.functions[1].id.clone()),
            Some(&mut scratch),
        )
        .unwrap();
        let expected = old.validate_function_before_frame_reuse(&function, &execution);
        argument().unwrap();
        let actual =
            current.validate_function_with_frames(&function, &execution, Some(&mut scratch));
        HirValidator::assert_validation_oracle(
            &actual,
            &expected,
            &current,
            &old,
            &BTreeMap::new(),
            &BTreeMap::new(),
            phase,
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, &old);
        actual.unwrap_err();
        assert_eq!((scratch.frames.len(), scratch.scopes.len()), (0, 0));
    }
}

#[test]
fn root_publication_matches_independent_previous_driver_on_success_and_late_failure() {
    let program = program(crate::hir::iterative_validator_tests::SOURCE);
    for function in &program.functions {
        for hostile in [false, true] {
            let mut body = function.body.clone();
            if hostile {
                if let ResolvedExprKind::Block { tail, .. } = &mut body.kind {
                    tail.id = ExpressionId::from_owned("forged.tail".to_owned());
                } else {
                    body.ownership = OwnershipMode::Borrow;
                }
            }
            let execution = FunctionExecutionId::Monomorphic(function.id.clone());
            let mut scope = BTreeMap::new();
            for parameter in &function.params {
                scope.insert(
                    parameter.id.clone(),
                    ValidationBinding {
                        contract_local: false,
                        mutable: false,
                        ty: parameter.ty.clone(),
                        ownership: parameter.ownership,
                        availability: Availability::Available,
                        active_loans: BTreeSet::new(),
                        moved_places: BTreeMap::new(),
                        definitely_partial: BTreeSet::new(),
                    },
                );
            }
            let mut expected_scope = scope.clone();
            let mut current = HirValidator::new(&program).unwrap();
            let mut old = current.clone();
            let effects = function.effects.iter().cloned().collect();
            let expected = old.validate_expr_before_frame_reuse(
                &execution,
                &body,
                &mut expected_scope,
                "body".into(),
                true,
                Some(effect_keys::Allowed::Owned(&effects)),
            );
            let mut scratch = Scratch::new().unwrap();
            argument().unwrap();
            let actual = current.validate_expr_with_frames(
                &execution,
                &body,
                &mut scope,
                "body".into(),
                true,
                Some(effect_keys::Allowed::Owned(&effects)),
                Some(&mut scratch),
            );
            HirValidator::assert_validation_oracle(
                &actual,
                &expected,
                &current,
                &old,
                &scope,
                &expected_scope,
                function.id.as_str(),
            );
            HirValidator::assert_scope_auxiliary_oracle(&current, &old);
            assert_eq!((scratch.frames.len(), scratch.scopes.len()), (0, 0));
        }
    }
}

fn directory_run(
    reuse: bool,
    roots: usize,
    limit: usize,
) -> (
    Result<(), Diagnostic>,
    crate::test_allocation_observer::Metrics,
    bool,
    usize,
) {
    let ((result, metrics), refused, paid, _) = crate::bounded_output::with_limit_usage_refusal(
        limit,
        || {
            measure(|| -> Result<(), Diagnostic> {
                let mut scratch = if reuse { Some(Scratch::new()?) } else { None };
                for _ in 0..roots {
                    let (mut frames, mut scopes) = if let Some(scratch) = &mut scratch {
                        scratch.take()?
                    } else {
                        (
                            scope_checkpoint::Storage::new(),
                            scope_checkpoint::Storage::new(),
                        )
                    };
                    for index in 0..31 {
                        frames.push(Frame::RestorePublication(index % 2 == 0))?;
                        scopes.push(scope_checkpoint::Scope::default())?;
                    }
                    for index in (0..31).rev() {
                        assert!(
                            matches!(frames.pop(), Some(Frame::RestorePublication(value)) if value == (index % 2 == 0))
                        );
                        assert!(scopes.pop().is_some());
                    }
                    if let Some(scratch) = &mut scratch {
                        scope_checkpoint::charge::reserve(Some(
                            std::mem::size_of_val(&frames) + std::mem::size_of_val(&scopes),
                        ))?;
                        scratch.restore(frames, scopes)?;
                    }
                }
                Ok(())
            })
        },
    );
    (result, metrics, refused, paid)
}

#[test]
fn actual_directory_backing_and_complete_drop_are_paid_with_exact_and_short_boundaries() {
    let (old, old_metrics, old_refused, old_paid) = directory_run(false, 32, LIMIT);
    let (new, metrics, refused, paid) = directory_run(true, 32, LIMIT);
    old.unwrap();
    new.unwrap();
    assert!(!old_refused && !refused);
    assert!(!old_metrics.invalid && !metrics.invalid);
    assert_eq!((metrics.live, old_metrics.live), (0, 0));
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated < old_metrics.allocated);
    assert!(paid >= metrics.allocated && old_paid >= old_metrics.allocated);
    assert!(paid < old_paid);
    assert!(directory_run(true, 32, paid).0.is_ok());
    let (short, dropped, refused, _) = directory_run(true, 32, paid - 1);
    assert!(short.is_err() && refused && !dropped.invalid);
    assert_eq!(dropped.live, 0);
    assert!(directory_run(true, 1, 0).0.is_err());
}

#[test]
fn retained_backing_authenticates_sticky_refusal_without_reacquisition_or_retry_credit() {
    let mut scratch = Scratch::new().unwrap();
    let (mut frames, scopes) = scratch.take().unwrap();
    frames.push(Frame::RestorePublication(true)).unwrap();
    frames.pop().unwrap();
    scratch.restore(frames, scopes).unwrap();
    let capacity = scratch.frames.capacity();
    let (_, refused, paid, receipt) = crate::bounded_output::with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert!(scratch.take().is_err());
        assert!(scratch.take().is_err());
    });
    assert!(refused && receipt.is_some());
    assert_eq!(paid, 0);
    assert_eq!(scratch.frames.capacity(), capacity);
    assert_eq!(scratch.frames.len(), 0);
}

fn whole_invocation(
    program: &ResolvedProgram,
    reuse: bool,
    limit: usize,
) -> (crate::test_allocation_observer::Metrics, bool, usize) {
    let mut validator = HirValidator::new(program).unwrap();
    let (((), metrics), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut scratch = if reuse {
                    Some(Scratch::new().unwrap())
                } else {
                    None
                };
                for function in &program.functions {
                    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
                    let result = if let Some(scratch) = &mut scratch {
                        argument().unwrap();
                        validator.validate_function_with_frames(function, &execution, Some(scratch))
                    } else {
                        validator.validate_function_before_frame_reuse(function, &execution)
                    };
                    result.unwrap();
                }
                drop(scratch);
                drop(validator);
            })
        });
    assert!(receipt.is_none());
    (metrics, refused, paid)
}

#[test]
fn whole_ordinary_invocation_observes_complete_allocations_and_drop_against_independent_old_driver()
{
    for (label, program, expects_saving) in [
        ("tiny", program("module tiny.reuse; @id(\"main\") fn main()->i64 {0}"), false),
        ("shallow", program("module shallow.reuse; @id(\"f\") fn f(x:i64)->i64 {x+1} @id(\"main\") fn main()->i64 {f(1)}"), false),
        ("repeated", scalar_program(), true),
        ("decimal", decimal_program(), true),
    ] {
        let (old, old_refused, old_paid) = whole_invocation(&program, false, LIMIT);
        let (current, refused, paid) = whole_invocation(&program, true, LIMIT);
        assert!(!old_refused && !refused && !old.invalid && !current.invalid);
        assert_eq!((old.live, current.live), (0, 0));
        assert_eq!(current.allocated, current.freed);
        assert!(current.allocated <= paid && current.highwater <= paid);
        if expects_saving {
            assert!(current.allocated < old.allocated, "{label}");
        }
        eprintln!("OPT912 scenario={label} old_paid={old_paid} new_paid={paid} old={old:?} current={current:?}");
        let (exact, refused, exact_paid) = whole_invocation(&program, true, paid);
        assert!(!refused && !exact.invalid);
        assert_eq!(exact_paid, paid);
        assert_eq!(exact.allocated, current.allocated);
    }
}

#[test]
fn floor_refusal_and_retry_preserve_first_receipt_and_empty_backing() {
    let mut scratch = Scratch::new().unwrap();
    let (_, refused, paid, first) = crate::bounded_output::with_limit_usage_refusal(LIMIT, || {
        assert!(crate::bounded_output::set_active_floor(LIMIT));
        assert!(scratch.take().is_err());
        let first = crate::bounded_output::active_remaining();
        crate::bounded_output::clear_active_floor();
        assert!(scratch.take().is_err());
        assert_eq!(crate::bounded_output::active_remaining(), first);
    });
    assert!(refused && first.is_some());
    assert_eq!(paid, 0);
    assert_eq!((scratch.frames.len(), scratch.scopes.len()), (0, 0));
}
