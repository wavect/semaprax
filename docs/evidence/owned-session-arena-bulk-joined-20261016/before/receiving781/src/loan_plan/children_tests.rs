//! Pointer-identity fixtures for every arm of the borrowed child iterator.
//!
//! These are hand-built private `ResolvedExprKind` fixtures. They carry no
//! valid-HIR authority: identities are distinct, types are placeholders, and
//! nothing here is resolved, verified or planned. Each case writes the admitted
//! original child order out as explicit positions of its own fields, so the
//! expected list never comes from the iterator under test, and compares it by
//! pointer against both the borrowed iterator and the saved original
//! `Vec`-returning collector.

use crate::ast::{LexicalExitKind, UnaryOp};
use crate::hir::{
    ControlTarget, DeclarationId, FunctionExecutionId, ResolvedBinding, ResolvedClosureCapture,
    ResolvedFieldInitializer, ResolvedHostCommandCall, ResolvedHostCommandOperation,
    ResolvedImportResultKind, ResolvedMatchArm, ResolvedMatchMode, ResolvedMatchPattern,
    ResolvedNativeRustImportCall, RowLoan, RowLoanVersion,
};

use super::*;

fn execution() -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(DeclarationId::new("children.fixture"))
}

fn expr(path: &str, kind: ResolvedExprKind) -> ResolvedExpr {
    ResolvedExpr {
        id: ExpressionId::new(&execution(), path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind,
        span: Span::default(),
    }
}

fn leaf(path: &str) -> ResolvedExpr {
    expr(path, ResolvedExprKind::Bool(true))
}

fn leaves(prefix: &str, count: usize) -> Vec<ResolvedExpr> {
    (0..count)
        .map(|index| leaf(&format!("{prefix}.{index}")))
        .collect()
}

fn boxed(path: &str) -> Box<ResolvedExpr> {
    Box::new(leaf(path))
}

fn binding(path: &str) -> ResolvedBinding {
    ResolvedBinding {
        id: ValueId::local(&execution(), path),
        name: path.to_string(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Bool,
        span: Span::default(),
    }
}

fn field(name: &str, value: ResolvedExpr) -> ResolvedFieldInitializer {
    ResolvedFieldInitializer {
        field: DeclarationId::new(name),
        value,
    }
}

fn fields(prefix: &str, count: usize) -> Vec<ResolvedFieldInitializer> {
    (0..count)
        .map(|index| {
            field(
                &format!("{prefix}.field.{index}"),
                leaf(&format!("{prefix}.value.{index}")),
            )
        })
        .collect()
}

/// Deref a boxed child to the plain reference every comparison list holds.
fn at(child: &ResolvedExpr) -> &ResolvedExpr {
    child
}

fn assert_identical(label: &str, who: &str, actual: &[&ResolvedExpr], expected: &[&ResolvedExpr]) {
    assert_eq!(actual.len(), expected.len(), "{label}: {who} child count");
    for (position, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            std::ptr::eq(*actual, *expected),
            "{label}: {who} child {position}"
        );
    }
}

/// `expected` is the admitted order; `excluded` are sub-expressions that must
/// never be reported as evaluation children.
fn assert_children(
    label: &str,
    parent: &ResolvedExpr,
    expected: &[&ResolvedExpr],
    excluded: &[&ResolvedExpr],
) {
    let mut borrowed = children::evaluation_children(parent);
    let actual = borrowed.by_ref().collect::<Vec<_>>();
    assert!(
        borrowed.next().is_none(),
        "{label}: iterator stays exhausted"
    );
    assert_identical(label, "borrowed", &actual, expected);
    let saved = saved_planner::evaluation_children(parent);
    assert_identical(label, "saved original", &saved, expected);
    for child in &actual {
        assert!(
            !excluded.iter().any(|other| std::ptr::eq(*child, *other)),
            "{label}: an excluded sub-expression was reported"
        );
    }
}

#[test]
fn closure_children_are_only_the_captured_values_in_order() {
    let capture = |index: usize| ResolvedClosureCapture {
        binding: binding(&format!("closure.capture.{index}")),
        value: leaf(&format!("closure.value.{index}")),
    };
    for count in [0, 1, 3] {
        let parent = expr(
            "closure",
            ResolvedExprKind::Closure {
                parameters: vec![binding("closure.parameter")],
                captures: (0..count).map(capture).collect(),
                body: boxed("closure.body"),
            },
        );
        let ResolvedExprKind::Closure { captures, body, .. } = &parent.kind else {
            unreachable!()
        };
        let body = at(body);
        match captures.as_slice() {
            [] => assert_children("closure/0", &parent, &[], &[body]),
            [only] => assert_children("closure/1", &parent, &[&only.value], &[body]),
            [first, second, third] => assert_children(
                "closure/3",
                &parent,
                &[&first.value, &second.value, &third.value],
                &[body],
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn invoke_children_are_the_callable_then_each_argument() {
    for count in [0, 1, 3] {
        let parent = expr(
            "invoke",
            ResolvedExprKind::Invoke {
                callable: boxed("invoke.callable"),
                args: leaves("invoke.argument", count),
            },
        );
        let ResolvedExprKind::Invoke { callable, args } = &parent.kind else {
            unreachable!()
        };
        let callable = at(callable);
        match args.as_slice() {
            [] => assert_children("invoke/0", &parent, &[callable], &[]),
            [only] => assert_children("invoke/1", &parent, &[callable, only], &[]),
            [first, second, third] => {
                assert_children("invoke/3", &parent, &[callable, first, second, third], &[])
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn byte_range_children_are_source_start_end() {
    let parent = expr(
        "range",
        ResolvedExprKind::ByteRange {
            operation: DeclarationId::new("range.operation"),
            source: boxed("range.source"),
            start: boxed("range.start"),
            end: boxed("range.end"),
        },
    );
    let ResolvedExprKind::ByteRange {
        source, start, end, ..
    } = &parent.kind
    else {
        unreachable!()
    };
    assert_children("range", &parent, &[at(source), at(start), at(end)], &[]);
}

#[test]
fn row_loan_children_are_the_vec_place_then_the_row_index() {
    let parent = expr(
        "row_loan",
        ResolvedExprKind::RowLoan {
            loan: Box::new(RowLoan {
                version: RowLoanVersion::V1,
                element: ResolvedType::Bool,
                traversal: ExpressionId::new(&execution(), "row_loan.traversal"),
                bound: ValueId::local(&execution(), "row_loan.bound"),
                callee: DeclarationId::new("row_loan.callee"),
                parameter: ValueId::local(&execution(), "row_loan.parameter"),
            }),
            args: leaves("row_loan.argument", 2),
        },
    );
    let ResolvedExprKind::RowLoan { args, .. } = &parent.kind else {
        unreachable!()
    };
    assert_children("row_loan", &parent, &[&args[0], &args[1]], &[]);
}

#[test]
fn record_variant_and_update_children_follow_field_order() {
    for count in [0, 1, 3] {
        let record = expr(
            "record",
            ResolvedExprKind::ConstructRecord {
                record: DeclarationId::new("record.declaration"),
                fields: fields("record", count),
            },
        );
        let ResolvedExprKind::ConstructRecord {
            fields: record_fields,
            ..
        } = &record.kind
        else {
            unreachable!()
        };
        match record_fields.as_slice() {
            [] => assert_children("record/0", &record, &[], &[]),
            [only] => assert_children("record/1", &record, &[&only.value], &[]),
            [first, second, third] => assert_children(
                "record/3",
                &record,
                &[&first.value, &second.value, &third.value],
                &[],
            ),
            _ => unreachable!(),
        }

        let update = expr(
            "update",
            ResolvedExprKind::UpdateRecord {
                base: boxed("update.base"),
                record: DeclarationId::new("update.declaration"),
                fields: fields("update", count),
            },
        );
        let ResolvedExprKind::UpdateRecord { base, fields, .. } = &update.kind else {
            unreachable!()
        };
        // The base always evaluates first, before every field initializer.
        let base = at(base);
        match fields.as_slice() {
            [] => assert_children("update/0", &update, &[base], &[]),
            [only] => assert_children("update/1", &update, &[base, &only.value], &[]),
            [first, second, third] => assert_children(
                "update/3",
                &update,
                &[base, &first.value, &second.value, &third.value],
                &[],
            ),
            _ => unreachable!(),
        }
    }
    let variant = expr(
        "variant",
        ResolvedExprKind::ConstructVariant {
            variant: DeclarationId::new("variant.declaration"),
            case: DeclarationId::new("variant.case"),
            fields: fields("variant", 2),
        },
    );
    let ResolvedExprKind::ConstructVariant { fields, .. } = &variant.kind else {
        unreachable!()
    };
    assert_children(
        "variant/2",
        &variant,
        &[&fields[0].value, &fields[1].value],
        &[],
    );
}

#[test]
fn native_and_host_call_children_are_their_arguments_in_order() {
    for count in [0, 2] {
        let native = expr(
            "native",
            ResolvedExprKind::NativeRustImportCall(ResolvedNativeRustImportCall {
                expression: ExpressionId::new(&execution(), "native.call"),
                import: DeclarationId::new("native.import"),
                args: leaves("native.argument", count),
                result: ResolvedImportResultKind::Unit,
            }),
        );
        let ResolvedExprKind::NativeRustImportCall(call) = &native.kind else {
            unreachable!()
        };
        match call.args.as_slice() {
            [] => assert_children("native/0", &native, &[], &[]),
            [first, second] => assert_children("native/2", &native, &[first, second], &[]),
            _ => unreachable!(),
        }

        let host = expr(
            "host",
            ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                expression: ExpressionId::new(&execution(), "host.call"),
                operation: ResolvedHostCommandOperation::ArgsLen,
                args: leaves("host.argument", count),
            }),
        );
        let ResolvedExprKind::HostCommandCall(call) = &host.kind else {
            unreachable!()
        };
        match call.args.as_slice() {
            [] => assert_children("host/0", &host, &[], &[]),
            [first, second] => assert_children("host/2", &host, &[first, second], &[]),
            _ => unreachable!(),
        }
    }
}

#[test]
fn argument_list_arms_report_their_arguments_in_order() {
    let call = expr(
        "call",
        ResolvedExprKind::Call {
            callee: DeclarationId::new("call.callee"),
            type_arguments: Vec::new(),
            instance: None,
            args: leaves("call.argument", 3),
        },
    );
    let ResolvedExprKind::Call { args, .. } = &call.kind else {
        unreachable!()
    };
    assert_children("call", &call, &[&args[0], &args[1], &args[2]], &[]);

    let format = expr(
        "format",
        ResolvedExprKind::LiteralFormat {
            template: "{} {}".to_string(),
            args: leaves("format.argument", 2),
        },
    );
    let ResolvedExprKind::LiteralFormat { args, .. } = &format.kind else {
        unreachable!()
    };
    assert_children("format", &format, &[&args[0], &args[1]], &[]);

    let read = expr(
        "read",
        ResolvedExprKind::VecFieldRead {
            origin: crate::hir::VecFieldReadOrigin::Explicit,
            element: ResolvedType::Bool,
            field: DeclarationId::new("read.field"),
            bytes: false,
            args: leaves("read.argument", 2),
        },
    );
    let ResolvedExprKind::VecFieldRead { args, .. } = &read.kind else {
        unreachable!()
    };
    assert_children("read", &read, &[&args[0], &args[1]], &[]);
}

#[test]
fn single_child_and_binary_arms_report_their_operands() {
    let unary = expr(
        "unary",
        ResolvedExprKind::Unary {
            op: UnaryOp::Not,
            value: boxed("unary.value"),
        },
    );
    let ResolvedExprKind::Unary { value, .. } = &unary.kind else {
        unreachable!()
    };
    assert_children("unary", &unary, &[at(value)], &[]);

    let attempt = expr(
        "try",
        ResolvedExprKind::Try {
            operand: boxed("try.operand"),
            result: DeclarationId::new("try.result"),
            ok_case: DeclarationId::new("try.ok_case"),
            ok_field: DeclarationId::new("try.ok_field"),
            err_case: DeclarationId::new("try.err_case"),
            err_field: DeclarationId::new("try.err_field"),
            residual_type: ResolvedType::Bool,
        },
    );
    let ResolvedExprKind::Try { operand, .. } = &attempt.kind else {
        unreachable!()
    };
    assert_children("try", &attempt, &[at(operand)], &[]);

    let option = expr(
        "try_option",
        ResolvedExprKind::TryOption {
            operand: boxed("try_option.operand"),
            option: DeclarationId::new("try_option.option"),
            some_case: DeclarationId::new("try_option.some_case"),
            some_field: DeclarationId::new("try_option.some_field"),
            none_case: DeclarationId::new("try_option.none_case"),
            residual_type: ResolvedType::Bool,
        },
    );
    let ResolvedExprKind::TryOption { operand, .. } = &option.kind else {
        unreachable!()
    };
    assert_children("try_option", &option, &[at(operand)], &[]);

    let project = expr(
        "project",
        ResolvedExprKind::Project {
            base: boxed("project.base"),
            field: DeclarationId::new("project.field"),
        },
    );
    let ResolvedExprKind::Project { base, .. } = &project.kind else {
        unreachable!()
    };
    assert_children("project", &project, &[at(base)], &[]);

    let upcast = expr(
        "upcast",
        ResolvedExprKind::Upcast {
            source: boxed("upcast.source"),
        },
    );
    let ResolvedExprKind::Upcast { source } = &upcast.kind else {
        unreachable!()
    };
    assert_children("upcast", &upcast, &[at(source)], &[]);

    let yielded = expr(
        "yield",
        ResolvedExprKind::Yield {
            request: boxed("yield.request"),
        },
    );
    let ResolvedExprKind::Yield { request } = &yielded.kind else {
        unreachable!()
    };
    assert_children("yield", &yielded, &[at(request)], &[]);

    for op in [BinaryOp::And, BinaryOp::Or] {
        let binary = expr(
            "binary",
            ResolvedExprKind::Binary {
                op,
                left: boxed("binary.left"),
                right: boxed("binary.right"),
            },
        );
        let ResolvedExprKind::Binary { left, right, .. } = &binary.kind else {
            unreachable!()
        };
        assert_children("binary", &binary, &[at(left), at(right)], &[]);
    }
}

#[test]
fn lexical_exit_children_are_its_optional_value() {
    let exit = |value: Option<Box<ResolvedExpr>>| {
        expr(
            "exit",
            ResolvedExprKind::LexicalExit {
                kind: LexicalExitKind::Return,
                target: ControlTarget::Function(execution()),
                value,
            },
        )
    };
    let bare = exit(None);
    assert_children("exit/none", &bare, &[], &[]);
    let valued = exit(Some(boxed("exit.value")));
    let ResolvedExprKind::LexicalExit { value, .. } = &valued.kind else {
        unreachable!()
    };
    let value = value.as_deref().unwrap();
    assert_children("exit/some", &valued, &[value], &[]);
}

#[test]
fn structured_and_leaf_arms_have_no_evaluation_children_even_with_subexpressions() {
    let reference = expr(
        "reference",
        ResolvedExprKind::FunctionReference {
            target: DeclarationId::new("reference.target"),
        },
    );
    assert_children("function reference", &reference, &[], &[]);

    let place = Place {
        root: ValueId::local(&execution(), "place.root"),
        projections: Vec::new(),
    };
    let borrow = expr(
        "borrow",
        ResolvedExprKind::BorrowPlace {
            operation: DeclarationId::new("borrow.operation"),
            place: place.clone(),
        },
    );
    assert_children("borrow place", &borrow, &[], &[]);
    assert_children(
        "place",
        &expr("place", ResolvedExprKind::Place(place)),
        &[],
        &[],
    );
    assert_children("int", &expr("int", ResolvedExprKind::Int(7)), &[], &[]);
    assert_children(
        "string",
        &expr("string", ResolvedExprKind::String("text".to_string())),
        &[],
        &[],
    );

    let branch = expr(
        "branch",
        ResolvedExprKind::If {
            condition: boxed("branch.condition"),
            then_branch: boxed("branch.then"),
            else_branch: boxed("branch.else"),
        },
    );
    let ResolvedExprKind::If {
        condition,
        then_branch,
        else_branch,
    } = &branch.kind
    else {
        unreachable!()
    };
    assert_children(
        "if",
        &branch,
        &[],
        &[at(condition), at(then_branch), at(else_branch)],
    );

    let block = expr(
        "block",
        ResolvedExprKind::Block {
            statements: Vec::new(),
            tail: boxed("block.tail"),
        },
    );
    let ResolvedExprKind::Block { tail, .. } = &block.kind else {
        unreachable!()
    };
    assert_children("block", &block, &[], &[at(tail)]);

    let constructor = expr(
        "constructor",
        ResolvedExprKind::VecConstructor {
            element: ResolvedType::Bool,
            count: 0,
            statements: Vec::new(),
            tail: boxed("constructor.tail"),
        },
    );
    let ResolvedExprKind::VecConstructor { tail, .. } = &constructor.kind else {
        unreachable!()
    };
    assert_children("vec constructor", &constructor, &[], &[at(tail)]);

    let matched = expr(
        "match",
        ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Value,
            scrutinee: boxed("match.scrutinee"),
            arms: vec![ResolvedMatchArm {
                pattern: ResolvedMatchPattern::Wildcard,
                guard: Some(boxed("match.guard")),
                value: leaf("match.value"),
                span: Span::default(),
            }],
        },
    );
    let ResolvedExprKind::Match {
        scrutinee, arms, ..
    } = &matched.kind
    else {
        unreachable!()
    };
    let guard = arms[0].guard.as_deref().unwrap();
    assert_children(
        "match",
        &matched,
        &[],
        &[at(scrutinee), guard, &arms[0].value],
    );
}
