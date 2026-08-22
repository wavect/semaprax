//! Unpublished allocation contracts shared with the private native builder.
//!
//! This file is path-included by the unpublished builder so the opaque HIR
//! declaration-index allowance cannot drift from the root-side proof.

pub(crate) const PRELUDE_CAPACITY_IDENTITIES: [&str; 9] = [
    "core.option",
    "core.option.none",
    "core.option.some",
    "core.option.some.value",
    "core.result",
    "core.result.ok",
    "core.result.ok.value",
    "core.result.err",
    "core.result.err.error",
];

pub(crate) fn declaration_index_upper(
    canonical_source_bytes: usize,
    type_count: usize,
    interface_count: usize,
    function_count: usize,
    type_facts_layout_upper: usize,
) -> Option<usize> {
    let declarations = type_count
        .checked_add(interface_count)?
        .checked_add(function_count)?;
    let per_declaration = std::mem::size_of::<crate::hir::Declaration>()
        .checked_mul(8)?
        .checked_add(640)?;
    let compiler_owned_prelude =
        std::mem::size_of::<crate::hir::DeclarationIndex>().checked_mul(12)?;
    // A TypeFacts layout key may embed child keys for each authored field.
    // The root builder independently pre-rejects exponential declaration-DAG
    // expansion; source² is a checked upper for all retained index key/value
    // bytes after that admission and covers the compiler-owned prelude rows.
    std::mem::size_of::<crate::hir::DeclarationIndex>()
        .checked_add(compiler_owned_prelude)?
        .checked_add(canonical_source_bytes.checked_mul(4)?)?
        .checked_add(type_facts_layout_upper)?
        .checked_add(declarations.max(1).checked_mul(per_declaration)?)
}

pub(crate) fn type_facts_layout_upper(
    canonical_source_bytes: usize,
    type_count: usize,
    maximum_type_occurrences: usize,
) -> Option<usize> {
    let occurrence_bytes = canonical_source_bytes
        .checked_add(canonical_source_bytes.checked_ilog10().unwrap_or(0) as usize * 4)?
        .checked_add("variant::record::resource:".len())?;
    maximum_type_occurrences
        .checked_mul(occurrence_bytes)?
        .checked_mul(type_count.max(1))
}

fn resolved_type_owned_capacity(ty: &crate::hir::ResolvedType) -> Option<usize> {
    match ty {
        crate::hir::ResolvedType::Unit
        | crate::hir::ResolvedType::I64
        | crate::hir::ResolvedType::I32
        | crate::hir::ResolvedType::Char
        | crate::hir::ResolvedType::F32
        | crate::hir::ResolvedType::F64
        | crate::hir::ResolvedType::Bool => Some(0),
        crate::hir::ResolvedType::TypeParameter { owner, .. } => Some(owner.as_str().len()),
        crate::hir::ResolvedType::Nominal {
            declaration,
            arguments,
        } => arguments
            .iter()
            .try_fold(declaration.as_str().len(), |bytes, argument| {
                bytes.checked_add(resolved_type_owned_capacity(argument)?)
            })?
            .checked_add(
                arguments
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::hir::ResolvedType>())?,
            ),
    }
}

#[allow(
    unreachable_patterns,
    reason = "non-exhaustive across the builder crate boundary"
)]
fn shape_owned_capacity(shape: &crate::cleanup::FieldLivenessShape) -> Option<usize> {
    match shape {
        crate::cleanup::FieldLivenessShape::NoDrop => Some(0),
        crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
            Some(lifecycle.as_str().len())
        }
        crate::cleanup::FieldLivenessShape::Record {
            declaration,
            fields,
        } => fields
            .iter()
            .try_fold(declaration.as_str().len(), |bytes, field| {
                bytes
                    .checked_add(field.field.as_str().len())?
                    .checked_add(shape_owned_capacity(&field.shape)?)
            })?
            .checked_add(
                fields
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::cleanup::FieldLiveness>())?,
            ),
        _ => None,
    }
}

#[allow(
    unreachable_patterns,
    reason = "non-exhaustive across the builder crate boundary"
)]
pub(crate) fn cleanup_inventory_owned_capacity(
    inventory: &crate::cleanup::CleanupInventory,
) -> Option<usize> {
    let slots = inventory.slots.iter().try_fold(0usize, |bytes, slot| {
        let origin = match &slot.origin {
            crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }
            | crate::cleanup::CleanupStorageOrigin::Binding { value }
            | crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value } => {
                value.as_str().len()
            }
            crate::cleanup::CleanupStorageOrigin::Temporary { expression } => {
                expression.as_str().len()
            }
            _ => return None,
        };
        bytes
            .checked_add(origin)?
            .checked_add(resolved_type_owned_capacity(&slot.ty)?)?
            .checked_add(shape_owned_capacity(&slot.shape)?)
    })?;
    let flags = inventory.flags.iter().try_fold(0usize, |bytes, flag| {
        let projections = flag
            .place
            .projections
            .iter()
            .try_fold(0usize, |bytes, id| bytes.checked_add(id.as_str().len()))?;
        bytes
            .checked_add(flag.lifecycle.as_str().len())?
            .checked_add(
                flag.place
                    .projections
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::hir::DeclarationId>())?,
            )?
            .checked_add(projections)
    })?;
    inventory
        .slots
        .capacity()
        .checked_mul(std::mem::size_of::<crate::cleanup::CleanupStorageSlot>())?
        .checked_add(
            inventory
                .flags
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup::CleanupFlag>())?,
        )?
        .checked_add(
            inventory
                .entry_state
                .live_owned_parameters
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup::CleanupStorageId>())?,
        )?
        .checked_add(slots)?
        .checked_add(flags)
}

fn storage_owned_capacity(storage: &crate::cleanup_plan::StorageId) -> Option<usize> {
    match storage {
        crate::cleanup_plan::StorageId::Value(value) => Some(value.as_str().len()),
        crate::cleanup_plan::StorageId::Temporary(expression) => Some(expression.as_str().len()),
        crate::cleanup_plan::StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => call
            .as_str()
            .len()
            .checked_add(value_expression.as_str().len()),
        crate::cleanup_plan::StorageId::ProvisionalResult => Some(0),
    }
}

fn cleanup_place_owned_capacity(place: &crate::cleanup_plan::CleanupPlace) -> Option<usize> {
    place
        .projections
        .iter()
        .try_fold(
            storage_owned_capacity(&place.storage)?,
            |bytes, projection| bytes.checked_add(projection.as_str().len()),
        )?
        .checked_add(
            place
                .projections
                .capacity()
                .checked_mul(std::mem::size_of::<crate::hir::DeclarationId>())?,
        )
}

fn staged_result_owned_capacity(
    source: &crate::cleanup_plan::StagedCopyResultSource,
) -> Option<usize> {
    use crate::cleanup_plan::StagedCopyResultSource;
    match source {
        StagedCopyResultSource::Body {
            expression,
            instance,
        } => expression
            .as_str()
            .len()
            .checked_add(resolved_type_owned_capacity(instance)?),
        StagedCopyResultSource::TryResidual {
            expression,
            operand,
            source_instance,
            target_instance,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
        } => [
            expression.as_str().len(),
            operand.as_str().len(),
            result.as_str().len(),
            ok_case.as_str().len(),
            ok_field.as_str().len(),
            err_case.as_str().len(),
            err_field.as_str().len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)?
        .checked_add(resolved_type_owned_capacity(source_instance)?)?
        .checked_add(resolved_type_owned_capacity(target_instance)?),
        StagedCopyResultSource::TryOptionNone {
            expression,
            operand,
            source_instance,
            target_instance,
            option,
            some_case,
            some_field,
            none_case,
        } => [
            expression.as_str().len(),
            operand.as_str().len(),
            option.as_str().len(),
            some_case.as_str().len(),
            some_field.as_str().len(),
            none_case.as_str().len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)?
        .checked_add(resolved_type_owned_capacity(source_instance)?)?
        .checked_add(resolved_type_owned_capacity(target_instance)?),
    }
}

pub(crate) fn cleanup_plan_owned_capacity(
    plan: &crate::cleanup_plan::CleanupPlan,
) -> Option<usize> {
    use crate::cleanup_plan::{
        CleanupResultSource, CleanupTerminator, CleanupTransition, EdgeCondition, ExitContinuation,
        StatusProducer,
    };
    let status_id = |id: &crate::cleanup_plan::StatusSourceId| id.expression.as_str().len();
    let mut bytes = [
        plan.entry_state
            .live_owned_parameters
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupPlace>())?,
        plan.slots
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupSlot>())?,
        plan.status_sources
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::StatusSource>())?,
        plan.blocks
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupBlock>())?,
        plan.edges
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupEdge>())?,
        plan.regions
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupRegion>())?,
        plan.exits
            .capacity()
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::ExitTarget>())?,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)?;
    bytes = bytes.checked_add(
        plan.entry_state
            .live_owned_parameters
            .iter()
            .try_fold(0usize, |bytes, place| {
                bytes.checked_add(cleanup_place_owned_capacity(place)?)
            })?,
    )?;
    bytes = bytes.checked_add(plan.slots.iter().try_fold(0usize, |bytes, slot| {
        bytes.checked_add(
            storage_owned_capacity(&slot.storage)?
                .checked_add(resolved_type_owned_capacity(&slot.ty)?)?
                .checked_add(shape_owned_capacity(&slot.field_liveness_shape)?)?,
        )
    })?)?;
    for status in &plan.status_sources {
        bytes = bytes.checked_add(status_id(&status.id))?;
        bytes = bytes.checked_add(match &status.producer {
            StatusProducer::PropagatedCall { callee } => callee.as_str().len(),
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => normalized_cases
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup_plan::StatusCase>())?,
            StatusProducer::ContractFalse { .. } => 0,
        })?;
    }
    for block in &plan.blocks {
        bytes = bytes.checked_add(
            block
                .transitions
                .capacity()
                .checked_mul(std::mem::size_of::<CleanupTransition>())?,
        )?;
        for transition in &block.transitions {
            let transition_bytes = match transition {
                CleanupTransition::Initialize { at, destination } => at
                    .as_str()
                    .len()
                    .checked_add(cleanup_place_owned_capacity(destination)?)?,
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => at
                    .as_str()
                    .len()
                    .checked_add(cleanup_place_owned_capacity(source)?)?
                    .checked_add(cleanup_place_owned_capacity(destination)?)?,
                CleanupTransition::CallCommit { call, arguments } => call
                    .as_str()
                    .len()
                    .checked_add(arguments.capacity().checked_mul(std::mem::size_of::<
                        crate::cleanup_plan::CallArgumentTransfer,
                    >())?)?
                    .checked_add(arguments.iter().try_fold(0usize, |bytes, argument| {
                        bytes.checked_add(cleanup_place_owned_capacity(&argument.source)?)
                    })?)?,
                CleanupTransition::SelectFailure { source } => status_id(source),
                // Staged-copy metadata identities/types are also represented
                // in the owning HIR expression; charge its full inline value
                // plus one source-derived identity payload here.
                CleanupTransition::StageCopyResult { source } => {
                    staged_result_owned_capacity(source)?
                }
            };
            bytes = bytes.checked_add(transition_bytes)?;
        }
        if let CleanupTerminator::Branch(edges) = &block.terminator {
            bytes = bytes.checked_add(
                edges
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::cleanup_plan::EdgeId>())?,
            )?;
        }
    }
    for edge in &plan.edges {
        bytes = bytes.checked_add(match &edge.condition {
            EdgeCondition::Always => 0,
            EdgeCondition::BooleanResult(expression, _) => expression.as_str().len(),
            EdgeCondition::VariantCase {
                scrutinee, case, ..
            } => scrutinee.as_str().len().checked_add(case.as_str().len())?,
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                status_id(source)
            }
        })?;
    }
    for region in &plan.regions {
        bytes = bytes.checked_add(
            region
                .slots
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup_plan::StorageId>())?,
        )?;
        bytes = bytes.checked_add(region.slots.iter().try_fold(0usize, |bytes, storage| {
            bytes.checked_add(storage_owned_capacity(storage)?)
        })?)?;
    }
    for exit in &plan.exits {
        bytes = bytes.checked_add(
            exit.leaves_regions
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupRegionId>())?,
        )?;
        bytes = bytes.checked_add(
            exit.finalize_in_order
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup_plan::FinalizeAction>())?,
        )?;
        bytes = bytes.checked_add(exit.finalize_in_order.iter().try_fold(
            0usize,
            |bytes, action| {
                bytes
                    .checked_add(cleanup_place_owned_capacity(&action.source)?)?
                    .checked_add(action.lifecycle_id.as_str().len())
            },
        )?)?;
        bytes = bytes.checked_add(match &exit.continuation {
            ExitContinuation::Continue(_) | ExitContinuation::ReturnUnit => 0,
            ExitContinuation::CommitResult { source } => match source {
                CleanupResultSource::Scalar { expression } => expression.as_str().len(),
                CleanupResultSource::Owned { storage } => cleanup_place_owned_capacity(storage)?,
            },
            ExitContinuation::ReturnFailure { source } => status_id(source),
        })?;
    }
    Some(bytes)
}
