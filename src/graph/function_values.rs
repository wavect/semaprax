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
        ResolvedExprKind::Closure {
            parameters,
            captures,
            ..
        } => {
            let parameters = parameters
                .iter()
                .map(binding_json)
                .collect::<Vec<_>>()
                .join(",");
            let captures = captures
                .iter()
                .map(|capture| {
                    Ok(format!(
                        "{{\"binding\":{},\"value\":{}}}",
                        binding_json(&capture.binding),
                        super::expr_json(program, &capture.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .join(",");
            Ok(format!("{{{header},\"kind\":\"closure\",\"target\":{},\"parameters\":[{parameters}],\"captures\":[{captures}]}}", quote_json(hir::closure::closure_id(&expression.id).as_str())))
        }
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
            let mut candidates = hir::function_value::compatible_targets(program, &callable.ty)
                .into_iter()
                .map(|f| f.id.clone())
                .collect::<Vec<_>>();
            candidates.extend(
                hir::closure::inventory(program)
                    .into_iter()
                    .filter(|closure| closure.ty == callable.ty)
                    .map(|closure| hir::closure::closure_id(&closure.id)),
            );
            candidates.sort();
            let candidates = candidates
                .iter()
                .map(|id| quote_json(id.as_str()))
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

fn binding_json(binding: &hir::ResolvedBinding) -> String {
    format!(
        "{{\"id\":{},\"name\":{},\"type\":{},\"ownership_mode\":{}}}",
        quote_json(binding.id.as_str()),
        quote_json(&binding.name),
        super::type_json(&binding.ty),
        quote_json(ownership_text(binding.ownership))
    )
}

pub(super) fn append_closures(
    mut graph: String,
    program: &ResolvedProgram,
) -> Result<String, Diagnostic> {
    let mut definitions = Vec::new();
    for expression in hir::closure::inventory(program) {
        let function = hir::closure::closure_function(program, expression)?;
        let parameters = function
            .params
            .iter()
            .map(|parameter| {
                format!(
                    "{{\"id\":{},\"type\":{},\"ownership_mode\":{}}}",
                    quote_json(parameter.id.as_str()),
                    super::type_json(&parameter.ty),
                    quote_json(ownership_text(parameter.ownership))
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        definitions.push(format!("{{\"id\":{},\"creation\":{},\"signature\":{},\"persistent\":false,\"parameters\":[{parameters}],\"result_id\":{},\"return_type\":{},\"effects\":[],\"requires\":[],\"ensures\":[],\"body\":{},\"cleanup\":{},\"loans\":{}}}", quote_json(function.id.as_str()), quote_json(expression.id.as_str()), super::type_json(&expression.ty), quote_json(function.result_id.as_str()), super::type_json(&function.return_type), super::expr_json(program, &function.body)?, crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan), crate::graph_loan::loan_plan_json(&function.loan_plan)));
    }
    graph.pop();
    graph.push_str(&format!(
        ",\"closure_definitions\":[{}]}}",
        definitions.join(",")
    ));
    Ok(graph)
}

pub(super) fn function_has_closure(function: &ResolvedFunction) -> bool {
    let mut found = false;
    hir::function_value::walk(function, |expression| {
        found |= matches!(expression.kind, ResolvedExprKind::Closure { .. })
    });
    found
}
