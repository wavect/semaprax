//! Test-only recursive verification oracle.
//!
//! Mirrors the iterative verifier so tests can cross-check the frame machine
//! against a direct recursive reading of the same rules.

use self::calls::{oracle_call, oracle_method_call};
use self::matching::oracle_match;
use self::while_oracle::check_while_statement;
use super::binding::{Availability, Binding, CheckedValue};
use super::declared_type::{
    check_declared_type, ordinary_option_argument, ordinary_result_arguments,
};
use super::diagnostics::{error, reject_native_unit_value, source_identifier};
use super::loans::{
    activate_local_loan, join_conditional, local_borrow_origin, mark_value_sources_moved,
    merge_moved, release_dead_local_loans,
};
use super::place::{
    check_source_place_availability, join_definitely_partial, join_moved_places,
    overlapping_place_state, source_place,
};
use super::type_table::{effective_record_fields, TypeTable};
use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, ParamMode, Program, Statement, Type, TypeDeclarationKind,
    UnaryOp,
};
#[cfg(test)]
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeSet, HashMap, HashSet};

mod calls;
mod matching;
mod while_oracle;

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr(
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    match &expr.kind {
        ExprKind::Int(_) => Some(CheckedValue::value(Type::I64)),
        ExprKind::Int32(_) => Some(CheckedValue::value(Type::I32)),
        ExprKind::Char(_) => Some(CheckedValue::value(Type::Char)),
        ExprKind::Uint8(_) => Some(CheckedValue::value(Type::U8)),
        ExprKind::Usize(_) => Some(CheckedValue::value(Type::Usize)),
        ExprKind::ArrayU8(values) => Some(CheckedValue::value(Type::ArrayU8(
            u32::try_from(values.len()).expect("parsed array length fits u32"),
        ))),
        ExprKind::RepeatArrayU8 { count, .. } => {
            Some(CheckedValue::value(Type::ArrayU8(*count)))
        }
        ExprKind::Float32(_) => Some(CheckedValue::value(Type::F32)),
        ExprKind::Float64(_) => Some(CheckedValue::value(Type::F64)),
        ExprKind::Bool(_) => Some(CheckedValue::value(Type::Bool)),
        ExprKind::String(_) => Some(CheckedValue::value(Type::String)),
        // This walker only traverses generic-function and contract
        // expressions; `super` is meaningful only inside a class-method
        // override, whose body is resolved by the HIR layer instead.
        ExprKind::SuperMethod { method_span, .. } => {
            diagnostics.push(error(
                program,
                "SPX-T231",
                "`super` is only allowed inside a class-method override",
                *method_span,
            ));
            None
        }
        ExprKind::Var(name) if name == "result" => result_type
            .map(|ty| CheckedValue::returned(ty.clone(), types.needs_drop(ty)))
            .or_else(|| {
                diagnostics.push(error(
                    program,
                    "SPX-T201",
                    "`result` is only available in postconditions",
                    expr.span,
                ));
                None
            }),
        ExprKind::Var(name) => variables
            .get(name.as_str())
            .map(|binding| {
                match binding.availability {
                    Availability::Moved => diagnostics.push(
                        error(
                            program,
                            "SPX-O101",
                            format!("use of resource `{name}` after ownership was moved"),
                            expr.span,
                        )
                        .with_help("borrow the resource if the callee does not need ownership"),
                    ),
                    Availability::MaybeMoved => diagnostics.push(
                        error(
                            program,
                            "SPX-O107",
                            format!(
                                "resource `{name}` may have been moved on another control-flow path"
                            ),
                            expr.span,
                        )
                        .with_help("move the resource on every path or keep it borrowed"),
                    ),
                    Availability::Available => match overlapping_place_state(binding, &[]) {
                        Availability::Moved => diagnostics.push(
                            error(
                                program,
                                "SPX-O109",
                                format!("use of partially moved place `{name}`"),
                                expr.span,
                            )
                            .with_help(
                                "use an available sibling field or avoid moving this place earlier",
                            ),
                        ),
                        Availability::MaybeMoved => diagnostics.push(
                            error(
                                program,
                                "SPX-O110",
                                format!(
                                    "place `{name}` may have been moved on another control-flow path"
                                ),
                                expr.span,
                            )
                            .with_help("move the field on every path or keep it borrowed"),
                        ),
                        Availability::Available => {}
                    },
                }
                if binding.native_unit_discard {
                    diagnostics.push(error(
                        program,
                        "SPX-B107",
                        "Native Rust Interop declaration set is unsupported: scalar value signature required",
                        expr.span,
                    ));
                }
                CheckedValue {
                    ty: binding.ty.clone(),
                    mode: binding.mode,
                    native_unit: binding.native_unit_discard,
                }
            })
            .or_else(|| {
                diagnostics.push(error(
                    program,
                    "SPX-T202",
                    format!("unknown value `{name}` in `{}`", current.name),
                    expr.span,
                ));
                None
            }),
        ExprKind::Call { name, type_arguments, args } => oracle_call(
            name,
            type_arguments,
            args,
            program,
            current,
            expr,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        ),
        ExprKind::MethodCall { receiver, method, type_arguments, args, .. } => oracle_method_call(
            receiver,
            method,
            type_arguments,
            args,
            program,
            current,
            expr,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        ),
        ExprKind::Unary { op, value } => {
            // Peel maximal unary chains iteratively. The language admits an
            // exact semantic depth of 512, which must not consume one verifier
            // call frame per node on an ordinary caller stack.
            let mut unary = vec![(*op, value.as_ref(), expr.span)];
            let mut leaf = value.as_ref();
            while let ExprKind::Unary { op, value } = &leaf.kind {
                unary.push((*op, value.as_ref(), leaf.span));
                leaf = value;
            }
            let mut actual = check_expr(
                program,
                current,
                leaf,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            for (op, operand, span) in unary.into_iter().rev() {
                let numeric = matches!(op, UnaryOp::Neg)
                    .then(|| actual.ty.clone())
                    .filter(|ty| matches!(ty, Type::I64 | Type::I32 | Type::F32 | Type::F64));
                let expected = match (&op, &numeric) {
                    (UnaryOp::Neg, Some(ty)) => ty.clone(),
                    (UnaryOp::Neg, None) => Type::I64,
                    (UnaryOp::Not, _) => Type::Bool,
                };
                if !actual.native_unit && actual.ty != expected {
                    diagnostics.push(error(
                        program,
                        "SPX-T206",
                        format!("unary operator expects {expected}, received {}", actual.ty),
                        span,
                    ));
                }
                reject_native_unit_value(program, operand, &actual, diagnostics);
                actual = CheckedValue::value(expected);
            }
            Some(actual)
        }
        ExprKind::Binary { op, left, right } => {
            let left_ty = check_expr(
                program,
                current,
                left,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let right_ty = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                let names = variables.keys().cloned().collect::<Vec<_>>();
                let mut right_variables = variables.clone();
                let value = check_expr(
                    program,
                    current,
                    right,
                    &mut right_variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                join_conditional(variables, &right_variables, &names);
                value
            } else {
                check_expr(
                    program,
                    current,
                    right,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                )
            };
            if let Some(value) = &left_ty {
                reject_native_unit_value(program, left, value, diagnostics);
            }
            if let Some(value) = &right_ty {
                reject_native_unit_value(program, right, value, diagnostics);
            }
            let native_unit_operand = left_ty.as_ref().is_some_and(|value| value.native_unit)
                || right_ty.as_ref().is_some_and(|value| value.native_unit);
            let left_ordered = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| {
                    matches!(
                        ty,
                        Type::I64 | Type::I32 | Type::Char | Type::U8 | Type::Usize | Type::F32 | Type::F64
                    )
                });
            let left_narrow = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| matches!(ty, Type::U8));
            let left_usize = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| matches!(ty, Type::Usize));
            let left_numeric = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| matches!(ty, Type::F32 | Type::F64));
            let left_integer = left_ty
                .as_ref()
                .map(|value| value.ty.clone())
                .filter(|ty| matches!(ty, Type::I32));
            if !native_unit_operand
                && matches!(op, BinaryOp::Rem)
                && (left_numeric.is_some()
                    || left_integer.is_some()
                    || left_narrow.is_some())
            {
                diagnostics.push(error(
                    program,
                    "SPX-T208",
                    format!("operator `{}` expects i64 operands", op.text()),
                    expr.span,
                ));
            }
            if !native_unit_operand
                && !matches!(op, BinaryOp::Eq | BinaryOp::Ne)
                && (left_ty.as_ref().is_some_and(|value: &CheckedValue| value.ty == Type::String)
                    || right_ty
                        .as_ref()
                        .is_some_and(|value: &CheckedValue| value.ty == Type::String))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T250",
                    format!("operator `{}` does not support string operands", op.text()),
                    expr.span,
                ));
            }
            let (expected, output) = match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    let expected = left_numeric
                        .clone()
                        .or(left_integer)
                        .or(left_narrow)
                        .or(left_usize)
                        .unwrap_or(Type::I64);
                    (expected.clone(), expected)
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    let expected = left_ordered.unwrap_or(Type::I64);
                    (expected, Type::Bool)
                }
                BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
                BinaryOp::Eq | BinaryOp::Ne => {
                    if !native_unit_operand
                        && left_ty.is_some()
                        && right_ty.is_some()
                        && left_ty.as_ref().map(|value| &value.ty)
                            != right_ty.as_ref().map(|value| &value.ty)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T207",
                            "equality operands must have the same type",
                            expr.span,
                        ));
                    }
                    return Some(CheckedValue::value(Type::Bool));
                }
            };
            if !native_unit_operand
                && (left_ty.as_ref().is_some_and(|value| value.ty != expected)
                    || right_ty.as_ref().is_some_and(|value| value.ty != expected))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T208",
                    format!("operator `{}` expects {expected} operands", op.text()),
                    expr.span,
                ));
            }
            Some(CheckedValue::value(output))
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            let declaration = types.declaration(type_name);
            let instance = Type::Named {
                name: type_name.clone(),
                arguments: type_arguments.clone(),
            };
            check_declared_type(
                program,
                &instance,
                expr.span,
                types,
                &HashSet::new(),
                diagnostics,
            );
            let declared_fields = declaration.and_then(|declaration| match &declaration.kind {
                TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => Some(fields.as_slice()),
                TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => None,
            });
            if declared_fields.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!("`{type_name}` is not a declared record type"),
                    expr.span,
                ));
            }

            let mut supplied = HashSet::new();
            for field in fields {
                let declared = declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                });
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T212",
                        format!(
                            "unknown or duplicate field `{}` in `{type_name}` construction",
                            field.name
                        ),
                        field.span,
                    ));
                }
                let actual = check_expr(
                    program,
                    current,
                    &field.value,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let (Some(declared), Some(actual)) = (declared, actual) {
                    reject_native_unit_value(program, &field.value, &actual, diagnostics);
                    let expected = declaration
                        .and_then(|declaration| {
                            TypeTable::substitute_variant_type(
                                declaration,
                                type_arguments,
                                &declared.ty,
                            )
                        })
                        .unwrap_or_else(|| declared.ty.clone());
                    if actual.ty != expected {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "field `{}.{}` expects {}, received {}",
                                type_name, field.name, expected, actual.ty
                            ),
                            field.value.span,
                        ));
                    }
                    if types.needs_drop(&declared.ty) && actual.mode == ParamMode::Own {
                        if allow_moves {
                            mark_value_sources_moved(
                                program,
                                &field.value,
                                variables,
                                types,
                                diagnostics,
                            );
                        } else {
                            diagnostics.push(error(
                                program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned record field",
                                field.value.span,
                            ));
                        }
                    } else if types.needs_drop(&declared.ty)
                        && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O108",
                            "cannot move an owned field through a borrowed or shared record",
                            field.value.span,
                        ));
                    }
                }
            }
            if let Some(declared_fields) = declared_fields {
                for field in declared_fields {
                    if !supplied.contains(field.name.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-T213",
                            format!(
                                "record `{type_name}` construction is missing field `{}`",
                                field.name
                            ),
                            expr.span,
                        ));
                    }
                }
            }

            declared_fields.map(|_| {
                CheckedValue::returned(instance.clone(), types.needs_drop(&instance))
            })
        }
        ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            case_name,
            fields,
            ..
        } => {
            let declaration = types.declaration(type_name);
            let instance = Type::Named {
                name: type_name.clone(),
                arguments: type_arguments.clone(),
            };
            check_declared_type(
                program,
                &instance,
                expr.span,
                types,
                &HashSet::new(),
                diagnostics,
            );
            let cases = declaration.and_then(|declaration| match &declaration.kind {
                TypeDeclarationKind::Variant { cases } => Some(cases.as_slice()),
                TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => None,
            });
            let case = cases.and_then(|cases| cases.iter().find(|case| case.name == *case_name));
            if cases.is_none() || case.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!("`{type_name}::{case_name}` is not a declared variant constructor"),
                    expr.span,
                ));
            }
            let mut supplied = HashSet::new();
            for field in fields {
                let declared = case.and_then(|case| {
                    case.fields
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                });
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T212",
                        format!(
                            "unknown or duplicate payload field `{}` in `{type_name}::{case_name}` construction",
                            field.name
                        ),
                        field.span,
                    ));
                }
                let actual = check_expr(
                    program,
                    current,
                    &field.value,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let (Some(declaration), Some(declared), Some(actual)) =
                    (declaration, declared, actual)
                {
                    reject_native_unit_value(program, &field.value, &actual, diagnostics);
                    let expected = TypeTable::substitute_variant_type(
                        declaration,
                        type_arguments,
                        &declared.ty,
                    )
                        .unwrap_or_else(|| declared.ty.clone());
                    if actual.ty != expected {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "payload `{}::{}.{}` expects {}, received {}",
                                type_name, case_name, field.name, expected, actual.ty
                            ),
                            field.value.span,
                        ));
                    }
                    if types.needs_drop(&expected) && actual.mode == ParamMode::Own {
                        if allow_moves {
                            mark_value_sources_moved(
                                program,
                                &field.value,
                                variables,
                                types,
                                diagnostics,
                            );
                        } else {
                            diagnostics.push(error(
                                program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned variant payload",
                                field.value.span,
                            ));
                        }
                    } else if types.needs_drop(&expected)
                        && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O108",
                            "cannot move an owned variant payload through a borrowed or shared value",
                            field.value.span,
                        ));
                    }
                }
            }
            if let Some(case) = case {
                for field in &case.fields {
                    if !supplied.contains(field.name.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-T213",
                            format!(
                                "variant construction `{type_name}::{case_name}` is missing payload field `{}`",
                                field.name
                            ),
                            expr.span,
                        ));
                    }
                }
            }
            case.map(|_| CheckedValue::returned(instance.clone(), types.needs_drop(&instance)))
        }
        ExprKind::Match { mode, scrutinee, arms } => oracle_match(
            mode,
            scrutinee,
            arms,
            program,
            current,
            expr,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        ),
        ExprKind::Try { operand } => {
            let operand_value = check_expr(
                program,
                current,
                operand,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let operand_value = operand_value?;
            reject_native_unit_value(program, operand, &operand_value, diagnostics);
            if !allow_moves {
                diagnostics.push(error(
                    program,
                    "SPX-T218",
                    "`?` is only valid in an executable function body",
                    expr.span,
                ));
            }
            if variables
                .values()
                .any(|binding| types.needs_drop(&binding.ty))
            {
                diagnostics.push(error(
                    program,
                    "SPX-T218",
                    "`?` with a live resource binding is not supported yet",
                    expr.span,
                ));
            }
            if let Some((ok, error_ty)) = ordinary_result_arguments(&operand_value.ty) {
                let Some((_, residual_error_ty)) =
                    ordinary_result_arguments(&current.return_type)
                else {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        format!(
                            "function `{}` must return the ordinary compiler-owned Result to propagate a Result with `?`",
                            current.name
                        ),
                        expr.span,
                    ));
                    return Some(CheckedValue::value(ok.clone()));
                };
                if error_ty != residual_error_ty {
                    diagnostics.push(error(
                        program,
                        "SPX-T219",
                        format!(
                            "`?` cannot propagate error type {error_ty} into Result error type {residual_error_ty}"
                        ),
                        expr.span,
                    ));
                }
                if !matches!(ok, Type::I64 | Type::Bool)
                    || !matches!(error_ty, Type::I64 | Type::Bool)
                    || !matches!(residual_error_ty, Type::I64 | Type::Bool)
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        "Result `?` accepts only direct `i64` or `bool` success and error payloads",
                        expr.span,
                    ));
                }
                return Some(CheckedValue::value(ok.clone()));
            }
            if let Some(some) = ordinary_option_argument(&operand_value.ty) {
                let outer = ordinary_option_argument(&current.return_type);
                if outer.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        format!(
                            "function `{}` must return the ordinary compiler-owned Option to propagate an Option with `?`",
                            current.name
                        ),
                        expr.span,
                    ));
                } else if !matches!(some, Type::I64 | Type::Bool)
                    || outer.is_some_and(|value| !matches!(value, Type::I64 | Type::Bool))
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T218",
                        "Option `?` accepts only direct `i64` or `bool` source and enclosing payloads",
                        expr.span,
                    ));
                }
                return Some(CheckedValue::value(some.clone()));
            }
            diagnostics.push(error(
                program,
                "SPX-T218",
                format!(
                    "`?` operand must be an ordinary compiler-owned Result or Option, received {}",
                    operand_value.ty
                ),
                expr.span,
            ));
            None
        }
        ExprKind::UpdateRecord { base, fields } => {
            let base_value = check_expr(
                program,
                current,
                base,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            reject_native_unit_value(program, base, &base_value, diagnostics);
            let declared_fields = effective_record_fields(types, &base_value.ty);
            if declared_fields.is_none() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!(
                        "record update requires a record base, received {}",
                        base_value.ty
                    ),
                    base.span,
                ));
                return None;
            }

            let nested_update = types.is_nested_owned_byte_record(&base_value.ty)
                && !types.is_flat_owned_byte_record(&base_value.ty);
            if nested_update
                && source_place(base, variables, types)
                    .is_none_or(|place| !place.projections.is_empty())
            {
                diagnostics.push(error(
                    program,
                    "SPX-O117",
                    "nested owned-record update requires an exact named owned base place",
                    expr.span,
                ));
            }

            if types.needs_drop(&base_value.ty) {
                match base_value.mode {
                    ParamMode::Own if allow_moves => {
                        mark_value_sources_moved(
                            program,
                            base,
                            variables,
                            types,
                            diagnostics,
                        );
                    }
                    ParamMode::Own => diagnostics.push(error(
                        program,
                        "SPX-O105",
                        "contract expression cannot transfer an owned record update base",
                        base.span,
                    )),
                    ParamMode::Borrow | ParamMode::Shared => diagnostics.push(error(
                        program,
                        "SPX-O108",
                        "cannot update an owned record through a borrowed or shared base",
                        base.span,
                    )),
                    ParamMode::Value => {}
                }
            }

            let mut supplied = HashSet::new();
            for field in fields {
                let declared = declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                });
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-T212",
                        format!(
                            "unknown or duplicate field `{}` in `{}` update",
                            field.name, base_value.ty
                        ),
                        field.span,
                    ));
                }
                let actual = check_expr(
                    program,
                    current,
                    &field.value,
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let (Some(declared), Some(actual)) = (declared, actual) {
                    reject_native_unit_value(program, &field.value, &actual, diagnostics);
                    let expected = types
                        .record_field_type(&base_value.ty, declared)
                        .unwrap_or_else(|| declared.ty.clone());
                    if actual.ty != expected {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "field `{}.{}` expects {}, received {}",
                                base_value.ty, field.name, expected, actual.ty
                            ),
                            field.value.span,
                        ));
                    }
                    if types.needs_drop(&expected) && actual.mode == ParamMode::Own {
                        if allow_moves {
                            mark_value_sources_moved(
                                program,
                                &field.value,
                                variables,
                                types,
                                diagnostics,
                            );
                        } else {
                            diagnostics.push(error(
                                program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned record replacement",
                                field.value.span,
                            ));
                        }
                    } else if types.needs_drop(&expected)
                        && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O108",
                            "cannot move an owned replacement through a borrowed or shared value",
                            field.value.span,
                        ));
                    }
                }
            }

            Some(CheckedValue::returned(
                base_value.ty.clone(),
                types.needs_drop(&base_value.ty),
            ))
        }
        ExprKind::Project { base, field, .. } => {
            if let Some(place) = source_place(expr, variables, types) {
                check_source_place_availability(
                    program,
                    &place,
                    variables,
                    expr.span,
                    diagnostics,
                );
                return Some(CheckedValue {
                    ty: place.ty,
                    mode: place.mode,
                    native_unit: false,
                });
            }
            let base_value = check_expr(
                program,
                current,
                base,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )?;
            reject_native_unit_value(program, base, &base_value, diagnostics);
            let Some(fields) = effective_record_fields(types, &base_value.ty) else {
                diagnostics.push(error(
                    program,
                    "SPX-T214",
                    format!("cannot project field `{field}` from `{}`", base_value.ty),
                    expr.span,
                ));
                return None;
            };
            let Some(declared) = fields.iter().find(|candidate| candidate.name == *field) else {
                diagnostics.push(error(
                    program,
                    "SPX-T214",
                    format!("record `{}` has no field `{field}`", base_value.ty),
                    expr.span,
                ));
                return None;
            };
            let projected = types
                .record_field_type(&base_value.ty, declared)
                .unwrap_or_else(|| declared.ty.clone());
            let mode = if types.needs_drop(&projected) {
                base_value.mode
            } else {
                ParamMode::Value
            };
            Some(CheckedValue {
                ty: projected,
                mode,
                native_unit: false,
            })
        }
        ExprKind::Block { statements, tail } => {
            let outer_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut scope = variables.clone();
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    Statement::Let {
                        name,
                        name_span,
                        value,
                        ..
                    } => {                        if !source_identifier(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-S109",
                                format!("`{name}` is reserved and cannot name a local binding"),
                                *name_span,
                            ));
                        }
                        let actual = check_expr(
                            program,
                            current,
                            value,
                            &mut scope,
                            functions,
                            types,
                            result_type,
                            allow_moves,
                            diagnostics,
                        );
                        if scope.contains_key(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-T209",
                                format!("local binding `{name}` shadows an existing value"),
                                *name_span,
                            ));
                            continue;
                        }
                        if let Some(actual) = actual {
                            if types.needs_drop(&actual.ty)
                                && actual.mode == ParamMode::Own
                            {
                                if allow_moves {
                                    mark_value_sources_moved(
                                        program,
                                        value,
                                        &mut scope,
                                        types,
                                        diagnostics,
                                    );
                                } else {
                                    diagnostics.push(error(
                                        program,
                                        "SPX-O105",
                                        "contract expression cannot transfer an owned resource into a local binding",
                                        value.span,
                                    ));
                                }
                            }
                            let borrow_origin = matches!(actual.ty, Type::SliceU8 | Type::Str)
                                .then(|| {
                                    local_borrow_origin(
                                        value,
                                        name,
                                        *name_span,
                                        &scope,
                                        types,
                                    )
                                })
                                .flatten();
                            if let Some(origin) = &borrow_origin {
                                activate_local_loan(&mut scope, origin);
                            }
                            scope.insert(
                                name.clone(),
                                Binding {
                                    ty: actual.ty,
                                    mode: actual.mode,
                                    availability: Availability::Available,
                                    moved_places: HashMap::new(),
                                    definitely_partial: HashSet::new(),
                                    native_unit_discard: actual.native_unit,
                                    mutable: false,
                                    active_loans: BTreeSet::new(),
                                    borrow_origin,
                                },
                            );
                        }
                    }
                    Statement::Assign { value, span, .. } => {
                        diagnostics.push(error(
                            program,
                            "SPX-U106",
                            "assignment statements are not allowed in contract expressions",
                            *span,
                        ));
                        check_expr(
                            program,
                            current,
                            value,
                            &mut scope,
                            functions,
                            types,
                            result_type,
                            allow_moves,
                            diagnostics,
                        );
                    }
                    Statement::Unsafe { body, span, .. } => {
                        // Contract expressions stay pure: unsafe boundary
                        // statements are meaningless inside them.
                        diagnostics.push(error(
                            program,
                            "SPX-N105",
                            "unsafe boundary statements are not allowed in contract expressions",
                            *span,
                        ));
                        check_expr(
                            program,
                            current,
                            body,
                            &mut scope,
                            functions,
                            types,
                            result_type,
                            allow_moves,
                            diagnostics,
                        );
                    }
                    Statement::While { condition, body, .. } => {
                        // Contract expressions stay pure: while statements
                        // never execute inside them. The condition and body
                        // are still checked so their own diagnostics surface.
                        check_while_statement(
                            program,
                            current,
                            condition,
                            body,
                            condition.span.merge(body.span),
                            &mut scope,
                            functions,
                            types,
                            result_type,
                            allow_moves,
                            diagnostics,
                        );
                    }
                }
                release_dead_local_loans(
                    &mut scope,
                    statements.get(index + 1..).unwrap_or_default(),
                    tail,
                );
            }
            let actual = check_expr(
                program,
                current,
                tail,
                &mut scope,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            merge_moved(variables, &scope, &outer_names);
            actual
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
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
                        "SPX-T210",
                        "`if` condition must be bool",
                        condition.span,
                    ));
                }
            }
            let original_names = variables.keys().cloned().collect::<Vec<_>>();
            let mut then_variables = variables.clone();
            let mut else_variables = variables.clone();
            let then_value = check_expr(
                program,
                current,
                then_branch,
                &mut then_variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let else_value = check_expr(
                program,
                current,
                else_branch,
                &mut else_variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            for name in &original_names {
                if let Some(binding) = variables.get_mut(name) {
                    let then_state = then_variables
                        .get(name)
                        .map_or(Availability::Available, |value| value.availability);
                    let else_state = else_variables
                        .get(name)
                        .map_or(Availability::Available, |value| value.availability);
                    binding.availability = then_state.join(else_state);
                    if let (Some(then_binding), Some(else_binding)) =
                        (then_variables.get(name), else_variables.get(name))
                    {
                        binding.active_loans = then_binding
                            .active_loans
                            .union(&else_binding.active_loans)
                            .cloned()
                            .collect();
                        binding.moved_places = join_moved_places(then_binding, else_binding);
                        binding.definitely_partial =
                            join_definitely_partial(then_binding, else_binding);
                    }
                }
            }
            match (then_value, else_value) {
                (Some(then_value), Some(else_value)) => {
                    if then_value.native_unit || else_value.native_unit {
                        reject_native_unit_value(
                            program,
                            then_branch,
                            &then_value,
                            diagnostics,
                        );
                        reject_native_unit_value(
                            program,
                            else_branch,
                            &else_value,
                            diagnostics,
                        );
                    } else if then_value.ty != else_value.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T211",
                            format!(
                                "`if` branches return different types: {} and {}",
                                then_value.ty, else_value.ty
                            ),
                            expr.span,
                        ));
                    }
                    if types.needs_drop(&then_value.ty) && then_value.mode != else_value.mode
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-O106",
                            "`if` branches must produce the same resource ownership mode",
                            expr.span,
                        ));
                    }
                    Some(then_value)
                }
                _ => None,
            }
        }
    }
}
