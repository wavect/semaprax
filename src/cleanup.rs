//! Deterministic cleanup storage inventory.
//!
//! This is deliberately smaller than the executable `CleanupPlan` specified by
//! RFC 0003. It catalogs every storage candidate and resource leaf that a later
//! control-flow plan must govern. Beyond owned-parameter entry state, it does
//! not describe path-sensitive liveness, transfers, initialization/finalization
//! order, failure status, or exits.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind, ValueId,
};

#[cfg(test)]
thread_local! {
    static INVENTORY_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_capacity_high_water() {
    INVENTORY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(crate) fn capacity_high_water() -> usize {
    INVENTORY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_capacity_high_water(bytes: usize) {
    INVENTORY_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => 0,
        ResolvedType::String | ResolvedType::Bytes | ResolvedType::Str | ResolvedType::SliceU8 => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            declaration.as_str().len()
                + arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn shape_owned_capacity(shape: &FieldLivenessShape) -> usize {
    let mut total = 0usize;
    let mut pending = vec![shape];
    while let Some(shape) = pending.pop() {
        match shape {
            FieldLivenessShape::NoDrop => {}
            FieldLivenessShape::Leaf { lifecycle, .. } => {
                total = total.saturating_add(lifecycle.as_str().len());
            }
            FieldLivenessShape::Record {
                declaration,
                fields,
            } => {
                total = total
                    .saturating_add(declaration.as_str().len())
                    .saturating_add(
                        fields
                            .capacity()
                            .saturating_mul(std::mem::size_of::<FieldLiveness>()),
                    );
                for field in fields {
                    total = total.saturating_add(field.field.as_str().len());
                    pending.push(&field.shape);
                }
            }
            FieldLivenessShape::Variant { declaration, cases } => {
                total = total
                    .saturating_add(declaration.as_str().len())
                    .saturating_add(
                        cases
                            .capacity()
                            .saturating_mul(std::mem::size_of::<VariantCaseLiveness>()),
                    );
                for case in cases {
                    total = total
                        .saturating_add(case.case.as_str().len())
                        .saturating_add(
                            case.fields
                                .capacity()
                                .saturating_mul(std::mem::size_of::<FieldLiveness>()),
                        );
                    for field in &case.fields {
                        total = total.saturating_add(field.field.as_str().len());
                        pending.push(&field.shape);
                    }
                }
            }
        }
    }
    total
}

#[cfg(test)]
fn slot_owned_capacity(slot: &CleanupStorageSlot) -> usize {
    let origin = match &slot.origin {
        CleanupStorageOrigin::Parameter { value, .. }
        | CleanupStorageOrigin::Binding { value }
        | CleanupStorageOrigin::ProvisionalResult { value } => value.as_str().len(),
        CleanupStorageOrigin::Temporary { expression } => expression.as_str().len(),
    };
    origin + resolved_type_owned_capacity(&slot.ty) + shape_owned_capacity(&slot.shape)
}

#[cfg(test)]
fn flag_owned_capacity(flag: &CleanupFlag) -> usize {
    flag.lifecycle.as_str().len()
        + flag.place.projections.capacity() * std::mem::size_of::<DeclarationId>()
        + flag
            .place
            .projections
            .iter()
            .map(|id| id.as_str().len())
            .sum::<usize>()
}

#[cfg(test)]
fn inventory_builder_live_capacity(builder: &InventoryBuilder<'_>) -> usize {
    builder.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
        + builder.slots.iter().map(slot_owned_capacity).sum::<usize>()
        + builder.flags.capacity() * std::mem::size_of::<CleanupFlag>()
        + builder.flags.iter().map(flag_owned_capacity).sum::<usize>()
        + builder.live_owned_parameters.capacity() * std::mem::size_of::<CleanupStorageId>()
        + builder.conditional_owned_parameters.capacity()
            * std::mem::size_of::<ConditionalVariantEntry>()
}

pub const CLEANUP_INVENTORY_SCHEMA_V1: &str = "semaprax.cleanup-inventory.v1";
pub const CLEANUP_INVENTORY_SCHEMA_V2: &str = "semaprax.cleanup-inventory.v2";
pub(crate) const MAX_CLEANUP_SHAPE_DEPTH: usize = 64;
pub(crate) const MAX_CLEANUP_OWNED_LEAVES: usize = 256;
pub(crate) const MAX_CLEANUP_VISITED_FIELDS: usize = 4_096;
/// Canonical compiler-owned lifecycle for one uniquely owned `Bytes` payload.
///
/// This identity is derived from the primitive type by both the inventory and
/// CleanupPlan replay. It is never supplied by source or backend metadata.
pub const BYTES_DROP_LIFECYCLE_ID: &str = "core.bytes.drop";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CleanupStorageId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LivenessFlagId(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupInventory {
    pub schema: &'static str,
    pub entry_state: CleanupEntryState,
    pub slots: Vec<CleanupStorageSlot>,
    pub flags: Vec<CleanupFlag>,
}

impl CleanupInventory {
    pub(crate) fn unresolved() -> Self {
        Self {
            schema: CLEANUP_INVENTORY_SCHEMA_V1,
            entry_state: CleanupEntryState {
                live_owned_parameters: Vec::new(),
                conditional_owned_parameters: Vec::new(),
            },
            slots: Vec::new(),
            flags: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupEntryState {
    pub live_owned_parameters: Vec<CleanupStorageId>,
    pub conditional_owned_parameters: Vec<ConditionalVariantEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalVariantEntry {
    pub storage: CleanupStorageId,
    pub variant: DeclarationId,
    pub cases: Vec<ConditionalVariantCase>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalVariantCase {
    pub case: DeclarationId,
    pub live_flags: Vec<LivenessFlagId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupStorageSlot {
    pub id: CleanupStorageId,
    /// Canonical structural discovery order, not runtime initialization order.
    pub discovery_index: u32,
    pub origin: CleanupStorageOrigin,
    pub ty: ResolvedType,
    pub shape: FieldLivenessShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CleanupStorageOrigin {
    Parameter {
        value: ValueId,
        parameter_index: u32,
    },
    Binding {
        value: ValueId,
    },
    Temporary {
        expression: ExpressionId,
    },
    ProvisionalResult {
        value: ValueId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FieldLivenessShape {
    NoDrop,
    Leaf {
        flag: LivenessFlagId,
        lifecycle: DeclarationId,
    },
    Record {
        declaration: DeclarationId,
        fields: Vec<FieldLiveness>,
    },
    /// A conditional sum inventory. Every leaf path is qualified by its stable
    /// case identity before its stable field identity. At runtime exactly the
    /// authenticated active case may contribute live flags.
    Variant {
        declaration: DeclarationId,
        cases: Vec<VariantCaseLiveness>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CleanupShapeProfile {
    pub(crate) owned_leaves: usize,
    pub(crate) visited_fields: usize,
    pub(crate) maximum_record_depth: usize,
    pub(crate) has_nested_owned_bytes: bool,
}

/// Inspect an already-derived cleanup shape without recursion or normalization.
/// Field order is observed exactly as supplied; callers remain responsible for
/// deriving and comparing the canonical declaration order independently.
pub(crate) fn cleanup_shape_profile(
    shape: &FieldLivenessShape,
) -> Result<CleanupShapeProfile, Diagnostic> {
    let mut profile = CleanupShapeProfile::default();
    let mut pending = vec![(shape, 0usize)];
    while let Some((shape, record_depth)) = pending.pop() {
        match shape {
            FieldLivenessShape::NoDrop => {}
            FieldLivenessShape::Leaf { lifecycle, .. } => {
                profile.owned_leaves = profile
                    .owned_leaves
                    .checked_add(1)
                    .ok_or_else(|| cleanup_error("cleanup owned-leaf count overflowed"))?;
                profile.has_nested_owned_bytes |=
                    record_depth >= 2 && lifecycle.as_str() == BYTES_DROP_LIFECYCLE_ID;
            }
            FieldLivenessShape::Record { fields, .. } => {
                let child_depth = record_depth
                    .checked_add(1)
                    .ok_or_else(|| cleanup_error("cleanup record depth overflowed"))?;
                profile.maximum_record_depth = profile.maximum_record_depth.max(child_depth);
                profile.visited_fields = profile
                    .visited_fields
                    .checked_add(fields.len())
                    .ok_or_else(|| cleanup_error("cleanup visited-field count overflowed"))?;
                pending.try_reserve(fields.len()).map_err(|_| {
                    cleanup_error("cleanup shape inspection capacity exceeds address space")
                })?;
                for field in fields.iter().rev() {
                    pending.push((&field.shape, child_depth));
                }
            }
            FieldLivenessShape::Variant { cases, .. } => {
                let field_count = cases
                    .iter()
                    .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))
                    .ok_or_else(|| cleanup_error("cleanup visited-field count overflowed"))?;
                profile.visited_fields = profile
                    .visited_fields
                    .checked_add(field_count)
                    .ok_or_else(|| cleanup_error("cleanup visited-field count overflowed"))?;
                pending.try_reserve(field_count).map_err(|_| {
                    cleanup_error("cleanup shape inspection capacity exceeds address space")
                })?;
                for case in cases.iter().rev() {
                    for field in case.fields.iter().rev() {
                        pending.push((&field.shape, record_depth));
                    }
                }
            }
        }
    }
    if profile.has_nested_owned_bytes {
        if profile.maximum_record_depth > MAX_CLEANUP_SHAPE_DEPTH {
            return Err(cleanup_error(format!(
                "cleanup shape exceeds the {MAX_CLEANUP_SHAPE_DEPTH} record-depth limit"
            )));
        }
        if profile.owned_leaves > MAX_CLEANUP_OWNED_LEAVES {
            return Err(cleanup_error(format!(
                "cleanup shape exceeds the {MAX_CLEANUP_OWNED_LEAVES} owned-leaf limit"
            )));
        }
        if profile.visited_fields > MAX_CLEANUP_VISITED_FIELDS {
            return Err(cleanup_error(format!(
                "cleanup shape exceeds the {MAX_CLEANUP_VISITED_FIELDS} visited-field limit"
            )));
        }
    }
    Ok(profile)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantCaseLiveness {
    pub case: DeclarationId,
    pub case_index: u32,
    pub fields: Vec<FieldLiveness>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldLiveness {
    pub field: DeclarationId,
    pub field_index: u32,
    pub shape: FieldLivenessShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupFlag {
    pub id: LivenessFlagId,
    pub place: CleanupPlace,
    pub lifecycle: DeclarationId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CleanupPlace {
    pub storage: CleanupStorageId,
    pub projections: Vec<DeclarationId>,
}

pub(crate) fn build_inventory(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<CleanupInventory, Diagnostic> {
    let mut builder = InventoryBuilder {
        program,
        slots: Vec::new(),
        flags: Vec::new(),
        live_owned_parameters: Vec::new(),
        conditional_owned_parameters: Vec::new(),
    };

    for (parameter_index, parameter) in function.params.iter().enumerate() {
        if parameter.ownership == OwnershipMode::Own && builder.needs_drop(&parameter.ty)? {
            let storage = builder.add_slot(
                CleanupStorageOrigin::Parameter {
                    value: parameter.id.clone(),
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| cleanup_error("too many function parameters"))?,
                },
                parameter.ty.clone(),
            )?;
            if let Some(entry) = builder.conditional_entry(storage)? {
                builder.conditional_owned_parameters.push(entry);
            } else {
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
                value: function.result_id.clone(),
            },
            function.return_type.clone(),
        )?;
    }
    for expression in &function.ensures {
        builder.collect_expression(expression)?;
    }

    #[cfg(test)]
    note_capacity_high_water(inventory_builder_live_capacity(&builder));

    let schema = if builder
        .slots
        .iter()
        .any(|slot| shape_contains_variant(&slot.shape))
    {
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

/// Whether a type contains a leaf governed by the resource-lifecycle cleanup
/// plan. Owned `String` storage is settled inline by the backends, including
/// when nested in an aggregate, so `TypeFacts::needs_drop` alone is too broad.
pub(crate) fn type_needs_resource_cleanup(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<bool, String> {
    let mut pending = vec![ty.clone()];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        let facts = program
            .declarations
            .type_facts(&ty)
            .ok_or_else(|| format!("type `{}` has no cleanup facts", ty.identity_key()))?;
        if !facts.needs_drop || !visited.insert(ty.clone()) {
            continue;
        }
        match ty {
            ResolvedType::Bytes => return Ok(true),
            ResolvedType::String | ResolvedType::Str | ResolvedType::SliceU8 => {}
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                let item = program
                    .types
                    .iter()
                    .find(|item| item.id == declaration)
                    .ok_or_else(|| format!("unknown cleanup type `{declaration}`"))?;
                match &item.kind {
                    ResolvedTypeDeclarationKind::Resource { .. } => return Ok(true),
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        pending.try_reserve(fields.len()).map_err(|_| {
                            "cleanup type traversal capacity exceeds address space".to_owned()
                        })?;
                        for field in fields.iter().rev() {
                            pending.push(
                                crate::hir::substitute_type(&field.ty, &declaration, &arguments)
                                    .map_err(|_| {
                                        format!("type `{declaration}` cleanup substitution failed")
                                    })?,
                            );
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let fields = cases
                            .iter()
                            .try_fold(0usize, |total, case| total.checked_add(case.fields.len()))
                            .ok_or_else(|| "cleanup type traversal work overflowed".to_owned())?;
                        pending.try_reserve(fields).map_err(|_| {
                            "cleanup type traversal capacity exceeds address space".to_owned()
                        })?;
                        for case in cases.iter().rev() {
                            for field in case.fields.iter().rev() {
                                pending.push(
                                    crate::hir::substitute_type(
                                        &field.ty,
                                        &declaration,
                                        &arguments,
                                    )
                                    .map_err(|_| {
                                        format!("type `{declaration}` cleanup substitution failed")
                                    })?,
                                );
                            }
                        }
                    }
                }
            }
            // Preserve the existing unsupported-generic cleanup path: the
            // shape builder will issue its more specific diagnostic.
            ResolvedType::TypeParameter { .. } => return Ok(true),
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool => {
                return Err(format!(
                    "copy type `{}` unexpectedly requires cleanup",
                    ty.identity_key()
                ));
            }
        }
    }
    Ok(false)
}

fn shape_contains_variant(shape: &FieldLivenessShape) -> bool {
    let mut pending = vec![shape];
    while let Some(shape) = pending.pop() {
        match shape {
            FieldLivenessShape::Variant { .. } => return true,
            FieldLivenessShape::Record { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.shape));
            }
            FieldLivenessShape::NoDrop | FieldLivenessShape::Leaf { .. } => {}
        }
    }
    false
}

pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in &program.functions {
        let expected = build_inventory(program, function)?;
        if !inventories_equal(&function.cleanup, &expected)? {
            return Err(cleanup_error(format!(
                "function `{}` has a non-canonical cleanup inventory",
                function.id
            )));
        }
    }
    for instance in &program.function_instances {
        let expected = build_inventory(program, &instance.function)?;
        if !inventories_equal(&instance.function.cleanup, &expected)? {
            return Err(cleanup_error(format!(
                "function instance `{}` has a non-canonical cleanup inventory",
                instance.id
            )));
        }
    }
    Ok(())
}

fn inventories_equal(
    actual: &CleanupInventory,
    expected: &CleanupInventory,
) -> Result<bool, Diagnostic> {
    if actual.schema != expected.schema
        || actual.entry_state != expected.entry_state
        || actual.flags != expected.flags
        || actual.slots.len() != expected.slots.len()
    {
        return Ok(false);
    }
    for (actual, expected) in actual.slots.iter().zip(&expected.slots) {
        if actual.id != expected.id
            || actual.discovery_index != expected.discovery_index
            || actual.origin != expected.origin
            || actual.ty != expected.ty
            || !field_liveness_shapes_equal(&actual.shape, &expected.shape)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn field_liveness_shapes_equal(
    actual: &FieldLivenessShape,
    expected: &FieldLivenessShape,
) -> Result<bool, Diagnostic> {
    let mut pending = vec![(actual, expected)];
    while let Some((actual, expected)) = pending.pop() {
        match (actual, expected) {
            (FieldLivenessShape::NoDrop, FieldLivenessShape::NoDrop) => {}
            (
                FieldLivenessShape::Leaf {
                    flag: actual_flag,
                    lifecycle: actual_lifecycle,
                },
                FieldLivenessShape::Leaf {
                    flag: expected_flag,
                    lifecycle: expected_lifecycle,
                },
            ) if actual_flag == expected_flag && actual_lifecycle == expected_lifecycle => {}
            (
                FieldLivenessShape::Record {
                    declaration: actual_declaration,
                    fields: actual_fields,
                },
                FieldLivenessShape::Record {
                    declaration: expected_declaration,
                    fields: expected_fields,
                },
            ) => {
                if actual_declaration != expected_declaration
                    || actual_fields.len() != expected_fields.len()
                {
                    return Ok(false);
                }
                pending.try_reserve(actual_fields.len()).map_err(|_| {
                    cleanup_error("cleanup shape comparison capacity exceeds address space")
                })?;
                for (actual, expected) in actual_fields.iter().zip(expected_fields).rev() {
                    if actual.field != expected.field || actual.field_index != expected.field_index
                    {
                        return Ok(false);
                    }
                    pending.push((&actual.shape, &expected.shape));
                }
            }
            (
                FieldLivenessShape::Variant {
                    declaration: actual_declaration,
                    cases: actual_cases,
                },
                FieldLivenessShape::Variant {
                    declaration: expected_declaration,
                    cases: expected_cases,
                },
            ) => {
                if actual_declaration != expected_declaration
                    || actual_cases.len() != expected_cases.len()
                {
                    return Ok(false);
                }
                let fields = actual_cases
                    .iter()
                    .try_fold(0usize, |total, case| total.checked_add(case.fields.len()))
                    .ok_or_else(|| cleanup_error("cleanup shape comparison work overflowed"))?;
                pending.try_reserve(fields).map_err(|_| {
                    cleanup_error("cleanup shape comparison capacity exceeds address space")
                })?;
                for (actual_case, expected_case) in actual_cases.iter().zip(expected_cases).rev() {
                    if actual_case.case != expected_case.case
                        || actual_case.case_index != expected_case.case_index
                        || actual_case.fields.len() != expected_case.fields.len()
                    {
                        return Ok(false);
                    }
                    for (actual, expected) in
                        actual_case.fields.iter().zip(&expected_case.fields).rev()
                    {
                        if actual.field != expected.field
                            || actual.field_index != expected.field_index
                        {
                            return Ok(false);
                        }
                        pending.push((&actual.shape, &expected.shape));
                    }
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

struct InventoryBuilder<'a> {
    program: &'a ResolvedProgram,
    slots: Vec<CleanupStorageSlot>,
    flags: Vec<CleanupFlag>,
    live_owned_parameters: Vec<CleanupStorageId>,
    conditional_owned_parameters: Vec<ConditionalVariantEntry>,
}

impl InventoryBuilder<'_> {
    fn needs_drop(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
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
            #[cfg(test)]
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
            match frame {
                Frame::Enter(ty) => {
                    if !self.needs_drop(ty)? {
                        shapes.push(FieldLivenessShape::NoDrop);
                        continue;
                    }
                    if matches!(ty, ResolvedType::Bytes) {
                        let flag_index = u32::try_from(self.flags.len())
                            .map_err(|_| cleanup_error("too many cleanup liveness flags"))?;
                        let flag = LivenessFlagId(flag_index);
                        let lifecycle = DeclarationId::new(BYTES_DROP_LIFECYCLE_ID);
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
                        .types
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
                                return Err(cleanup_error(format!(
                                    "droppable type `{}` has unsupported generic arguments",
                                    ty.identity_key()
                                )));
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
                                    let shape = if self.needs_drop(&field_ty)? {
                                        if field_ty != ResolvedType::Bytes {
                                            return Err(cleanup_error(
                                                "droppable variant field is outside the direct-Bytes v1 slice",
                                            ));
                                        }
                                        let flag_index =
                                            u32::try_from(self.flags.len()).map_err(|_| {
                                                cleanup_error("too many cleanup liveness flags")
                                            })?;
                                        let flag = LivenessFlagId(flag_index);
                                        let lifecycle = DeclarationId::new(BYTES_DROP_LIFECYCLE_ID);
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

        let mut frames = vec![Frame::Enter(expression)];
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_capacity_high_water(
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + self.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>()
                    + self.flags.capacity() * std::mem::size_of::<CleanupFlag>()
                    + self.slots.iter().map(slot_owned_capacity).sum::<usize>()
                    + self.flags.iter().map(flag_owned_capacity).sum::<usize>(),
            );
            match frame {
                Frame::Enter(expression) => {
                    frames.try_reserve(2).map_err(|_| {
                        cleanup_error("cleanup traversal capacity exceeds address space")
                    })?;
                    frames.push(Frame::Finish(expression));
                    frames.push(Frame::Children(expression, 0));
                }
                Frame::Children(expression, index) => {
                    frames.try_reserve(2).map_err(|_| {
                        cleanup_error("cleanup traversal capacity exceeds address space")
                    })?;
                    let mut enter = None;
                    let mut action = None;
                    match &expression.kind {
                        ResolvedExprKind::Call { args, .. } => {
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
                        | ResolvedExprKind::Upcast { source: value } => {
                            enter = (index == 0).then_some(value.as_ref());
                        }
                        ResolvedExprKind::Binary { left, right, .. } => {
                            enter = match index {
                                0 => Some(left),
                                1 => Some(right),
                                _ => None,
                            };
                        }
                        ResolvedExprKind::Block { statements, tail } => {
                            // Only let statements need a trailing binding step.
                            // A no-op step for assignment/unsafe/while would
                            // end this traversal before later storage or tail.
                            let per_statement = |statement: &ResolvedStatement| {
                                statement.child_count().saturating_add(usize::from(matches!(
                                    statement,
                                    ResolvedStatement::Let { .. }
                                )))
                            };
                            let mut offset = 0usize;
                            let mut matched = None;
                            for statement in statements.iter() {
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
                                value: binding.id.clone(),
                            },
                            binding.ty.clone(),
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
                                expression: base.id.clone(),
                            },
                            base.ty.clone(),
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

        enum Item<'a> {
            Pattern(&'a ResolvedMatchPattern),
            RecordField(&'a ResolvedRecordMatchFieldPattern),
        }
        let mut pending = vec![Item::Pattern(pattern)];
        while let Some(item) = pending.pop() {
            match item {
                Item::Pattern(ResolvedMatchPattern::Binding(binding)) => {
                    if binding.ownership == OwnershipMode::Own && self.needs_drop(&binding.ty)? {
                        self.add_slot(
                            CleanupStorageOrigin::Binding {
                                value: binding.id.clone(),
                            },
                            binding.ty.clone(),
                        )?;
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Record { record, fields, .. }) => {
                    let declared = self
                        .program
                        .declarations
                        .record_fields(record)
                        .ok_or_else(|| cleanup_error("record pattern has no field inventory"))?;
                    for declaration in declared.iter().rev() {
                        let field = fields
                            .iter()
                            .find(|field| field.field == declaration.id)
                            .ok_or_else(|| {
                                cleanup_error("record pattern field inventory is incomplete")
                            })?;
                        pending.push(Item::RecordField(&field.pattern));
                    }
                }
                Item::Pattern(ResolvedMatchPattern::Variant { fields, .. }) => {
                    for field in fields {
                        if field.binding.ownership == OwnershipMode::Own
                            && self.needs_drop(&field.binding.ty)?
                        {
                            self.add_slot(
                                CleanupStorageOrigin::Binding {
                                    value: field.binding.id.clone(),
                                },
                                field.binding.ty.clone(),
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
                                value: binding.id.clone(),
                            },
                            binding.ty.clone(),
                        )?;
                    }
                }
                Item::RecordField(ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields,
                    ..
                }) => {
                    let declared =
                        self.program
                            .declarations
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
            && !matches!(expression.kind, ResolvedExprKind::Place(_))
        {
            self.add_slot(
                CleanupStorageOrigin::Temporary {
                    expression: expression.id.clone(),
                },
                expression.ty.clone(),
            )?;
        }
        Ok(())
    }
}

fn collect_shape_flags(shape: &FieldLivenessShape, flags: &mut Vec<LivenessFlagId>) {
    let mut pending = vec![shape];
    while let Some(shape) = pending.pop() {
        match shape {
            FieldLivenessShape::Leaf { flag, .. } => flags.push(*flag),
            FieldLivenessShape::Record { fields, .. } => {
                pending.extend(fields.iter().rev().map(|field| &field.shape));
            }
            FieldLivenessShape::Variant { cases, .. } => {
                pending.extend(
                    cases
                        .iter()
                        .rev()
                        .flat_map(|case| case.fields.iter().rev())
                        .map(|field| &field.shape),
                );
            }
            FieldLivenessShape::NoDrop => {}
        }
    }
}

fn cleanup_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

#[cfg(test)]
#[path = "cleanup/nested_owned_records_tests.rs"]
mod nested_owned_records_tests;
