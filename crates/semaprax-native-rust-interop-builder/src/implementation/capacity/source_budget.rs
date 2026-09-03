//! Source-level expression budget admission and the closure budget
//! validators that gate resolution.

use super::*;

pub(super) fn source_functions(program: &Program) -> impl Iterator<Item = &crate::ast::Function> {
    program
        .functions
        .iter()
        .chain(
            program
                .types
                .iter()
                .flat_map(|declaration| match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
                    _ => &[],
                }),
        )
}

pub(in crate::implementation) fn validate_native_rust_source_expression_budget(
    program: &Program,
) -> Result<(), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for function in source_functions(program) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut stack_len = 1;
            stack[0] = Some((root, 1_usize, 0_usize));
            while stack_len != 0 {
                stack_len -= 1;
                let (expression, depth, next_child) = stack[stack_len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child == 0 {
                    debit(std::mem::size_of::<&crate::ast::Expr>())?;
                    if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                }
                let mut child_cursor = next_child;
                if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                    if stack_len + 2 > stack.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    stack[stack_len] = Some((expression, depth, child_cursor));
                    stack[stack_len + 1] = Some((child, depth + 1, 0));
                    stack_len += 2;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::implementation) fn validate_native_rust_expression_budget(
    resolved: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    let functions = resolved.functions.iter().collect::<Vec<_>>();
    validate_native_rust_expression_budget_for_closure(&functions, false)
}

pub(in crate::implementation) fn validate_native_rust_expression_budget_for_closure(
    functions: &[&ResolvedFunction],
    preauthorized: bool,
) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(1);
    let mut pending = Vec::new();
    for function in functions {
        pending.extend(
            function
                .requires
                .iter()
                .map(|expression| (expression, 1_usize)),
        );
        pending.push((&function.body, 1));
        pending.extend(
            function
                .ensures
                .iter()
                .map(|expression| (expression, 1_usize)),
        );
    }
    let mut visited = 0_usize;
    while let Some((expression, depth)) = pending.pop() {
        note_hir_post_resolve_capacity(
            0,
            pending.capacity() * std::mem::size_of::<(&ResolvedExpr, usize)>(),
        );
        visited = visited
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if !preauthorized {
            debit(std::mem::size_of::<&ResolvedExpr>())?;
        }
        if visited > MAX_SOURCE_BYTES {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
            return Err(b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            ));
        }
        let child_depth = depth
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                pending.extend(args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push((source, child_depth));
                pending.push((start, child_depth));
                pending.push((end, child_depth));
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                pending.extend(call.args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::HostCommandCall(call) => {
                pending.extend(call.args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push((value, child_depth)),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push((left, child_depth));
                pending.push((right, child_depth));
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    pending.extend(
                        (0..statement.child_count())
                            .filter_map(|index| statement.child(index))
                            .map(|child| (child, child_depth)),
                    );
                }
                pending.push((tail, child_depth));
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push((condition, child_depth));
                pending.push((then_branch, child_depth));
                pending.push((else_branch, child_depth));
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| (&field.value, child_depth)));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push((scrutinee, child_depth));
                pending.extend(
                    arms.iter()
                        .filter_map(|arm| arm.guard.as_deref())
                        .map(|guard| (guard, child_depth)),
                );
                pending.extend(arms.iter().map(|arm| (&arm.value, child_depth)));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push((base, child_depth));
                pending.extend(fields.iter().map(|field| (&field.value, child_depth)));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    Ok(())
}
