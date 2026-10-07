//! Exact native bindings for the owned-record iterator `Yield.item` field.

use super::*;

pub(super) fn is_exact(
    program: &hir::ResolvedProgram,
    scrutinee: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    field_ty: &ResolvedType,
) -> bool {
    case.as_str() == crate::iterator_ops::YIELD_ID
        && field.as_str() == crate::iterator_ops::ITEM_ID
        && crate::iterator_ops::step_shape(&program.declarations, scrutinee)
        && crate::iterator_ops::element(scrutinee) == Some(field_ty)
        && crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
            &program.declarations,
            field_ty,
        )
}

pub(super) fn bind<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    staged: &str,
    (case, field): (&DeclarationId, &DeclarationId),
    field_ty: &ResolvedType,
    binding: &hir::ResolvedBinding,
    mode: hir::ResolvedMatchMode,
    source_storage: Option<&crate::cleanup_plan::StorageId>,
) -> Result<String, Diagnostic> {
    let carrier = format!(
        "({staged}).spx_payload.{}.{}",
        c_case_symbol(case),
        c_field_symbol(field)
    );
    match (mode, binding.ownership) {
        (hir::ResolvedMatchMode::Own, hir::OwnershipMode::Own) => {
            let storage = crate::cleanup_plan::StorageId::Value(binding.id.clone());
            if !emitter
                .bytes_plan
                .ok_or_else(|| backend_error("owned record iterator item has no cleanup plan"))?
                .has_projected_leaves(&storage)
            {
                return Err(backend_error(
                    "owned record iterator item has no projected cleanup leaves",
                ));
            }
        }
        (hir::ResolvedMatchMode::Borrow, hir::OwnershipMode::Borrow) => {
            bind_borrowed_leaves(emitter, case, field, field_ty, binding, source_storage)?;
        }
        _ => {
            return Err(backend_error(
                "record iterator item binding ownership disagrees with match mode",
            ));
        }
    }
    Ok(carrier)
}

fn bind_borrowed_leaves<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    case: &DeclarationId,
    field: &DeclarationId,
    field_ty: &ResolvedType,
    binding: &hir::ResolvedBinding,
    source_storage: Option<&crate::cleanup_plan::StorageId>,
) -> Result<(), Diagnostic> {
    let ResolvedType::Nominal { declaration, .. } = field_ty else {
        return Err(backend_error("record iterator item is not nominal"));
    };
    let source = source_storage
        .ok_or_else(|| backend_error("borrowed record iterator item is not place-rooted"))?;
    let fields = emitter
        .program
        .declarations
        .record_fields(declaration)
        .ok_or_else(|| backend_error("record iterator item has no field inventory"))?;
    let plan = emitter
        .bytes_plan
        .ok_or_else(|| backend_error("borrowed record iterator item has no cleanup plan"))?;
    for leaf in fields.iter().filter(|leaf| leaf.ty == ResolvedType::Bytes) {
        let path = vec![case.clone(), field.clone(), leaf.id.clone()];
        let alias = plan
            .projected_value_if_present(source, &path)
            .ok_or_else(|| backend_error("borrowed record iterator leaf has no source slot"))?;
        if emitter
            .borrowed_aggregate_bytes
            .insert(
                (binding.id.clone(), vec![leaf.id.clone()]),
                alias.to_owned(),
            )
            .is_some()
        {
            return Err(backend_error(
                "borrowed record iterator leaf alias is duplicated",
            ));
        }
    }
    Ok(())
}
