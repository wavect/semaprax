//! Declaration-shaped source cost for the expected projection: what one
//! function, parameter, field, or type contributes to the builder pre-bound,
//! split into the part every retained projection keeps and the contract an
//! imported stub discards.

use crate::ast::{Expr, ExprKind, Function, Type};
use crate::diagnostic::Diagnostic;

use super::ast_pattern_cost;
use super::cost::StructuralCost;

pub(super) fn ast_field_cost(
    field: &crate::ast::FieldDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(field)?;
    cost.string(&field.stable_id)?;
    cost.string(&field.name)?;
    ast_type_cost(&field.ty, cost)
}

pub(super) fn ast_function_cost(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    ast_function_signature_cost(function, cost)?;
    ast_function_contract_cost_into(function, cost)
}

/// The part of a function every retained projection keeps: its identity, its
/// name, its type parameters, its parameters, its return type, and its effect
/// row. An imported function is retained as exactly this much plus a synthetic
/// default body.
pub(super) fn ast_function_signature_cost(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(function)?;
    cost.string(&function.stable_id)?;
    cost.string(&function.name)?;
    for parameter in &function.type_parameters {
        cost.value(parameter)?;
        cost.string(&parameter.name)?;
    }
    for param in &function.params {
        ast_param_cost(param, cost)?;
    }
    ast_type_cost(&function.return_type, cost)?;
    for effect in &function.effects {
        cost.string(effect)?;
    }
    Ok(())
}

/// The preconditions, body, and postconditions of a function. An imported
/// function's contract is cloned transiently and then discarded, so it is
/// charged as a peak rather than as retained structure.
pub(super) fn ast_function_contract_cost(
    function: &Function,
) -> Result<StructuralCost, Vec<Diagnostic>> {
    let mut cost = StructuralCost::new();
    ast_function_contract_cost_into(function, &mut cost)?;
    Ok(cost)
}

fn ast_function_contract_cost_into(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        ast_expr_cost(expression, cost)?;
    }
    Ok(())
}

pub(super) fn ast_param_cost(
    param: &crate::ast::Param,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(param)?;
    cost.string(&param.name)?;
    ast_type_cost(&param.ty, cost)
}

pub(super) fn ast_type_cost(ty: &Type, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(ty)?;
    if let Type::Named { name, arguments } = ty {
        cost.string(name)?;
        for argument in arguments {
            ast_type_cost(argument, cost)?;
        }
    }
    Ok(())
}

pub(super) fn ast_expr_cost(
    expression: &Expr,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(expression)?;
    match &expression.kind {
        ExprKind::Closure {
            params,
            return_type,
            body,
        } => {
            for parameter in params {
                cost.value(parameter)?;
                cost.string(&parameter.name)?;
                ast_type_cost(&parameter.ty, cost)?;
            }
            ast_type_cost(return_type, cost)?;
            ast_expr_cost(body, cost)?;
        }
        ExprKind::Var(name) => cost.string(name)?,
        ExprKind::ArrayU8(values) => cost.add(values.len())?,
        ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            cost.string(name)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::Unary { value, .. } => ast_expr_cost(value, cost)?,
        ExprKind::Binary { left, right, .. } => {
            ast_expr_cost(left, cost)?;
            ast_expr_cost(right, cost)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                cost.value(statement)?;
                // Charge the string every statement actually carries. Unsafe
                // boundaries carry their verbatim audit summary, `while`
                // carries no binding at all, bounded `for` traversal carries
                // its item binding, and `let`/assignment carry their name.
                // Matching the statement exhaustively keeps a later variant a
                // compile error here rather than a panic at check time:
                // `Statement::name` panics for every statement that is not a
                // `let` or an assignment.
                match statement {
                    crate::ast::Statement::Unsafe { audit, .. } => cost.string(audit)?,
                    crate::ast::Statement::While { .. } => cost.string("")?,
                    crate::ast::Statement::For { item, .. } => cost.string(item)?,
                    crate::ast::Statement::Let { name, .. }
                    | crate::ast::Statement::Assign { name, .. } => cost.string(name)?,
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        ast_expr_cost(child, cost)?;
                    }
                }
            }
            ast_expr_cost(tail, cost)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_cost(condition, cost)?;
            ast_expr_cost(then_branch, cost)?;
            ast_expr_cost(else_branch, cost)?;
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            cost.string(type_name)?;
            if let ExprKind::ConstructVariant { case_name, .. } = &expression.kind {
                cost.string(case_name)?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                ast_expr_cost(&field.value, cost)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            ast_expr_cost(scrutinee, cost)?;
            for arm in arms {
                cost.value(arm)?;
                ast_pattern_cost(&arm.pattern, cost)?;
                ast_expr_cost(&arm.value, cost)?;
            }
        }
        ExprKind::Try { operand } => ast_expr_cost(operand, cost)?,
        ExprKind::UpdateRecord { base, fields } => {
            ast_expr_cost(base, cost)?;
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                ast_expr_cost(&field.value, cost)?;
            }
        }
        ExprKind::Project { base, field, .. } => {
            ast_expr_cost(base, cost)?;
            cost.string(field)?;
        }
        ExprKind::MethodCall {
            receiver,
            method,
            type_arguments,
            args,
            ..
        } => {
            ast_expr_cost(receiver, cost)?;
            cost.string(method)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::SuperMethod { method, args, .. } => {
            cost.string(method)?;
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_) => {}
    }
    Ok(())
}
