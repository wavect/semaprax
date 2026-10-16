//! Initial complete selected inventory constructor closure: direct primitive
//! cleanup leaves and Copy/borrowed traversal. A separate borrowed flat-record
//! constructor closes its own backing; retained replay admission stays primitive.
use super::*;
pub(super) mod flat_record;
pub(crate) mod vector;

type Physical<'a> = Option<RefCell<&'a mut ReplayPhysicalReservation>>;
pub(super) fn reserve<T>(
    physical: &Physical<'_>,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), Diagnostic> {
    if let Some(physical) = physical {
        replay_reserve_vec(&mut physical.borrow_mut(), values, additional)
            .map_err(|_| cleanup_error("selected inventory physical reservation refused"))?;
    }
    Ok(())
}
pub(super) fn reserve_legacy<T>(
    physical: &Physical<'_>,
    values: &mut Vec<T>,
    additional: usize,
    message: &'static str,
) -> Result<(), Diagnostic> {
    if physical.is_some() {
        reserve(physical, values, additional)
    } else {
        values
            .try_reserve(additional)
            .map_err(|_| cleanup_error(message))
    }
}
pub(super) fn needs_drop(ty: &ResolvedType) -> Result<bool, Diagnostic> {
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        return Ok(false);
    }
    match ty {
        ResolvedType::Bytes
        | ResolvedType::String
        | ResolvedType::StringMap
        | ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair => Ok(true),
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
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::MutFunctionI64 => Ok(false),
        ResolvedType::Never
        | ResolvedType::Function { .. }
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::Nominal { .. } => Err(cleanup_error(
            "selected physical inventory does not support this cleanup type",
        )),
    }
}
pub(super) fn needs_drop_in(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
) -> Result<bool, Diagnostic> {
    // Complete selected replay has already authenticated abrupt expression
    // nodes. Their Never type carries no value; the primitive-only constructor
    // boundary remains closed to Never through needs_drop().
    if *ty == ResolvedType::Never {
        return Ok(false);
    }
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        return if crate::hir::borrowed_scalar::option_u8::authenticated(program.declarations(), ty)
        {
            Ok(false)
        } else {
            Err(cleanup_error("selected Option metadata is unauthenticated"))
        };
    }
    if vector::element(program, ty)?.is_some() {
        return Ok(true);
    }
    if let Some((_, fields)) = flat_record::fields(program, ty)? {
        return Ok(fields
            .iter()
            .any(|field| matches!(field.ty, ResolvedType::String | ResolvedType::Bytes)));
    }
    needs_drop(ty)
}
impl InventoryBuilder<'_> {
    pub(super) fn clone_owned<T: Clone + ReplayClonePayload>(
        &self,
        value: &T,
    ) -> Result<T, Diagnostic> {
        if let Some(physical) = &self.physical {
            let bytes = value
                .clone_payload_bytes()
                .ok_or_else(|| cleanup_error("selected inventory clone backing overflow"))?;
            self.charge(bytes)?;
            physical
                .borrow_mut()
                .reserve(bytes)
                .map_err(|_| cleanup_error("selected inventory clone reservation refused"))?;
        }
        Ok(value.clone())
    }
    pub(super) fn singleton<T>(&self, value: T) -> Result<Vec<T>, Diagnostic> {
        let mut values = Vec::new();
        reserve(&self.physical, &mut values, 1)?;
        values.push(value);
        Ok(values)
    }
    pub(super) fn selected_shape(
        &mut self,
        ty: &ResolvedType,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        let program = self.program;
        self.prepay_collection(ty)?;
        if self.work.is_some() && !self.collection_work {
            if !self.needs_drop(ty)? {
                return Ok(FieldLivenessShape::NoDrop);
            }
            return self.selected_leaf_shape(ty, storage, None);
        }
        if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
            needs_drop_in(program, ty)?;
            return Ok(FieldLivenessShape::NoDrop);
        }
        if self.flat_records && vector::element(program, ty)?.is_some() {
            return self.selected_vector_shape(storage);
        }
        if let Some((declaration, fields)) = if self.flat_records {
            flat_record::fields(program, ty)?
        } else {
            None
        } {
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
    ) -> Result<FieldLivenessShape, Diagnostic> {
        if !needs_drop(ty)? {
            return Ok(FieldLivenessShape::NoDrop);
        }
        let id = crate::cleanup::primitive_leaf_lifecycle_identity(ty)
            .ok_or_else(|| cleanup_error("selected inventory has no primitive lifecycle"))?;
        self.selected_lifecycle_shape(id, storage, projection)
    }
    fn selected_vector_shape(
        &mut self,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, Diagnostic> {
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
    ) -> Result<FieldLivenessShape, Diagnostic> {
        let flag = LivenessFlagId(
            u32::try_from(self.flags.len())
                .map_err(|_| cleanup_error("too many cleanup liveness flags"))?,
        );
        if self.flags.len() >= MAX_CLEANUP_OWNED_LEAVES {
            return Err(cleanup_error("selected inventory exceeds owned-leaf limit"));
        }
        self.physical
            .as_ref()
            .expect("selected shape")
            .borrow_mut()
            .reserve(id.text().len())
            .map_err(|_| cleanup_error("selected inventory lifecycle reservation refused"))?;
        let lifecycle = DeclarationId::compiler(id);
        let flag_lifecycle = self.clone_owned(&lifecycle)?;
        let mut projections = Vec::new();
        if let Some(projection) = projection {
            reserve(&self.physical, &mut projections, 1)?;
            projections.push(self.clone_owned(projection)?);
        }
        reserve(&self.physical, &mut self.flags, 1)?;
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
}
