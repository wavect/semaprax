//! Exact native bindings for authenticated record payloads in conditional owners.

use super::*;

pub(super) fn is_exact(
    program: &hir::ResolvedProgram,
    scrutinee: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    field_ty: &ResolvedType,
) -> bool {
    crate::cleanup::variant_record_field(program, scrutinee, case, field, field_ty)
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
    let source = source_storage
        .ok_or_else(|| backend_error("borrowed variant record is not place-rooted"))?;
    for relative in super::super::borrowed_aggregate_byte_paths(
        emitter.program,
        emitter.record_layouts,
        emitter.variant_layouts,
        field_ty,
    )? {
        let path = [vec![case.clone(), field.clone()], relative.clone()].concat();
        let alias = emitter
            .bytes_plan
            .and_then(|plan| plan.projected_value_if_present(source, &path))
            .map(str::to_owned)
            .or_else(|| {
                let crate::cleanup_plan::StorageId::Value(root) = source else {
                    return None;
                };
                emitter
                    .borrowed_aggregate_bytes
                    .get(&(root.clone(), path))
                    .cloned()
            })
            .ok_or_else(|| backend_error("borrowed variant record leaf has no source alias"))?;
        if emitter
            .borrowed_aggregate_bytes
            .insert((binding.id.clone(), relative), alias)
            .is_some()
        {
            return Err(backend_error(
                "borrowed variant record leaf alias is duplicated",
            ));
        }
    }
    Ok(())
}
