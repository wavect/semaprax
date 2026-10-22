//! Entire immediate old consuming traversal receiver; only visibility differs.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::hir) fn lower(
    function: &FunctionExecutionId,
    path: &str,
    item: &str,
    item_span: Span,
    source: ResolvedExpr,
    element: ResolvedType,
    user: ResolvedExpr,
    span: Span,
) -> ResolvedStatement {
    let vp = format!("{path}.value");
    let sp = format!("{vp}.s0");
    let wp = format!("{vp}.s1");
    let cp = format!("{wp}.condition");
    let bp = format!("{wp}.body");
    let mp = format!("{bp}.s0.value");
    let step_ty = crate::iterator_ops::resolved_iter_step(element.clone());
    let iter_ty = crate::iterator_ops::resolved_iter(element.clone());
    let expr = |p: &str, ty: ResolvedType, ownership, kind| ResolvedExpr {
        id: ExpressionId::new(function, p),
        ty,
        ownership,
        kind,
        span,
    };
    let binding = |p: &str, name: &str, ty: ResolvedType, ownership| ResolvedBinding {
        id: ValueId::local(function, p),
        name: name.to_owned(),
        ty,
        ownership,
        span,
    };
    let zero = |p: &str| {
        expr(
            p,
            ResolvedType::I64,
            OwnershipMode::Value,
            ResolvedExprKind::Int(0),
        )
    };
    let place = |p: &str, b: &ResolvedBinding| {
        expr(
            p,
            b.ty.clone(),
            b.ownership,
            ResolvedExprKind::Place(Place {
                root: b.id.clone(),
                projections: vec![],
            }),
        )
    };
    let step = binding(&sp, "#for-own-step", step_ty.clone(), OwnershipMode::Own);
    let next = |p: &str, arg: ResolvedExpr| {
        expr(
            p,
            step_ty.clone(),
            OwnershipMode::Own,
            ResolvedExprKind::Call {
                callee: DeclarationId::new(crate::iterator_ops::NEXT_ID),
                type_arguments: vec![element.clone()],
                instance: None,
                args: vec![arg],
            },
        )
    };
    let pattern = |yielded: bool, fields| ResolvedMatchPattern::Variant {
        variant: DeclarationId::new(crate::iterator_ops::STEP_ID),
        case: DeclarationId::new(if yielded {
            crate::iterator_ops::YIELD_ID
        } else {
            crate::iterator_ops::DONE_ID
        }),
        fields,
    };
    let field_bindings = |p: &str, ownership| {
        vec![
            ResolvedMatchPatternField {
                field: DeclarationId::new(crate::iterator_ops::ITEM_ID),
                binding: ResolvedBinding {
                    span: item_span,
                    ..binding(
                        &format!("{p}.binding.0"),
                        item,
                        element.clone(),
                        crate::iterator_ops::item_ownership(
                            &element,
                            ownership == OwnershipMode::Borrow,
                        ),
                    )
                },
            },
            ResolvedMatchPatternField {
                field: DeclarationId::new(crate::iterator_ops::REST_ID),
                binding: binding(
                    &format!("{p}.binding.1"),
                    "#for-own-rest",
                    iter_ty.clone(),
                    ownership,
                ),
            },
        ]
    };
    let arm = |pattern, value| ResolvedMatchArm {
        pattern,
        guard: None,
        value,
        span,
    };
    let condition = expr(
        &cp,
        ResolvedType::Bool,
        OwnershipMode::Value,
        ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Borrow,
            scrutinee: Box::new(place(&format!("{cp}.scrutinee"), &step)),
            arms: vec![
                arm(
                    pattern(false, vec![]),
                    expr(
                        &format!("{cp}.arm.0.value"),
                        ResolvedType::Bool,
                        OwnershipMode::Value,
                        ResolvedExprKind::Bool(false),
                    ),
                ),
                arm(
                    pattern(
                        true,
                        field_bindings(&format!("{cp}.arm.1"), OwnershipMode::Borrow),
                    ),
                    expr(
                        &format!("{cp}.arm.1.value"),
                        ResolvedType::Bool,
                        OwnershipMode::Value,
                        ResolvedExprKind::Bool(true),
                    ),
                ),
            ],
        },
    );
    let fields = field_bindings(&format!("{mp}.arm.1"), OwnershipMode::Own);
    let yp = format!("{mp}.arm.1.value");
    let advance = next(
        &format!("{yp}.tail"),
        place(&format!("{yp}.tail.arg.0"), &fields[1].binding),
    );
    let yielded = expr(
        &yp,
        step_ty.clone(),
        OwnershipMode::Own,
        ResolvedExprKind::Block {
            statements: vec![ResolvedStatement::Let {
                binding: binding(
                    &format!("{yp}.s0"),
                    "#for-own-body",
                    user.ty.clone(),
                    OwnershipMode::Value,
                ),
                mutable: false,
                value: user,
                span,
            }],
            tail: Box::new(advance),
        },
    );
    let done = expr(
        &format!("{mp}.arm.0.value"),
        step_ty.clone(),
        OwnershipMode::Own,
        ResolvedExprKind::ConstructVariant {
            variant: DeclarationId::new(crate::iterator_ops::STEP_ID),
            case: DeclarationId::new(crate::iterator_ops::DONE_ID),
            fields: vec![],
        },
    );
    let replacement = expr(
        &mp,
        step_ty.clone(),
        OwnershipMode::Own,
        ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Own,
            scrutinee: Box::new(place(&format!("{mp}.scrutinee"), &step)),
            arms: vec![
                arm(pattern(false, vec![]), done),
                arm(pattern(true, fields), yielded),
            ],
        },
    );
    let body = expr(
        &bp,
        ResolvedType::I64,
        OwnershipMode::Value,
        ResolvedExprKind::Block {
            statements: vec![ResolvedStatement::Assign {
                binding: step.clone(),
                field: None,
                value: replacement,
                span,
            }],
            tail: Box::new(zero(&format!("{bp}.tail"))),
        },
    );
    ResolvedStatement::Let {
        binding: binding(path, "#for-own", ResolvedType::I64, OwnershipMode::Value),
        mutable: false,
        span,
        value: expr(
            &vp,
            ResolvedType::I64,
            OwnershipMode::Value,
            ResolvedExprKind::Block {
                statements: vec![
                    ResolvedStatement::Let {
                        binding: step,
                        mutable: true,
                        value: next(&format!("{sp}.value"), source),
                        span,
                    },
                    crate::hir::resolve_expr::lexical_exit::loop_statement(
                        Box::new(condition),
                        Box::new(body),
                        crate::hir::LoopLatchKind::OwnedIteratorStep {
                            next: ExpressionId::new(function, &format!("{yp}.tail")),
                            assignment: ExpressionId::new(function, &mp),
                            step: ValueId::local(function, &sp),
                            remainder: ValueId::local(function, &format!("{mp}.arm.1.binding.1")),
                        },
                        span,
                    ),
                ],
                tail: Box::new(zero(&format!("{vp}.tail"))),
            },
        ),
    }
}
