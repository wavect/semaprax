//! Diagnostic construction for source verification, plus the shared
//! predicates that decide whether an identifier or scalar type is admitted.

use super::binding::{Availability, Binding, CheckedValue};
use super::iterative::{check_expr_with_helpers_view, check_expr_with_signature_view};
use super::type_table::TypeTable;
use crate::ast::program_source::ProgramSource;
use crate::ast::{Expr, ExprKind, Function, ParamMode, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) fn reject_native_unit_value(
    program: &dyn ProgramSource,
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
    program: &dyn ProgramSource,
    expression: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.mode == ParamMode::Value && crate::loop_calls::ast_copy_variant(program, &value.ty) {
        return;
    }
    if !matches!(
        value.ty,
        Type::Never
            | Type::I64
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

/// Owned String Loops v2: a `match` inside a loop condition or body admits a
/// Copy scalar scrutinee or a variant whose every payload is a Copy scalar.
/// HIR validation re-checks the same rule on resolved types.
pub(super) fn reject_loop_match_scrutinee(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    scrutinee: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.native_unit || is_scalar_source_type(&value.ty) {
        return;
    }
    if let (Type::Named { name, arguments }, Some(cases)) =
        (&value.ty, types.variant_cases(&value.ty))
    {
        let declaration = types.declaration(name);
        if cases.iter().all(|case| {
            case.fields.iter().all(|field| {
                declaration
                    .and_then(|declaration| {
                        TypeTable::substitute_variant_type(declaration, arguments, &field.ty)
                    })
                    .is_some_and(|ty| {
                        is_scalar_source_type(&ty)
                            || super::declared_type::copy_record_collection::admitted(types, &ty)
                    })
            })
        }) {
            return;
        }
    }
    diagnostics.push(error(
        program,
        "SPX-T252",
        crate::loop_calls::match_scrutinee_refusal(&value.ty.to_string()),
        scrutinee.span,
    ));
}

pub(super) fn reject_aggregate_equality(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    expression: &Expr,
    value: &CheckedValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if types.payload_free_variant(&value.ty) {
        return;
    }
    if let (Type::Named { name, .. }, Some(cases)) = (&value.ty, types.variant_cases(&value.ty)) {
        let example = cases
            .iter()
            .find(|case| case.fields.is_empty())
            .map(|case| {
                format!(
                    ", e.g. `match value {{ {name}::{} {{}} => true, _ => false, }}`",
                    case.name
                )
            })
            .unwrap_or_default();
        diagnostics.push(
            error(
                program,
                "SPX-T207",
                format!(
                    "equality on variant `{}` requires a non-generic variant whose cases all \
                     carry no payload",
                    value.ty
                ),
                expression.span,
            )
            .with_help(format!("test the case with `match` instead{example}")),
        );
        return;
    }
    if matches!(
        value.ty,
        Type::OnceFunction
            | Type::OnceFunctionI64
            | Type::OnceFunctionI64Pair
            | Type::MutFunctionI64
            | Type::Function { .. }
            | Type::Named { .. }
            | Type::ArrayU8(_)
            | Type::Bytes
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
    program: &dyn ProgramSource,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    require_bool_with_signature(
        program,
        function,
        contract,
        variables,
        functions,
        types,
        result_type,
        diagnostics,
        kind,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn require_bool_with_signature<'p>(
    program: &'p dyn ProgramSource,
    function: &'p Function,
    contract: &'p Expr,
    variables: &HashMap<String, Binding>,
    functions: &'p HashMap<&'p str, &'p Function>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
    signature: Option<super::declaration::SignatureOrigin<'p>>,
) {
    contract.visit_calls(&mut |callee, span| {
        if variables.contains_key(callee) { return; }
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
    // No freshness from a body or previous clause enters this contract domain.
    for binding in contract_variables.values_mut() {
        binding.contract_local = false;
    }
    if let Some(ty) = result_type {
        // A provisional owned result is readable only in this postcondition
        // scope. It stays immutable and contract transfers still select O105.
        if ty == &function.return_type
            && (matches!(ty, Type::Bytes | Type::String)
                || matches!(ty, Type::Named { name, arguments }
                if name == "Result" && arguments.as_slice() == [Type::Bytes, Type::Bytes]))
        {
            contract_variables.insert(
                "result".to_owned(),
                Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: ty.clone(),
                    mode: ParamMode::Own,
                    availability: Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: BTreeSet::new(),
                    borrow_origin: None,
                },
            );
        }
    }
    let (contract_value, refusal_view) = match signature {
        Some(signature) => check_expr_with_signature_view(
            program,
            function,
            contract,
            &mut contract_variables,
            functions,
            types,
            result_type,
            false,
            diagnostics,
            None,
            None,
            Some(signature),
        ),
        None => check_expr_with_helpers_view(
            program,
            function,
            contract,
            &mut contract_variables,
            functions,
            types,
            result_type,
            false,
            diagnostics,
            None,
            None,
        ),
    };
    if let Some(value) = contract_value {
        if value.native_unit {
            reject_native_unit_value(program, contract, &value, diagnostics);
        } else if value.ty != Type::Bool
            && !refusal_view.is_some_and(|view| view.declared == &Type::Bool)
        {
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
    program: &dyn ProgramSource,
    code: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Diagnostic {
    let diagnostic = Diagnostic::error(code, message, span).at_path(program.path());
    super::hints::with_authoring_repair_help(diagnostic)
}

pub(super) fn invalid_stable_id(
    program: &dyn ProgramSource,
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
    program: &dyn ProgramSource,
    stable_id: &str,
    kind: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if crate::vec_ops::by_id(stable_id).is_some() || stable_id == crate::vec_field::ID {
        diagnostics.push(error(
            program,
            "SPX-S113",
            format!(
                "authored {kind} uses stable ID `{stable_id}`, which is reserved by the compiler-owned vector operations"
            ),
            span,
        ));
    } else if crate::string_ops::by_id(stable_id).is_some() {
        diagnostics.push(error(
            program,
            "SPX-S113",
            format!(
                "authored {kind} uses stable ID `{stable_id}`, which is reserved by the compiler-owned string operations"
            ),
            span,
        ));
    } else if crate::host_io_ops::by_id(stable_id).is_some()
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
                | "vec_of"
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
