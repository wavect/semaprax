//! Differential ordinary return checking, distinct from the iterative resume frame.
use super::*;
use crate::ast::program_source::ProgramSource;
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    program: &dyn ProgramSource,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    let ExprKind::LexicalExit { kind, value } = &expr.kind else {
        unreachable!()
    };
    if *kind == crate::ast::LexicalExitKind::Return && value.is_none() {
        diagnostics.push(error(
            program,
            "SPX-T312",
            "return requires a value operand",
            expr.span,
        ));
        return None;
    }
    if let Some(operand) = value {
        let value = check_expr(
            program,
            current,
            operand,
            variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        )?;
        if value.native_unit || value.ty != current.return_type {
            diagnostics.push(error(
                program,
                "SPX-T103",
                "lexical return operand differs from the function result type",
                operand.span,
            ));
        }
        if types.needs_drop(&value.ty) && value.mode != ParamMode::Own {
            diagnostics.push(error(
                program,
                "SPX-O104",
                "lexical return must transfer an owned result",
                operand.span,
            ));
        }
        if !allow_moves {
            diagnostics.push(error(
                program,
                "SPX-T312",
                "return is only admitted in the executable function body",
                expr.span,
            ));
        } else if types.needs_drop(&value.ty) {
            mark_value_sources_moved(program, operand, variables, types, diagnostics);
        }
    }
    Some(CheckedValue::value(Type::Never))
}
