//! Logical collection declarations; deliberately no executable carrier authority.
use super::*;
use std::collections::BTreeSet;

pub(crate) fn text_element(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    if *ty == ResolvedType::String {
        return true;
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty()
        || index.declaration(declaration).is_none_or(|d| {
            d.kind != DeclarationKind::Record || d.identity_origin != IdentityOrigin::Explicit
        })
        || index
            .type_parameters(declaration)
            .is_none_or(|p| !p.is_empty())
    {
        return false;
    }
    let Some(fields) = index.record_fields(declaration) else {
        return false;
    };
    (1..=8).contains(&fields.len())
        && fields
            .iter()
            .filter(|f| f.ty == ResolvedType::String)
            .count()
            == 1
        && fields.iter().all(|f| {
            f.ty == ResolvedType::String || crate::vec_ops::resolved_element_is_admitted(&f.ty)
        })
}
pub(crate) fn vector(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty,ResolvedType::Nominal{declaration,arguments} if declaration.as_str()==crate::prelude::VEC_ID
        && index.declaration(declaration).is_some_and(|d|d.identity_origin==IdentityOrigin::CompilerOwned)
        && matches!(arguments.as_slice(),[element] if text_element(index,element)))
}
/// Transitive refusal, independent of selected entry/reachability and cache facts.
pub(crate) fn contains(index: &DeclarationIndex, root: &ResolvedType) -> bool {
    // v14 gives this direct carrier independent runtime authority. A record
    // containing such a vector remains a logical schema, never an implicit ABI.
    if super::owned_leaf_collection::is_vec(index, root) {
        return false;
    }
    if !matches!(
        root,
        ResolvedType::Nominal { .. } | ResolvedType::Function { .. }
    ) {
        return false;
    }
    let mut pending = vec![root.clone()];
    let mut seen = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if vector(index, &ty) {
            return true;
        }
        if !matches!(
            &ty,
            ResolvedType::Nominal { .. } | ResolvedType::Function { .. }
        ) {
            continue;
        }
        if !seen.insert(ty.clone()) {
            continue;
        }
        match ty {
            ResolvedType::Function { parameters, result } => {
                pending.extend(parameters);
                pending.push(*result);
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if let Some(fields) = index.record_fields(&declaration) {
                    for field in fields {
                        pending.push(field.ty.clone());
                    }
                }
                if let Some(cases) = index.variant_cases(&declaration) {
                    for field in cases.iter().flat_map(|c| &c.fields) {
                        pending.push(field.ty.clone());
                    }
                }
                pending.extend(arguments);
            }
            _ => {}
        }
    }
    false
}
pub(crate) fn validate_function(
    index: &DeclarationIndex,
    f: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let refuses = |ty: &ResolvedType| contains(index, ty);
    let refusal = || {
        hir_error(
            "declaration-only text collection cannot occur in an executable signature or body",
        )
    };
    if refuses(&f.return_type)
        || f.params.iter().any(|p| refuses(&p.ty))
        || f.yields
            .as_ref()
            .is_some_and(|y| refuses(&y.request_type) || refuses(&y.response_type))
    {
        return Err(refusal());
    }
    let mut pending = std::iter::once(&f.body)
        .chain(f.requires.iter())
        .chain(f.ensures.iter())
        .collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        if refuses(&expression.ty) {
            return Err(refusal());
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
