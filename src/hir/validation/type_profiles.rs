//! Closed type-profile predicates shared by both HIR validation passes.

use super::*;

pub(super) fn template_ownership(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> OwnershipMode {
    match ty {
        ResolvedType::String => OwnershipMode::Own,
        ResolvedType::Str => OwnershipMode::Borrow,
        _ if crate::hir::type_reachability::is_flat_owned_byte_record_template(
            &program.declarations,
            ty,
            &template.id,
            template.type_parameters.len(),
        ) =>
        {
            OwnershipMode::Own
        }
        _ => OwnershipMode::Value,
    }
}

pub(super) fn template_has_owned_record_slot(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> bool {
    template.params.iter().any(|parameter| {
        parameter.ownership == OwnershipMode::Own
            && crate::hir::type_reachability::is_flat_owned_byte_record_template(
                &program.declarations,
                &parameter.ty,
                &template.id,
                template.type_parameters.len(),
            )
    }) && crate::hir::type_reachability::is_flat_owned_byte_record_template(
        &program.declarations,
        &template.return_type,
        &template.id,
        template.type_parameters.len(),
    )
}

pub(super) fn generic_instance_arguments_are_admitted(
    program: &ResolvedProgram,
    template_id: &DeclarationId,
    arguments: &[ResolvedType],
) -> bool {
    arguments
        .iter()
        .all(|argument| matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        || program
            .function_templates
            .iter()
            .find(|template| &template.id == template_id)
            .is_some_and(|template| {
                arguments.len() == template.type_parameters.len()
                    && arguments.iter().all(|argument| {
                        crate::hir::type_reachability::nested_record_copy_scalar_is_admitted(
                            argument,
                        )
                    })
                    && template_has_owned_record_slot(program, template)
            })
}

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
