//! Independent original receivers and physical scratch controls; source-only pending execution.
use super::*;
use crate::hir::*;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
fn program() -> ResolvedProgram {
    let ast = crate::check(
        crate::hir::iterative_validator_tests::SOURCE,
        "ordinary-scans.spx",
    )
    .unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn wide(function: &ResolvedFunction, width: usize) -> ResolvedFunction {
    let mut function = function.clone();
    let mut leaf = function.body.clone();
    leaf.kind = ResolvedExprKind::Bool(false);
    function.body.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new("scan-only"),
        type_arguments: Vec::new(),
        instance: None,
        args: vec![leaf; width],
    };
    function
}
#[test]
fn complete_iterator_and_renewal_receivers_match_every_original_corpus_function() {
    let program = program();
    for function in &program.functions {
        assert_eq!(
            crate::hir::iterator_loop::validate_function_paid(function),
            crate::hir::iterator_loop::validate_function(function)
        );
        assert_eq!(
            crate::hir::iterator_loop::function_has_record_renewal_outside_loop_paid(
                &program, function
            )
            .unwrap(),
            crate::hir::iterator_loop::function_has_record_renewal_outside_loop(&program, function)
        );
    }
}
#[test]
fn borrowed_child_append_is_exact_on_the_complete_original_corpus() {
    let program = program();
    for function in &program.functions {
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            let mut old = Vec::new();
            crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut old);
            let mut current = Vec::new();
            children(expression, &mut |child| {
                current.push(child);
                Ok(())
            })
            .unwrap();
            assert_eq!(current, old);
            for refused_at in 0..old.len() {
                let mut prefix = Vec::new();
                let mut visits = 0;
                let result = children(expression, &mut |child| {
                    let ordinal = visits;
                    visits += 1;
                    if ordinal == refused_at {
                        return Err(Diagnostic::io_static("SPX-H006", "injected child refusal"));
                    }
                    prefix.push(child);
                    Ok(())
                });
                assert!(result.is_err());
                assert_eq!(visits, refused_at + 1);
                assert_eq!(prefix, old[..refused_at]);
            }
            pending.extend(old);
        }
    }
}
#[test]
fn inline_spill_exact_short_sticky_and_complete_drop_use_the_actual_payer() {
    let lifetime = |limit, count| {
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let mut stack = Stack::new()?;
                for i in 0..count {
                    stack.push(i)?;
                }
                assert_eq!(stack.iter().count(), count);
                for i in (0..count).rev() {
                    assert_eq!(stack.pop(), Some(i));
                }
                drop(stack);
                Ok::<_, Diagnostic>(())
            })
        })
    };
    for count in [0, 1, 16, 17, 65, 1024] {
        let ((result, metrics), refused, paid) = lifetime(LIMIT, count);
        assert!(result.is_ok() && !refused && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
        if count <= 16 {
            assert_eq!(metrics.allocations, 0);
            assert_eq!(paid, 0);
        } else {
            let ((result, exact), refused, exact_paid) = lifetime(paid, count);
            assert!(result.is_ok() && !refused && !exact.invalid);
            assert_eq!(exact_paid, paid);
            assert_eq!(exact.live, 0);
            let ((result, short), refused, _) = lifetime(paid - 1, count);
            assert!(result.is_err() && refused && !short.invalid);
            assert_eq!(short.live, 0);
            let ((result, zero), refused, _) = lifetime(0, count);
            assert!(result.is_err() && refused && !zero.invalid);
            assert_eq!(zero.live, 0);
        }
    }
    // Spill backing remains owned across inline reentry; storage reuse does not refund debits.
    let ((result, metrics), refused, paid) = crate::bounded_output::with_limit_usage(LIMIT, || {
        measure(|| {
            let mut stack = Stack::new()?;
            for i in 0..20 {
                stack.push(i)?;
            }
            for i in (14..20).rev() {
                assert_eq!(stack.pop(), Some(i));
            }
            for i in 14..20 {
                stack.push(i)?;
            }
            assert!(stack.iter().copied().eq(0..20));
            for i in (0..20).rev() {
                assert_eq!(stack.pop(), Some(i));
            }
            drop(stack);
            Ok::<_, Diagnostic>(())
        })
    });
    assert!(result.is_ok() && !refused && !metrics.invalid);
    assert_eq!(metrics.allocations, 1);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid);
    let (sticky, refused, _) = crate::bounded_output::with_limit_usage(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        Stack::<usize>::new().is_err()
    });
    assert!(sticky && refused);
}
#[test]
fn paid_preflight_wide_bodies_keep_complete_saved_verdicts_through_drop() {
    let program = program();
    for count in [0, 1, 16, 17, 65, 1024] {
        let function = wide(&program.functions[0], count);
        let old_iterator = crate::hir::iterator_loop::validate_function(&function);
        let old_renewal = crate::hir::iterator_loop::function_has_record_renewal_outside_loop(
            &program, &function,
        );
        let (((iterator, renewal), metrics), refused, paid) =
            crate::bounded_output::with_limit_usage(LIMIT, || {
                measure(|| {
                    let iterator = crate::hir::iterator_loop::validate_function_paid(&function);
                    let renewal =
                        crate::hir::iterator_loop::function_has_record_renewal_outside_loop_paid(
                            &program, &function,
                        );
                    (iterator, renewal)
                })
            });
        assert_eq!(iterator, old_iterator);
        assert_eq!(renewal.unwrap(), old_renewal);
        assert!(!refused && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    }
}
#[test]
fn complete_old_preflight_bodies_and_real_function_scope_paid_joins_are_locked() {
    let saved = include_str!("../../../docs/evidence/ordinary-hir-scans/original-scans.txt");
    let source = [
        include_str!("../iterator_loop.rs"),
        include_str!("../iterator_loop/renewal.rs"),
        include_str!("../validation/unsafe_scan.rs"),
        concat!(
            include_str!("../byte_capacity.rs"),
            include_str!("../byte_capacity/fused_selection.rs"),
            include_str!("../../byte_data_capacity.rs"),
            include_str!("../../byte_data_capacity/witnesses.rs"),
        ),
    ];
    // Each archive delimiter marks a full independently compiled definition.
    for (old, current) in saved
        .trim_end()
        .split("\n\n// NEXT COMPLETE RECEIVER\n\n")
        .zip(source)
    {
        assert!(current.contains(old), "complete original receiver changed");
    }
    assert_eq!(
        saved
            .trim_end()
            .split("\n\n// NEXT COMPLETE RECEIVER\n\n")
            .count(),
        4
    );
    let dispatch = include_str!("../validation/function_scope.rs");
    assert_eq!(
        dispatch
            .matches("self.validate_function_with_frames(function, execution, None)")
            .count(),
        1
    );
    let production = include_str!("../validation/frame_reuse/function.rs");
    assert!(production.contains("iterator_loop::validate_function_paid(function)?"));
    assert!(production.contains("owner_renewal::validate_function_paid(self.program"));
    assert!(production.contains("unsafe_scan::contains_unsafe_boundary_paid(&function.body)?"));
}

#[test]
fn borrowed_authorization_directory_is_funded_before_each_physical_insert_and_drop() {
    let ids: Vec<_> = (0..65)
        .map(|i| ExpressionId::from_owned(format!("scan.{i}")))
        .collect();
    let run = |limit| {
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let mut set = BTreeSet::new();
                for id in &ids {
                    insert(&mut set, id)?;
                }
                for id in &ids {
                    assert!(set.contains(&id));
                }
                drop(set);
                Ok::<_, Diagnostic>(())
            })
        })
    };
    let ((result, metrics), refused, paid) = run(LIMIT);
    assert!(result.is_ok() && !refused && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    let ((result, exact), refused, exact_paid) = run(paid);
    assert!(result.is_ok() && !refused && !exact.invalid);
    assert_eq!(exact_paid, paid);
    assert_eq!(exact.live, 0);
    let ((result, short), refused, _) = run(paid - 1);
    assert!(result.is_err() && refused && !short.invalid);
    assert_eq!(short.live, 0);
}
#[test]
fn full_record_renewal_loop_state_and_outside_assignment_keep_the_original_answer() {
    let source = r#"module record.renewal;
@id("cursor") record Cursor { @id("cursor.bytes") bytes: Bytes, @id("cursor.index") index: usize, }
@id("advance") fn advance(value: own Cursor)->Cursor { value }
@id("main") fn main()->i64 {
 let mut cursor=Cursor{bytes:bytes_zeroed(0usize),index:0usize};
 while false { cursor=advance(cursor); continue; }
 0
}"#;
    let ast = crate::check(source, "scan-renewal.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    for outside in [false, true] {
        let mut function = function.clone();
        if outside {
            let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
                panic!("block");
            };
            let (ResolvedStatement::While { body, .. }
            | ResolvedStatement::LexicalWhile { body, .. }) = &statements[1]
            else {
                panic!("while");
            };
            let ResolvedExprKind::Block {
                statements: inner, ..
            } = &body.kind
            else {
                panic!("loop block");
            };
            let assignment = inner[0].clone();
            statements.insert(1, assignment);
        }
        let old = crate::hir::iterator_loop::function_has_record_renewal_outside_loop(
            &program, &function,
        );
        assert_eq!(old, outside);
        assert_eq!(
            crate::hir::iterator_loop::function_has_record_renewal_outside_loop_paid(
                &program, &function
            )
            .unwrap(),
            old
        );
    }
}

#[test]
fn delayed_unauthorized_replacement_keeps_later_seed_errors_and_full_static_diagnostics() {
    let source = r#"module scan.iterator;
@id("main") fn main()->i64 {
 for own item in vec_into_iter<i64>(vec_with_capacity<i64>(0usize)) {0}
 0
}"#;
    let ast = crate::check(source, "scan-iterator.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let original = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let mut pending = vec![&original.body];
    let mut wrapper = None;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            if statements.iter().any(|s| match s {
                ResolvedStatement::While {
                    condition, body, ..
                }
                | ResolvedStatement::LexicalWhile {
                    condition, body, ..
                } => crate::hir::iterator_loop::recognize(condition, body).is_some(),
                _ => false,
            }) {
                wrapper = Some(expression.clone());
                break;
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    let wrapper = wrapper.expect("retained iterator wrapper");
    let ResolvedExprKind::Block { statements, .. } = &wrapper.kind else {
        panic!("wrapper");
    };
    let ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("Step seed");
    };
    let binding = binding.clone();
    for fault in ["unauthorized", "seed", "slot"] {
        let mut function = original.clone();
        let mut value = original.body.clone();
        value.kind = ResolvedExprKind::Int(0);
        let mut statements = vec![ResolvedStatement::Assign {
            binding: binding.clone(),
            field: None,
            value: value.clone(),
            span: value.span,
        }];
        if fault != "unauthorized" {
            let mut bad = wrapper.clone();
            let ResolvedExprKind::Block {
                statements: nested, ..
            } = &mut bad.kind
            else {
                panic!("wrapper");
            };
            if fault == "slot" {
                nested.remove(0);
            } else {
                let ResolvedStatement::Let { value: seed, .. } = &mut nested[0] else {
                    panic!("seed");
                };
                seed.ty = ResolvedType::I64;
            }
            statements.push(ResolvedStatement::CopyDiscard {
                span: bad.span,
                value: bad,
            });
        }
        function.body.kind = ResolvedExprKind::Block {
            statements,
            tail: Box::new(value),
        };
        let expected = crate::hir::iterator_loop::validate_function(&function).unwrap_err();
        let ((actual, metrics), refused, paid) =
            crate::bounded_output::with_limit_usage(LIMIT, || {
                measure(|| crate::hir::iterator_loop::validate_function_paid(&function))
            });
        assert_eq!(actual.unwrap_err(), expected);
        assert!(!refused && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid);
        assert!(expected.message.contains(match fault {
            "unauthorized" => "outside an authenticated",
            "seed" => "seed, slot, or wrapper",
            _ => "retain one mutable",
        }));
    }
}
