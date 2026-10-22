//! Target capture belongs to the resolver contour, never an authored name.
use super::path_arena::{ExprPath, PathArena};
use super::*;
use crate::ast::{LexicalExitKind, Span};
use crate::hir::{ControlTarget, LoopLatchKind};
#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'a>(
    function: &FunctionExecutionId,
    expr: &'a Expr,
    kind: LexicalExitKind,
    value: Option<&'a Expr>,
    bindings: &Rc<BTreeMap<String, Binding>>,
    path: PathId,
    nearest: Option<&ControlTarget>,
    frames: &mut Vec<Frame<'a>>,
    paths: &mut PathArena<'_>,
) -> Result<(), Diagnostic> {
    let target = match kind {
        LexicalExitKind::Return => ControlTarget::Function(function.clone()),
        LexicalExitKind::Break | LexicalExitKind::Continue => nearest
            .cloned()
            .ok_or_else(|| Diagnostic::io("SPX-T313", "lexical loop exit has no enclosing loop"))?,
    };
    if (kind == LexicalExitKind::Return) != value.is_some() {
        return Err(Diagnostic::io(
            "SPX-T312",
            "lexical exit operand differs from its kind",
        ));
    }
    frames.push(Frame::FinishLexicalExit {
        span: expr.span,
        path,
        kind,
        target,
        has_value: value.is_some(),
    });
    if let Some(value) = value {
        frames.push(Frame::Enter {
            expr: value,
            bindings: bindings.clone(),
            path: paths.child(frames, path, Seg::Value),
        });
    }
    Ok(())
}
pub(super) fn finish(
    function: &FunctionExecutionId,
    span: Span,
    path: ExprPath<'_>,
    kind: LexicalExitKind,
    target: ControlTarget,
    value: Option<Box<ResolvedExpr>>,
) -> ResolvedExpr {
    ResolvedExpr {
        id: path.expression_id(function),
        ty: ResolvedType::Never,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        },
        span,
    }
}
pub(in crate::hir) fn loop_statement(
    condition: Box<ResolvedExpr>,
    body: Box<ResolvedExpr>,
    latch: LoopLatchKind,
    span: Span,
) -> ResolvedStatement {
    if crate::hir::lexical_exit::contains(&body) {
        ResolvedStatement::LexicalWhile {
            condition,
            body,
            latch,
            span,
        }
    } else {
        ResolvedStatement::While {
            condition,
            body,
            span,
        }
    }
}

pub(super) fn enter_loop(
    targets: &mut [Option<ControlTarget>],
    depth: &mut usize,
    target: ControlTarget,
) -> Result<(), Diagnostic> {
    let slot = targets
        .get_mut(*depth)
        .ok_or_else(|| Diagnostic::io("SPX-H006", "lexical loop nesting exceeds admitted depth"))?;
    *slot = Some(target);
    *depth += 1;
    Ok(())
}

pub(super) fn finish_results(
    function: &FunctionExecutionId,
    span: Span,
    path: ExprPath<'_>,
    kind: LexicalExitKind,
    target: ControlTarget,
    has_value: bool,
    results: &mut Vec<ResolvedExpr>,
) {
    let value = has_value.then(|| Box::new(results.pop().expect("lexical operand retained")));
    results.push(finish(function, span, path, kind, target, value));
}
