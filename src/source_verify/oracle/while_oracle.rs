//! Test-only recursive `while` statement checking and the oracle form of the
//! `while` admission rules.

use crate::ast::{Expr, ExprKind, Function, ParamMode, Program, Span, Statement, Type};
#[cfg(test)]
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::Binding;
use crate::source_verify::diagnostics::{error, is_scalar_source_type, reject_native_unit_value};
use crate::source_verify::oracle::check_expr;
use crate::source_verify::type_table::TypeTable;
use std::collections::HashMap;

/// Recursive-oracle twin of the iterative verifier's `while` handling: the
/// contract-context rejection, the collect-all admission scan, the condition
/// typing check, ordinary body-block checking, and ownership-drift detection,
/// emitted in exactly the same diagnostic order.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn check_while_statement(
    program: &Program,
    current: &Function,
    condition: &Expr,
    body: &Expr,
    statement_span: Span,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !allow_moves {
        diagnostics.push(error(
            program,
            "SPX-T253",
            "while statements are not allowed in contract expressions",
            condition.span,
        ));
    }
    let _ = reject_while_disallowed_oracle(program, condition, functions, diagnostics);
    let _ = reject_while_disallowed_oracle(program, body, functions, diagnostics);
    let baseline = variables.clone();
    if let Some(value) = check_expr(
        program,
        current,
        condition,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    ) {
        if value.native_unit {
            reject_native_unit_value(program, condition, &value, diagnostics);
        } else if value.ty != Type::Bool {
            diagnostics.push(error(
                program,
                "SPX-T251",
                "`while` condition must be bool",
                condition.span,
            ));
        }
    }
    let _ = check_expr(
        program,
        current,
        body,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
    for (name, before) in &baseline {
        let drifted = match variables.get(name) {
            Some(now) => {
                now.availability != before.availability
                    || now.moved_places != before.moved_places
                    || now.definitely_partial != before.definitely_partial
            }
            None => true,
        };
        if drifted {
            diagnostics.push(error(
                program,
                "SPX-T252",
                format!(
                    "ownership of `{name}` changes inside a while loop, which is not yet admitted"
                ),
                statement_span,
            ));
        }
    }
}

/// Collect-all admission scan used by the recursive oracle; mirrors
/// `IterativeVerifier::reject_while_disallowed` diagnostic for diagnostic.
#[cfg(test)]
pub(super) fn reject_while_disallowed_oracle(
    program: &Program,
    expression: &Expr,
    functions: &HashMap<&str, &Function>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), ()> {
    match &expression.kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::Var(_) => Ok(()),
        ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "fixed-array literals are not admitted in bounded while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::SuperMethod { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "super method calls are not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::String(_) => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "string literals are not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::Unary { value, .. } => {
            reject_while_disallowed_oracle(program, value, functions, diagnostics)
        }
        ExprKind::Binary { left, right, .. } => {
            let left = reject_while_disallowed_oracle(program, left, functions, diagnostics);
            let right = reject_while_disallowed_oracle(program, right, functions, diagnostics);
            left.and(right)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition =
                reject_while_disallowed_oracle(program, condition, functions, diagnostics);
            let then = reject_while_disallowed_oracle(program, then_branch, functions, diagnostics);
            let else_branch =
                reject_while_disallowed_oracle(program, else_branch, functions, diagnostics);
            condition.and(then).and(else_branch)
        }
        ExprKind::Block { statements, tail } => {
            let mut result = Ok(());
            for statement in statements {
                result = reject_while_disallowed_statement_oracle(
                    program,
                    statement,
                    functions,
                    diagnostics,
                );
                result?;
            }
            result.and(reject_while_disallowed_oracle(
                program,
                tail,
                functions,
                diagnostics,
            ))
        }
        ExprKind::Call {
            type_arguments,
            args,
            name,
            ..
        } => {
            if !type_arguments.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T252",
                    "generic calls are not yet admitted in while bodies",
                    expression.span,
                ));
                return Err(());
            }
            if crate::command_io_ops::by_name(name)
                .is_some_and(|operation| !crate::command_io_ops::admitted_in_while(operation))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T270",
                    format!("command I/O operation `{name}` is not admitted in while bodies"),
                    expression.span,
                ));
                return Err(());
            }
            if crate::command_io_ops::by_name(name)
                .is_some_and(|operation| args.len() != crate::command_io_ops::arity(operation))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T270",
                    format!("invalid command I/O operation `{name}` call shape"),
                    expression.span,
                ));
                return Err(());
            }
            if let Some(operation) = crate::byte_ops::by_name(name) {
                if !matches!(
                    operation,
                    crate::byte_ops::ByteOp::Len
                        | crate::byte_ops::ByteOp::Get
                        | crate::byte_ops::ByteOp::Range
                ) || args.len() != operation.arity()
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T252",
                        format!(
                            "byte operation `{name}` is not admitted in while bodies; only exact byte_len and byte_get reads qualify"
                        ),
                        expression.span,
                    ));
                    return Err(());
                }
            }
            if let Some(declared) = functions.get(name.as_str()) {
                let scalar_signature = declared.effects.is_empty()
                    && is_scalar_source_type(&declared.return_type)
                    && declared.params.iter().all(|param| {
                        (param.mode == ParamMode::Value && is_scalar_source_type(&param.ty))
                            || (param.mode == ParamMode::Borrow && param.ty == Type::SliceU8)
                    });
                if !scalar_signature {
                    diagnostics.push(error(
                        program,
                        "SPX-T252",
                        format!(
                            "call `{name}` is not admitted in while bodies; only scalar functions qualify"
                        ),
                        expression.span,
                    ));
                    return Err(());
                }
            }
            let mut result = Ok(());
            for argument in args {
                result = reject_while_disallowed_oracle(program, argument, functions, diagnostics);
                result?;
            }
            result
        }
        ExprKind::MethodCall { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "method calls are not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::Project { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "record field projection is not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::ConstructRecord { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "record construction is not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::ConstructVariant { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "variant construction is not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::UpdateRecord { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "record updates are not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } if crate::byte_ops::is_indexed_byte_option_match_source(expression) => {
            reject_while_disallowed_oracle(program, scrutinee, functions, diagnostics)?;
            let mut result = Ok(());
            for arm in arms {
                result =
                    reject_while_disallowed_oracle(program, &arm.value, functions, diagnostics);
                result?;
            }
            result
        }
        ExprKind::Match { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "match expressions are not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
        ExprKind::Try { .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "postfix `?` propagation is not yet admitted in while bodies",
                expression.span,
            ));
            Err(())
        }
    }
}

#[cfg(test)]
pub(super) fn reject_while_disallowed_statement_oracle(
    program: &Program,
    statement: &Statement,
    functions: &HashMap<&str, &Function>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), ()> {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            reject_while_disallowed_oracle(program, value, functions, diagnostics)
        }
        Statement::Unsafe { span, .. } => {
            diagnostics.push(error(
                program,
                "SPX-T252",
                "unsafe boundary statements are not yet admitted in while bodies",
                *span,
            ));
            Err(())
        }
        Statement::While {
            condition, body, ..
        } => {
            let condition =
                reject_while_disallowed_oracle(program, condition, functions, diagnostics);
            let body = reject_while_disallowed_oracle(program, body, functions, diagnostics);
            condition.and(body)
        }
    }
}
