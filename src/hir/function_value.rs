//! Exact, authority-free callable identities for Function Value v1.
use super::{
    DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedType,
};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeSet;

pub fn scalar(ty: &ResolvedType) -> bool {
    super::nodes::is_scalar_resolved_type(ty)
}
pub fn is_signature(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Function { parameters, result } if parameters.len() <= 8 && parameters.iter().all(scalar) && scalar(result))
}
pub fn signature(function: &ResolvedFunction) -> Option<ResolvedType> {
    (function.effects.is_empty()
        && function.params.len() <= 8
        && function
            .params
            .iter()
            .all(|p| p.ownership == OwnershipMode::Value && scalar(&p.ty))
        && scalar(&function.return_type))
    .then(|| ResolvedType::Function {
        parameters: function.params.iter().map(|p| p.ty.clone()).collect(),
        result: Box::new(function.return_type.clone()),
    })
}
pub(crate) fn walk<'a>(function: &'a ResolvedFunction, mut visit: impl FnMut(&'a ResolvedExpr)) {
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expr) = pending.pop() {
        visit(expr);
        super::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
}
pub fn target_universe(program: &ResolvedProgram) -> Vec<&ResolvedFunction> {
    let mut ids = BTreeSet::new();
    for f in &program.functions {
        walk(f, |e| {
            if let ResolvedExprKind::FunctionReference { target } = &e.kind {
                ids.insert(target.clone());
            }
        });
    }
    let mut functions = program
        .functions
        .iter()
        .filter(|f| ids.contains(&f.id))
        .collect::<Vec<_>>();
    functions.sort_by(|a, b| a.id.cmp(&b.id));
    functions
}
pub fn compatible_targets<'a>(
    program: &'a ResolvedProgram,
    ty: &ResolvedType,
) -> Vec<&'a ResolvedFunction> {
    target_universe(program)
        .into_iter()
        .filter(|f| signature(f).as_ref() == Some(ty))
        .collect()
}
pub fn requires_function_values(program: &ResolvedProgram) -> bool {
    program.functions.iter().any(|f| {
        let mut found = matches!(f.return_type, ResolvedType::Function { .. })
            || f.params
                .iter()
                .any(|p| matches!(p.ty, ResolvedType::Function { .. }));
        walk(f, |e| {
            found |= matches!(
                e.kind,
                ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::Invoke { .. }
            )
        });
        found
    })
}
pub(crate) fn validate_reference(
    program: &ResolvedProgram,
    target: &DeclarationId,
    ty: &ResolvedType,
) -> Result<(), Diagnostic> {
    let target = program
        .functions
        .iter()
        .find(|f| f.id == *target)
        .ok_or_else(|| error("function reference does not name a local monomorphic declaration"))?;
    if signature(target).as_ref() != Some(ty) {
        return Err(error(
            "function reference has an ineligible or mismatched signature",
        ));
    }
    Ok(())
}
pub(crate) fn validate_invocation(expression: &ResolvedExpr) -> Result<(), Diagnostic> {
    let ResolvedExprKind::Invoke { callable, args } = &expression.kind else {
        return Err(error("expected invocation"));
    };
    let ResolvedType::Function { parameters, result } = &callable.ty else {
        return Err(error("invocation target is not a function value"));
    };
    if !matches!(&callable.kind,ResolvedExprKind::Place(place) if place.projections.is_empty())
        || !is_signature(&callable.ty)
        || callable.ownership != OwnershipMode::Value
        || args.len() != parameters.len()
        || args
            .iter()
            .zip(parameters)
            .any(|(a, p)| a.ty != *p || a.ownership != OwnershipMode::Value)
        || expression.ty != **result
        || expression.ownership != OwnershipMode::Value
    {
        return Err(error("invocation signature, argument or result mismatch"));
    }
    Ok(())
}
pub(crate) fn error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

pub(crate) mod resolve;

pub(crate) static INVOKE_ID: std::sync::LazyLock<DeclarationId> =
    std::sync::LazyLock::new(|| DeclarationId::new("core.function.invoke"));
pub(crate) fn invocation_params(
    expression: &ResolvedExpr,
) -> Result<Vec<super::ResolvedParam>, Diagnostic> {
    validate_invocation(expression)?;
    let ResolvedExprKind::Invoke { callable, .. } = &expression.kind else {
        unreachable!()
    };
    let ResolvedType::Function { parameters, .. } = &callable.ty else {
        unreachable!()
    };
    Ok(parameters
        .iter()
        .enumerate()
        .map(|(index, ty)| super::ResolvedParam {
            id: super::ValueId::intrinsic_parameter(INVOKE_ID.as_str(), index),
            name: format!("arg{index}"),
            ty: ty.clone(),
            ownership: OwnershipMode::Value,
            span: expression.span,
        })
        .collect())
}

pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if !requires_function_values(program) {
        return Ok(());
    }
    if target_universe(program).len() > 256 {
        return Err(error(
            "function value target universe exceeds 256 declarations",
        ));
    }
    let mut edges = std::collections::BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    for f in &program.functions {
        let mut targets = BTreeSet::new();
        super::visit_resolved_calls(&f.body, &mut |target, _, _| {
            targets.insert(target.clone());
        });
        for contract in f.requires.iter().chain(&f.ensures) {
            super::visit_resolved_calls(contract, &mut |target, _, _| {
                targets.insert(target.clone());
            });
        }
        walk(f, |e| {
            if let ResolvedExprKind::Invoke { callable, .. } = &e.kind {
                targets.extend(
                    compatible_targets(program, &callable.ty)
                        .into_iter()
                        .map(|f| f.id.clone()),
                );
            }
        });
        edges.insert(f.id.clone(), targets);
    }
    for root in edges.keys() {
        let mut visited = BTreeSet::new();
        let mut pending = edges[root].iter().collect::<Vec<_>>();
        while let Some(next) = pending.pop() {
            if next == root {
                return Err(error(
                    "function value candidate dependencies contain a cycle",
                ));
            }
            if visited.insert(next) {
                if let Some(children) = edges.get(next) {
                    pending.extend(children);
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn function_uses_value(f: &ResolvedFunction) -> bool {
    let mut found = matches!(f.return_type, ResolvedType::Function { .. })
        || f.params
            .iter()
            .any(|p| matches!(p.ty, ResolvedType::Function { .. }));
    walk(f, |e| {
        found |= matches!(
            e.kind,
            ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::Invoke { .. }
        )
    });
    found
}

#[cfg(test)]
mod tests;
