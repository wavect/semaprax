use super::super::*;
use crate::bounded_output::{active_remaining, set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    let mut bytes = [0; 1024];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    let result = Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    };
    drop(value);
    result
}

fn program() -> ResolvedProgram {
    let source = "module completed.funded; @id(\"f\") fn f(a:bool,b:bool,c:bool)->i64 {let x=if a{1}else{2};let y=if b{3}else{4};if c{x+y}else{x-y}} @id(\"main\") fn main()->i64 {f(true,false,true)}";
    let parsed = crate::check(source, "completed-funded.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let parsed = crate::check(&canonical, "completed-funded.spx").unwrap();
    assert_eq!(crate::format::canonical(&parsed), canonical);
    crate::hir::resolve(&parsed).unwrap()
}

#[test]
fn lexical_completed_capacity_keeps_full_hir_outputs_work_and_drop() {
    let program = program();
    for function in &program.functions {
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut old = ReplayBudget::new();
                let mut current = ReplayBudget::new();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut old)
                    .unwrap();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut current)
                    .unwrap();
                let materializations = skeleton_materializations();
                let expected =
                    super::main696::hir_skeleton_paths(&program, function, &mut old).unwrap();
                let old_materializations = skeleton_materializations() - materializations;
                let materializations = skeleton_materializations();
                let actual = hir_skeleton_paths(&program, function, &mut current).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(
                    skeleton_materializations() - materializations,
                    old_materializations
                );
                assert_eq!(current.skeleton_spent, old.skeleton_spent);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                assert_eq!(current.remaining, old.remaining);
                drop((actual, expected, old, current));
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}

#[test]
fn lexical_completed_first_push_preserves_work_before_count_floor_and_real_bounds() {
    let program = program();
    let function = &program.functions[0];
    for count in [1usize, 7, 8, super::COMPLETED_MINIMUM, 17, 512] {
        let run = |old, limit, output_work, floor| {
            let mut caller = ReplayBudget::unobserved();
            let mut work = SkeletonWork::new(&program, function, &mut caller);
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    *work.budget = ReplayBudget::new();
                    let result = (|| {
                        work.budget.prepare_builder(function)?;
                        work.budget.reserve_skeleton(function, output_work)?;
                        let mut output = Vec::new();
                        let mut remaining = (0..count)
                            .map(|_| empty_expr_path())
                            .collect::<Vec<_>>()
                            .into_iter();
                        let path = remaining.next().unwrap();
                        if floor {
                            assert!(set_active_floor(active_remaining().unwrap()));
                        }
                        if old {
                            work.push_skeleton_path(
                                &mut output,
                                SkeletonPath {
                                    observations: path.observations,
                                    terminal: SkeletonTerminal::Success,
                                },
                                "completed HIR skeleton path",
                            )?;
                        } else {
                            super::completed_first!(
                                work,
                                remaining,
                                output,
                                SkeletonPath {
                                    observations: path.observations,
                                    terminal: SkeletonTerminal::Success,
                                },
                                "completed HIR skeleton path"
                            );
                        }
                        for path in remaining {
                            work.push_skeleton_path(
                                &mut output,
                                SkeletonPath {
                                    observations: path.observations,
                                    terminal: SkeletonTerminal::Success,
                                },
                                "completed HIR skeleton path",
                            )?;
                        }
                        assert_eq!(output.len(), count);
                        assert!(output.iter().all(|p| p.observations.is_empty()));
                        let capacity = output.capacity();
                        drop(output);
                        Ok::<_, Diagnostic>(capacity)
                    })();
                    let result = result.map_err(failure);
                    let work_spent = work.budget.skeleton_spent;
                    let retry = if result.is_err() {
                        Some(
                            work.budget
                                .reserve_bytes(function, 0, "completed retry")
                                .map_err(failure),
                        )
                    } else {
                        None
                    };
                    *work.budget = ReplayBudget::unobserved();
                    (result, work_spent, retry)
                })
            })
        };
        // Real work failure selects before both the first count bank and the
        // original backing request, even with no physical headroom above floor.
        let ((old, _), _, _, _) = run(true, 1 << 24, 0, true);
        let ((current, _), _, _, _) = run(false, 1 << 24, 0, true);
        assert_eq!(current.0, old.0, "all six Diagnostic fields");
        assert_eq!(current.1, 0);
        assert!(current.2.unwrap().is_err());
        let ((ample, allocation), refused, exact, first) = run(false, usize::MAX, count, false);
        assert!(ample.0.is_ok() && !refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        for limit in [0, exact - 1, exact] {
            let ((outcome, allocation), refused, paid, first) = run(false, limit, count, false);
            assert_eq!(outcome.0.is_ok(), limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            // Input iterator construction above is ordinary immutable fixture
            // data, so this owner reports rather than claims its byte funding.
            eprintln!("completed count={count} limit={limit} reserved={paid} heap={allocation:?}");
        }
    }
}
