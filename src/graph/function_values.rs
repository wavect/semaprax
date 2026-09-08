//! Additive canonical Function Values v1 facts; frozen graph wires stay closed.
use super::*;
use serde_json::json;
pub(super) fn type_json(parameters: &[ResolvedType], result: &ResolvedType) -> String {
    let parameters = parameters
        .iter()
        .map(super::type_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"kind\":\"function\",\"parameters\":[{parameters}],\"result\":{}}}",
        super::type_json(result)
    )
}
pub(super) fn expression_json(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    header: &str,
) -> Result<String, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::FunctionReference { target } => Ok(format!(
            "{{{header},\"kind\":\"function_reference\",\"target\":{}}}",
            quote_json(target.as_str())
        )),
        ResolvedExprKind::Invoke { callable, args } => {
            let args = args
                .iter()
                .map(|a| super::expr_json(program, a))
                .collect::<Result<Vec<_>, _>>()?
                .join(",");
            let candidates = hir::function_value::compatible_targets(program, &callable.ty)
                .into_iter()
                .map(|f| quote_json(f.id.as_str()))
                .collect::<Vec<_>>()
                .join(",");
            Ok(format!("{{{header},\"kind\":\"invoke\",\"callable\":{},\"args\":[{args}],\"candidate_targets\":[{candidates}]}}",super::expr_json(program,callable)?))
        }
        _ => Err(Diagnostic::io(
            "SPX-G411",
            "expected function value expression",
        )),
    }
}
pub(super) fn append_targets(mut graph: String, program: &ResolvedProgram) -> String {
    let targets=hir::function_value::target_universe(program).into_iter().map(|f|json!({"target":f.id.as_str(),"signature":hir::function_value::signature(f).expect("validated target").identity_key()})).collect::<Vec<_>>();
    graph.pop();
    graph.push_str(&format!(
        ",\"function_value_targets\":{}}}",
        serde_json::to_string(&targets).expect("JSON values serialize")
    ));
    graph
}
