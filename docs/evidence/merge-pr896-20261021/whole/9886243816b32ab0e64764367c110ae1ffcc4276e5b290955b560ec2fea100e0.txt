//! Complete immediate old traversal helpers; namespace-only access changes.
use super::*;

fn place(
    function: &FunctionExecutionId,
    path: &str,
    binding: &ResolvedBinding,
    span: Span,
) -> ResolvedExpr {
    place_at(ExpressionId::new(function, path), binding, span)
}

fn place_at(id: ExpressionId, binding: &ResolvedBinding, span: Span) -> ResolvedExpr {
    ResolvedExpr {
        id,
        ty: binding.ty.clone(),
        ownership: binding.ownership,
        kind: ResolvedExprKind::Place(Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        }),
        span,
    }
}

fn usize_literal(
    function: &FunctionExecutionId,
    path: &str,
    value: u64,
    span: Span,
) -> ResolvedExpr {
    ResolvedExpr {
        id: ExpressionId::new(function, path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Usize(value),
        span,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::hir) fn lower(
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    element: ResolvedType,
    authored_body: ResolvedExpr,
    span: Span,
) -> ResolvedStatement {
    let length_arguments = vec![element.clone()];
    lower_profile(
        function,
        statement_path,
        item_name,
        item_span,
        source,
        authored_body,
        span,
        Profile {
            wrapper: "#for",
            length: "#for-length",
            length_callee: crate::vec_ops::LEN_ID,
            length_arguments,
            item: element.clone(),
            kind: crate::hir::SourceForKind::Copy,
        },
        |get_value_path, index| ResolvedExpr {
            id: ExpressionId::new(function, get_value_path),
            ty: element.clone(),
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Call {
                callee: DeclarationId::new(crate::vec_ops::GET_ID),
                type_arguments: vec![element],
                instance: None,
                args: vec![
                    place(function, &format!("{get_value_path}.arg.0"), source, span),
                    place(function, &format!("{get_value_path}.arg.1"), index, span),
                ],
            },
            span,
        },
    )
}

/// What distinguishes one source profile on the shared `for` paths: its
/// hidden names, its length snapshot operation, its item type and latch kind.
/// Each profile constructs its own item read in place, so no other profile's
/// carrier is ever built and discarded.
pub(in crate::hir) struct Profile {
    pub(in crate::hir) wrapper: &'static str,
    pub(in crate::hir) length: &'static str,
    pub(in crate::hir) length_callee: &'static str,
    pub(in crate::hir) length_arguments: Vec<ResolvedType>,
    pub(in crate::hir) item: ResolvedType,
    pub(in crate::hir) kind: crate::hir::SourceForKind,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::hir) fn lower_profile(
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    authored_body: ResolvedExpr,
    span: Span,
    profile: Profile,
    read: impl FnOnce(&str, &ResolvedBinding) -> ResolvedExpr,
) -> ResolvedStatement {
    let value_path = format!("{statement_path}.value");
    let length_path = format!("{value_path}.s0");
    let index_path = format!("{value_path}.s1");
    let while_path = format!("{value_path}.s2");
    let while_body_path = format!("{while_path}.body");

    let length = ResolvedBinding {
        id: ValueId::local(function, &length_path),
        name: profile.length.to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Usize,
        span,
    };
    let index = ResolvedBinding {
        id: ValueId::local(function, &index_path),
        name: "#for-index".to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Usize,
        span,
    };
    let item_path = format!("{while_body_path}.s0");
    let item = ResolvedBinding {
        id: ValueId::local(function, &item_path),
        name: item_name.to_owned(),
        ownership: OwnershipMode::Value,
        ty: profile.item,
        span: item_span,
    };
    let authored_path = format!("{while_body_path}.s1");
    let authored = ResolvedBinding {
        id: ValueId::local(function, &authored_path),
        name: "#for-body".to_owned(),
        ownership: authored_body.ownership,
        ty: authored_body.ty.clone(),
        span: authored_body.span,
    };

    let length_value = ResolvedExpr {
        id: ExpressionId::new(function, &format!("{length_path}.value")),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(profile.length_callee),
            type_arguments: profile.length_arguments,
            instance: None,
            args: vec![place(
                function,
                &format!("{length_path}.value.arg.0"),
                source,
                span,
            )],
        },
        span,
    };
    let condition = ResolvedExpr {
        id: ExpressionId::new(function, &format!("{while_path}.condition")),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Binary {
            op: BinaryOp::Lt,
            left: Box::new(place(
                function,
                &format!("{while_path}.condition.left"),
                &index,
                span,
            )),
            right: Box::new(place(
                function,
                &format!("{while_path}.condition.right"),
                &length,
                span,
            )),
        },
        span,
    };
    let get_value = read(&format!("{item_path}.value"), &index);
    let increment_path = format!("{while_body_path}.s2.value");
    let increment = ResolvedExpr {
        id: ExpressionId::new(function, &increment_path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Binary {
            op: BinaryOp::Add,
            left: Box::new(place(
                function,
                &format!("{increment_path}.left"),
                &index,
                span,
            )),
            right: Box::new(usize_literal(
                function,
                &format!("{increment_path}.right"),
                1,
                span,
            )),
        },
        span,
    };
    let while_body = ResolvedExpr {
        id: ExpressionId::new(function, &while_body_path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Block {
            statements: vec![
                ResolvedStatement::Let {
                    binding: item,
                    mutable: false,
                    value: get_value,
                    span,
                },
                ResolvedStatement::Let {
                    binding: authored,
                    mutable: false,
                    value: authored_body,
                    span,
                },
                ResolvedStatement::Assign {
                    binding: index.clone(),
                    field: None,
                    value: increment,
                    span,
                },
            ],
            tail: Box::new(usize_literal(
                function,
                &format!("{while_body_path}.tail"),
                0,
                span,
            )),
        },
        span,
    };
    let lowered = ResolvedExpr {
        id: ExpressionId::new(function, &value_path),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Block {
            statements: vec![
                ResolvedStatement::Let {
                    binding: length,
                    mutable: false,
                    value: length_value,
                    span,
                },
                ResolvedStatement::Let {
                    binding: index.clone(),
                    mutable: true,
                    value: usize_literal(function, &format!("{index_path}.value"), 0, span),
                    span,
                },
                crate::hir::resolve_expr::lexical_exit::loop_statement(
                    Box::new(condition),
                    Box::new(while_body),
                    crate::hir::LoopLatchKind::SourceForStep {
                        kind: profile.kind,
                        step: ExpressionId::new(function, &format!("{while_body_path}.s2.value")),
                    },
                    span,
                ),
            ],
            tail: Box::new(usize_literal(
                function,
                &format!("{value_path}.tail"),
                0,
                span,
            )),
        },
        span,
    };
    ResolvedStatement::Let {
        binding: ResolvedBinding {
            id: ValueId::local(function, statement_path),
            name: profile.wrapper.to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Usize,
            span,
        },
        mutable: false,
        value: lowered,
        span,
    }
}
