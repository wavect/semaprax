//! SPX-AI-021 bounded owning-capture closure: interpreter-only AST sugar.
//!
//! `resolve_closure` refuses every `own fn(...)` program before HIR
//! lowering (see [`super::resolve`] and `docs/CLOSURES-OWNING-V1.md`): this
//! stays true for every caller of [`crate::hir::resolve`], including native
//! C11 (`crate::codegen::emit_c`) and Core Wasm (`crate::wasm::emit_module`),
//! which both resolve the *original* program directly and are therefore
//! unaffected by anything in this module.
//!
//! The interpreter alone gets real execution, and it needs no new runtime
//! carrier, cleanup obligation, or HIR variant to get it. `own fn() -> R {
//! target(payload) }` carries no state beyond "which target function" and
//! "which captured local" -- both already fully known from its own
//! construction syntax. `source_verify::owning_closure` has already proven,
//! for any program that reaches here, that: the checked sentinel type can
//! never be spelled, escaped, aliased, or read as a value; the only use of
//! a bound closure other than the construction itself is a direct
//! zero-argument call `name()`; and construction moves its one capture
//! exactly once. Those facts make construction-plus-its-at-most-one-call
//! *semantically* identical to substituting the call site directly with the
//! target call and dropping the now-dead binding: whichever control-flow
//! path executes the call runs `target(payload)` exactly where `name()`
//! stood, and a path that never calls it leaves `payload` exactly as
//! unmoved as it always was, so the existing scope-exit drop for an
//! ordinary unmoved owned local settles it -- no parallel cleanup mechanism
//! needed.
//!
//! This module performs exactly that substitution on an already-verified
//! [`Program`] (never on unchecked source: callers must run
//! `source_verify::verify` first and only desugar on success), producing an
//! ordinary program containing no `own fn` construct at all. Only
//! `interpreter::interpret` calls it.
use crate::ast::{Expr, ExprKind, Function, Program, Statement};
use std::collections::HashMap;

/// Substitute every bounded owning-capture closure construction and its
/// single admitted call with an ordinary direct call, across every function
/// in `program`. Returns `None` when `program` contains no `own fn`
/// construct at all, so callers can skip cloning and keep resolving the
/// original program unchanged.
pub(crate) fn desugar_owning_closures(program: &Program) -> Option<Program> {
    if !program
        .functions
        .iter()
        .any(|function| expr_has_owning_closure(&function.body))
    {
        return None;
    }
    let mut rewritten = program.clone();
    for function in &mut rewritten.functions {
        lower_function(function);
    }
    Some(rewritten)
}

fn expr_has_owning_closure(expr: &Expr) -> bool {
    if matches!(
        expr.kind,
        ExprKind::Closure {
            owning: true,
            retained: false,
            mutable: false,
            ..
        }
    ) {
        return true;
    }
    match &expr.kind {
        ExprKind::Closure { body, .. } => expr_has_owning_closure(body),
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            args.iter().any(expr_has_owning_closure)
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_has_owning_closure(receiver) || args.iter().any(expr_has_owning_closure)
        }
        ExprKind::Unary { value, .. } | ExprKind::Try { operand: value } => {
            expr_has_owning_closure(value)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_has_owning_closure(left) || expr_has_owning_closure(right)
        }
        ExprKind::Block { statements, tail } => {
            statements.iter().any(statement_has_owning_closure) || expr_has_owning_closure(tail)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_has_owning_closure(condition)
                || expr_has_owning_closure(then_branch)
                || expr_has_owning_closure(else_branch)
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            fields
                .iter()
                .any(|field| expr_has_owning_closure(&field.value))
        }
        ExprKind::UpdateRecord { base, fields } => {
            expr_has_owning_closure(base)
                || fields
                    .iter()
                    .any(|field| expr_has_owning_closure(&field.value))
        }
        ExprKind::Project { base, .. } => expr_has_owning_closure(base),
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            expr_has_owning_closure(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_deref().is_some_and(expr_has_owning_closure)
                        || expr_has_owning_closure(&arm.value)
                })
        }
        _ => false,
    }
}

fn statement_has_owning_closure(statement: &Statement) -> bool {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            expr_has_owning_closure(value)
        }
        Statement::Unsafe { body, .. } => expr_has_owning_closure(body),
        Statement::While {
            condition, body, ..
        } => expr_has_owning_closure(condition) || expr_has_owning_closure(body),
        Statement::For { values, body, .. } | Statement::ForOwn { values, body, .. } => {
            expr_has_owning_closure(values) || expr_has_owning_closure(body)
        }
    }
}

fn lower_function(function: &mut Function) {
    let mut substitutions = HashMap::new();
    lower_expr(&mut function.body, &mut substitutions);
}

/// The closure body the parser always produces is a block; this bounded
/// profile admits only a bare tail call and no statements (checked by
/// `source_verify::owning_closure::check_construction`), so unwrapping that
/// one layer recovers the exact target call.
fn unwrap_target_call(body: &Expr) -> Expr {
    match &body.kind {
        ExprKind::Block { statements, tail } if statements.is_empty() => (**tail).clone(),
        _ => body.clone(),
    }
}

fn lower_expr(expr: &mut Expr, substitutions: &mut HashMap<String, Expr>) {
    if let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expr.kind
    {
        if type_arguments.is_empty() && args.is_empty() {
            if let Some(replacement) = substitutions.get(name.as_str()) {
                *expr = replacement.clone();
                return;
            }
        }
    }
    match &mut expr.kind {
        ExprKind::Closure { body, .. } => lower_expr(body, substitutions),
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            for arg in args {
                lower_expr(arg, substitutions);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            lower_expr(receiver, substitutions);
            for arg in args {
                lower_expr(arg, substitutions);
            }
        }
        ExprKind::Unary { value, .. } => lower_expr(value, substitutions),
        ExprKind::Try { operand } => lower_expr(operand, substitutions),
        ExprKind::Binary { left, right, .. } => {
            lower_expr(left, substitutions);
            lower_expr(right, substitutions);
        }
        ExprKind::Block { statements, tail } => {
            lower_statements(statements, substitutions);
            lower_expr(tail, substitutions);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            lower_expr(condition, substitutions);
            lower_expr(then_branch, substitutions);
            lower_expr(else_branch, substitutions);
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                lower_expr(&mut field.value, substitutions);
            }
        }
        ExprKind::UpdateRecord { base, fields } => {
            lower_expr(base, substitutions);
            for field in fields {
                lower_expr(&mut field.value, substitutions);
            }
        }
        ExprKind::Project { base, .. } => lower_expr(base, substitutions),
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            lower_expr(scrutinee, substitutions);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    lower_expr(guard, substitutions);
                }
                lower_expr(&mut arm.value, substitutions);
            }
        }
        _ => {}
    }
}

fn lower_statements(statements: &mut Vec<Statement>, substitutions: &mut HashMap<String, Expr>) {
    statements.retain_mut(|statement| match statement {
        Statement::Let { name, value, .. } => {
            if let ExprKind::Closure {
                owning: true,
                retained: false,
                mutable: false,
                body,
                ..
            } = &value.kind
            {
                substitutions.insert(name.clone(), unwrap_target_call(body));
                return false;
            }
            lower_expr(value, substitutions);
            true
        }
        Statement::Assign { value, .. } => {
            lower_expr(value, substitutions);
            true
        }
        Statement::Unsafe { body, .. } => {
            lower_expr(body, substitutions);
            true
        }
        Statement::While {
            condition, body, ..
        } => {
            lower_expr(condition, substitutions);
            lower_expr(body, substitutions);
            true
        }
        Statement::For { values, body, .. } | Statement::ForOwn { values, body, .. } => {
            lower_expr(values, substitutions);
            lower_expr(body, substitutions);
            true
        }
    });
}

#[cfg(test)]
mod tests;
