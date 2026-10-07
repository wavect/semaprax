//! Recursive oracle for the closed streaming operation signatures.
use super::*;
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
    let (params, result, own) = if let Some(op) = crate::stdin_stream_ops::pure_by_name(name) {
        (op.params(), op.ast_result(), false)
    } else {
        let op = crate::stdin_stream_ops::host_by_name(name)?;
        (
            crate::stdin_stream_ops::host_params(op),
            crate::stdin_stream_ops::ast_reader(),
            true,
        )
    };
    if !type_arguments.is_empty() || args.len() != params.len() {
        diagnostics.push(error(
            program,
            "SPX-T270",
            "invalid streaming stdin call shape",
            expression.span,
        ));
    }
    let loans = crate::source_verify::arguments::activate_borrowed_bytes_call_loans(
        args, &params, variables, types,
    );
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
        if let Some(value) = actual.as_ref().filter(|value| value.ty != param.ty) {
            diagnostics.push(error(
                program,
                "SPX-T205",
                format!(
                    "argument `{}` to `{name}` expects {}, received {}",
                    param.name, param.ty, value.ty
                ),
                arg.span,
            ));
        }
        crate::source_verify::arguments::check_argument_ownership(
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
    crate::source_verify::arguments::release_borrowed_bytes_call_loans(variables, &loans);
    Some(CheckedValue::returned(result, own))
}
