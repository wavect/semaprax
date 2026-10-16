use super::{
    flat_fields, needs_drop_in, primitive_drop, receiving, reserve, vector_element,
    InventoryBuilder, InventoryFailure, Kind, Physical, Reason,
};
use crate::cleanup::{
    CleanupFlag, CleanupPlace, CleanupStorageId, FieldLiveness, FieldLivenessShape, LivenessFlagId,
    MAX_CLEANUP_OWNED_LEAVES,
};
use crate::cleanup_plan::ReplayClonePayload;
use crate::hir::{DeclarationId, ResolvedFieldDeclaration, ResolvedType};
impl InventoryBuilder<'_> {
    pub(super) fn clone_owned<T: Clone + ReplayClonePayload>(
        &self,
        value: &T,
    ) -> Result<T, InventoryFailure> {
        receiving!(&self.ledger, T, super::receiving::REASON_OR_PHYSICAL, 1);
        {
            let bytes = value.clone_payload_bytes().ok_or_else(|| {
                InventoryFailure(Kind::Finite(Reason::SelectedInventoryCloneBackingOverflow))
            })?;
            self.charge(bytes)?;
            self.ledger
                .borrow_mut()
                .physical
                .reserve(bytes)
                .map_err(|_| InventoryFailure(Kind::Physical(Physical::Clone)))?;
        }
        Ok(value.clone())
    }
    pub(super) fn singleton<T>(&self, value: T) -> Result<Vec<T>, InventoryFailure> {
        receiving!(&self.ledger, Vec<T>);
        let mut values = Vec::new();
        reserve(&self.ledger, &mut values, 1)?;
        values.push(value);
        Ok(values)
    }
    pub(super) fn selected_shape(
        &mut self,
        ty: &ResolvedType,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, InventoryFailure> {
        receiving!(
            &self.ledger,
            FieldLivenessShape,
            super::receiving::CLASSIFIER,
            1
        );
        let program = self.program;
        self.prepay_collection(ty)?;
        if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
            needs_drop_in!(program, ty);
            return Ok(FieldLivenessShape::NoDrop);
        }
        if vector_element!(program, ty).is_some() {
            return self.selected_vector_shape(storage);
        }
        if let Some((declaration, fields)) = flat_fields!(program, ty) {
            if !fields
                .iter()
                .any(|field| matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
            {
                return Ok(FieldLivenessShape::NoDrop);
            }
            return self.selected_flat_record_shape(declaration, fields, storage);
        }
        self.selected_leaf_shape(ty, storage, None)
    }
    pub(super) fn selected_leaf_shape(
        &mut self,
        ty: &ResolvedType,
        storage: CleanupStorageId,
        projection: Option<&DeclarationId>,
    ) -> Result<FieldLivenessShape, InventoryFailure> {
        receiving!(
            &self.ledger,
            FieldLivenessShape,
            super::receiving::REASON_OR_TYPE_DROP,
            1
        );
        if !primitive_drop!(ty) {
            return Ok(FieldLivenessShape::NoDrop);
        }
        let id = crate::cleanup::primitive_leaf_lifecycle_identity(ty).ok_or_else(|| {
            InventoryFailure(Kind::Finite(
                Reason::SelectedInventoryHasNoPrimitiveLifecycle,
            ))
        })?;
        self.selected_lifecycle_shape(id, storage, projection)
    }
    fn selected_vector_shape(
        &mut self,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, InventoryFailure> {
        receiving!(&self.ledger, FieldLivenessShape);
        self.selected_lifecycle_shape(crate::hir::CompilerDeclaration::VecDrop, storage, None)
    }
    /// The lifecycle is a fixed compiler identity and allocates nothing. Its
    /// original text term stays reserved on the selected ledger, unchanged, so
    /// the cap and refusal order are those of the owned-text inventory.
    fn selected_lifecycle_shape(
        &mut self,
        id: crate::hir::CompilerDeclaration,
        storage: CleanupStorageId,
        projection: Option<&DeclarationId>,
    ) -> Result<FieldLivenessShape, InventoryFailure> {
        receiving!(
            &self.ledger,
            FieldLivenessShape,
            super::receiving::REASON_OR_PHYSICAL,
            1
        );
        let flag =
            LivenessFlagId(u32::try_from(self.flags.len()).map_err(|_| {
                InventoryFailure(Kind::Finite(Reason::TooManyCleanupLivenessFlags))
            })?);
        if self.flags.len() >= MAX_CLEANUP_OWNED_LEAVES {
            return Err(InventoryFailure(Kind::Finite(
                Reason::SelectedInventoryExceedsOwnedLeafLimit,
            )));
        }
        self.ledger
            .borrow_mut()
            .physical
            .reserve(id.text().len())
            .map_err(|_| InventoryFailure(Kind::Physical(Physical::Lifecycle)))?;
        let lifecycle = DeclarationId::compiler(id);
        let flag_lifecycle = self.clone_owned(&lifecycle)?;
        let mut projections = Vec::new();
        if let Some(projection) = projection {
            reserve(&self.ledger, &mut projections, 1)?;
            projections.push(self.clone_owned(projection)?);
        }
        reserve(&self.ledger, &mut self.flags, 1)?;
        self.flags.push(CleanupFlag {
            id: flag,
            place: CleanupPlace {
                storage,
                projections,
            },
            lifecycle: flag_lifecycle,
        });
        Ok(FieldLivenessShape::Leaf { flag, lifecycle })
    }
    pub(super) fn selected_flat_record_shape(
        &mut self,
        declaration: &DeclarationId,
        fields: &[ResolvedFieldDeclaration],
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, InventoryFailure> {
        receiving!(&self.ledger, FieldLivenessShape);
        let declaration = self.clone_owned(declaration)?;
        let mut shapes = Vec::new();
        reserve(&self.ledger, &mut shapes, fields.len())?;
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
