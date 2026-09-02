use super::*;

fn call(name: &str, marker: usize) -> Expr {
    Expr {
        span: Span {
            start: marker,
            end: marker + 1,
            line: 1,
            column: marker + 1,
        },
        kind: ExprKind::Call {
            name: name.to_owned(),
            type_arguments: vec![Type::Named {
                name: format!("T{marker}"),
                arguments: Vec::new(),
            }],
            args: Vec::new(),
        },
    }
}

#[test]
fn iterative_call_visitors_preserve_preorder_and_authored_child_order() {
    let span = Span::default();
    let expression = Expr {
        span,
        kind: ExprKind::Call {
            name: "outer".to_owned(),
            type_arguments: vec![Type::Named {
                name: "T0".to_owned(),
                arguments: Vec::new(),
            }],
            args: vec![
                Expr {
                    span,
                    kind: ExprKind::Block {
                        statements: vec![Statement::Let {
                            name: "value".to_owned(),
                            name_span: span,
                            mutable: false,
                            declared: None,
                            value: call("first", 1),
                            span,
                        }],
                        tail: Box::new(Expr {
                            span,
                            kind: ExprKind::If {
                                condition: Box::new(call("second", 2)),
                                then_branch: Box::new(call("third", 3)),
                                else_branch: Box::new(call("fourth", 4)),
                            },
                        }),
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::ConstructRecord {
                        type_name: "Pair".to_owned(),
                        type_span: span,
                        type_arguments: Vec::new(),
                        fields: vec![
                            FieldInitializer {
                                name: "left".to_owned(),
                                name_span: span,
                                value: call("fifth", 5),
                                span,
                            },
                            FieldInitializer {
                                name: "right".to_owned(),
                                name_span: span,
                                value: call("sixth", 6),
                                span,
                            },
                        ],
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::Match {
                        mode: MatchMode::Value,
                        scrutinee: Box::new(call("seventh", 7)),
                        arms: vec![
                            MatchArm {
                                pattern: MatchPattern::Wildcard { span },
                                guard: None,
                                value: call("eighth", 8),
                                span,
                            },
                            MatchArm {
                                pattern: MatchPattern::Wildcard { span },
                                guard: None,
                                value: call("ninth", 9),
                                span,
                            },
                        ],
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::UpdateRecord {
                        base: Box::new(call("tenth", 10)),
                        fields: vec![
                            FieldInitializer {
                                name: "left".to_owned(),
                                name_span: span,
                                value: call("eleventh", 11),
                                span,
                            },
                            FieldInitializer {
                                name: "right".to_owned(),
                                name_span: span,
                                value: call("twelfth", 12),
                                span,
                            },
                        ],
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::Try {
                        operand: Box::new(call("thirteenth", 13)),
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::Project {
                        base: Box::new(call("fourteenth", 14)),
                        field: "value".to_owned(),
                        field_span: span,
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::ConstructVariant {
                        type_name: "Choice".to_owned(),
                        type_span: span,
                        type_arguments: Vec::new(),
                        case_name: "Value".to_owned(),
                        case_span: span,
                        fields: vec![FieldInitializer {
                            name: "value".to_owned(),
                            name_span: span,
                            value: call("fifteenth", 15),
                            span,
                        }],
                    },
                },
                Expr {
                    span,
                    kind: ExprKind::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(call("sixteenth", 16)),
                        right: Box::new(call("seventeenth", 17)),
                    },
                },
            ],
        },
    };

    let expected_names = [
        "outer",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
    ];
    let mut calls = Vec::new();
    expression.visit_calls(&mut |name, span| calls.push((name.to_owned(), span.start)));
    assert_eq!(
        calls
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        expected_names
    );
    assert_eq!(
        calls.iter().map(|(_, marker)| *marker).collect::<Vec<_>>(),
        (0..=17).collect::<Vec<_>>()
    );

    let mut instances = Vec::new();
    expression.visit_call_instances(&mut |name, arguments, span| {
        instances.push((name.to_owned(), arguments[0].to_string(), span.start));
    });
    assert_eq!(
        instances
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect::<Vec<_>>(),
        expected_names
    );
    assert_eq!(
        instances
            .iter()
            .map(|(_, ty, _)| ty.as_str())
            .collect::<Vec<_>>(),
        (0..=17)
            .map(|marker| format!("T{marker}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        instances
            .iter()
            .map(|(_, _, marker)| *marker)
            .collect::<Vec<_>>(),
        (0..=17).collect::<Vec<_>>()
    );
}
