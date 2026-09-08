//! Scalar snapshot closures with expression-scoped private body identities.
use super::*;

pub const MAX_CAPTURES: usize = 8;

pub fn closure_id(expression: &ExpressionId) -> DeclarationId {
    DeclarationId::new(format!("semaprax.closure.v1:{}", expression.as_str()))
}

/// The canonical body product used by checked graph and backend projections.
/// Capture parameters precede source parameters; runtime adapters unpack the
/// fixed scalar environment before entering this existing function ABI.
pub fn closure_function(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> Result<ResolvedFunction, Diagnostic> {
    validate_shape(program, expression)?;
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        body,
    } = &expression.kind
    else {
        return Err(hir_error("expected closure expression"));
    };
    let ResolvedType::Function { result, .. } = &expression.ty else {
        return Err(hir_error("closure has no callable signature"));
    };
    let id = closure_id(&expression.id);
    let execution = FunctionExecutionId::Monomorphic(id.clone());
    let params = captures
        .iter()
        .map(|capture| &capture.binding)
        .chain(parameters)
        .map(|binding| ResolvedParam {
            id: binding.id.clone(),
            name: binding.name.clone(),
            ownership: binding.ownership,
            ty: binding.ty.clone(),
            span: binding.span,
        })
        .collect();
    let mut function = ResolvedFunction {
        id,
        name: "closure".to_owned(),
        params,
        result_id: ValueId::result(&execution),
        return_type: *result.clone(),
        effects: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        body: *body.clone(),
        cleanup: crate::cleanup::CleanupInventory::unresolved(),
        cleanup_plan: crate::cleanup_plan::CleanupPlan::unresolved(),
        loan_plan: crate::loan_plan::LoanPlan::unresolved(),
        span: expression.span,
    };
    function.loan_plan = crate::loan_plan::build_plan(program, &function)?;
    function.cleanup = crate::cleanup::build_inventory(program, &function)?;
    function.cleanup_plan = crate::cleanup_plan::build_plan(program, &function)?;
    Ok(function)
}

/// Creation sites in deterministic expression identity order. A closure body is
/// a separate callable scope; nested anonymous closures are not admitted here.
pub fn inventory(program: &ResolvedProgram) -> Vec<&ResolvedExpr> {
    let mut found = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        super::function_value::walk(function, |expression| {
            if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
                found.push(expression);
            }
        });
    }
    found.sort_by(|left, right| left.id.cmp(&right.id));
    found
}

pub fn requires_closures(program: &ResolvedProgram) -> bool {
    !inventory(program).is_empty()
}

/// Whether the retained HIR needs the Graph v37 snapshot-closure projection.
///
/// A generic template can retain an admitted closure literal without a
/// materialized executable instance. That source fact still needs Graph v37
/// and SemanticProgram v5 replay, but it must not make target emitters invent
/// a runtime closure product for an uninstantiated template. Keep the latter
/// distinction in `requires_closures`, which is intentionally based only on
/// concrete function bodies.
pub fn requires_closure_projection(program: &ResolvedProgram) -> bool {
    requires_closures(program) || program.function_templates.iter().any(template_has_closure)
}

pub fn template_has_closure(template: &super::ResolvedFunctionTemplate) -> bool {
    let mut pending = template
        .requires
        .iter()
        .chain(std::iter::once(&template.body))
        .chain(&template.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            return true;
        }
        super::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

mod resolve;
mod validation;
pub(crate) use validation::validate_shape;
pub(crate) use validation::validate_shape_scoped;

#[cfg(test)]
mod tests;

mod materialize;
pub(super) use materialize::materialize;
