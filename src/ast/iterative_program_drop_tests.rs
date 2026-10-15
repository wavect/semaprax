use super::*;
use std::path::Path;

const EXACT_DEPTH: usize = 512;

fn nested_type() -> Type {
    let mut ty = Type::I64;
    for index in 0..EXACT_DEPTH {
        ty = Type::Named {
            name: format!("T{index}"),
            arguments: vec![ty],
        };
    }
    ty
}

fn nested_expression() -> Expr {
    let mut expression = Expr {
        kind: ExprKind::Int(0),
        span: Span::default(),
    };
    for _ in 0..EXACT_DEPTH {
        expression = Expr {
            kind: ExprKind::Unary {
                op: UnaryOp::Neg,
                value: Box::new(expression),
            },
            span: Span::default(),
        };
    }
    expression
}

fn nested_if_expression() -> Expr {
    let span = Span::default();
    let mut expression = Expr {
        kind: ExprKind::Bool(false),
        span,
    };
    for _ in 0..EXACT_DEPTH {
        expression = Expr {
            kind: ExprKind::If {
                condition: Box::new(Expr {
                    kind: ExprKind::Bool(true),
                    span,
                }),
                then_branch: Box::new(expression),
                else_branch: Box::new(Expr {
                    kind: ExprKind::Bool(false),
                    span,
                }),
            },
            span,
        };
    }
    expression
}

fn nested_record_pattern() -> MatchPattern {
    let span = Span::default();
    let mut pattern = RecordMatchFieldPattern::Binding {
        name: "value".to_owned(),
        span,
    };
    for index in 0..EXACT_DEPTH {
        pattern = RecordMatchFieldPattern::Record {
            type_name: format!("R{index}"),
            type_span: span,
            fields: vec![RecordMatchPatternField {
                name: "next".to_owned(),
                name_span: span,
                pattern,
                span,
            }],
            span,
        };
    }
    MatchPattern::Record {
        type_name: "Root".to_owned(),
        type_span: span,
        fields: vec![RecordMatchPatternField {
            name: "next".to_owned(),
            name_span: span,
            pattern,
            span,
        }],
        span,
    }
}

fn nested_or_pattern() -> MatchPattern {
    let span = Span::default();
    let mut pattern = MatchPattern::Literal {
        value: PatternLiteral::Int(0),
        span,
    };
    for _ in 0..EXACT_DEPTH {
        pattern = MatchPattern::Or {
            alternatives: vec![pattern],
            span,
        };
    }
    pattern
}

#[test]
fn program_drop_is_iterative_for_every_recursive_ast_root_at_exact_depth() {
    let span = Span::default();
    let mut program = crate::parse(
        "module ast.drop; @id(\"ast.drop.method\") fn method() -> i64 { 0 }",
        Path::new("ast-drop.spx"),
    )
    .unwrap();
    let mut method = program.functions.pop().unwrap();
    method.return_type = nested_type();
    method.requires.push(Expr {
        kind: ExprKind::Match {
            mode: MatchMode::Value,
            scrutinee: Box::new(Expr {
                kind: ExprKind::Int(0),
                span,
            }),
            arms: vec![
                MatchArm {
                    pattern: nested_record_pattern(),
                    guard: None,
                    value: Expr {
                        kind: ExprKind::Int(0),
                        span,
                    },
                    span,
                },
                MatchArm {
                    pattern: nested_or_pattern(),
                    guard: Some(Box::new(nested_expression())),
                    value: Expr {
                        kind: ExprKind::Int(0),
                        span,
                    },
                    span,
                },
            ],
        },
        span,
    });
    method.body = Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: Box::new(nested_expression()),
            right: Box::new(nested_if_expression()),
        },
        span,
    };
    program.types.push(TypeDeclaration {
        stable_id: "ast.drop.class".to_owned(),
        explicit_id: true,
        name: "DropClass".to_owned(),
        name_span: span,
        type_parameters: Vec::new(),
        kind: TypeDeclarationKind::Class {
            fields: vec![FieldDeclaration {
                stable_id: "ast.drop.class.value".to_owned(),
                explicit_id: true,
                name: "value".to_owned(),
                name_span: span,
                ty: nested_type(),
                span,
            }],
            methods: vec![method],
        },
        extends: Some(nested_type()),
        invariants: None,
        span,
    });

    // Normal lexical scope owns teardown: no leak, subprocess, or stack
    // configuration may mask a recursively dropped AST carrier.
    drop(program);
}

#[test]
fn inline_program_teardown_and_zero_refusal_input_keep_full_owned_roots_without_acquisition() {
    for original in [false, true] {
        let program = crate::parse(
            "module drop.small; @id(\"drop.small.keep\") fn keep(value:i64)->i64 {value+1} @id(\"drop.small.unused\") fn unused()->i64 {99}",
            "small-drop.spx",
        ).unwrap();
        let (((), metrics), refused) = crate::bounded_output::with_limit(0, || {
            crate::test_allocation_observer::measure(|| {
                if original {
                    std::mem::drop(crate::ast::drop_original::Original(program));
                } else {
                    std::mem::drop(program);
                }
            })
        });
        assert!(!refused && !metrics.invalid);
        assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
        if original {
            assert!(metrics.allocated > 0);
        } else {
            assert_eq!(metrics.allocated, 0);
        }
    }
}

#[test]
fn complete_program_drop_keeps_original_actual_function_body_and_metadata_free_order() {
    let run = |original| {
        let (((), order), metrics) = crate::test_allocation_observer::measure(|| {
            let mut program = crate::parse(
                "module drop.order; @id(\"drop.order.first\") fn first()->i64 {0} @id(\"drop.order.second\") fn second()->i64 {0} @id(\"drop.order.third\") fn third()->i64 {0}",
                "drop-order.spx",
            ).unwrap();
            let mut pointers = [0usize; 8];
            pointers[0] = program.path.as_ptr() as usize;
            pointers[1] = program.module.as_ptr() as usize;
            for (index, function) in program.functions.iter_mut().enumerate() {
                pointers[2 + index] = function.stable_id.as_ptr() as usize;
                let text = format!("body.{index}.é");
                pointers[5 + index] = text.as_ptr() as usize;
                function.body = Expr {
                    kind: ExprKind::String(text),
                    span: Span::default(),
                };
            }
            crate::test_allocation_observer::watch_free_order(&pointers, || {
                if original {
                    std::mem::drop(crate::ast::drop_original::Original(program));
                } else {
                    std::mem::drop(program);
                }
            })
        });
        assert!(!metrics.invalid && !order.invalid);
        assert_eq!(order.count, 8);
        assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
        order
    };
    let original = run(true);
    let current = run(false);
    assert_eq!(
        &current.order[..current.count],
        &original.order[..original.count]
    );
}

#[test]
fn ordinary_spill_teardown_keeps_exact_depth_wide_patterns_and_no_recursive_fallback() {
    for original in [false, true] {
        let mut program = crate::parse(
            "module drop.wide; @id(\"drop.wide.main\") fn main()->i64 {0}",
            "wide-drop.spx",
        )
        .unwrap();
        let function = &mut program.functions[0];
        function.return_type = nested_type();
        function.body = nested_if_expression();
        function.requires.extend((0..257).map(|value| Expr {
            kind: ExprKind::Int(value),
            span: Span::default(),
        }));
        function.ensures.push(Expr {
            kind: ExprKind::Match {
                mode: MatchMode::Value,
                scrutinee: Box::new(Expr {
                    kind: ExprKind::Int(0),
                    span: Span::default(),
                }),
                arms: vec![MatchArm {
                    pattern: nested_record_pattern(),
                    guard: Some(Box::new(nested_expression())),
                    value: Expr {
                        kind: ExprKind::Match {
                            mode: MatchMode::Value,
                            scrutinee: Box::new(Expr {
                                kind: ExprKind::Int(0),
                                span: Span::default(),
                            }),
                            arms: vec![MatchArm {
                                pattern: nested_or_pattern(),
                                guard: None,
                                value: Expr {
                                    kind: ExprKind::Int(0),
                                    span: Span::default(),
                                },
                                span: Span::default(),
                            }],
                        },
                        span: Span::default(),
                    },
                    span: Span::default(),
                }],
            },
            span: Span::default(),
        });
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            if original {
                std::mem::drop(crate::ast::drop_original::Original(program));
            } else {
                std::mem::drop(program);
            }
        });
        assert!(!metrics.invalid && metrics.allocated > 0);
        assert_eq!((metrics.live, metrics.allocated), (0, metrics.freed));
    }
}

#[test]
fn entire_program_source_and_original_destructor_test_stay_exact_with_four_carrier_hooks() {
    let mut current = super::drop_method_roots::restore_source(ci_str!("../ast.rs").to_owned());
    let registration = "#[path = \"ast/drop_frontier.rs\"]\nmod drop_frontier;\n#[cfg(test)]\n#[path = \"ast/drop_original.rs\"]\nmod drop_original;\n\n";
    assert_eq!(current.matches(registration).count(), 1);
    current = current.replacen(registration, "", 1);
    for name in ["types", "expressions", "patterns", "record_patterns"] {
        let new = format!("        let mut {name} = drop_frontier::Stack::new();");
        let old = format!("        let mut {name} = Vec::new();");
        assert_eq!(current.matches(&new).count(), 1);
        current = current.replacen(&new, &old, 1);
    }
    assert_eq!(
        current,
        ci_str!("../../docs/evidence/custody-ast-drop-frontier/ast.rs.before.txt")
    );
    let original = ci_str!("drop_original.rs");
    let start = original.find("impl Drop for Original {").unwrap();
    let end = original[start..].find("\n// This lexical adapter").unwrap() + start;
    let old_drop =
        original[start..end].replace("impl Drop for Original {", "impl Drop for Program {");
    assert_eq!(
        old_drop,
        ci_str!("../../docs/evidence/custody-ast-drop-frontier/program-drop.before.txt")
    );
    assert_eq!(
        ci_str!("drop_original_tests.rs"),
        ci_str!(
            "../../docs/evidence/custody-ast-drop-frontier/iterative-program-drop-tests.before.txt"
        )
    );
    let children = concat!(
        include_str!("drop_method_roots.rs"),
        include_str!("drop_method_roots/tests.rs"),
        include_str!("drop_method_original.rs"),
        include_str!("drop_frontier.rs"),
        include_str!("drop_original.rs"),
        include_str!("drop_original_tests.rs")
    );
    assert!(children.contains("impl<T> Extend<T> for Stack<T>"));
    assert!(children.contains("impl Drop for Original"));
}
