//! Project parser-authenticated statement syntax from the single normalized tree.
use crate::ast::{BranchTail, Expr, ExprKind, LetSyntax, Statement, StatementIfSyntax};

pub(super) fn prepare(statement: &Statement) -> Option<(&Expr, &StatementIfSyntax)> {
    let Statement::Let {
        syntax: LetSyntax::StatementIf(syntax),
        mutable: false,
        declared: None,
        value,
        ..
    } = statement
    else {
        return None;
    };
    if syntax.branches().is_empty() {
        return None;
    }
    let mut current = value;
    for (index, tail) in syntax.branches().iter().enumerate() {
        let ExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &current.kind
        else {
            return None;
        };
        branch(then_branch, *tail)?;
        if index + 1 == syntax.branches().len() {
            match syntax.alternative() {
                Some(tail) => {
                    branch(else_branch, tail)?;
                }
                None if empty_zero(else_branch) => {}
                None => return None,
            }
        } else {
            current = continuation(else_branch)?;
        }
    }
    Some((value, syntax))
}

pub(super) fn continuation(expression: &Expr) -> Option<&Expr> {
    let ExprKind::Block { statements, tail } = &expression.kind else {
        return None;
    };
    (statements.is_empty() && matches!(tail.kind, ExprKind::If { .. })).then_some(tail)
}

pub(super) fn branch(
    expression: &Expr,
    syntax: BranchTail,
) -> Option<(&[Statement], Option<&Expr>)> {
    let ExprKind::Block { statements, tail } = &expression.kind else {
        return None;
    };
    match syntax {
        BranchTail::Retained => Some((statements, Some(tail))),
        BranchTail::Absent if matches!(tail.kind, ExprKind::Int(0)) => Some((statements, None)),
        BranchTail::Discarded if matches!(tail.kind, ExprKind::Int(0)) => {
            let (discard, statements) = statements.split_last()?;
            let Statement::Let {
                syntax: LetSyntax::BranchTail,
                mutable: false,
                declared: None,
                value,
                ..
            } = discard
            else {
                return None;
            };
            (!matches!(value.kind, ExprKind::Int(_))).then_some((statements, Some(value)))
        }
        _ => None,
    }
}
fn empty_zero(expression: &Expr) -> bool {
    matches!(&expression.kind, ExprKind::Block {statements,tail}
        if statements.is_empty() && matches!(tail.kind,ExprKind::Int(0)))
}

pub(super) fn write(output: &mut impl std::fmt::Write, value: &Expr, syntax: &StatementIfSyntax) {
    super::write_format_frames(
        output,
        super::ExprFormatFrame::StatementIf(value, syntax.branches(), syntax.alternative(), true),
        None,
    );
}
pub(super) fn begin_measure<'a>(
    frames: &mut super::FormatFrameStack<super::ExprFormatFrame<'a>>,
    measured: bool,
    value: &'a Expr,
    start: usize,
) {
    if measured {
        frames.push(super::ExprFormatFrame::MeasureEnd(
            value as *const Expr as usize,
            0,
            start,
        ));
    }
}
/// Omitted normalization nodes occupy zero source bytes. Visible branch and if
/// frames overwrite their own entries with their exact emitted lengths.
pub(super) fn measure_erased(
    value: &Expr,
    tails: &[BranchTail],
    alternative: Option<BranchTail>,
    lengths: &mut std::collections::HashMap<(usize, u8), usize>,
) {
    let zero = |value: &Expr, lengths: &mut std::collections::HashMap<(usize, u8), usize>| {
        lengths.insert((value as *const Expr as usize, 0), 0);
    };
    let mut current = value;
    for (index, syntax) in tails.iter().enumerate() {
        let ExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &current.kind
        else {
            unreachable!("prepared if")
        };
        let erase_tail =
            |branch: &Expr,
             syntax: BranchTail,
             lengths: &mut std::collections::HashMap<(usize, u8), usize>| {
                if syntax != BranchTail::Retained {
                    let ExprKind::Block { tail, .. } = &branch.kind else {
                        unreachable!("prepared branch")
                    };
                    zero(tail, lengths);
                }
            };
        erase_tail(then_branch, *syntax, lengths);
        if index + 1 == tails.len() {
            if let Some(syntax) = alternative {
                erase_tail(else_branch, syntax, lengths);
            } else {
                zero(else_branch, lengths);
                let ExprKind::Block { tail, .. } = &else_branch.kind else {
                    unreachable!("prepared else")
                };
                zero(tail, lengths);
            }
        } else {
            zero(else_branch, lengths);
            current = continuation(else_branch).expect("prepared chain");
        }
    }
}
