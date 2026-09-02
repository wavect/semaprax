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
        span,
    });

    // Normal lexical scope owns teardown: no leak, subprocess, or stack
    // configuration may mask a recursively dropped AST carrier.
    drop(program);
}
