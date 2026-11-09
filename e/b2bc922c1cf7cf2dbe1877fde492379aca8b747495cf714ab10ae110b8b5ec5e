use super::super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    const ORIGINAL_SOURCE: &str = "module funded_arrays; @id(\"f\") fn f(a:bool,b:bool,c:bool,d:bool)->i64 {let x=if a {1}else{2};let y=if b {3}else{4};let z=if c {5}else{6};let w=if d {7}else{8};x+y+z+w}";
    // The original standalone-library rejection remains a complete negative.
    let rejected = crate::check(ORIGINAL_SOURCE, "funded-arrays.spx").unwrap_err();
    assert_eq!(
        rejected,
        [Diagnostic {
            code: "SPX-T105",
            severity: crate::diagnostic::Severity::Error,
            message: "executable module must define `fn main() -> i64`".into(),
            path: Some("funded-arrays.spx".to_owned()),
            span: Some(crate::ast::Span { start: 31, end: 167, line: 1, column: 32 }),
            help: Some("a module without `fn main() -> i64` is a library module: check it through the project that owns it with `semaprax check <project-dir>`, or add `main` to run this file alone".to_owned()),
        }]
    );
    // Preserve the complete original function and give it an executable caller.
    let source =
        format!("{ORIGINAL_SOURCE} @id(\"main\") fn main()->i64 {{f(true,false,true,false)}}");
    let checked = crate::check(&source, "funded-arrays.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "funded-arrays.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn same_error(actual: &Diagnostic, expected: &Diagnostic) {
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.severity, expected.severity);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.path, expected.path);
    assert_eq!(actual.span, expected.span);
    assert_eq!(actual.help, expected.help);
}

#[test]
fn funded_capacity_preserves_complete_main_hir_work_and_drop() {
    let program = program();
    for paid in [false, true] {
        let (((), allocation), refused, reserved, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    for function in &program.functions {
                        let mut old = if paid {
                            ReplayBudget::new()
                        } else {
                            ReplayBudget::unobserved()
                        };
                        let mut current = if paid {
                            ReplayBudget::new()
                        } else {
                            ReplayBudget::unobserved()
                        };
                        // The owning HIR enumerator consumes the exact preflight
                        // bank; a newly paid budget starts with no skeleton work.
                        reserve_program_skeleton_work(
                            &program,
                            std::iter::once(function),
                            &mut old,
                        )
                        .unwrap();
                        reserve_program_skeleton_work(
                            &program,
                            std::iter::once(function),
                            &mut current,
                        )
                        .unwrap();
                        let before = skeleton_materializations();
                        let expected =
                            super::main696::hir_skeleton_paths(&program, function, &mut old)
                                .unwrap();
                        let old_materializations = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = hir_skeleton_paths(&program, function, &mut current).unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(skeleton_materializations() - before, old_materializations);
                        assert_eq!(current.remaining, old.remaining);
                        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                        assert_eq!(current.skeleton_spent, old.skeleton_spent);
                        drop((actual, expected, current, old));
                    }
                })
            });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        if paid {
            assert!(allocation.allocated <= reserved);
        }
    }
}

#[test]
fn funded_capacity_exact_short_zero_and_first_output_work_frontier() {
    let program = program();
    let function = &program.functions[0];
    // Ordinary SkeletonWork inventories are immutable caller input here. Their
    // legacy collectors have unpriced traversal scratch: construct AND drop
    // them outside the array account/observer, keeping the borrow alive. This
    // probe owns seeds, append, partial paths, Diagnostics and the paid budget;
    // it does not claim funding for that independent ordinary setup.
    let observe = |old: bool, count: usize, output_work: usize, limit| {
        let mut caller_budget = ReplayBudget::unobserved();
        let mut work =
            SkeletonWork::with_index(&program, function, &mut caller_budget, None).unwrap();
        let observed = with_limit_usage_refusal(limit, || {
            measure(|| {
                // Bind the real array payer only after caller input exists.
                *work.budget = ReplayBudget::new();
                let result = (|| {
                    work.budget
                        .reserve_skeleton(function, count + output_work)?;
                    let mut input = Vec::new();
                    for _ in 0..count {
                        work.push_expr_path(&mut input, empty_expr_path(), "seed")?;
                    }
                    let mut target = Vec::new();
                    if old {
                        super::before::append_expr_paths(
                            &mut target,
                            input,
                            &mut work,
                            "funded append",
                        )?;
                    } else {
                        append_expr_paths(&mut target, input, &mut work, "funded append")?;
                    }
                    assert_eq!(target.len(), count);
                    assert!(target.iter().all(|path| path.observations.is_empty()
                        && path.owned_source.is_none()
                        && !path.failed
                        && !path.residual));
                    drop(target);
                    Ok::<_, Diagnostic>(())
                })();
                let remaining = work.budget.skeleton_remaining;
                let spent = work.budget.skeleton_spent;
                // A second selected failure cannot replace or allocate the first.
                let retry = if result.is_err() {
                    Some(work.budget.reserve_bytes(function, 0, "retry"))
                } else {
                    None
                };
                let failed = result.is_err();
                if let Some(Err(retry)) = retry.as_ref() {
                    assert_eq!(retry.code, "SPX-H006");
                    assert_eq!(retry.severity, crate::diagnostic::Severity::Error);
                    assert!(retry.message.is_empty());
                    assert!(retry.path.is_none() && retry.span.is_none() && retry.help.is_none());
                }
                let retry_failed = retry.as_ref().is_some_and(Result::is_err);
                // All paid-budget backing and partial failure owners die in
                // this same observer. The replacement is an unobserved empty
                // placeholder for the still-borrowed ordinary caller context.
                let budget = std::mem::replace(work.budget, ReplayBudget::unobserved());
                drop((result, retry, budget));
                (failed, retry_failed, remaining, spent)
            })
        });
        drop(work);
        drop(caller_budget);
        observed
    };
    for count in [0usize, 1, 7, 8, 9, 17] {
        let ((old, old_allocation), old_refused, old_paid, old_first) =
            observe(true, count, count, usize::MAX);
        let ((actual, allocation), refused, paid, first) = observe(false, count, count, usize::MAX);
        assert!(!old.0 && !actual.0);
        assert!(!old_refused && !refused && old_first.is_none() && first.is_none());
        assert_eq!((actual.2, actual.3), (old.2, old.3));
        eprintln!("funded capacity count={count} old_paid={old_paid} current_paid={paid} old_allocated={} current_allocated={}", old_allocation.allocated, allocation.allocated);
        for limit in [0, paid - 1, paid] {
            let ((outcome, allocation), refused, reserved, first) =
                observe(false, count, count, limit);
            assert_eq!(!outcome.0 && !refused, limit == paid);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= reserved);
            if limit != paid {
                assert!(first.is_some());
                assert!(outcome.1);
            }
        }
        assert!(!old_allocation.invalid && old_allocation.live == 0);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(old_allocation.allocated, old_allocation.freed);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    // Both routes have genuine zero first-output work, after input work.
    // Compare the complete selected diagnostics before dropping both owners.
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut errors = Vec::new();
            for old in [true, false] {
                let mut budget = ReplayBudget::new();
                budget.reserve_skeleton(function, 8).unwrap();
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let mut input = Vec::new();
                for _ in 0..8 {
                    work.push_expr_path(&mut input, empty_expr_path(), "seed")
                        .unwrap();
                }
                let mut target = Vec::new();
                let error = if old {
                    super::before::append_expr_paths(&mut target, input, &mut work, "funded append")
                } else {
                    append_expr_paths(&mut target, input, &mut work, "funded append")
                }
                .unwrap_err();
                assert!(target.is_empty());
                assert_eq!(work.budget.skeleton_spent, 8);
                assert_eq!(work.budget.skeleton_remaining, 0);
                let retry = work.budget.reserve_bytes(function, 0, "retry").unwrap_err();
                errors.push((error, retry));
                drop((target, work));
                drop(budget);
            }
            same_error(&errors[0].0, &errors[1].0);
            same_error(&errors[0].1, &errors[1].1);
            drop(errors);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    // The diagnostic comparison Vec is a test observer, outside producer fees.
    assert!(paid > 0);
}
