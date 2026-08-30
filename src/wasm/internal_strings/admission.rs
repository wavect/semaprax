//! Closed selection and iterative work admission before recursive lowering.

use super::{error, Export, PreparedSelection};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, OwnershipMode, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement, ResolvedType,
};
use std::collections::BTreeSet;

pub(super) fn prepare(
    program: &ResolvedProgram,
    ids: &[String],
) -> Result<PreparedSelection, Diagnostic> {
    if !(1..=32).contains(&ids.len()) {
        return Err(error("standalone String selection requires 1..=32 exports"));
    }
    let mut roots = BTreeSet::new();
    for id in ids {
        if !roots.insert(DeclarationId::new(id.clone())) {
            return Err(error("standalone String selection repeats an identity"));
        }
    }
    let mut exports = Vec::new();
    for id in &roots {
        let function = program
            .functions
            .iter()
            .find(|function| &function.id == id)
            .ok_or_else(|| error("standalone String export is absent"))?;
        if !program
            .declarations
            .declaration(id)
            .is_some_and(|declaration| declaration.identity_origin == IdentityOrigin::Explicit)
            || function.params.len() > 8
            || !public_scalar(&function.return_type)
            || function.params.iter().any(|parameter| {
                parameter.ownership != OwnershipMode::Value || !public_scalar(&parameter.ty)
            })
        {
            return Err(error("standalone String export requires an explicit identity and bounded value i64/bool signature"));
        }
        exports.push(Export {
            id: id.clone(),
            parameters: function
                .params
                .iter()
                .map(|parameter| parameter.ty.clone())
                .collect(),
            result: function.return_type.clone(),
        });
    }
    let calls = crate::call_index::PersistentCallIndex::build(program)?;
    let mut closure = BTreeSet::new();
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if !closure.insert(id.clone()) {
            continue;
        }
        if closure.len() > 256 {
            return Err(error("standalone String closure exceeds 256 functions"));
        }
        let children = calls
            .calls_by_owner()
            .get(&id)
            .ok_or_else(|| error("standalone String callee is absent"))?;
        pending.extend(children.iter().filter(|id| !closure.contains(*id)).cloned());
    }
    let mut nodes = 0usize;
    for function in program
        .functions
        .iter()
        .filter(|function| closure.contains(&function.id))
    {
        if !function.effects.is_empty()
            || !internal_type(&function.return_type)
            || function.params.iter().any(|parameter| {
                // By-value String source parameters are implicitly Own in
                // validated HIR; only the internal Copy scalars are Value.
                let ownership = if parameter.ty == ResolvedType::String {
                    OwnershipMode::Own
                } else {
                    OwnershipMode::Value
                };
                parameter.ownership != ownership || !internal_type(&parameter.ty)
            })
        {
            return Err(error(
                "standalone String internal signature is outside the closed profile",
            ));
        }
        let mut expressions = function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
            .map(|expression| (expression, 1usize))
            .collect::<Vec<_>>();
        while let Some((expression, depth)) = expressions.pop() {
            nodes = nodes
                .checked_add(1)
                .filter(|nodes| *nodes <= 65_536)
                .ok_or_else(|| {
                    error("standalone String expression inventory exceeds 65536 nodes")
                })?;
            if depth > 256 || !internal_type(&expression.ty) {
                return Err(error(
                    "standalone String expression depth or type is outside the profile",
                ));
            }
            match &expression.kind {
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::Unary { .. }
                | ResolvedExprKind::Binary { .. }
                | ResolvedExprKind::If { .. } => {}
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    type_arguments,
                    ..
                } if instance.is_none()
                    && type_arguments.is_empty()
                    && (closure.contains(callee)
                        || crate::string_ops::by_id(callee.as_str()).is_some()) => {}
                ResolvedExprKind::Block { statements, .. } => {
                    if statements.iter().any(|statement| {
                        matches!(
                            statement,
                            ResolvedStatement::Unsafe { .. }
                                | ResolvedStatement::Assign { field: Some(_), .. }
                        )
                    }) {
                        return Err(error(
                            "standalone String profile excludes unsafe or projected mutation",
                        ));
                    }
                }
                ResolvedExprKind::Match {
                    scrutinee,
                    arms,
                    mode,
                } if *mode == hir::ResolvedMatchMode::Value
                    && matches!(
                        scrutinee.ty,
                        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Char
                    )
                    && arms
                        .iter()
                        .all(|arm| arm.pattern_is_literal_or_irrefutable()) => {}
                _ => {
                    return Err(error(
                        "standalone String expression is outside the closed profile",
                    ))
                }
            }
            for child in crate::interpreter::trace_child_expressions(expression) {
                expressions.push((child, depth + 1));
            }
        }
    }
    Ok((exports, closure))
}

fn public_scalar(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}
fn internal_type(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Char | ResolvedType::String
    )
}
