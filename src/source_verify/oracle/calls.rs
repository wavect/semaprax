//! Test-only recursive checking of call and method-call expressions.

use crate::ast::program_source::ProgramSource;
use crate::ast::{Expr, Function, ImportResult, Param, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::arguments::{
    activate_borrowed_bytes_call_loans, check_argument_ownership, release_borrowed_bytes_call_loans,
};
use crate::source_verify::binding::{Binding, CheckedValue};
use crate::source_verify::declared_type::{
    generic_function_arguments_are_admitted, generic_function_arguments_are_forwarded,
    validation_specialize_function,
};
use crate::source_verify::diagnostics::{error, reject_native_unit_value};
use crate::source_verify::hints;
use crate::source_verify::oracle::check_expr;
use crate::source_verify::type_table::{resolve_class_method, TypeTable};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments, clippy::ptr_arg)]
pub(super) fn oracle_call(
    name: &String,
    type_arguments: &Vec<Type>,
    args: &Vec<Expr>,
    program: &dyn ProgramSource,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &crate::source_verify::function_map::FunctionMap<'_>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    if let Some(result) = crate::source_verify::owning_closure::check_call(
        program,
        name,
        type_arguments,
        args,
        expr.span,
        allow_moves,
        variables,
        types,
        diagnostics,
    ) {
        return result;
    }
    // Independently check the readonly byte vocabulary used by normalization.
    if let Some(op) = crate::byte_ops::by_name(name).filter(|op| {
        matches!(
            op,
            crate::byte_ops::ByteOp::Len
                | crate::byte_ops::ByteOp::Range
                | crate::byte_ops::ByteOp::BytesAsSlice
        )
    }) {
        if !type_arguments.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T263",
                format!("byte operation `{name}` does not accept type arguments"),
                expr.span,
            ));
        }
        if args.len() != op.arity() {
            diagnostics.push(error(
                program,
                "SPX-T263",
                format!(
                    "byte operation `{name}` expects {} arguments, received {}",
                    op.arity(),
                    args.len()
                ),
                expr.span,
            ));
        }
        if (op.is_view() || op == crate::byte_ops::ByteOp::Range)
            && args.first().is_none_or(|argument| {
                !crate::source_verify::arguments::source_byte_view_place_is_admitted(
                    op, argument, variables, types,
                )
            })
        {
            let diagnostic = error(
                program,
                "SPX-T266",
                format!("borrowed view `{name}` requires an exact admitted storage place"),
                expr.span,
            );
            diagnostics.push(match args.first() {
                Some(argument) => diagnostic.with_help(hints::view_place_help(name, argument)),
                None => diagnostic,
            });
        }
        for (index, argument) in args.iter().enumerate() {
            if let Some(actual) = check_expr(
                program,
                current,
                argument,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            ) {
                reject_native_unit_value(program, argument, &actual, diagnostics);
                if !actual.native_unit && !op.accepts_ast(index, &actual.ty) {
                    diagnostics.push(hints::with_optional_help(
                        error(
                            program,
                            "SPX-T263",
                            format!("byte operation `{name}` argument {index} has the wrong type"),
                            argument.span,
                        ),
                        hints::view_argument_help(name, &actual.ty),
                    ));
                }
            }
        }
        return Some(CheckedValue::returned(op.ast_return_type(), false));
    }
    if crate::stdin_stream_ops::pure_by_name(name).is_some()
        || crate::stdin_stream_ops::host_by_name(name).is_some()
    {
        return super::stdin_stream::check_call(
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
        );
    }
    if let Some(binding_type) = variables.get(name.as_str()).map(|binding| {
        crate::source_verify::mutable_closure::invocation_signature(
            program,
            binding,
            name,
            args,
            allow_moves,
            expr.span,
            diagnostics,
        )
    }) {
        let Type::Function { parameters, result } = binding_type else {
            diagnostics.push(error(
                program,
                "SPX-T287",
                format!("`{name}` is a bound value, not a function value"),
                expr.span,
            ));
            return None;
        };
        if !type_arguments.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T225",
                "function values do not accept type arguments",
                expr.span,
            ));
        }
        if args.len() != parameters.len() {
            diagnostics.push(error(
                program,
                "SPX-T204",
                format!(
                    "`{name}` expects {} arguments, received {}",
                    parameters.len(),
                    args.len()
                ),
                expr.span,
            ));
        }
        let params = parameters
            .into_iter()
            .enumerate()
            .map(|(index, ty)| Param {
                name: format!("argument{index}"),
                mode: ParamMode::Value,
                ty,
                span: expr.span,
            })
            .collect::<Vec<_>>();
        for (index, arg) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(actual) = actual
                .as_ref()
                .filter(|actual| !actual.native_unit && actual.ty != param.ty)
            {
                diagnostics.push(hints::with_optional_help(
                    error(
                        program,
                        "SPX-T205",
                        format!(
                            "argument `{}` to `{name}` expects {}, received {}",
                            param.name, param.ty, actual.ty
                        ),
                        arg.span,
                    ),
                    hints::argument_view_help(name, &param.ty, &actual.ty),
                ));
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        return Some(CheckedValue::returned(*result, false));
    }
    let args = if name == crate::vec_ops::sort_keys::NAME {
        if !current.type_parameters.is_empty()
            || crate::source_verify::sort_keys::descriptor(types, type_arguments, args).is_none()
        {
            diagnostics.push(error(program,"SPX-T311","vec_sort_by requires an explicit monomorphic record and 1..8 unique literal field:asc/field:desc keys",expr.span));
            return None;
        }
        &args[..1]
    } else if name == crate::vec_ops::field_unique::NAME {
        if !current.type_parameters.is_empty()
            || crate::source_verify::field_unique::descriptor(
                types,
                type_arguments,
                args,
                variables,
            )
            .is_none()
        {
            diagnostics.push(error(
                program,
                "SPX-T319",
                crate::vec_ops::field_unique::SHAPE,
                expr.span,
            ));
            return None;
        }
        &args[..1]
    } else {
        args
    };
    if let Some(op) = crate::vec_ops::by_name(name) {
        if op == crate::vec_ops::VecOp::StrAt
            && (!current.type_parameters.is_empty()
                || crate::source_verify::string_slot::root(type_arguments, args, variables)
                    .is_none())
        {
            diagnostics.push(error(
                program,
                "SPX-T281",
                crate::vec_ops::string_slot::SHAPE,
                expr.span,
            ));
        }
        let element = if op == crate::vec_ops::VecOp::StrAt {
            Some(&Type::String)
        } else {
            type_arguments.first()
        };
        if (if op == crate::vec_ops::VecOp::StrAt {
            !type_arguments.is_empty()
        } else {
            type_arguments.len() != 1
        }) || element.is_none_or(|ty| {
            !crate::source_verify::declared_type::owned_leaf_collection::vec_operation_admitted(
                types, op, ty,
            ) && !crate::vec_ops::source_parameter_is_admitted(program, current, op, ty)
        }) {
            diagnostics.push(error(
                program,
                "SPX-T281",
                format!("vector operation `{name}` requires one explicit admitted type argument"),
                expr.span,
            ));
        }
        if args.len() != op.arity() {
            diagnostics.push(error(
                program,
                "SPX-T281",
                format!(
                    "vector operation `{name}` expects {} arguments, received {}",
                    op.arity(),
                    args.len()
                ),
                expr.span,
            ));
        }
        if op.capacity_argument().is_some_and(|index| {
            matches!(args.get(index).map(|arg| &arg.kind), Some(crate::ast::ExprKind::Usize(value)) if *value > crate::vec_ops::MAX_CAPACITY)
        })
        {
            diagnostics.push(error(
                program,
                "SPX-T282",
                format!(
                    "`{name}` literal capacity must be no greater than {}",
                    crate::vec_ops::MAX_CAPACITY
                ),
                expr.span,
            ));
        }
        let params = element
            .map(|element| {
                crate::vec_ops::ast_params_with_owned_element(
                    op,
                    element,
                    *element == crate::ast::Type::Bytes
                        || crate::source_verify::declared_type::owned_leaf_collection::admitted(types, element)
                        || crate::source_verify::declared_type::owned_leaf_collection::runtime_element(types, element),
                )
            })
            .unwrap_or_default();
        for (index, arg) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(actual) = actual.as_ref().filter(|actual| actual.ty != param.ty) {
                diagnostics.push(error(
                    program,
                    "SPX-T205",
                    format!(
                        "argument `{}` to `{name}` expects {}, received {}",
                        param.name, param.ty, actual.ty
                    ),
                    arg.span,
                ));
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        return element.map(|element| {
            if op == crate::vec_ops::VecOp::StrAt {
                CheckedValue {
                    ty: Type::Str,
                    mode: ParamMode::Borrow,
                    native_unit: false,
                }
            } else {
                CheckedValue::returned(op.ast_return_type(element), op.returns_owner())
            }
        });
    }
    if let Some(op) = crate::list_ops::by_name(name) {
        if !type_arguments.is_empty() || args.len() != op.argument_count() {
            diagnostics.push(error(
                program,
                "SPX-T291",
                format!(
                    "immutable list operation `{name}` requires {} arguments and no type arguments",
                    op.argument_count()
                ),
                expr.span,
            ));
        }
        let params = op.ast_params();
        for (index, arg) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(actual) = actual.as_ref().filter(|actual| actual.ty != param.ty) {
                diagnostics.push(error(
                    program,
                    "SPX-T205",
                    format!(
                        "argument `{}` to `{name}` expects {}, received {}",
                        param.name, param.ty, actual.ty
                    ),
                    arg.span,
                ));
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        return Some(CheckedValue::returned(op.ast_return_type(), false));
    }
    if let Some(op) = crate::iterator_ops::by_name(name) {
        let element = type_arguments.first();
        if type_arguments.len() != 1
            || element.is_none_or(|ty| {
                !crate::iterator_ops::ast_element_is_admitted(ty)
                    && !crate::source_verify::declared_type::owned_leaf_collection::runtime_element(
                        types, ty,
                    )
            })
        {
            diagnostics.push(error(
                program,
                "SPX-T290",
                format!("iterator operation `{name}` requires one explicit admitted type argument"),
                expr.span,
            ));
        }
        if args.len() != 1 {
            diagnostics.push(error(
                program,
                "SPX-T290",
                format!(
                    "iterator operation `{name}` expects 1 argument, received {}",
                    args.len()
                ),
                expr.span,
            ));
        }
        let params = element
            .map(|element| crate::iterator_ops::ast_params(op, element))
            .unwrap_or_default();
        for (index, arg) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(actual) = actual.as_ref().filter(|actual| actual.ty != param.ty) {
                diagnostics.push(error(
                    program,
                    "SPX-T205",
                    format!(
                        "argument `{}` to `{name}` expects {}, received {}",
                        param.name, param.ty, actual.ty
                    ),
                    arg.span,
                ));
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        return element.map(|element| CheckedValue::returned(op.ast_return_type(element), true));
    }
    if let Some(op) = crate::box_ops::by_name(name) {
        let element = type_arguments.first();
        if type_arguments.len() != 1
            || element.is_none_or(|ty| {
                !crate::box_ops::ast_operation_element_is_admitted(op, ty)
                    && !crate::box_ops::source_parameter_is_admitted(program, current, op, ty)
            })
        {
            diagnostics.push(error(
                program,
                "SPX-T285",
                format!("box operation `{name}` requires one explicit admitted type argument"),
                expr.span,
            ));
        }
        if args.len() != 1 {
            diagnostics.push(error(
                program,
                "SPX-T285",
                format!(
                    "box operation `{name}` expects 1 argument, received {}",
                    args.len()
                ),
                expr.span,
            ));
        }
        let params = element
            .map(|element| crate::box_ops::ast_params(op, element))
            .unwrap_or_default();
        for (index, arg) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(actual) = actual.as_ref().filter(|actual| actual.ty != param.ty) {
                diagnostics.push(error(
                    program,
                    "SPX-T205",
                    format!(
                        "argument `{}` to `{name}` expects {}, received {}",
                        param.name, param.ty, actual.ty
                    ),
                    arg.span,
                ));
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        return element.map(|element| {
            CheckedValue::returned(
                op.ast_return_type(element),
                op == crate::box_ops::BoxOp::New || *element == Type::Bytes,
            )
        });
    }
    if name == crate::vec_field::NAME {
        let Some((params, ty)) =
            crate::source_verify::vec_field::signature(types, type_arguments, args, expr.span)
        else {
            diagnostics.push(error(program,"SPX-T310","vec_field requires one admitted owned record type, a named Vec and usize index, and an exact literal field selector",expr.span));
            return None;
        };
        if !current.type_parameters.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T310",
                "vec_field is not admitted in generic function templates",
                expr.span,
            ));
        }
        let loans = activate_borrowed_bytes_call_loans(program, args, &params, variables, types);
        for (arg, param) in args.iter().zip(&params) {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            if let Some(actual) = &actual {
                reject_native_unit_value(program, arg, actual, diagnostics);
                if actual.ty != param.ty {
                    diagnostics.push(error(
                        program,
                        "SPX-T205",
                        "vec_field argument type differs",
                        arg.span,
                    ));
                }
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        release_borrowed_bytes_call_loans(variables, &loans);
        let mode = if matches!(ty, Type::Str | Type::SliceU8) {
            ParamMode::Borrow
        } else {
            ParamMode::Value
        };
        return Some(CheckedValue {
            ty,
            mode,
            native_unit: false,
        });
    }
    if name == crate::literal_format::NAME {
        if !type_arguments.is_empty() || !current.type_parameters.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T225",
                "string_format is admitted only in monomorphic functions without type arguments",
                expr.span,
            ));
        }
        let Some(Expr {
            kind: crate::ast::ExprKind::String(template),
            ..
        }) = args.first()
        else {
            diagnostics.push(error(
                program,
                "SPX-T204",
                "string_format requires a compile-time string literal as its first argument",
                expr.span,
            ));
            return None;
        };
        match crate::literal_format::scan(template) {
            Ok(pieces) if crate::literal_format::field_count(&pieces) != args.len() - 1 => {
                diagnostics.push(error(
                    program,
                    "SPX-T204",
                    format!(
                        "string_format literal has {} fields, received {} values",
                        crate::literal_format::field_count(&pieces),
                        args.len() - 1
                    ),
                    expr.span,
                ))
            }
            Err(reason) => {
                diagnostics.push(error(program, "SPX-T204", reason.message(), args[0].span))
            }
            _ => {}
        }
        for (index, arg) in args.iter().enumerate().skip(1) {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            if let Some(actual) = actual {
                reject_native_unit_value(program, arg, &actual, diagnostics);
                if !actual.native_unit && !crate::literal_format::accepts_ast_type(&actual.ty) {
                    diagnostics.push(error(
                        program,
                        "SPX-T205",
                        format!(
                            "string_format value {} must be i64, u8, usize, bool, or own string",
                            index - 1
                        ),
                        arg.span,
                    ));
                }
                let parameter = Param {
                    name: format!("value{}", index - 1),
                    mode: if actual.ty == Type::String {
                        ParamMode::Own
                    } else {
                        ParamMode::Value
                    },
                    ty: actual.ty.clone(),
                    span: arg.span,
                };
                check_argument_ownership(
                    program,
                    current,
                    name,
                    arg,
                    &parameter,
                    Some(&actual),
                    variables,
                    types,
                    allow_moves,
                    false,
                    false,
                    diagnostics,
                );
            }
        }
        return Some(CheckedValue::returned(Type::String, true));
    }
    if crate::map_ops::by_generic_name(name, type_arguments).is_some()
        || crate::string_ops::by_name(name).is_some()
        || crate::str_ops::by_name(name).is_some()
    {
        return super::intrinsic_calls::check_call(
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
        );
    }
    let native_import = program
        .interfaces()
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| import.native_rust && import.name == *name);
    if let Some(import) = native_import {
        if !type_arguments.is_empty() || args.len() != import.params.len() {
            diagnostics.push(error(
                program,
                "SPX-B107",
                "Native Rust Interop declaration set is unsupported: scalar value signature required",
                expr.span,
            ));
        }
        for effect in &import.effects {
            if !current.effects.contains(effect) {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                    expr.span,
                ));
            }
        }
        for (index, argument) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                argument,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            if let (Some(actual), Some(parameter)) = (actual, import.params.get(index)) {
                reject_native_unit_value(program, argument, &actual, diagnostics);
                check_argument_ownership(
                    program,
                    current,
                    name,
                    argument,
                    parameter,
                    Some(&actual),
                    variables,
                    types,
                    allow_moves,
                    false,
                    false,
                    diagnostics,
                );
                if !actual.native_unit
                    && (actual.ty != parameter.ty
                        || (actual.mode != parameter.mode
                            && !(parameter.mode == ParamMode::Borrow
                                && actual.mode == ParamMode::Own
                                && matches!(&argument.kind, crate::ast::ExprKind::Var(_)))))
                {
                    diagnostics.push(error(
                        program,
                        "SPX-B107",
                        "Native Rust Interop declaration set is unsupported: scalar value signature required",
                        argument.span,
                    ));
                }
            }
        }
        let native_unit = import.result == ImportResult::Unit;
        let mut checked =
            CheckedValue::returned(import.result.value_type(), import.result.is_owned());
        checked.native_unit = native_unit;
        if import.result.is_borrowed() {
            checked.mode = ParamMode::Borrow;
        }
        return Some(checked);
    }
    // The condition profile needs the reserved non-transferring Len signature
    // in the recursive oracle as well as the iterative verifier.
    if crate::string_ops::by_name(name) == Some(crate::string_ops::StringOp::Len) {
        let params = crate::string_ops::ast_params(crate::string_ops::StringOp::Len);
        if !type_arguments.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T225",
                format!("monomorphic function `{name}` does not accept type arguments"),
                expr.span,
            ));
        }
        if args.len() != params.len() {
            diagnostics.push(error(
                program,
                "SPX-T204",
                format!(
                    "`{name}` expects {} arguments, received {}",
                    params.len(),
                    args.len()
                ),
                expr.span,
            ));
        }
        for (index, arg) in args.iter().enumerate() {
            let actual = check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            let Some(param) = params.get(index) else {
                continue;
            };
            if let Some(actual) = &actual {
                reject_native_unit_value(program, arg, actual, diagnostics);
            }
            if let Some(actual) = actual
                .as_ref()
                .filter(|actual| !actual.native_unit && actual.ty != param.ty)
            {
                diagnostics.push(hints::with_optional_help(
                    error(
                        program,
                        "SPX-T205",
                        format!(
                            "argument `{}` to `{name}` expects {}, received {}",
                            param.name, param.ty, actual.ty
                        ),
                        arg.span,
                    ),
                    hints::argument_view_help(name, &param.ty, &actual.ty),
                ));
            }
            check_argument_ownership(
                program,
                current,
                name,
                arg,
                param,
                actual.as_ref(),
                variables,
                types,
                allow_moves,
                false,
                false,
                diagnostics,
            );
        }
        return Some(CheckedValue::returned(Type::I64, false));
    }
    let target = functions.get(name.as_str()).copied();
    if target.is_none() {
        diagnostics.push(hints::unknown_function(program, name, functions, expr.span));
    }
    if target.is_some_and(|target| args.len() != target.payload().params.len()) {
        let target = target.expect("checked above");
        diagnostics.push(error(
            program,
            "SPX-T204",
            format!(
                "`{name}` expects {} arguments, received {}",
                target.payload().params.len(),
                args.len()
            ),
            expr.span,
        ));
    }
    let specialized_target = target.and_then(|target| {
        if target.payload().type_parameters.is_empty() {
            if !type_arguments.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T225",
                    format!("monomorphic function `{name}` does not accept type arguments"),
                    expr.span,
                ));
                return None;
            }
            let mut complete = target.payload().clone();
            if complete.name != target.name() { complete.name = target.name().to_owned(); }
            return Some(complete);
        }
        let inferred_arguments;
        let type_arguments = if type_arguments.is_empty() {
            inferred_arguments = crate::source_verify::generic_inference::arguments(
                program, current, target.payload(), args, variables, functions, types,
            );
            inferred_arguments.as_deref().unwrap_or(type_arguments)
        } else {
            type_arguments
        };
        if type_arguments.len() != target.payload().type_parameters.len() {
            diagnostics.push(
                error(
                    program,
                    "SPX-T225",
                    format!(
                        "generic function `{name}` expects {} explicit type arguments, received {}",
                        target.payload().type_parameters.len(),
                        type_arguments.len()
                    ),
                    expr.span,
                )
                .with_help(hints::generic_call_help(name)),
            );
            return None;
        }
        if !generic_function_arguments_are_admitted(target.payload(), type_arguments, types)
            && !generic_function_arguments_are_forwarded(current, target.payload(), type_arguments)
            && !crate::vec_ops::source_arguments_are_admitted_ref(program, &target, type_arguments)
            && !crate::box_ops::source_arguments_are_admitted_ref(program, &target, type_arguments)
        {
            diagnostics.push(error(
                program,
                "SPX-T225",
                format!(
                    "generic function `{name}` accepts only direct `i64` or `bool` type arguments, except admitted Copy scalars in an owned-record relay"
                ),
                expr.span,
            ));
            return None;
        }
        validation_specialize_function(target.payload(), type_arguments).map(|mut complete| {
            if complete.name != target.name() { complete.name = target.name().to_owned(); }
            complete
        })
    });
    let borrowed_bytes_loans = specialized_target.as_ref().map_or_else(Vec::new, |target| {
        activate_borrowed_bytes_call_loans(
            program,
            args,
            &target.payload().params,
            variables,
            types,
        )
    });
    for (index, arg) in args.iter().enumerate() {
        // Row Loan v1: the one admitted value use of a traversal row.
        let loan = target.and_then(|target| {
            super::super::record_traversal::row_loan::loan_argument(
                arg,
                target.payload(),
                index,
                variables,
                types,
            )
            .inspect(|_| {
                if !super::super::record_traversal::row_loan::reader_proof(
                    types,
                    target.payload(),
                    index,
                ) {
                    diagnostics.push(error(
                        program,
                        "SPX-T284",
                        super::super::record_traversal::row_loan::HELPER_REFUSAL,
                        arg.span,
                    ));
                }
            })
        });
        let actual = loan.or_else(|| {
            check_expr(
                program,
                current,
                arg,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            )
        });
        let Some(param) = specialized_target
            .as_ref()
            .and_then(|target| target.payload().params.get(index))
        else {
            continue;
        };
        if let Some(actual) = &actual {
            reject_native_unit_value(program, arg, actual, diagnostics);
        }
        if let Some(actual) = actual
            .as_ref()
            .filter(|actual| !actual.native_unit && actual.ty != param.ty)
        {
            diagnostics.push(hints::with_optional_help(
                error(
                    program,
                    "SPX-T205",
                    format!(
                        "argument `{}` to `{name}` expects {}, received {}",
                        param.name, param.ty, actual.ty
                    ),
                    arg.span,
                ),
                hints::argument_view_help(name, &param.ty, &actual.ty),
            ));
        }
        check_argument_ownership(
            program,
            current,
            name,
            arg,
            param,
            actual.as_ref(),
            variables,
            types,
            allow_moves,
            true,
            target.is_some_and(|target| target.payload().type_parameters.is_empty()),
            diagnostics,
        );
    }
    release_borrowed_bytes_call_loans(variables, &borrowed_bytes_loans);
    specialized_target.map(|target| {
        CheckedValue::returned(
            target.payload().return_type.clone(),
            types.needs_drop(&target.payload().return_type),
        )
    })
}

#[allow(clippy::too_many_arguments, clippy::borrowed_box, clippy::ptr_arg)]
pub(super) fn oracle_method_call(
    receiver: &Box<Expr>,
    method: &String,
    type_arguments: &Vec<Type>,
    args: &Vec<Expr>,
    program: &dyn ProgramSource,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &crate::source_verify::function_map::FunctionMap<'_>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    if !type_arguments.is_empty() {
        diagnostics.push(error(
            program,
            "SPX-T225",
            format!("method `{method}` does not accept type arguments in this slice"),
            expr.span,
        ));
    }
    let receiver_value = check_expr(
        program,
        current,
        receiver,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    )?;
    let Type::Named {
        name: class_name,
        arguments: class_arguments,
    } = &receiver_value.ty
    else {
        diagnostics.push(hints::with_optional_help(
            error(
                program,
                "SPX-T203",
                format!(
                    "method `{method}` requires a class receiver, found `{}`",
                    receiver_value.ty
                ),
                receiver.span,
            ),
            hints::method_receiver_help(&receiver_value.ty, method),
        ));
        return None;
    };
    if !class_arguments.is_empty() {
        diagnostics.push(error(
            program,
            "SPX-T203",
            format!(
                "method `{method}` on generic class `{class_name}` is not supported in this slice"
            ),
            expr.span,
        ));
        return None;
    }
    // Class Inheritance v1: nearest-definition ancestor walk; the
    // declaring class owns the expected `self` type.
    let Some((holder_name, method_fn)) = resolve_class_method(types, class_name, method) else {
        diagnostics.push(hints::with_optional_help(
            error(
                program,
                "SPX-T203",
                format!("unknown method `{method}` on `{class_name}`"),
                expr.span,
            ),
            hints::non_class_method_help(types, class_name),
        ));
        return None;
    };
    let _ = holder_name;
    let Some(self_param) = method_fn.params.first() else {
        diagnostics.push(error(
            program,
            "SPX-T205",
            format!("method `{method}` on `{class_name}` has no `self` parameter"),
            method_fn.span,
        ));
        return None;
    };
    if self_param.mode != ParamMode::Value
        || self_param.ty
            != (Type::Named {
                name: holder_name.to_owned(),
                arguments: Vec::new(),
            })
    {
        diagnostics.push(error(
            program,
            "SPX-T205",
            format!(
                "method `{method}` expects a value-mode `self: {class_name}` receiver, found `{}`",
                self_param.ty
            ),
            self_param.span,
        ));
        return None;
    }
    if method_fn.params.len() - 1 != args.len() {
        diagnostics.push(error(
            program,
            "SPX-T204",
            format!(
                "`{}.{}` expects {} arguments, received {}",
                class_name,
                method,
                method_fn.params.len() - 1,
                args.len()
            ),
            expr.span,
        ));
        return None;
    }
    check_argument_ownership(
        program,
        current,
        method_fn.name.as_str(),
        receiver,
        self_param,
        Some(&receiver_value),
        variables,
        types,
        allow_moves,
        true,
        false,
        diagnostics,
    );
    for (index, (argument, param)) in args.iter().zip(method_fn.params.iter().skip(1)).enumerate() {
        let actual = check_expr(
            program,
            current,
            argument,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        );
        if let Some(actual) = &actual {
            reject_native_unit_value(program, argument, actual, diagnostics);
        }
        if actual
            .as_ref()
            .is_some_and(|actual| !actual.native_unit && actual.ty != param.ty)
        {
            diagnostics.push(error(
                program,
                "SPX-T205",
                format!(
                    "argument `{}` to `{method}` expects {}, received {}",
                    param.name,
                    param.ty,
                    actual.as_ref().expect("type checked above").ty
                ),
                argument.span,
            ));
        }
        check_argument_ownership(
            program,
            current,
            method_fn.name.as_str(),
            argument,
            param,
            actual.as_ref(),
            variables,
            types,
            allow_moves,
            true,
            false,
            diagnostics,
        );
        let _ = index;
    }
    Some(CheckedValue::returned(
        method_fn.return_type.clone(),
        types.needs_drop(&method_fn.return_type),
    ))
}
