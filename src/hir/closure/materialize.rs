//! Rebase a symbolic closure into its exact instantiated creation/body scopes.
use super::*;
use std::collections::BTreeMap;

pub(in crate::hir) fn materialize(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    values: &BTreeMap<ValueId, ValueId>,
    path: &str,
) -> Result<ResolvedExprKind, Diagnostic> {
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        body,
    } = &expression.kind
    else {
        unreachable!()
    };
    let body_execution =
        FunctionExecutionId::Monomorphic(closure_id(&ExpressionId::new(execution, path)));
    let mut body_values = BTreeMap::new();
    let mut binding =
        |old: &ResolvedBinding, ordinal: usize| -> Result<ResolvedBinding, Diagnostic> {
            let id = ValueId::parameter(&body_execution, ordinal);
            if body_values.insert(old.id.clone(), id.clone()).is_some() {
                return Err(hir_error("closure template binding identity repeats"));
            }
            Ok(ResolvedBinding {
                id,
                name: old.name.clone(),
                ownership: old.ownership,
                ty: super::super::monomorphize::substitute_type(&old.ty, &template.id, arguments)?,
                span: old.span,
            })
        };
    let mut concrete_captures = Vec::new();
    for (index, capture) in captures.iter().enumerate() {
        concrete_captures.push(ResolvedClosureCapture {
            binding: binding(&capture.binding, index)?,
            value: super::super::monomorphize::materialize_template_expr(
                template,
                arguments,
                execution,
                &capture.value,
                values,
                &format!("{path}.capture.{index}"),
            )?,
        });
    }
    let parameters = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| binding(parameter, captures.len() + index))
        .collect::<Result<_, _>>()?;
    let body = super::super::monomorphize::materialize_template_expr(
        template,
        arguments,
        &body_execution,
        body,
        &body_values,
        "body",
    )?;
    Ok(ResolvedExprKind::Closure {
        parameters,
        captures: concrete_captures,
        body: Box::new(body),
    })
}
