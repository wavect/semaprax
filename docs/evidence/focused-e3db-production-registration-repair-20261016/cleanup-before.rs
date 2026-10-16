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

mod replay_context;
pub(crate) use replay_context::ReplayContext;
mod generic_record;
#[cfg(test)]
pub(crate) use inventory_builder::build_borrowed_helper_inventory_with_budget;
mod inventory_builder;
pub(crate) use inventory_builder::selected_inline;
#[cfg(test)]
pub(crate) mod schema_summary;
use inventory_builder::InventoryBuilder;
pub(crate) use inventory_builder::{
    authenticate_selected_vector, build_flat_record_inventory_with_budget, build_inventory,
    prepay_selected_collection_probes, selected_flat_record_declaration,
    selected_flat_record_fields, selected_inventory_needs_drop, selected_inventory_needs_drop_in,
    selected_vector_element,
};
mod iterator;
#[cfg(test)]
mod owned_capacity;
pub(crate) use iterator::{
    primitive_leaf_lifecycle, primitive_leaf_lifecycle_identity, variant_leaf_lifecycle,
    variant_record_field,
};
#[cfg(test)]
use owned_capacity::resolved_type_owned_capacity;
pub const ITER_DROP_LIFECYCLE_ID: &str = "core.iter.drop";

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
pub const STRING_DROP_LIFECYCLE_ID: &str = "core.string.drop";
/// Canonical compiler-owned lifecycle for one uniquely owned bounded `Vec<T>`
/// carrier. The element profile is authenticated by the HIR type rather than
/// supplied by source or backend metadata.
pub const VEC_DROP_LIFECYCLE_ID: &str = "core.vec.drop";
/// Canonical compiler-owned lifecycle for one uniquely owned bounded `Box<T>`
/// carrier. The scalar element type is authenticated by the HIR type.
pub const BOX_DROP_LIFECYCLE_ID: &str = "core.box.drop";

pub(crate) fn is_owned_bounded_vec_type(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::VEC_ID
                && arguments.len() == 1
                && crate::vec_ops::resolved_vec_element_is_admitted(&arguments[0])
    )
}

pub(crate) fn is_owned_bounded_box_type(ty: &ResolvedType) -> bool {
    crate::box_ops::is_type(ty)
}

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
                profile.has_nested_owned_bytes |= record_depth >= 2
                    && matches!(
                        lifecycle.as_str(),
                        BYTES_DROP_LIFECYCLE_ID | STRING_DROP_LIFECYCLE_ID
                    );
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

/// Whether a type contains a leaf governed by the resource-lifecycle cleanup
/// plan. Owned strings use the compiler-owned string lifecycle; borrowed views
/// carry no cleanup authority.
pub(crate) fn type_needs_resource_cleanup(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
) -> Result<bool, String> {
    // A terminal expression has no storage. Nested types still require facts.
    if *ty == ResolvedType::Never {
        return Ok(false);
    }
    let mut pending = vec![ty.clone()];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        let facts = program
            .declarations()
            .type_flags(&ty)
            .ok_or_else(|| format!("type `{}` has no cleanup facts", ty.identity_key()))?;
        if !facts.needs_drop || !visited.insert(ty.clone()) {
            continue;
        }
        // The sealed stdin reader is compiler-owned and has no user-visible
        // fields, but it still owns the invocation's reusable input buffer.
        // Classify its exact nominal type as droppable so inventory and plan
        // construction retain the canonical lexical drop leaf.
        if crate::map_ops::is_typed_collection(&ty) || crate::stdin_stream_ops::is_reader(&ty) {
            return Ok(true);
        }
        match ty {
            ResolvedType::Bytes
            | ResolvedType::String
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair => return Ok(true),
            ResolvedType::MutFunctionI64 => {}
            ResolvedType::StringMap => return Ok(true),
            ResolvedType::Function { .. } | ResolvedType::Str | ResolvedType::SliceU8 => {}
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if declaration.as_str() == crate::iterator_ops::ITER_ID
                    && matches!(arguments.as_slice(), [element]
                    if crate::iterator_ops::resolved_element_is_admitted_in(
                        program.declarations(),
                        element,
                    ))
                {
                    return Ok(true);
                }
                if declaration.as_str() == crate::prelude::BOX_ID
                    && matches!(arguments.as_slice(), [element] if crate::box_ops::resolved_box_element_is_admitted(element))
                {
                    return Ok(true);
                }
                if declaration.as_str() == crate::prelude::VEC_ID
                    && arguments.len() == 1
                    && (crate::vec_ops::resolved_vec_element_is_admitted(&arguments[0])
                        || crate::hir::owned_leaf_collection::copy_or_leaf_admitted(program.declarations(), &arguments[0])
                        || crate::hir::owned_record_collection::
                            is_admitted_owned_record_collection_element(
                                program.declarations(),
                                &arguments[0],
                            ))
                {
                    return Ok(true);
                }
                let item = program
                    .types()
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
            // Preserve the shape builder's unsupported-generic diagnostic.
            ResolvedType::TypeParameter { .. } => return Ok(true),
            ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
            | ResolvedType::Never
            | ResolvedType::Unit
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
    for function in program.functions() {
        let expected = build_inventory(program, function)?;
        if !inventories_equal(&function.cleanup, &expected)? {
            return Err(cleanup_error(format!(
                "function `{}` has a non-canonical cleanup inventory",
                function.id
            )));
        }
    }
    for instance in program.function_instances() {
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

#[cfg(test)]
pub(crate) use inventory_builder::typed::before_initial::build as build_typed_flat_record_inventory;
pub(crate) use inventory_builder::typed::{
    build_after_initial_receiving as build_prepaid_typed_flat_record_inventory,
    debit as debit_typed_inventory_initial, InitialReceiving as TypedInventoryInitialReceiving,
    InventoryFailure, INITIAL_CONTROL_INITIALIZERS as TYPED_INVENTORY_INITIAL_FIELDS,
};

#[cfg(test)]
pub(crate) use inventory_builder::typed::{
    metadata_work as typed_inventory_metadata_work,
    reset_metadata_work as reset_typed_inventory_metadata_work,
};

#[cfg(test)]
pub(crate) use inventory_builder::typed::{
    begin_logical_trace as begin_typed_inventory_trace,
    take_logical_trace as take_typed_inventory_trace,
};

#[cfg(test)]
pub(crate) use inventory_builder::typed::{
    initial_entries as typed_inventory_initial_entries,
    reset_initial_entries as reset_typed_inventory_initial_entries,
};
