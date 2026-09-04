//! Identity-slot accounting for the builder pre-bound: how many declaration
//! identities the resolver may retain for each authored program shape.

use crate::ast::{Expr, ExprKind, Function, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

use super::super::{active_builder_limit, limit_error};
use super::checked_builder_sum;

pub(super) fn ast_program_identity_slots(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 0usize;
    for declaration in &program.types {
        slots = checked_builder_sum(slots, ast_type_declaration_identity_slots(declaration)?)?;
    }
    for interface in &program.interfaces {
        for import in &interface.imports {
            for param in &import.params {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&param.ty)?)?;
            }
        }
    }
    for function in &program.functions {
        slots = checked_builder_sum(slots, ast_function_identity_slots(function)?)?;
    }
    Ok(slots)
}

pub(super) fn ast_type_declaration_identity_slots(
    declaration: &TypeDeclaration,
) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 0usize;
    match &declaration.kind {
        TypeDeclarationKind::Resource { .. } => {}
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
            }
            for method in methods {
                slots = checked_builder_sum(slots, ast_function_identity_slots(method)?)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
                }
            }
        }
    }
    Ok(slots)
}

pub(super) fn ast_function_identity_slots(function: &Function) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = ast_type_identity_slots(&function.return_type)?;
    for param in &function.params {
        slots = checked_builder_sum(slots, ast_type_identity_slots(&param.ty)?)?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        slots = checked_builder_sum(slots, ast_expr_identity_slots(expression)?)?;
    }
    Ok(slots)
}

pub(super) fn ast_type_identity_slots(ty: &Type) -> Result<usize, Vec<Diagnostic>> {
    let Type::Named { arguments, .. } = ty else {
        return Ok(0);
    };
    let mut slots = 1usize;
    for argument in arguments {
        slots = checked_builder_sum(slots, ast_type_identity_slots(argument)?)?;
    }
    Ok(slots)
}

/// Declaration identities one resolved expression retains besides those of
/// its children: every shape holds its expression identity, its result-type
/// identity, and one cleanup owner; a call, construction, update, or
/// projection adds its callee, record, or field; a variant construction adds
/// its variant and case; and `Try` adds the five `Option`/`Result`
/// declaration identities it names. Variable-size field, projection, and
/// pattern identities are debited separately by the caller.
fn fixed_expression_identity_slots(kind: &ExprKind) -> usize {
    const BASE: usize = 3;
    match kind {
        ExprKind::Try { .. } => BASE + 5,
        ExprKind::ConstructVariant { .. } => BASE + 2,
        ExprKind::Call { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::SuperMethod { .. }
        | ExprKind::ConstructRecord { .. }
        | ExprKind::UpdateRecord { .. }
        | ExprKind::Project { .. } => BASE + 1,
        _ => BASE,
    }
}

fn ast_expr_identity_slots(expression: &Expr) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = fixed_expression_identity_slots(&expression.kind);
    match &expression.kind {
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::MethodCall {
            receiver,
            type_arguments,
            args,
            ..
        } => {
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            slots = checked_builder_sum(slots, ast_expr_identity_slots(receiver)?)?;
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::SuperMethod { args, .. } => {
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::Unary { value, .. } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(value)?)?;
        }
        ExprKind::Binary { left, right, .. } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(left)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(right)?)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                // A `let` creates one new value identity (its binding); an
                // assignment reuses its target's existing identity.
                if matches!(statement, crate::ast::Statement::Let { .. }) {
                    slots = checked_builder_sum(slots, 1)?;
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        slots = checked_builder_sum(slots, ast_expr_identity_slots(child)?)?;
                    }
                }
            }
            slots = checked_builder_sum(slots, ast_expr_identity_slots(tail)?)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(condition)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(then_branch)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(else_branch)?)?;
        }
        ExprKind::ConstructRecord {
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_arguments,
            fields,
            ..
        } => {
            slots = checked_builder_sum(slots, fields.len())?;
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            for field in fields {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&field.value)?)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(scrutinee)?)?;
            for arm in arms {
                slots = checked_builder_sum(slots, ast_pattern_identity_slots(&arm.pattern)?)?;
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&arm.value)?)?;
            }
        }
        ExprKind::Try { operand } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(operand)?)?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            slots = checked_builder_sum(slots, fields.len())?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(base)?)?;
            for field in fields {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&field.value)?)?;
            }
        }
        ExprKind::Project { base, .. } => {
            slots = checked_builder_sum(slots, 1)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(base)?)?;
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Var(_) => {}
    }
    Ok(slots)
}

fn ast_pattern_identity_slots(
    pattern: &crate::ast::MatchPattern,
) -> Result<usize, Vec<Diagnostic>> {
    match pattern {
        crate::ast::MatchPattern::Variant { fields, .. } => {
            let field_slots = fields
                .len()
                .checked_mul(2)
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            checked_builder_sum(2, field_slots)
        }
        crate::ast::MatchPattern::Record { fields, .. } => {
            let mut slots = 1usize;
            for field in fields {
                slots = checked_builder_sum(slots, record_pattern_identity_slots(field)?)?;
            }
            Ok(slots)
        }
        crate::ast::MatchPattern::Wildcard { .. } | crate::ast::MatchPattern::Literal { .. } => {
            Ok(0)
        }
        // Refutable Match v1: a binding arm contributes one identity slot;
        // or-patterns contribute their alternatives.
        crate::ast::MatchPattern::Binding { .. } => Ok(1),
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            let mut slots = 0usize;
            for alternative in alternatives {
                slots = checked_builder_sum(slots, ast_pattern_identity_slots(alternative)?)?;
            }
            Ok(slots)
        }
    }
}

fn record_pattern_identity_slots(
    field: &crate::ast::RecordMatchPatternField,
) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 1usize;
    if let crate::ast::RecordMatchFieldPattern::Record { fields, .. } = &field.pattern {
        slots = checked_builder_sum(slots, 1)?;
        for nested in fields {
            slots = checked_builder_sum(slots, record_pattern_identity_slots(nested)?)?;
        }
    }
    Ok(slots)
}
