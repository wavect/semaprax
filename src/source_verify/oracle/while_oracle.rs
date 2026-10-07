//! Test-only recursive `while` statement checking and the oracle form of the
//! `while` admission rules.

use crate::ast::{Expr, ExprKind, Function, Program, Span, Statement, Type};
#[cfg(test)]
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::Binding;
use crate::source_verify::diagnostics::{error, reject_native_unit_value};
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
    if let Some(span) = crate::string_ops::owned_string_in_condition(condition, &|name| {
        functions.get(name).is_some_and(|function| {
            function.return_type == Type::String
                || function.params.iter().any(|param| param.ty == Type::String)
        })
    }) {
        diagnostics.push(error(
            program,
            "SPX-T252",
            crate::string_ops::OWNED_STRING_CONDITION_MESSAGE,
            span,
        ));
    }
    let _ = reject_while_disallowed_oracle(program, condition, functions, diagnostics);
    let _ = reject_while_disallowed_oracle(program, body, functions, diagnostics);
    let baseline = variables.clone();
    let condition_value = super::matching::in_loop(|| {
        check_expr(
            program,
            current,
            condition,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        )
    });
    if let Some(value) = condition_value {
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
    let _ = super::matching::in_loop(|| {
        check_expr(
            program,
            current,
            body,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        )
    });
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
        ExprKind::Closure { .. } => Ok(()),

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
        // Owned String Loops v1: a literal allocates one owned String in the
        // per-iteration body region.
        ExprKind::String(_) => Ok(()),
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
            let vec_operation = crate::vec_ops::by_name(name);
            if !type_arguments.is_empty() && vec_operation.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T252",
                    "generic calls are not yet admitted in while bodies",
                    expression.span,
                ));
                return Err(());
            }
            if let Some(operation) = vec_operation {
                if !operation.admitted_in_while()
                    || type_arguments.len() != 1
                    || !crate::vec_ops::ast_element_is_admitted(&type_arguments[0])
                    || args.len() != operation.arity()
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T283",
                        "only exact typed Vec push/read operations are admitted in while bodies",
                        expression.span,
                    ));
                    return Err(());
                }
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
                if !operation.admitted_in_while() || args.len() != operation.arity() {
                    diagnostics.push(error(
                        program,
                        "SPX-T252",
                        format!(
                            "byte operation `{name}` is not admitted in while bodies; only exact byte_len, byte_get and byte_range reads and the loop-carried bytes_set fill qualify"
                        ),
                        expression.span,
                    ));
                    return Err(());
                }
            }
            if let Some(declared) = functions.get(name.as_str()) {
                let scalar_signature = crate::stdin_stream_ops::ast_forward_signature(declared)
                    || (crate::loop_calls::effects_admitted(&declared.effects)
                        && crate::loop_calls::ast_result_admitted(&declared.return_type)
                        && declared.params.iter().all(|param| {
                            crate::loop_calls::ast_param_admitted(param.mode, &param.ty)
                                || (param.mode == crate::ast::ParamMode::Borrow
                                    && crate::source_verify::declared_type::owned_record_collection::is_owner_renewal_record(
                                        &TypeTable::new(program),
                                        &param.ty,
                                    ))
                        }));
                if !scalar_signature {
                    diagnostics.push(error(
                        program,
                        "SPX-T252",
                        format!(
                            "call `{name}` is not admitted in loop bodies; use scalar/text signatures with read-only input effects. For output, build one string in the loop and write it once afterwards"
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
        // Owned String Loops v2: the typed scrutinee rule lives in the
        // match checker; the admission scan visits every operand.
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            reject_while_disallowed_oracle(program, scrutinee, functions, diagnostics)?;
            let mut result = Ok(());
            for arm in arms {
                result = match &arm.guard {
                    Some(guard) => {
                        let guard =
                            reject_while_disallowed_oracle(program, guard, functions, diagnostics);
                        let value = reject_while_disallowed_oracle(
                            program,
                            &arm.value,
                            functions,
                            diagnostics,
                        );
                        guard.and(value)
                    }
                    None => {
                        reject_while_disallowed_oracle(program, &arm.value, functions, diagnostics)
                    }
                };
                result?;
            }
            result
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
        // Resumable Effects control profile (issue #296): the parser admits a
        // direct statement-value `yield`; its request is an ordinary operand.
        ExprKind::Yield { request } => {
            reject_while_disallowed_oracle(program, request, functions, diagnostics)
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
        Statement::Assign {
            name,
            field: None,
            value,
            ..
        } if source_record_owner_renewal(program, functions, name, value) => {
            let ExprKind::Call { args, .. } = &value.kind else {
                unreachable!("record renewal admission requires a call")
            };
            let mut result = Ok(());
            for argument in args {
                result = reject_while_disallowed_oracle(program, argument, functions, diagnostics);
                result?;
            }
            result
        }
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
        Statement::For { span, .. } | Statement::ForOwn { span, .. } => {
            diagnostics.push(error(
                program,
                "SPX-T284",
                "nested for traversal is not admitted in this bounded profile",
                *span,
            ));
            Err(())
        }
    }
}

#[cfg(test)]
fn source_record_owner_renewal(
    program: &Program,
    functions: &HashMap<&str, &Function>,
    binding: &str,
    value: &Expr,
) -> bool {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
        ..
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = functions.get(name.as_str()) else {
        return false;
    };
    let types = TypeTable::new(program);
    type_arguments.is_empty()
        && target.effects.is_empty()
        && crate::source_verify::declared_type::owned_record_collection::is_owner_renewal_record(
            &types,
            &target.return_type,
        )
        && target.params.len() == args.len()
        && target
            .params
            .iter()
            .zip(args)
            .filter(|(parameter, argument)| {
                parameter.mode == crate::ast::ParamMode::Own
                    && parameter.ty == target.return_type
                    && matches!(&argument.kind, ExprKind::Var(name) if name == binding)
            })
            .count()
            == 1
        && target.params.iter().zip(args).all(|(parameter, argument)| {
            match parameter.mode {
                crate::ast::ParamMode::Own => {
                    parameter.ty == target.return_type
                        && matches!(&argument.kind, ExprKind::Var(name) if name == binding)
                }
                crate::ast::ParamMode::Borrow => {
                    crate::source_verify::declared_type::owned_record_collection::
                        is_owner_renewal_record(&types, &parameter.ty)
                        && matches!(argument.kind, ExprKind::Var(_))
                }
                crate::ast::ParamMode::Value => true,
                crate::ast::ParamMode::Shared => false,
            }
        })
}
