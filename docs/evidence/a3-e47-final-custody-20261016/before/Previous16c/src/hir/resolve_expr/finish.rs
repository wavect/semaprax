//! Finish resolver result stacks and already-resolved class-prefix projections.
use super::path_arena::{ExprPath, PathArena};
use super::*;
impl Resolver<'_> {
    /// `id` is the slot identity, emitted first exactly as before.
    pub(super) fn finish_upcast(
        &self,
        id: ExpressionId,
        holder: DeclarationId,
        span: crate::ast::Span,
        source: ResolvedExpr,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let declared = ResolvedType::Nominal {
            declaration: holder,
            arguments: Vec::new(),
        };
        Ok(ResolvedExpr {
            id,
            ty: declared.clone(),
            ownership: self.expression_ownership(&declared, OwnershipMode::Own, span)?,
            kind: ResolvedExprKind::Upcast {
                source: Box::new(source),
            },
            span,
        })
    }
}

impl Resolver<'_> {
    pub(super) fn finish_expression_results(
        &self,
        mut results: Vec<ResolvedExpr>,
        span: crate::ast::Span,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if results.len() != 1 {
            return Err(self.error(
                "SPX-H006",
                "iterative expression resolver finished with an invalid result stack",
                span,
            ));
        }
        results.pop().ok_or_else(|| {
            self.error(
                "SPX-H006",
                "iterative expression resolver lost its root result",
                span,
            )
        })
    }
}

pub(super) fn finish_if(
    function: &FunctionExecutionId,
    span: crate::ast::Span,
    path: ExprPath<'_>,
    children: (ResolvedExpr, ResolvedExpr, ResolvedExpr),
) -> ResolvedExpr {
    let (condition, then_branch, else_branch) = children;
    let normal = if then_branch.ty == ResolvedType::Never {
        &else_branch
    } else {
        &then_branch
    };
    ResolvedExpr {
        id: path.expression_id(function),
        ty: normal.ty.clone(),
        ownership: normal.ownership,
        kind: ResolvedExprKind::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        },
        span,
    }
}

pub(super) fn finish_block(
    function: &FunctionExecutionId,
    span: crate::ast::Span,
    path: ExprPath<'_>,
    statements: Vec<ResolvedStatement>,
    tail: ResolvedExpr,
) -> ResolvedExpr {
    ResolvedExpr {
        id: path.expression_id(function),
        ty: tail.ty.clone(),
        ownership: tail.ownership,
        kind: ResolvedExprKind::Block {
            statements,
            tail: Box::new(tail),
        },
        span,
    }
}

pub(super) fn finish_unary(
    function: &FunctionExecutionId,
    span: crate::ast::Span,
    path: ExprPath<'_>,
    op: UnaryOp,
    value: ResolvedExpr,
) -> ResolvedExpr {
    let ty = match (&op, &value.ty) {
        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
        (UnaryOp::Neg, ResolvedType::I32) => ResolvedType::I32,
        (UnaryOp::Neg, _) => ResolvedType::I64,
        (UnaryOp::Not, _) => ResolvedType::Bool,
    };
    ResolvedExpr {
        id: path.expression_id(function),
        ty,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Unary {
            op,
            value: Box::new(value),
        },
        span,
    }
}

impl Resolver<'_> {
    pub(super) fn finish_lexical_match(
        &self,
        function: &FunctionExecutionId,
        span: crate::ast::Span,
        path: ExprPath<'_>,
        mode: ResolvedMatchMode,
        scrutinee: ResolvedExpr,
        arms: Vec<ResolvedMatchArm>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let first = super::super::lexical_exit::normal_arm(&arms)
            .ok_or_else(|| self.error("SPX-H006", "resolved match has no arms", span))?;
        Ok(ResolvedExpr {
            id: path.expression_id(function),
            ty: first.value.ty.clone(),
            ownership: first.value.ownership,
            kind: ResolvedExprKind::Match {
                mode,
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span,
        })
    }
}

pub(super) fn start_upcast<'a>(
    frame: Frame<'a>,
    frames: &mut Vec<Frame<'a>>,
    paths: &mut PathArena<'_>,
) {
    let Frame::StartUpcast {
        source,
        bindings,
        slot_parent,
        slot_edge,
        holder,
        span,
        resume,
    } = frame
    else {
        unreachable!("upcast continuation")
    };
    // Re-resolve the consumed expression at the canonical `.source` identity below its occupied slot, then wrap and resume the interrupted
    // continuation.
    frames.push(*resume);
    let slot_path = paths.child(frames, slot_parent, slot_edge);
    frames.push(Frame::FinishUpcast {
        slot_path,
        holder,
        span,
    });
    frames.push(Frame::Enter {
        expr: source,
        bindings,
        path: paths.child(frames, slot_path, Seg::Source),
    });
}
