//! Test-only recursive checking of call and method-call expressions.

use crate::ast::{Expr, Function, ImportResult, ParamMode, Program, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::arguments::{
    activate_borrowed_bytes_call_loans, check_argument_ownership, release_borrowed_bytes_call_loans,
};
use crate::source_verify::binding::{Binding, CheckedValue};
use crate::source_verify::declared_type::{
    direct_function_type_argument, validation_specialize_function,
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
    let native_import = program
        .interfaces
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
                if !actual.native_unit
                    && (actual.ty != parameter.ty || actual.mode != ParamMode::Value)
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
        let mut checked = CheckedValue::value(match import.result {
            ImportResult::Unit => Type::Named {
                name: "\0native-rust-unit".to_owned(),
                arguments: Vec::new(),
            },
            ImportResult::I64 => Type::I64,
            ImportResult::Bool => Type::Bool,
        });
        checked.native_unit = native_unit;
        return Some(checked);
    }
    let target = functions.get(name.as_str()).copied();
    if target.is_none() {
        diagnostics.push(hints::unknown_function(program, name, functions, expr.span));
    }
    if target.is_some_and(|target| args.len() != target.params.len()) {
        let target = target.expect("checked above");
        diagnostics.push(error(
            program,
            "SPX-T204",
            format!(
                "`{name}` expects {} arguments, received {}",
                target.params.len(),
                args.len()
            ),
            expr.span,
        ));
    }
    let specialized_target = target.and_then(|target| {
        if target.type_parameters.is_empty() {
            if !type_arguments.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T225",
                    format!("monomorphic function `{name}` does not accept type arguments"),
                    expr.span,
                ));
                return None;
            }
            return Some(target.clone());
        }
        if !current.type_parameters.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T226",
                format!(
                    "generic function `{}` cannot call generic function `{name}` in this slice",
                    current.name
                ),
                expr.span,
            ));
        }
        if type_arguments.len() != target.type_parameters.len() {
            diagnostics.push(
                error(
                    program,
                    "SPX-T225",
                    format!(
                        "generic function `{name}` expects {} explicit type arguments, received {}",
                        target.type_parameters.len(),
                        type_arguments.len()
                    ),
                    expr.span,
                )
                .with_help(hints::generic_call_help(name)),
            );
            return None;
        }
        if type_arguments
            .iter()
            .any(|argument| !direct_function_type_argument(argument))
        {
            diagnostics.push(error(
                program,
                "SPX-T225",
                format!(
                    "generic function `{name}` accepts only direct `i64` or `bool` type arguments"
                ),
                expr.span,
            ));
            return None;
        }
        validation_specialize_function(target, type_arguments)
    });
    let borrowed_bytes_loans = target
        .filter(|target| target.type_parameters.is_empty())
        .map_or_else(Vec::new, |target| {
            activate_borrowed_bytes_call_loans(args, &target.params, variables, types)
        });
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
        let Some(param) = specialized_target
            .as_ref()
            .and_then(|target| target.params.get(index))
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
            target.is_some_and(|target| target.type_parameters.is_empty()),
            diagnostics,
        );
    }
    release_borrowed_bytes_call_loans(variables, &borrowed_bytes_loans);
    specialized_target.map(|target| {
        CheckedValue::returned(
            target.return_type.clone(),
            types.needs_drop(&target.return_type),
        )
    })
}

#[allow(clippy::too_many_arguments, clippy::borrowed_box, clippy::ptr_arg)]
pub(super) fn oracle_method_call(
    receiver: &Box<Expr>,
    method: &String,
    type_arguments: &Vec<Type>,
    args: &Vec<Expr>,
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
        diagnostics.push(error(
            program,
            "SPX-T203",
            format!(
                "method `{method}` requires a class receiver, found `{}`",
                receiver_value.ty
            ),
            receiver.span,
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
        diagnostics.push(error(
            program,
            "SPX-T203",
            format!("unknown method `{method}` on `{class_name}`"),
            expr.span,
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
