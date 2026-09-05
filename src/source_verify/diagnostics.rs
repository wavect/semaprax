//! Diagnostic construction for source verification, plus the shared
//! predicates that decide whether an identifier or scalar type is admitted.

use super::binding::{Binding, CheckedValue};
use super::iterative::check_expr_iterative;
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Function, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

pub(super) fn reject_native_unit_value(
    program: &Program,
    expression: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.native_unit && !matches!(expression.kind, ExprKind::Var(_)) {
        diagnostics.push(error(
            program,
            "SPX-B107",
            "Native Rust Interop declaration set is unsupported: scalar value signature required",
            expression.span,
        ));
    }
}

pub(super) fn reject_aggregate_match_result(
    program: &Program,
    expression: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(
        value.ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::String
    ) {
        diagnostics.push(
            error(
                program,
                "SPX-T258",
                "aggregate-valued match arms are outside the executable match profile",
                expression.span,
            )
            .with_help(
                "build the aggregate with `if`, or make the match arms extract scalar values",
            ),
        );
    }
}

pub(super) fn reject_aggregate_equality(
    program: &Program,
    expression: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(
        value.ty,
        Type::Named { .. } | Type::ArrayU8(_) | Type::Bytes
    ) {
        diagnostics.push(
            error(
                program,
                "SPX-T207",
                "aggregate equality is outside the executable comparison profile",
                expression.span,
            )
            .with_help("compare the relevant scalar fields or match the aggregate case explicitly"),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn require_bool(
    program: &Program,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    contract.visit_calls(&mut |callee, span| {
        if let Some(target) = functions.get(callee) {
            if !target.effects.is_empty() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-C102",
                        format!(
                            "{kind} on `{}` calls effectful function `{callee}` with effects {{{}}}",
                            function.name,
                            target.effects.join(", ")
                        ),
                        span,
                    )
                    .with_help("contracts must be deterministic and effect-free"),
                );
            }
        }
    });
    let mut contract_variables = variables.clone();
    if let Some(value) = check_expr_iterative(
        program,
        function,
        contract,
        &mut contract_variables,
        functions,
        types,
        result_type,
        false,
        diagnostics,
    ) {
        if value.native_unit {
            reject_native_unit_value(program, contract, &value, diagnostics);
        } else if value.ty != Type::Bool {
            diagnostics.push(error(
                program,
                "SPX-C101",
                format!("{kind} on `{}` must be bool", function.name),
                contract.span,
            ));
        }
    }
}

pub(super) fn error(
    program: &Program,
    code: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(code, message, span).at_path(&program.path)
}

pub(super) fn invalid_stable_id(
    program: &Program,
    code: &'static str,
    subject: impl Into<String>,
    span: Span,
) -> Diagnostic {
    error(
        program,
        code,
        format!(
            "{} has an invalid stable id; persistent identities forbid NUL",
            subject.into()
        ),
        span,
    )
}

pub(super) fn reject_reserved_host_id(
    program: &Program,
    stable_id: &str,
    kind: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if crate::host_io_ops::by_id(stable_id).is_some()
        || crate::command_io_ops::by_id(stable_id).is_some()
    {
        diagnostics.push(error(
            program,
            "SPX-S113",
            format!(
                "authored {kind} uses stable ID `{stable_id}`, which is reserved by the compiler-owned host I/O operations"
            ),
            span,
        ));
    }
}

pub(super) fn source_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let plain = matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    plain
        && !matches!(
            value,
            "module"
                | "permit"
                | "resource"
                | "fn"
                | "own"
                | "borrow"
                | "shared"
                | "uses"
                | "requires"
                | "ensures"
                | "let"
                | "if"
                | "else"
                | "true"
                | "false"
                | "result"
        )
}

/// Explicit Mutation v1 admits exactly the checked Copy scalar value types.
pub(crate) fn is_scalar_source_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}
