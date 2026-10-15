use super::*;
use crate::ast::{Expr, ExprKind, Program, Span, TypeDeclarationKind};
use crate::test_allocation_observer::{measure, watch_free_order};

fn fixture(classes: usize, methods: usize) -> Program {
    let mut source = String::from("module drop.methods;");
    for class in 0..classes {
        source.push_str(&format!("@id(\"drop.class{class}\") class C{class} {{"));
        source.push_str(&format!("@id(\"drop.field{class}\") value:i64,"));
        for method in 0..methods {
            source.push_str(&format!(
                "@id(\"drop.method{class}.{method}\") fn m{method}(self:C{class})->i64 {{1}}"
            ));
        }
        source.push('}');
    }
    source.push_str("@id(\"drop.main\") fn main()->i64 {0}");
    let mut program = crate::parse(&source, "drop-methods.spx").unwrap();
    program.functions.shrink_to_fit();
    program
}

#[test]
fn existing_method_backing_fifo_inline_and_wide_spill_preserve_every_element() {
    for width in [0, 1, 16, 17, 129] {
        let values: Vec<_> = (0..width)
            .map(|index| vec![index * 2, index * 2 + 1])
            .collect();
        let expected: Vec<_> = values.iter().flatten().copied().collect();
        let (actual, metrics) = measure(|| {
            let mut roots = MethodRoots::new();
            for value in values {
                roots.push(value);
            }
            let mut count = 0;
            for (index, value) in roots.into_iter().enumerate() {
                assert_eq!(value, expected[index]);
                count += 1;
            }
            count
        });
        assert_eq!(actual, expected.len());
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        if width <= INLINE {
            assert_eq!((metrics.allocations, metrics.allocated), (0, 0));
        } else {
            assert!(metrics.allocations > 0 && metrics.allocated > 0);
        }
    }
}

#[test]
fn class_method_drop_keeps_full_prior_and_original_metadata_and_expression_order() {
    const WATCHED: usize = 20;
    let run = |before: bool, original: bool| {
        let (((), order), metrics) = measure(|| {
            let mut program = fixture(2, 2);
            let mut pointers = [0usize; WATCHED];
            pointers[0] = program.path.as_ptr() as usize;
            pointers[1] = program.module.as_ptr() as usize;
            pointers[2] = program.functions[0].stable_id.as_ptr() as usize;
            program.functions[0].body = Expr {
                kind: ExprKind::String("root body".to_owned()),
                span: Span::default(),
            };
            let ExprKind::String(text) = &program.functions[0].body.kind else {
                unreachable!()
            };
            pointers[3] = text.as_ptr() as usize;
            let mut next = 4;
            for declaration in &mut program.types {
                pointers[next] = declaration.stable_id.as_ptr() as usize;
                pointers[next + 1] = declaration.name.as_ptr() as usize;
                next += 2;
                let TypeDeclarationKind::Class { fields, methods } = &mut declaration.kind else {
                    unreachable!()
                };
                pointers[next] = fields[0].stable_id.as_ptr() as usize;
                pointers[next + 1] = fields[0].name.as_ptr() as usize;
                next += 2;
                for method in methods {
                    pointers[next] = method.stable_id.as_ptr() as usize;
                    method.body = Expr {
                        kind: ExprKind::String(format!("body.{next}")),
                        span: Span::default(),
                    };
                    let ExprKind::String(text) = &method.body.kind else {
                        unreachable!()
                    };
                    pointers[next + 1] = text.as_ptr() as usize;
                    next += 2;
                }
            }
            assert_eq!(next, pointers.len());
            watch_free_order(&pointers, || {
                if before {
                    drop(crate::ast::drop_method_original::Original(program));
                } else if original {
                    drop(crate::ast::drop_original::Original(program));
                } else {
                    drop(program);
                }
            })
        });
        assert!(!metrics.invalid && !order.invalid);
        assert_eq!(order.count, WATCHED);
        assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
        order
    };
    let current = run(false, false);
    for other in [run(true, false), run(false, true)] {
        assert_eq!(&current.order[..current.count], &other.order[..other.count]);
    }
}

#[test]
fn actual_small_class_method_drop_avoids_function_accumulator_growth_and_wide_drop_completes() {
    for (classes, methods) in [(1, 1), (1, 17), (16, 2), (17, 2), (129, 2)] {
        for root_function in [false, true] {
            let mut current = fixture(classes, methods);
            let mut before = fixture(classes, methods);
            if !root_function {
                current.functions.clear();
                current.functions.shrink_to_fit();
                before.functions.clear();
                before.functions.shrink_to_fit();
            }
            let (_, actual) = measure(|| drop(current));
            let (_, prior) = measure(|| drop(crate::ast::drop_method_original::Original(before)));
            for metrics in [actual, prior] {
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
            if classes <= INLINE {
                assert!(prior.allocated > actual.allocated);
                if classes == 1 && methods == 1 {
                    assert_eq!((actual.allocations, actual.allocated), (0, 0));
                }
            } else {
                assert!(actual.allocated > 0); // explicit ordinary queue spill
            }
        }
    }
}

#[test]
fn complete_prior_program_drop_and_new_method_root_source_projection_remain_exact() {
    let restored = super::restore_source(ci_str!("../../ast.rs").to_owned());
    assert_eq!(
        restored,
        ci_str!("../../../docs/evidence/class-method-drop-48e487/ast.rs.before.txt")
    );
    let original = ci_str!("../drop_method_original.rs");
    let start = original.find("impl Drop for Original {").unwrap();
    let end = original[start..].find("\n// This lexical adapter").unwrap() + start;
    assert_eq!(
        original[start..end]
            .trim_end()
            .replace("impl Drop for Original {", "impl Drop for Program {"),
        ci_str!("../../../docs/evidence/class-method-drop-48e487/program-drop.before.txt")
            .trim_end()
    );
    assert_eq!(
        ci_str!("../drop_original_tests.rs"),
        ci_str!(
            "../../../docs/evidence/custody-ast-drop-frontier/iterative-program-drop-tests.before.txt"
        )
    );
    let source = concat!(
        include_str!("../drop_method_roots.rs"),
        include_str!("tests.rs"),
        include_str!("../drop_method_original.rs"),
        include_str!("../drop_original_tests.rs")
    );
    assert!(source.contains(".chain(self.spill)"));
    assert!(source.contains("impl Drop for Original"));
    assert!(
        source.contains("program_drop_is_iterative_for_every_recursive_ast_root_at_exact_depth")
    );
}
