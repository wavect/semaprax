//! Native place lookup and authenticated record projection.

use super::*;

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_place(&mut self, place: &hir::Place) -> Result<CValue, Diagnostic> {
        let binding = self.variables.get(&place.root).cloned().ok_or_else(|| {
            backend_error(format!("resolved value `{}` is not in scope", place.root))
        })?;
        let mut code = binding.name;
        let mut ty = binding.ty;
        let storage = crate::cleanup_plan::StorageId::Value(place.root.clone());
        let mut field_path = Vec::with_capacity(place.projections.len());
        for projection in &place.projections {
            let PlaceProjection::Field(field) = projection else {
                return Err(backend_error(
                    "native variant-field projection is outside executable records v1",
                ));
            };
            let layout = self.record_layout(&ty)?;
            let field = layout.field(field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "native record `{}` has no place field `{field}`",
                    layout.record
                ))
            })?;
            field_path.push(field.field.clone());
            code = if matches!(field.ty, ResolvedType::Bytes | ResolvedType::String)
                || crate::hir::owned_collection_record::vector(
                    &self.program.declarations,
                    &field.ty,
                ) {
                self.generic_projected_bytes_value(&place.root, &storage, &field_path)?
            } else if field.size == 0 {
                self.emit_erased_record_field_value(&field.ty)?.code
            } else {
                format!("({code}).{}", c_field_symbol(&field.field))
            };
            ty = field.ty;
        }
        Ok(CValue { code, ty })
    }
}
