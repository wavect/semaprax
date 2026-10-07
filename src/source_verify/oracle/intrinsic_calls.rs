//! Recursive reference checking for exact String and borrowed-text intrinsics.
use super::*;
use crate::ast::Param;
use crate::source_verify::arguments::{
    activate_borrowed_bytes_call_loans, check_argument_ownership, release_borrowed_bytes_call_loans,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn check_call(
    name: &str,
    type_arguments: &[Type],
    args: &[Expr],
    program: &Program,
    current: &Function,
    expression: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    let (params, result): (Vec<Param>, Type) = if let Some(op) = crate::string_ops::by_name(name) {
        (crate::string_ops::ast_params(op), op.ast_return_type())
    } else {
        let op = crate::str_ops::by_name(name)?;
        (crate::str_ops::ast_params(op), op.ast_return_type())
    };
    if !type_arguments.is_empty() {
        diagnostics.push(error(
            program,
            "SPX-T225",
            format!("monomorphic function `{name}` does not accept type arguments"),
            expression.span,
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
            expression.span,
        ));
    }
    let loans = activate_borrowed_bytes_call_loans(args, &params, variables, types);
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
        let Some(parameter) = params.get(index) else {
            continue;
        };
        if let Some(actual) = &actual {
            reject_native_unit_value(program, argument, actual, diagnostics);
            if !actual.native_unit && actual.ty != parameter.ty {
                diagnostics.push(hints::with_optional_help(
                    error(
                        program,
                        "SPX-T205",
                        format!(
                            "argument `{}` to `{name}` expects {}, received {}",
                            parameter.name, parameter.ty, actual.ty
                        ),
                        argument.span,
                    ),
                    hints::argument_view_help(name, &parameter.ty, &actual.ty),
                ));
            }
        }
        check_argument_ownership(
            program,
            current,
            name,
            argument,
            parameter,
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
    let owns = types.needs_drop(&result);
    Some(CheckedValue::returned(result, owns))
}
