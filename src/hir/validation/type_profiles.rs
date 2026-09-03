//! Closed type-profile predicates shared by both HIR validation passes.

use super::*;

pub(crate) fn resolved_type_contains_owned_bytes(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> bool {
    let mut pending = vec![ty.clone()];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Bytes => return true,
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                pending.extend(arguments);
                if !visited.insert(declaration.clone()) {
                    continue;
                }
                let Some(item) = program
                    .types
                    .iter()
                    .find(|candidate| candidate.id == declaration)
                else {
                    continue;
                };
                match &item.kind {
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        pending.extend(fields.iter().map(|field| field.ty.clone()));
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => pending.extend(
                        cases
                            .iter()
                            .flat_map(|case| &case.fields)
                            .map(|field| field.ty.clone()),
                    ),
                    ResolvedTypeDeclarationKind::Resource { .. } => {}
                }
            }
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::TypeParameter { .. } => {}
        }
    }
    false
}

pub(super) fn validate_nested_update_base_shape(
    program: &ResolvedProgram,
    base: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    if crate::hir::type_reachability::is_nested_nonflat_owned_byte_record(
        &program.declarations,
        &base.ty,
    ) && !matches!(
        &base.kind,
        ResolvedExprKind::Place(place) if place.projections.is_empty()
    ) {
        return Err(hir_error(
            "SPX-O117: nested owned-record update requires an exact named owned base place",
        ));
    }
    Ok(())
}

pub(super) fn resolved_type_is_flat_owned_byte_variant(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if admitted_owned_byte_prelude_instance(declaration, arguments) {
        return true;
    }
    if !arguments.is_empty() {
        return false;
    }
    program.types.iter().any(|item| {
        item.id == *declaration
            && item.type_parameters.is_empty()
            && matches!(&item.kind, ResolvedTypeDeclarationKind::Variant { cases }
                if cases.iter().flat_map(|case| &case.fields).any(|field| field.ty == ResolvedType::Bytes)
                    && cases.iter().flat_map(|case| &case.fields).all(|field|
                        field.ty == ResolvedType::Bytes
                            || crate::hir::type_reachability::nested_record_copy_scalar_is_admitted(&field.ty)))
    })
}
