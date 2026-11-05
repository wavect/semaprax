//! Independently reconstructed cleanup inventory, with an explicit selected
//! physical constructor boundary. Legacy construction retains its old behavior.
use super::*;
use crate::cleanup_plan::{ReplayClonePayload, ReplayPhysicalReservation, replay_reserve_vec};
use std::cell::RefCell;
mod selected;
pub(crate) mod selected_inline;

pub(crate) fn selected_vector_element<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<Option<&'a ResolvedType>, Diagnostic> {
    selected::vector::element(program, ty)
}
pub(crate) fn prepay_selected_collection_probes(
    program: &dyn crate::cleanup::ReplayContext,
    query_bytes: usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    selected::vector::prepay(program, query_bytes, charge)
}

pub(crate) fn authenticate_selected_vector(
    program: &dyn crate::cleanup::ReplayContext,
    id: &DeclarationId,
) -> Result<(), Diagnostic> {
    selected::vector::authenticate(program, id)
}

pub(crate) fn selected_inventory_needs_drop(ty: &ResolvedType) -> Result<bool, Diagnostic> {
    selected::needs_drop(ty)
}

pub(crate) fn selected_flat_record_fields<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<
    Option<(
        &'a DeclarationId,
        &'a [crate::hir::ResolvedFieldDeclaration],
    )>,
    Diagnostic,
> {
    selected::flat_record::fields(program, ty)
}

pub(crate) fn selected_flat_record_declaration<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    declaration: &'a DeclarationId,
) -> Result<
    Option<(
        &'a DeclarationId,
        &'a [crate::hir::ResolvedFieldDeclaration],
    )>,
    Diagnostic,
> {
    selected::flat_record::declaration_fields(program, declaration)
}

pub(crate) fn selected_inventory_needs_drop_in(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
) -> Result<bool, Diagnostic> {
    selected::needs_drop_in(program, ty)
}

pub(crate) fn build_inventory(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<CleanupInventory, Diagnostic> {
    build_inventory_inner(program, function, None, false, None, false)
}

/// Selected physical constructor only: direct primitive owned resources and
/// Copy/borrowed traversal. Nominal carriers, variants, Vec/Box/generic
/// carriers, function types and type parameters refuse before delegated type
/// classification. Constructor support grants no retained replay admission.
#[cfg(test)]
pub(crate) fn build_inventory_with_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: &'a mut ReplayPhysicalReservation,
) -> Result<CleanupInventory, Diagnostic> {
    build_inventory_inner(
        program,
        function,
        Some(RefCell::new(physical)),
        false,
        None,
        false,
    )
}

// Selected source-bound replay uses this independently classified direct
// record constructor; it never accepts a supplied shape or layout verdict.
#[cfg(test)]
pub(crate) fn build_flat_record_inventory_with_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: &'a mut ReplayPhysicalReservation,
) -> Result<CleanupInventory, Diagnostic> {
    build_inventory_inner(
        program,
        function,
        Some(RefCell::new(physical)),
        true,
        None,
        false,
    )
}

pub(crate) fn build_flat_record_inventory_with_budget<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: &'a mut ReplayPhysicalReservation,
    work: &'a mut dyn FnMut(usize) -> Result<(), Diagnostic>,
) -> Result<CleanupInventory, Diagnostic> {
    build_inventory_inner(
        program,
        function,
        Some(RefCell::new(physical)),
        true,
        Some(RefCell::new(work)),
        true,
    )
}

// Inactive receiver until selected schema/profile and every receiving site have
// been closed over one shared session. This authenticates Copy Option metadata
// itself and refuses other nominal carriers before delegated type classification.
#[cfg(test)]
pub(crate) fn build_borrowed_helper_inventory_with_budget<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: &'a mut ReplayPhysicalReservation,
    work: &'a mut dyn FnMut(usize) -> Result<(), Diagnostic>,
) -> Result<CleanupInventory, Diagnostic> {
    build_inventory_inner(
        program,
        function,
        Some(RefCell::new(physical)),
        true,
        Some(RefCell::new(work)),
        false,
    )
}

/// Optional per-unit work charge shared by one selected inventory build.
type InventoryWork<'a> = Option<RefCell<&'a mut dyn FnMut(usize) -> Result<(), Diagnostic>>>;

fn build_inventory_inner<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    physical: Option<RefCell<&'a mut ReplayPhysicalReservation>>,
    flat_records: bool,
    work: InventoryWork<'a>,
    collection_work: bool,
) -> Result<CleanupInventory, Diagnostic> {
    let mut builder = InventoryBuilder {
        collection_work,
        work,
        physical,
        flat_records,
        program,
        slots: Vec::new(),
        flags: Vec::new(),
        live_owned_parameters: Vec::new(),
        conditional_owned_parameters: Vec::new(),
    };

    for (parameter_index, parameter) in function.params.iter().enumerate() {
        builder.charge(1)?;
        if parameter.ownership == OwnershipMode::Own && builder.needs_drop(&parameter.ty)? {
            let storage = builder.add_slot(
                CleanupStorageOrigin::Parameter {
                    value: builder.clone_owned(&parameter.id)?,
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| cleanup_error("too many function parameters"))?,
                },
                builder.clone_owned(&parameter.ty)?,
            )?;
            if let Some(entry) = builder.conditional_entry(storage)? {
                selected::reserve(
                    &builder.physical,
                    &mut builder.conditional_owned_parameters,
                    1,
                )?;
                builder.conditional_owned_parameters.push(entry);
            } else {
                selected::reserve(&builder.physical, &mut builder.live_owned_parameters, 1)?;
                builder.live_owned_parameters.push(storage);
            }
        }
    }

    for expression in &function.requires {
        builder.collect_expression(expression)?;
    }
    builder.collect_expression(&function.body)?;
    if builder.needs_drop(&function.return_type)? {
        builder.add_slot(
            CleanupStorageOrigin::ProvisionalResult {
                value: builder.clone_owned(&function.result_id)?,
            },
            builder.clone_owned(&function.return_type)?,
        )?;
    }
    for expression in &function.ensures {
        builder.collect_expression(expression)?;
    }

    #[cfg(test)]
    if builder.physical.is_none() {
        note_capacity_high_water(inventory_builder_live_capacity(&builder));
    }

    let schema = if builder.slots.iter().any(|slot| {
        if builder.physical.is_some() {
            matches!(slot.shape, FieldLivenessShape::Variant { .. })
        } else {
            shape_contains_variant(&slot.shape)
        }
    }) {
        CLEANUP_INVENTORY_SCHEMA_V2
    } else {
        CLEANUP_INVENTORY_SCHEMA_V1
    };
    Ok(CleanupInventory {
        schema,
        entry_state: CleanupEntryState {
            live_owned_parameters: builder.live_owned_parameters,
            conditional_owned_parameters: builder.conditional_owned_parameters,
        },
        slots: builder.slots,
        flags: builder.flags,
    })
}

pub(super) struct InventoryBuilder<'a> {
    pub(super) work: InventoryWork<'a>,
    pub(super) collection_work: bool,
    pub(super) flat_records: bool,
    pub(super) physical: Option<RefCell<&'a mut ReplayPhysicalReservation>>,
    pub(super) program: &'a dyn crate::cleanup::ReplayContext,
    pub(super) slots: Vec<CleanupStorageSlot>,
    pub(super) flags: Vec<CleanupFlag>,
    pub(super) live_owned_parameters: Vec<CleanupStorageId>,
    pub(super) conditional_owned_parameters: Vec<ConditionalVariantEntry>,
}

impl InventoryBuilder<'_> {
    pub(super) fn charge(&self, units: usize) -> Result<(), Diagnostic> {
        if let Some(work) = &self.work {
            work.borrow_mut()(units)?;
        }
        Ok(())
    }
    pub(super) fn prepay_collection(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        if !self.collection_work || !matches!(ty, ResolvedType::Nominal { .. }) {
            return Ok(());
        }
        let query = selected::vector::key_bytes(ty)?;
        selected::vector::prepay(self.program, query, &mut |units| self.charge(units))
    }
    pub(super) fn needs_drop(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        if let Some(work) = &self.work {
            if crate::hir::borrowed_scalar::option_u8::is_type_metered(ty, &mut |units| {
                work.borrow_mut()(units)
            })? {
                return if crate::hir::borrowed_scalar::option_u8::authenticated_metered(
                    self.program.declarations(),
                    ty,
                    &mut |units| work.borrow_mut()(units),
                )? {
                    Ok(false)
                } else {
                    Err(cleanup_error("selected Option metadata is unauthenticated"))
                };
            }
            if self.collection_work {
                self.prepay_collection(ty)?;
                return selected::needs_drop_in(self.program, ty);
            }
            return selected::needs_drop(ty);
        }
        if self.physical.is_some() {
            return if self.flat_records {
                selected::needs_drop_in(self.program, ty)
            } else {
                selected::needs_drop(ty)
            };
        }
        type_needs_resource_cleanup(self.program, ty).map_err(cleanup_error)
    }

    fn add_slot(
        &mut self,
        origin: CleanupStorageOrigin,
        ty: ResolvedType,
    ) -> Result<CleanupStorageId, Diagnostic> {
        let index = u32::try_from(self.slots.len())
            .map_err(|_| cleanup_error("too many cleanup storage candidates"))?;
        let storage = CleanupStorageId(index);
        let shape = self.shape_for_type(&ty, storage)?;
        selected::reserve(&self.physical, &mut self.slots, 1)?;
        self.slots.push(CleanupStorageSlot {
            id: storage,
            discovery_index: index,
            origin,
            ty,
            shape,
        });
        Ok(storage)
    }

    fn conditional_entry(
        &self,
        storage: CleanupStorageId,
    ) -> Result<Option<ConditionalVariantEntry>, Diagnostic> {
        let slot = self
            .slots
            .iter()
            .find(|slot| slot.id == storage)
            .ok_or_else(|| cleanup_error("conditional entry references unknown storage"))?;
        let FieldLivenessShape::Variant { declaration, cases } = &slot.shape else {
            return Ok(None);
        };
        let mut conditional_cases = Vec::with_capacity(cases.len());
        for case in cases {
            let mut live_flags = Vec::new();
            for field in &case.fields {
                collect_shape_flags(&field.shape, &mut live_flags);
            }
            conditional_cases.push(ConditionalVariantCase {
                case: case.case.clone(),
                live_flags,
            });
        }
        Ok(Some(ConditionalVariantEntry {
            storage,
            variant: declaration.clone(),
            cases: conditional_cases,
        }))
    }

    fn shape_for_type(
        &mut self,
        ty: &ResolvedType,
        storage: CleanupStorageId,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        if self.physical.is_some() {
            return self.selected_shape(ty, storage);
        }
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Children(
                &'a DeclarationId,
                &'a [crate::hir::ResolvedFieldDeclaration],
                usize,
            ),
            FinishField(&'a crate::hir::ResolvedFieldDeclaration),
            FinishRecord(&'a DeclarationId, usize),
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 40) };

        let mut frames = vec![Frame::Enter(ty)];
        let mut projections = Vec::<DeclarationId>::new();
        let mut shapes = Vec::new();
        while let Some(frame) = frames.pop() {
            self.charge(1)?;
            #[cfg(test)]
            if self.physical.is_none() {
                note_capacity_high_water(
                    frames.capacity() * std::mem::size_of::<Frame<'_>>()
                        + projections.capacity() * std::mem::size_of::<DeclarationId>()
                        + projections
                            .iter()
                            .map(|id| id.as_str().len())
                            .sum::<usize>()
                        + shapes.capacity() * std::mem::size_of::<FieldLivenessShape>()
                        + self.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
                        + self.flags.capacity() * std::mem::size_of::<CleanupFlag>()
                        + self.live_owned_parameters.capacity()
                            * std::mem::size_of::<CleanupStorageId>()
                        + self.slots.iter().map(slot_owned_capacity).sum::<usize>()
                        + self.flags.iter().map(flag_owned_capacity).sum::<usize>()
                        + shapes.iter().map(shape_owned_capacity).sum::<usize>(),
                );
            }
            match frame {
                Frame::Enter(ty) => {
                    if !self.needs_drop(ty)? {
                        shapes.push(FieldLivenessShape::NoDrop);
                        continue;
                    }
                    if let Some(id) = primitive_leaf_lifecycle(ty) {
                        let flag_index = u32::try_from(self.flags.len())
                            .map_err(|_| cleanup_error("too many cleanup liveness flags"))?;
                        let flag = LivenessFlagId(flag_index);
                        let lifecycle = DeclarationId::new(id);
                        self.flags.push(CleanupFlag {
                            id: flag,
                            place: CleanupPlace {
                                storage,
                                projections: projections.clone(),
                            },
                            lifecycle: lifecycle.clone(),
                        });
                        shapes.push(FieldLivenessShape::Leaf { flag, lifecycle });
                        continue;
                    }
                    if is_owned_bounded_vec_type(ty)
                        || crate::hir::owned_record_collection::is_owned_record_vec_type(
                            self.program.declarations(),
                            ty,
                        )
                        || crate::hir::owned_leaf_collection::is_copy_or_leaf_vec(
                            self.program.declarations(),
                            ty,
                        )
                        || crate::iterator_ops::is_iter(ty)
                    {
                        let flag_index = u32::try_from(self.flags.len())
                            .map_err(|_| cleanup_error("too many cleanup liveness flags"))?;
                        let flag = LivenessFlagId(flag_index);
                        let lifecycle = DeclarationId::new(if crate::iterator_ops::is_iter(ty) {
                            ITER_DROP_LIFECYCLE_ID
                        } else {
                            VEC_DROP_LIFECYCLE_ID
                        });
                        self.flags.push(CleanupFlag {
                            id: flag,
                            place: CleanupPlace {
                                storage,
                                projections: projections.clone(),
                            },
                            lifecycle: lifecycle.clone(),
                        });
                        shapes.push(FieldLivenessShape::Leaf { flag, lifecycle });
                        continue;
                    }
                    if is_owned_bounded_box_type(ty) {
                        let flag_index = u32::try_from(self.flags.len())
                            .map_err(|_| cleanup_error("too many cleanup liveness flags"))?;
                        let flag = LivenessFlagId(flag_index);
                        let lifecycle = DeclarationId::new(BOX_DROP_LIFECYCLE_ID);
                        self.flags.push(CleanupFlag {
                            id: flag,
                            place: CleanupPlace {
                                storage,
                                projections: projections.clone(),
                            },
                            lifecycle: lifecycle.clone(),
                        });
                        shapes.push(FieldLivenessShape::Leaf { flag, lifecycle });
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        return Err(cleanup_error(format!(
                            "droppable type `{}` is not nominal",
                            ty.identity_key()
                        )));
                    };
                    let declaration_item = self
                        .program
                        .types()
                        .iter()
                        .find(|item| item.id == *declaration)
                        .ok_or_else(|| {
                            cleanup_error(format!("unknown cleanup type `{declaration}`"))
                        })?;
                    match &declaration_item.kind {
                        ResolvedTypeDeclarationKind::Resource { drop } => {
                            if !arguments.is_empty() {
                                return Err(cleanup_error(format!(
                                    "droppable type `{}` has unsupported generic arguments",
                                    ty.identity_key()
                                )));
                            }
                            let flag_index = u32::try_from(self.flags.len())
                                .map_err(|_| cleanup_error("too many cleanup liveness flags"))?;
                            let flag = LivenessFlagId(flag_index);
                            self.flags.push(CleanupFlag {
                                id: flag,
                                place: CleanupPlace {
                                    storage,
                                    projections: projections.clone(),
                                },
                                lifecycle: drop.id.clone(),
                            });
                            shapes.push(FieldLivenessShape::Leaf {
                                flag,
                                lifecycle: drop.id.clone(),
                            });
                        }
                        ResolvedTypeDeclarationKind::Record { fields }
                        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                            if !arguments.is_empty() {
                                if !matches!(
                                    &declaration_item.kind,
                                    ResolvedTypeDeclarationKind::Record { .. }
                                ) {
                                    return Err(cleanup_error(format!(
                                        "droppable type `{}` has unsupported generic arguments",
                                        ty.identity_key()
                                    )));
                                }
                                shapes.push(self.shape_for_concrete_generic_record(
                                    ty,
                                    storage,
                                    &projections,
                                )?);
                                continue;
                            }
                            frames.try_reserve(2).map_err(|_| {
                                cleanup_error("cleanup shape capacity exceeds address space")
                            })?;
                            frames.push(Frame::FinishRecord(declaration, fields.len()));
                            frames.push(Frame::Children(declaration, fields, 0));
                        }
                        ResolvedTypeDeclarationKind::Variant { cases } => {
                            let mut case_shapes = Vec::with_capacity(cases.len());
                            for case in cases {
                                let mut fields = Vec::with_capacity(case.fields.len());
                                for field in &case.fields {
                                    let field_ty = crate::hir::substitute_type(
                                        &field.ty,
                                        declaration,
                                        arguments,
                                    )?;
                                    let shape = if variant_record_field(
                                        self.program,
                                        ty,
                                        &case.id,
                                        &field.id,
                                        &field_ty,
                                    ) {
                                        let mut nested = projections.clone();
                                        nested.push(case.id.clone());
                                        nested.push(field.id.clone());
                                        self.shape_for_concrete_generic_record(
                                            &field_ty, storage, &nested,
                                        )?
                                    } else if self.needs_drop(&field_ty)? {
                                        let leaf_lifecycle = variant_leaf_lifecycle(self.program, ty, &case.id, &field.id, &field_ty)
                                            .ok_or_else(|| cleanup_error("droppable variant field is outside its admitted cleanup profile"))?;
                                        let flag_index =
                                            u32::try_from(self.flags.len()).map_err(|_| {
                                                cleanup_error("too many cleanup liveness flags")
                                            })?;
                                        let flag = LivenessFlagId(flag_index);
                                        let lifecycle = DeclarationId::new(leaf_lifecycle);
                                        let mut leaf_projections = projections.clone();
                                        leaf_projections.push(case.id.clone());
                                        leaf_projections.push(field.id.clone());
                                        self.flags.push(CleanupFlag {
                                            id: flag,
                                            place: CleanupPlace {
                                                storage,
                                                projections: leaf_projections,
                                            },
                                            lifecycle: lifecycle.clone(),
                                        });
                                        FieldLivenessShape::Leaf { flag, lifecycle }
                                    } else {
                                        FieldLivenessShape::NoDrop
                                    };
                                    fields.push(FieldLiveness {
                                        field: field.id.clone(),
                                        field_index: field.index,
                                        shape,
                                    });
                                }
                                case_shapes.push(VariantCaseLiveness {
                                    case: case.id.clone(),
                                    case_index: case.index,
                                    fields,
                                });
                            }
                            shapes.push(FieldLivenessShape::Variant {
                                declaration: declaration.clone(),
                                cases: case_shapes,
                            });
                        }
                    }
                }
                Frame::Children(declaration, fields, index) => {
                    let Some(field) = fields.get(index) else {
                        continue;
                    };
                    frames.try_reserve(3).map_err(|_| {
                        cleanup_error("cleanup shape capacity exceeds address space")
                    })?;
                    frames.push(Frame::Children(declaration, fields, index + 1));
                    frames.push(Frame::FinishField(field));
                    projections.push(field.id.clone());
                    frames.push(Frame::Enter(&field.ty));
                }
                Frame::FinishField(field) => {
                    let shape = shapes
                        .pop()
                        .ok_or_else(|| cleanup_error("cleanup field shape is absent"))?;
                    if projections.pop().as_ref() != Some(&field.id) {
                        return Err(cleanup_error("cleanup projection stack is inconsistent"));
                    }
                    shapes.push(FieldLivenessShape::Record {
                        declaration: field.id.clone(),
                        fields: vec![FieldLiveness {
                            field: field.id.clone(),
                            field_index: field.index,
                            shape,
                        }],
                    });
                }
                Frame::FinishRecord(declaration, field_count) => {
                    let split = shapes
                        .len()
                        .checked_sub(field_count)
                        .ok_or_else(|| cleanup_error("cleanup record shapes are incomplete"))?;
                    let mut fields = Vec::with_capacity(field_count);
                    if fields.capacity() != field_count {
                        return Err(cleanup_error("cleanup record field capacity is not exact"));
                    }
                    for shape in shapes.drain(split..) {
                        let field = match shape {
                            FieldLivenessShape::Record {
                                declaration: field,
                                mut fields,
                            } if fields.len() == 1 && fields[0].field == field => fields.remove(0),
                            _ => unreachable!("field wrapper is internal to shape construction"),
                        };
                        if fields.len() == fields.capacity() {
                            return Err(cleanup_error(
                                "cleanup record field capacity was exhausted",
                            ));
                        }
                        fields.push(field);
                    }
                    if fields.len() != field_count || fields.capacity() != field_count {
                        return Err(cleanup_error(
                            "cleanup record field capacity disagrees with its shape",
                        ));
                    }
                    #[cfg(test)]
                    note_capacity_high_water(
                        frames.capacity() * std::mem::size_of::<Frame<'_>>()
                            + projections.capacity() * std::mem::size_of::<DeclarationId>()
                            + projections
                                .iter()
                                .map(|id| id.as_str().len())
                                .sum::<usize>()
                            + shapes.capacity() * std::mem::size_of::<FieldLivenessShape>()
                            + shapes.iter().map(shape_owned_capacity).sum::<usize>()
                            + fields.capacity() * std::mem::size_of::<FieldLiveness>()
                            + fields
                                .iter()
                                .map(|field| {
                                    field.field.as_str().len() + shape_owned_capacity(&field.shape)
                                })
                                .sum::<usize>()
                            + declaration.as_str().len(),
                    );
                    shapes.push(FieldLivenessShape::Record {
                        declaration: declaration.clone(),
                        fields,
                    });
                }
            }
        }
        if shapes.len() != 1 || !projections.is_empty() {
            return Err(cleanup_error("cleanup shape traversal did not settle"));
        }
        let shape = shapes.pop().expect("shape count checked above");
        cleanup_shape_profile(&shape)?;
        Ok(shape)
    }

    fn collect_expression(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedExpr),
            Children(&'a ResolvedExpr, usize),
            Finish(&'a ResolvedExpr),
            AddBinding(&'a ResolvedBinding),
            AddPatternBindings(&'a crate::hir::ResolvedMatchPattern),
            AddUpdateBase(&'a ResolvedExpr),
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 24) };

        let mut frames = if self.physical.is_some() {
            self.singleton(Frame::Enter(expression))?
        } else {
            vec![Frame::Enter(expression)]
        };
        while let Some(frame) = frames.pop() {
            self.charge(1)?;
            #[cfg(test)]
            if self.physical.is_none() {
                note_capacity_high_water(
                    frames.capacity() * std::mem::size_of::<Frame<'_>>()
                        + self.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
                        + self.flags.capacity() * std::mem::size_of::<CleanupFlag>()
                        + self.slots.iter().map(slot_owned_capacity).sum::<usize>()
                        + self.flags.iter().map(flag_owned_capacity).sum::<usize>(),
                );
            }
            match frame {
                Frame::Enter(expression) => {
                    if self.work.is_some() {
                        self.needs_drop(&expression.ty)?;
                    }
                    selected::reserve_legacy(
                        &self.physical,
                        &mut frames,
                        2,
                        "cleanup traversal capacity exceeds address space",
                    )?;
                    frames.push(Frame::Finish(expression));
                    frames.push(Frame::Children(expression, 0));
                }
                Frame::Children(expression, index) => {
                    selected::reserve_legacy(
                        &self.physical,
                        &mut frames,
                        2,
                        "cleanup traversal capacity exceeds address space",
                    )?;
                    let mut enter = None;
                    let mut action = None;
                    match &expression.kind {
                        ResolvedExprKind::Closure { captures, .. } => {
                            enter = captures.get(index).map(|capture| &capture.value);
                        }
                        ResolvedExprKind::FunctionReference { .. } => {}
                        ResolvedExprKind::Invoke { callable, args } => {
                            enter = if index == 0 {
                                Some(callable.as_ref())
                            } else {
                                args.get(index - 1)
                            };
                        }
                        ResolvedExprKind::Call { args, .. }
                        | ResolvedExprKind::LiteralFormat { args, .. }
                        | ResolvedExprKind::VecFieldRead { args, .. }
                        | ResolvedExprKind::RowLoan { args, .. } => {
                            enter = args.get(index);
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::HostCommandCall(call) => {
                            enter = call.args.get(index);
                        }
                        ResolvedExprKind::ByteRange {
                            source, start, end, ..
                        } => {
                            enter = [source.as_ref(), start.as_ref(), end.as_ref()]
                                .get(index)
                                .copied();
                        }
                        ResolvedExprKind::Unary { value, .. }
                        | ResolvedExprKind::Project { base: value, .. }
                        | ResolvedExprKind::Try { operand: value, .. }
                        | ResolvedExprKind::TryOption { operand: value, .. }
                        | ResolvedExprKind::Upcast { source: value }
                        | ResolvedExprKind::Yield { request: value } => {
                            enter = (index == 0).then_some(value.as_ref());
                        }
                        ResolvedExprKind::LexicalExit { value, .. } => {
                            enter = value.as_deref().filter(|_| index == 0);
                        }
                        ResolvedExprKind::Binary { left, right, .. } => {
                            enter = match index {
                                0 => Some(left),
                                1 => Some(right),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::Block { statements, tail }
                        | ResolvedExprKind::VecConstructor {
                            statements, tail, ..
                        } => {
                            // Only lets add a trailing binding step; other statements keep walking.
                            let per_statement = |statement: &ResolvedStatement| {
                                statement.child_count().saturating_add(usize::from(matches!(
                                    statement,
                                    ResolvedStatement::Let { .. }
                                )))
                            };
                            let mut offset = 0usize;
                            let mut matched = None;
                            for statement in statements.iter() {
                                self.charge(1)?;
                                let steps = per_statement(statement);
                                if index < offset + steps {
                                    let within = index - offset;
                                    if within < statement.child_count() {
                                        enter = statement.child(within);
                                    }
                                    // Only `let` statements carry bindings;
                                    // assignment, unsafe, and while
                                    // statements add none here.
                                    if within == statement.child_count() {
                                        if let ResolvedStatement::Let { binding, .. } = statement {
                                            action = Some(Frame::AddBinding(binding));
                                        }
                                    }
                                    matched = Some(());
                                    break;
                                }
                                offset += steps;
                            }
                            if matched.is_none() && index == offset {
                                enter = Some(tail);
                            }
                        }
                        ResolvedExprKind::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            enter = match index {
                                0 => Some(condition),
                                1 => Some(then_branch),
                                2 => Some(else_branch),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::ConstructRecord { fields, .. }
                        | ResolvedExprKind::ConstructVariant { fields, .. } => {
                            enter = fields.get(index).map(|field| &field.value);
                        }
                        ResolvedExprKind::Match {
                            scrutinee, arms, ..
                        } => {
                            enter = if index == 0 {
                                Some(scrutinee)
                            } else {
                                let mut cursor = index - 1;
                                let mut found = None;
                                for arm in arms {
                                    self.charge(1)?;
                                    if cursor == 0 {
                                        action = Some(Frame::AddPatternBindings(&arm.pattern));
                                        break;
                                    }
                                    cursor -= 1;
                                    if let Some(guard) = &arm.guard {
                                        if cursor == 0 {
                                            found = Some(guard.as_ref());
                                            break;
                                        }
                                        cursor -= 1;
                                    }
                                    if cursor == 0 {
                                        found = Some(&arm.value);
                                        break;
                                    }
                                    cursor -= 1;
                                }
                                found
                            };
                        }
                        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                            if index == 0 {
                                enter = Some(base);
                            } else if index == 1 {
                                action = Some(Frame::AddUpdateBase(base));
                            } else {
                                enter = fields.get(index - 2).map(|field| &field.value);
                            }
                        }
                        ResolvedExprKind::Int(_)
                        | ResolvedExprKind::Int32(_)
                        | ResolvedExprKind::Char(_)
                        | ResolvedExprKind::Uint8(_)
                        | ResolvedExprKind::Usize(_)
                        | ResolvedExprKind::ArrayU8(_)
                        | ResolvedExprKind::RepeatArrayU8 { .. }
                        | ResolvedExprKind::Float32(_)
                        | ResolvedExprKind::Float64(_)
                        | ResolvedExprKind::Bool(_)
                        | ResolvedExprKind::String(_)
                        | ResolvedExprKind::BorrowedUtf8Literal(_)
                        | ResolvedExprKind::Place(_)
                        | ResolvedExprKind::BorrowPlace { .. } => {}
                    }
                    if enter.is_some() || action.is_some() {
                        frames.push(Frame::Children(expression, index + 1));
                        if let Some(action) = action {
                            frames.push(action);
                        }
                        if let Some(child) = enter {
                            frames.push(Frame::Enter(child));
                        }
                    }
                }
                Frame::Finish(expression) => self.collect_owned_temporary(expression)?,
                Frame::AddBinding(binding) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: self.clone_owned(&binding.id)?,
                            },
                            self.clone_owned(&binding.ty)?,
                        )?;
                    }
                }
                Frame::AddPatternBindings(pattern) => {
                    self.collect_pattern_bindings(pattern)?;
                }
                Frame::AddUpdateBase(base) => {
                    // A place expression normally needs no temporary. Record
                    // update deliberately materializes an owned base epoch.
                    if matches!(base.kind, ResolvedExprKind::Place(_))
                        && base.ownership == OwnershipMode::Own
                        && self.needs_drop(&base.ty)?
                    {
                        self.add_slot(
                            CleanupStorageOrigin::Temporary {
                                expression: self.clone_owned(&base.id)?,
                            },
                            self.clone_owned(&base.ty)?,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_pattern_bindings(
        &mut self,
        pattern: &crate::hir::ResolvedMatchPattern,
    ) -> Result<(), Diagnostic> {
        use crate::hir::{ResolvedMatchPattern, ResolvedRecordMatchFieldPattern};
        if self.work.is_some() {
            self.charge(1)?;
            match pattern {
                ResolvedMatchPattern::Variant { .. }
                    if crate::hir::borrowed_scalar::option_u8::pattern(pattern) => {}
                ResolvedMatchPattern::Binding(binding)
                    if binding.ownership == OwnershipMode::Value
                        && crate::hir::is_scalar_resolved_type(&binding.ty) => {}
                ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
                _ => {
                    return Err(cleanup_error(
                        "budgeted helper inventory pattern is outside exact Copy metadata",
                    ));
                }
            }
        }

        enum Item<'a> {
            Pattern(&'a ResolvedMatchPattern),
            RecordField(&'a ResolvedRecordMatchFieldPattern),
        }
        let mut pending = if self.physical.is_some() {
            self.singleton(Item::Pattern(pattern))?
        } else {
            vec![Item::Pattern(pattern)]
        };
        while let Some(item) = pending.pop() {
            self.charge(1)?;
            match item {
                Item::Pattern(ResolvedMatchPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: self.clone_owned(&binding.id)?,
                            },
                            self.clone_owned(&binding.ty)?,
                        )?;
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Record { record, fields, .. }) => {
                    let declared = self
                        .program
                        .declarations()
                        .record_fields(record)
                        .ok_or_else(|| cleanup_error("record pattern has no field inventory"))?;
                    for declaration in declared.iter().rev() {
                        let field = fields
                            .iter()
                            .find(|field| field.field == declaration.id)
                            .ok_or_else(|| {
                                cleanup_error("record pattern field inventory is incomplete")
                            })?;
                        selected::reserve(&self.physical, &mut pending, 1)?;
                        pending.push(Item::RecordField(&field.pattern));
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Variant { fields, .. }) => {
                    for field in fields {
                        self.charge(1)?;
                        if field.binding.ownership == OwnershipMode::Own
                            && self.needs_drop(&field.binding.ty)?
                        {
                            self.add_slot(
                                CleanupStorageOrigin::Binding {
                                    value: self.clone_owned(&field.binding.id)?,
                                },
                                self.clone_owned(&field.binding.ty)?,
                            )?;
                        }
                    }
                }
                Item::Pattern(
                    ResolvedMatchPattern::Wildcard
                    | ResolvedMatchPattern::Literal(_)
                    | ResolvedMatchPattern::Or(_),
                ) => {}
                Item::RecordField(ResolvedRecordMatchFieldPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: self.clone_owned(&binding.id)?,
                            },
                            self.clone_owned(&binding.ty)?,
                        )?;
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields,
                    ..
                }) => {
                    let declared = self
                        .program
                        .declarations()
                        .record_fields(record)
                        .ok_or_else(|| {
                            cleanup_error("nested record pattern has no field inventory")
                        })?;
                    for declaration in declared.iter().rev() {
                        let field = fields
                            .iter()
                            .find(|field| field.field == declaration.id)
                            .ok_or_else(|| {
                                cleanup_error("nested record pattern field inventory is incomplete")
                            })?;
                        selected::reserve(&self.physical, &mut pending, 1)?;
                        pending.push(Item::RecordField(&field.pattern));
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Wildcard) => {}
            }
        }
        Ok(())
    }

    fn collect_owned_temporary(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        if expression.ownership == OwnershipMode::Own
            && self.needs_drop(&expression.ty)?
            && (expression.ty == ResolvedType::String
                || !matches!(expression.kind, ResolvedExprKind::Place(_)))
        {
            self.add_slot(
                CleanupStorageOrigin::Temporary {
                    expression: self.clone_owned(&expression.id)?,
                },
                self.clone_owned(&expression.ty)?,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
