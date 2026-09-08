//! Closure signature formatting and expression delimiter inspection.
use super::*;
pub(super) fn contains_record_construction(value: &Expr) -> bool {
    use ContainsRecordFrame as Frame;
    fn child(value: &Expr, index: usize) -> Option<&Expr> {
        match &value.kind {
            ExprKind::Closure { body, .. } => (index == 0).then_some(body),
            ExprKind::Call { args, .. } => args.get(index),
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => (index == 0).then_some(value),
            ExprKind::UpdateRecord { base, fields } => {
                if index == 0 {
                    Some(base)
                } else {
                    fields.get(index - 1).map(|field| &field.value)
                }
            }
            ExprKind::Binary { left, right, .. } => {
                [left.as_ref(), right.as_ref()].get(index).copied()
            }
            ExprKind::Block { statements, tail } => {
                let mut offset = 0;
                for statement in statements {
                    let count = statement.child_count();
                    if index < offset + count {
                        return statement.child(index - offset);
                    }
                    offset += count;
                }
                (index == offset).then_some(tail)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]
            .get(index)
            .copied(),
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                if index == 0 {
                    Some(scrutinee)
                } else {
                    let mut cursor = index - 1;
                    for arm in arms {
                        if let Some(guard) = arm.guard.as_deref() {
                            if cursor == 0 {
                                return Some(guard);
                            }
                            cursor -= 1;
                        }
                        if cursor == 0 {
                            return Some(&arm.value);
                        }
                        cursor -= 1;
                    }
                    None
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                if index == 0 {
                    Some(receiver)
                } else {
                    args.get(index - 1)
                }
            }
            ExprKind::SuperMethod { args, .. } => args.get(index),
            ExprKind::ConstructRecord { .. }
            | ExprKind::ConstructVariant { .. }
            | ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::ArrayU8(_)
            | ExprKind::RepeatArrayU8 { .. }
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => None,
        }
    }
    let mut frames = FormatFrameStack::new(Frame::Enter(value), ScratchStackKind::ContainsRecord);
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(value) => {
                if matches!(
                    value.kind,
                    ExprKind::ConstructRecord { .. } | ExprKind::ConstructVariant { .. }
                ) {
                    return true;
                }
                frames.push(Frame::Children(value, 0));
            }
            Frame::Children(value, index) => {
                if let Some(child) = child(value, index) {
                    frames.push(Frame::Children(value, index + 1));
                    frames.push(Frame::Enter(child));
                }
            }
        }
    }
    false
}

pub(super) fn write_signature(
    output: &mut impl std::fmt::Write,
    params: &[crate::ast::ClosureParam],
    result: &crate::ast::Type,
) {
    output.write_str("fn(").unwrap();
    for (index, param) in params.iter().enumerate() {
        if index != 0 {
            output.write_str(", ").unwrap();
        }
        write!(output, "{}: ", param.name).unwrap();
        write_type(output, &param.ty);
    }
    output.write_str(") -> ").unwrap();
    write_type(output, result);
    output.write_char(' ').unwrap();
}
