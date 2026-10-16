//! Allocation-free exact flat-record classification and charged shape backing.
//! This is constructor support only; source-bound replay admission is separate.
use super::*;
use crate::hir::{DeclarationKind, ResolvedFieldDeclaration};

// A fixed constructor tranche; no recursive type substitution or identity keys.
const MAX_DECLARATIONS: usize = 32;
const MAX_FIELDS: usize = 8;
pub(in crate::cleanup::inventory_builder) fn fields<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<Option<(&'a DeclarationId, &'a [ResolvedFieldDeclaration])>, Diagnostic> {
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        return Ok(None);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if !arguments.is_empty() {
        return Err(cleanup_error(
            "selected flat-record arguments are unsupported",
        ));
    }
    declaration_fields(program, declaration)
}
pub(in crate::cleanup::inventory_builder) fn declaration_fields<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    declaration: &'a DeclarationId,
) -> Result<Option<(&'a DeclarationId, &'a [ResolvedFieldDeclaration])>, Diagnostic> {
    let fail = || {
        cleanup_error("selected flat-record inventory has unauthenticated or unsupported fields")
    };
    if program.type_count() > MAX_DECLARATIONS {
        return Err(fail());
    }
    let mut matching = None;
    for ordinal in 0..program.type_count() {
        let item = program.type_at(ordinal).ok_or_else(fail)?;
        if item.id == *declaration && matching.replace(item).is_some() {
            return Err(fail());
        }
    }
    let record = matching.ok_or_else(fail)?;
    if !record.type_parameters.is_empty() {
        return Err(fail());
    }
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(fail());
    };
    if !(1..=MAX_FIELDS).contains(&fields.len()) {
        return Err(fail());
    }
    // Establish leaf-only equality before comparing against untrusted indexed
    // fields; no nested type tree is walked by the selected classifier.
    if fields.iter().any(|field| {
        !(crate::hir::is_scalar_resolved_type(&field.ty)
            || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
    }) {
        return Err(fail());
    }
    let index = program.declarations();
    let declared = index.declaration(declaration).ok_or_else(fail)?;
    if declared.kind != DeclarationKind::Record
        || declared.owner.is_some()
        || declared.name != record.name
        || index.record_fields(declaration) != Some(fields.as_slice())
    {
        return Err(fail());
    }
    for (position, field) in fields.iter().enumerate() {
        let field_fact = index.declaration(&field.id).ok_or_else(fail)?;
        if field.index as usize != position
            || field_fact.kind != DeclarationKind::Field
            || field_fact.owner.as_ref() != Some(declaration)
            || field_fact.name != field.name
            || fields[..position]
                .iter()
                .any(|old| old.id == field.id || old.name == field.name)
            || !(crate::hir::is_scalar_resolved_type(&field.ty)
                || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
        {
            return Err(fail());
        }
    }
    Ok(Some((declaration, fields)))
}
impl InventoryBuilder<'_> {
    pub(super) fn selected_flat_record_shape(
        &mut self,
        declaration: &DeclarationId,
        fields: &[ResolvedFieldDeclaration],
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        let declaration = self.clone_owned(declaration)?;
        let mut shapes = Vec::new();
        reserve(&self.physical, &mut shapes, fields.len())?;
        for field in fields {
            // Every field ID clone, lifecycle, flag and projection carrier is
            // reserved before its constructor. Copy fields create no live flag.
            let field_id = self.clone_owned(&field.id)?;
            let shape = self.selected_leaf_shape(&field.ty, storage, Some(&field.id))?;
            shapes.push(FieldLiveness {
                field: field_id,
                field_index: field.index,
                shape,
            });
        }
        Ok(FieldLivenessShape::Record {
            declaration,
            fields: shapes,
        })
    }
}
#[cfg(test)]
mod tests;
