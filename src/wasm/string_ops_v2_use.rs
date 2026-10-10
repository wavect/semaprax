use super::{ResolvedExpr, ResolvedExprKind, ResolvedProgram};

/// Whether any resolved function body or contract calls a breadth-v2
/// compiler-owned string operation intrinsic.
pub(super) fn program_uses_string_ops_v2(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_breadth_v2)
            {
                return true;
            }
        }
        match &expression.kind {
            ResolvedExprKind::Closure { captures, .. } => {
                pending.extend(captures.iter().map(|capture| &capture.value))
            }
            ResolvedExprKind::Call { args, .. } | ResolvedExprKind::LiteralFormat { args, .. } | ResolvedExprKind::VecFieldRead { args, .. } => {
                pending.extend(args.iter())
            }
            ResolvedExprKind::Invoke { callable, args } => {
                pending.push(callable);
                pending.extend(args.iter());
            }
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value }
            | ResolvedExprKind::Yield { request: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                pending.extend(arms.iter().map(|arm| &arm.value));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::FunctionReference { .. } => {}
        }
    }
    false
}

/// Whether a program reaches either canonical integer-to-decimal operation.
pub(super) fn program_uses_numeric_text(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_numeric_text)
            {
                return true;
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
