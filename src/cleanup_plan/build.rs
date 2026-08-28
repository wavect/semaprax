//! Canonical cleanup-plan construction from validated core HIR.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup::{
    CleanupStorageId as InventoryStorageId, FieldLiveness, FieldLivenessShape, LivenessFlagId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationKind, ExpressionId, IdentityOrigin, OwnershipMode, Place,
    PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedMatchArm,
    ResolvedMatchPattern, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind,
};
use crate::prelude;

use super::{
    BlockId, CallArgumentTransfer, CheckedOperation, CleanupBlock, CleanupEdge, CleanupEntryState,
    CleanupPlace, CleanupPlan, CleanupRegion, CleanupRegionId, CleanupResultSource, CleanupSlot,
    CleanupSlotId, CleanupTerminator, CleanupTransition, ContractPhase, EdgeCondition, EdgeId,
    ExitContinuation, ExitTarget, ExitTargetId, FinalizeAction, StagedCopyResultSource, StatusCase,
    StatusLane, StatusProducer, StatusSource, StatusSourceId, StorageId, CLEANUP_PLAN_SCHEMA_V2,
    CLEANUP_PLAN_SCHEMA_V3, CLEANUP_PLAN_SCHEMA_V4,
};

const UNRESOLVED_EXIT: ExitTargetId = ExitTargetId(u32::MAX);
#[cfg(test)]
const CLEANUP_EVAL_RESULT_SIZE_CEILING: usize = 128;

#[cfg(test)]
thread_local! {
    static LOWER_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_lower_capacity_high_water() {
    LOWER_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(crate) fn lower_capacity_high_water() -> usize {
    LOWER_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_lower_capacity_high_water(bytes: usize) {
    LOWER_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn storage_id_owned_capacity(storage: &StorageId) -> usize {
    match storage {
        StorageId::Value(id) => id.as_str().len(),
        StorageId::Temporary(id) => id.as_str().len(),
        StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => call.as_str().len() + value_expression.as_str().len(),
        StorageId::ProvisionalResult => 0,
    }
}

#[cfg(test)]
fn cleanup_place_owned_capacity(place: &CleanupPlace) -> usize {
    storage_id_owned_capacity(&place.storage)
        + place.projections.capacity() * std::mem::size_of::<DeclarationId>()
        + place
            .projections
            .iter()
            .map(|projection| projection.as_str().len())
            .sum::<usize>()
}

#[cfg(test)]
fn status_source_id_owned_capacity(source: &StatusSourceId) -> usize {
    source.expression.as_str().len()
}

#[cfg(test)]
fn field_shape_owned_capacity(shape: &FieldLivenessShape) -> usize {
    match shape {
        FieldLivenessShape::NoDrop => 0,
        FieldLivenessShape::Leaf { lifecycle, .. } => lifecycle.as_str().len(),
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => {
            declaration.as_str().len()
                + fields.capacity() * std::mem::size_of::<FieldLiveness>()
                + fields
                    .iter()
                    .map(|field| {
                        field.field.as_str().len() + field_shape_owned_capacity(&field.shape)
                    })
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn cleanup_slot_owned_capacity(slot: &CleanupSlot) -> usize {
    storage_id_owned_capacity(&slot.storage)
        + resolved_type_owned_capacity(&slot.ty)
        + field_shape_owned_capacity(&slot.field_liveness_shape)
}

#[cfg(test)]
fn transition_owned_capacity(transition: &CleanupTransition) -> usize {
    match transition {
        CleanupTransition::Initialize { at, destination } => {
            at.as_str().len() + cleanup_place_owned_capacity(destination)
        }
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            at.as_str().len()
                + cleanup_place_owned_capacity(source)
                + cleanup_place_owned_capacity(destination)
        }
        CleanupTransition::CallCommit { call, arguments } => {
            call.as_str().len()
                + arguments.capacity() * std::mem::size_of::<CallArgumentTransfer>()
                + arguments
                    .iter()
                    .map(|argument| cleanup_place_owned_capacity(&argument.source))
                    .sum::<usize>()
        }
        CleanupTransition::SelectFailure { source } => status_source_id_owned_capacity(source),
        CleanupTransition::StageCopyResult { source } => match source {
            StagedCopyResultSource::Body {
                expression,
                instance,
            } => expression.as_str().len() + resolved_type_owned_capacity(instance),
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
            } => {
                expression.as_str().len()
                    + operand.as_str().len()
                    + resolved_type_owned_capacity(source_instance)
                    + resolved_type_owned_capacity(target_instance)
                    + result.as_str().len()
                    + ok_case.as_str().len()
                    + ok_field.as_str().len()
                    + err_case.as_str().len()
                    + err_field.as_str().len()
            }
            StagedCopyResultSource::TryOptionNone {
                expression,
                operand,
                source_instance,
                target_instance,
                option,
                some_case,
                some_field,
                none_case,
            } => {
                expression.as_str().len()
                    + operand.as_str().len()
                    + resolved_type_owned_capacity(source_instance)
                    + resolved_type_owned_capacity(target_instance)
                    + option.as_str().len()
                    + some_case.as_str().len()
                    + some_field.as_str().len()
                    + none_case.as_str().len()
            }
        },
    }
}

#[cfg(test)]
fn edge_condition_owned_capacity(condition: &EdgeCondition) -> usize {
    match condition {
        EdgeCondition::Always => 0,
        EdgeCondition::BooleanResult(expression, _) => expression.as_str().len(),
        EdgeCondition::VariantCase {
            scrutinee, case, ..
        } => scrutinee.as_str().len() + case.as_str().len(),
        EdgeCondition::ArmSelected { scrutinee, .. } => scrutinee.as_str().len(),
        EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
            status_source_id_owned_capacity(source)
        }
    }
}

#[cfg(test)]
fn exit_continuation_owned_capacity(continuation: &ExitContinuation) -> usize {
    match continuation {
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar { expression },
        } => expression.as_str().len(),
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage },
        } => cleanup_place_owned_capacity(storage),
        ExitContinuation::ReturnFailure { source } => status_source_id_owned_capacity(source),
        ExitContinuation::Continue(_) | ExitContinuation::ReturnUnit => 0,
    }
}

#[cfg(test)]
fn builder_nested_capacity(builder: &PlanBuilder<'_>) -> usize {
    let block_payload = builder.blocks.iter().fold(0usize, |bytes, block| {
        bytes
            + block.transitions.capacity() * std::mem::size_of::<CleanupTransition>()
            + block
                .transitions
                .iter()
                .map(transition_owned_capacity)
                .sum::<usize>()
            + match &block.terminator {
                Some(CleanupTerminator::Branch(edges)) => {
                    edges.capacity() * std::mem::size_of::<EdgeId>()
                }
                Some(CleanupTerminator::Goto(_) | CleanupTerminator::Exit(_)) | None => 0,
            }
    });
    let region_payload = builder.regions.iter().fold(0usize, |bytes, region| {
        bytes
            + region.slots.capacity() * std::mem::size_of::<StorageId>()
            + region
                .slots
                .iter()
                .map(storage_id_owned_capacity)
                .sum::<usize>()
    });
    let exit_payload = builder.exits.iter().fold(0usize, |bytes, exit| {
        bytes
            + exit.leaves_regions.capacity() * std::mem::size_of::<CleanupRegionId>()
            + exit.finalize_in_order.capacity() * std::mem::size_of::<FinalizeAction>()
            + exit
                .finalize_in_order
                .iter()
                .map(|action| {
                    cleanup_place_owned_capacity(&action.source)
                        + action.lifecycle_id.as_str().len()
                })
                .sum::<usize>()
            + exit_continuation_owned_capacity(&exit.continuation)
    });
    let status_payload = builder.status_sources.iter().fold(0usize, |bytes, source| {
        bytes
            + match &source.producer {
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } => normalized_cases.capacity() * std::mem::size_of::<StatusCase>(),
                StatusProducer::PropagatedCall { callee } => callee.as_str().len(),
                StatusProducer::ContractFalse { .. } => 0,
            }
            + status_source_id_owned_capacity(&source.id)
    });
    let edge_payload = builder
        .edges
        .iter()
        .map(|edge| edge_condition_owned_capacity(&edge.condition))
        .sum::<usize>();
    block_payload
        + region_payload
        + exit_payload
        + status_payload
        + edge_payload
        + builder.initial_state.live_order.capacity() * std::mem::size_of::<LivenessFlagId>()
        + builder.pending_try_residuals.capacity() * std::mem::size_of::<PendingTryResidual>()
        + builder
            .pending_try_residuals
            .iter()
            .map(|pending| {
                pending.state.live_order.capacity() * std::mem::size_of::<LivenessFlagId>()
            })
            .sum::<usize>()
        + builder.slots.capacity() * std::mem::size_of::<CleanupSlot>()
        + builder
            .slots
            .iter()
            .map(cleanup_slot_owned_capacity)
            .sum::<usize>()
        + builder.storage_to_slot.len()
            * (std::mem::size_of::<(StorageId, CleanupSlotId)>()
                + std::mem::size_of::<BTreeMap<StorageId, CleanupSlotId>>())
        + builder
            .storage_to_slot
            .keys()
            .map(storage_id_owned_capacity)
            .sum::<usize>()
        + builder.inventory_storage.len()
            * (std::mem::size_of::<(InventoryStorageId, StorageId)>()
                + std::mem::size_of::<BTreeMap<InventoryStorageId, StorageId>>())
        + builder
            .inventory_storage
            .values()
            .map(storage_id_owned_capacity)
            .sum::<usize>()
        + builder.leaves.len()
            * (std::mem::size_of::<(LivenessFlagId, LeafMetadata)>()
                + std::mem::size_of::<BTreeMap<LivenessFlagId, LeafMetadata>>())
        + builder
            .leaves
            .values()
            .map(|leaf| cleanup_place_owned_capacity(&leaf.place) + leaf.lifecycle.as_str().len())
            .sum::<usize>()
        + builder.entry_state.live_owned_parameters.capacity() * std::mem::size_of::<CleanupPlace>()
        + builder
            .entry_state
            .live_owned_parameters
            .iter()
            .map(cleanup_place_owned_capacity)
            .sum::<usize>()
}

#[cfg(test)]
fn flow_state_owned_capacity(state: &FlowState) -> usize {
    state.live_order.capacity() * std::mem::size_of::<LivenessFlagId>()
}

#[cfg(test)]
fn eval_result_owned_capacity(result: &EvalResult) -> usize {
    flow_state_owned_capacity(&result.state).saturating_add(
        result
            .owned_source
            .as_ref()
            .map_or(0, cleanup_place_owned_capacity),
    )
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
fn resolved_param_owned_capacity(param: &crate::hir::ResolvedParam) -> usize {
    param.id.as_str().len() + param.name.capacity() + resolved_type_owned_capacity(&param.ty)
}

/// Build the one canonical cleanup plan for a validated resolved function.
///
/// The routine is pure: identifiers and ordering derive exclusively from HIR
/// identities, declaration order, and the independently checked inventory.
pub(crate) fn build_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<CleanupPlan, Diagnostic> {
    PlanBuilder::new(program, function)?.build()
}

#[cfg(test)]
pub(super) fn assert_expression_lowering_oracle(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) {
    let mut iterative = PlanBuilder::new(program, function).unwrap();
    let mut recursive = iterative.clone();
    let state = iterative.initial_state.clone();
    let region = CleanupRegionId(0);
    let block = BlockId(0);
    let actual = iterative.lower_expr_iterative(expression, block, state.clone(), region);
    let expected = recursive.lower_expr_recursive_reference(expression, block, state, region);
    PlanBuilder::assert_lowering_oracle(&iterative, &recursive, &actual, &expected, expression);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FlowState {
    /// Live flags in semantic initialization order.  A whole-aggregate
    /// boundary canonicalizes its destination leaves to recursive declaration
    /// order; an incomplete constructor intentionally retains actual field
    /// completion order.
    live_order: Vec<LivenessFlagId>,
}

impl FlowState {
    fn is_live(&self, flag: LivenessFlagId) -> bool {
        self.live_order.contains(&flag)
    }

    fn remove(&mut self, flags: &BTreeSet<LivenessFlagId>) {
        self.live_order.retain(|flag| !flags.contains(flag));
    }

    fn append_distinct(&mut self, flags: impl IntoIterator<Item = LivenessFlagId>) {
        for flag in flags {
            debug_assert!(!self.is_live(flag));
            self.live_order.push(flag);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvalResult {
    block: BlockId,
    state: FlowState,
    owned_source: Option<CleanupPlace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingTryResidual {
    block: BlockId,
    state: FlowState,
    region: CleanupRegionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenBlock {
    id: BlockId,
    region: CleanupRegionId,
    transitions: Vec<CleanupTransition>,
    terminator: Option<CleanupTerminator>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LeafMetadata {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

#[derive(Clone)]
struct PlanBuilder<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    slots: Vec<CleanupSlot>,
    storage_to_slot: BTreeMap<StorageId, CleanupSlotId>,
    inventory_storage: BTreeMap<InventoryStorageId, StorageId>,
    leaves: BTreeMap<LivenessFlagId, LeafMetadata>,
    next_flag: u32,
    status_sources: Vec<StatusSource>,
    blocks: Vec<OpenBlock>,
    edges: Vec<CleanupEdge>,
    regions: Vec<CleanupRegion>,
    exits: Vec<ExitTarget>,
    entry_state: CleanupEntryState,
    initial_state: FlowState,
    pending_try_residuals: Vec<PendingTryResidual>,
    schema: &'static str,
}

impl<'a> PlanBuilder<'a> {
    fn new(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
    ) -> Result<Self, Diagnostic> {
        let mut storage_to_slot = BTreeMap::new();
        let mut inventory_storage = BTreeMap::new();
        let mut slots = Vec::with_capacity(function.cleanup.slots.len());

        for inventory_slot in &function.cleanup.slots {
            let storage = match &inventory_slot.origin {
                crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }
                | crate::cleanup::CleanupStorageOrigin::Binding { value }
                | crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value } => {
                    if matches!(
                        &inventory_slot.origin,
                        crate::cleanup::CleanupStorageOrigin::ProvisionalResult { .. }
                    ) {
                        StorageId::ProvisionalResult
                    } else {
                        StorageId::Value(value.clone())
                    }
                }
                crate::cleanup::CleanupStorageOrigin::Temporary { expression } => {
                    StorageId::Temporary(expression.clone())
                }
            };
            let id = CleanupSlotId(inventory_slot.id.0);
            if storage_to_slot.insert(storage.clone(), id).is_some()
                || inventory_storage
                    .insert(inventory_slot.id, storage.clone())
                    .is_some()
            {
                return Err(plan_error("cleanup inventory has conflicting storage"));
            }
            slots.push(CleanupSlot {
                id,
                storage,
                ty: inventory_slot.ty.clone(),
                storage_index: inventory_slot.discovery_index,
                field_liveness_shape: inventory_slot.shape.clone(),
            });
        }

        let mut leaves = BTreeMap::new();
        for flag in &function.cleanup.flags {
            let storage = inventory_storage
                .get(&flag.place.storage)
                .cloned()
                .ok_or_else(|| plan_error("cleanup flag references unknown inventory storage"))?;
            if leaves
                .insert(
                    flag.id,
                    LeafMetadata {
                        place: CleanupPlace {
                            storage,
                            projections: flag.place.projections.clone(),
                        },
                        lifecycle: flag.lifecycle.clone(),
                    },
                )
                .is_some()
            {
                return Err(plan_error("cleanup inventory repeats a liveness flag"));
            }
        }
        let next_flag = u32::try_from(leaves.len())
            .map_err(|_| plan_error("too many cleanup liveness flags"))?;

        let root = CleanupRegionId(0);
        let entry = BlockId(0);
        let mut builder = Self {
            program,
            function,
            slots,
            storage_to_slot,
            inventory_storage,
            leaves,
            next_flag,
            status_sources: Vec::new(),
            blocks: vec![OpenBlock {
                id: entry,
                region: root,
                transitions: Vec::new(),
                terminator: None,
            }],
            edges: Vec::new(),
            regions: vec![CleanupRegion {
                id: root,
                parent: None,
                slots: Vec::new(),
                normal_scope_end: UNRESOLVED_EXIT,
            }],
            exits: Vec::new(),
            entry_state: CleanupEntryState {
                live_owned_parameters: Vec::new(),
            },
            initial_state: FlowState {
                live_order: Vec::new(),
            },
            pending_try_residuals: Vec::new(),
            schema: CLEANUP_PLAN_SCHEMA_V2,
        };
        builder.seed_entry(root)?;
        Ok(builder)
    }

    fn seed_entry(&mut self, root: CleanupRegionId) -> Result<(), Diagnostic> {
        for storage in &self.function.cleanup.entry_state.live_owned_parameters {
            let plan_storage = self
                .inventory_storage
                .get(storage)
                .cloned()
                .ok_or_else(|| plan_error("entry state references unknown storage"))?;
            self.assign_slot(&plan_storage, root)?;
            let place = CleanupPlace::whole(plan_storage);
            let flags = self.flags_under(&place);
            self.initial_state.append_distinct(flags);
            self.entry_state.live_owned_parameters.push(place);
        }
        if self
            .storage_to_slot
            .contains_key(&StorageId::ProvisionalResult)
        {
            self.assign_slot(&StorageId::ProvisionalResult, root)?;
        }
        Ok(())
    }

    fn build(mut self) -> Result<CleanupPlan, Diagnostic> {
        let root = CleanupRegionId(0);
        let mut current = BlockId(0);
        let mut state = self.initial_state.clone();

        for (ordinal, contract) in self.function.requires.iter().enumerate() {
            let continued = self.lower_contract_expression(
                contract,
                current,
                state,
                root,
                ContractPhase::Requires,
                ordinal,
            )?;
            current = continued.0;
            state = continued.1;
        }
        if !self.pending_try_residuals.is_empty() {
            return Err(plan_error("postfix `?` is invalid in a precondition"));
        }

        let body = self.lower_root_body(&self.function.body, current, state, root)?;
        current = body.block;
        state = body.state;
        let result_source_expression = self.function.body.id.clone();
        if let Some(source) = body.owned_source {
            let destination = CleanupPlace::whole(StorageId::ProvisionalResult);
            self.transfer(
                current,
                self.function.body.id.clone(),
                source,
                destination,
                &mut state,
                true,
            )?;
        }

        if !self.pending_try_residuals.is_empty() {
            if !self.slots.is_empty() || !state.live_order.is_empty() {
                return Err(plan_error(
                    "postfix `?` reached cleanup planning with resource leaves",
                ));
            }
            self.push_transition(
                current,
                CleanupTransition::StageCopyResult {
                    source: StagedCopyResultSource::Body {
                        expression: self.function.body.id.clone(),
                        instance: self.function.return_type.clone(),
                    },
                },
            );
            let epilogue = self.new_block(root)?;
            let normal_edge = self.new_edge(current, epilogue, EdgeCondition::Always)?;
            self.terminate(current, CleanupTerminator::Goto(normal_edge))?;

            for residual in std::mem::take(&mut self.pending_try_residuals) {
                if !residual.state.live_order.is_empty() {
                    return Err(plan_error(
                        "postfix `?` residual carries live resource leaves",
                    ));
                }
                let edge = self.new_edge(residual.block, epilogue, EdgeCondition::Always)?;
                let leaves_regions = self
                    .region_chain(residual.region)
                    .into_iter()
                    .take_while(|region| *region != root)
                    .collect::<Vec<_>>();
                if leaves_regions.is_empty() {
                    self.terminate(residual.block, CleanupTerminator::Goto(edge))?;
                } else {
                    self.emit_exit(
                        residual.block,
                        leaves_regions,
                        Vec::new(),
                        ExitContinuation::Continue(edge),
                    )?;
                }
            }
            current = epilogue;
        }

        for (ordinal, contract) in self.function.ensures.iter().enumerate() {
            let continued = self.lower_contract_expression(
                contract,
                current,
                state,
                root,
                ContractPhase::Ensures,
                ordinal,
            )?;
            current = continued.0;
            state = continued.1;
        }
        if !self.pending_try_residuals.is_empty() {
            return Err(plan_error("postfix `?` is invalid in a postcondition"));
        }

        let result = if self.result_needs_drop()? {
            CleanupResultSource::Owned {
                storage: CleanupPlace::whole(StorageId::ProvisionalResult),
            }
        } else {
            CleanupResultSource::Scalar {
                expression: result_source_expression,
            }
        };
        self.finish_success(current, state, root, result)?;

        #[cfg(test)]
        note_lower_capacity_high_water(
            self.blocks.capacity() * std::mem::size_of::<OpenBlock>()
                + self.edges.capacity() * std::mem::size_of::<CleanupEdge>()
                + self.regions.capacity() * std::mem::size_of::<CleanupRegion>()
                + self.exits.capacity() * std::mem::size_of::<ExitTarget>()
                + self.status_sources.capacity() * std::mem::size_of::<StatusSource>()
                + builder_nested_capacity(&self),
        );

        let blocks = self
            .blocks
            .into_iter()
            .map(|block| {
                Ok(CleanupBlock {
                    id: block.id,
                    region: block.region,
                    transitions: block.transitions,
                    terminator: block.terminator.ok_or_else(|| {
                        plan_error(format!("cleanup block {} is unterminated", block.id.0))
                    })?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        Ok(CleanupPlan {
            schema: self.schema,
            entry: BlockId(0),
            entry_state: self.entry_state,
            slots: self.slots,
            status_sources: self.status_sources,
            blocks,
            edges: self.edges,
            regions: self.regions,
            exits: self.exits,
        })
    }

    fn result_needs_drop(&self) -> Result<bool, Diagnostic> {
        self.needs_drop(&self.function.return_type)
    }

    fn needs_drop(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        // Owned strings free their heap buffer inline in each backend; they
        // never join the resource-lifecycle cleanup plan.
        Ok(self
            .program
            .declarations
            .type_facts(ty)
            .map(|facts| facts.needs_drop)
            .ok_or_else(|| {
                plan_error(format!("type `{}` has no cleanup facts", ty.identity_key()))
            })?
            && !matches!(ty, ResolvedType::String | ResolvedType::Str))
    }

    fn assign_slot(
        &mut self,
        storage: &StorageId,
        region: CleanupRegionId,
    ) -> Result<(), Diagnostic> {
        if !self.storage_to_slot.contains_key(storage) {
            return Err(plan_error(format!(
                "cleanup storage `{storage:?}` has no slot"
            )));
        }
        let slots = &mut self.regions[region.0 as usize].slots;
        if !slots.contains(storage) {
            slots.push(storage.clone());
        }
        Ok(())
    }

    fn add_supplemental_slot(
        &mut self,
        storage: StorageId,
        ty: ResolvedType,
        region: CleanupRegionId,
    ) -> Result<(), Diagnostic> {
        if self.storage_to_slot.contains_key(&storage) {
            return Err(plan_error(format!(
                "cleanup storage `{storage:?}` is duplicated"
            )));
        }
        let index = u32::try_from(self.slots.len())
            .map_err(|_| plan_error("too many cleanup plan slots"))?;
        let shape = self.shape_for_type(&ty, &storage, &mut Vec::new())?;
        self.slots.push(CleanupSlot {
            id: CleanupSlotId(index),
            storage: storage.clone(),
            ty,
            storage_index: index,
            field_liveness_shape: shape,
        });
        self.storage_to_slot
            .insert(storage.clone(), CleanupSlotId(index));
        self.regions[region.0 as usize].slots.push(storage);
        Ok(())
    }

    fn shape_for_type(
        &mut self,
        ty: &ResolvedType,
        storage: &StorageId,
        projections: &mut Vec<DeclarationId>,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        if !self.needs_drop(ty)? {
            return Ok(FieldLivenessShape::NoDrop);
        }
        if matches!(ty, ResolvedType::Bytes) {
            if !projections.is_empty() {
                return Err(plan_error(
                    "compiler-owned Bytes cleanup leaf is not direct",
                ));
            }
            let flag = LivenessFlagId(self.next_flag);
            self.next_flag = self
                .next_flag
                .checked_add(1)
                .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
            let lifecycle = DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID);
            self.leaves.insert(
                flag,
                LeafMetadata {
                    place: CleanupPlace {
                        storage: storage.clone(),
                        projections: projections.clone(),
                    },
                    lifecycle: lifecycle.clone(),
                },
            );
            return Ok(FieldLivenessShape::Leaf { flag, lifecycle });
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Err(plan_error("droppable cleanup-plan type is not nominal"));
        };
        if !arguments.is_empty() {
            return Err(plan_error("generic cleanup-plan storage is unsupported"));
        }
        let item = self
            .program
            .types
            .iter()
            .find(|item| item.id == *declaration)
            .ok_or_else(|| plan_error(format!("unknown cleanup type `{declaration}`")))?;
        match &item.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                let flag = LivenessFlagId(self.next_flag);
                self.next_flag = self
                    .next_flag
                    .checked_add(1)
                    .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
                self.leaves.insert(
                    flag,
                    LeafMetadata {
                        place: CleanupPlace {
                            storage: storage.clone(),
                            projections: projections.clone(),
                        },
                        lifecycle: drop.id.clone(),
                    },
                );
                Ok(FieldLivenessShape::Leaf {
                    flag,
                    lifecycle: drop.id.clone(),
                })
            }
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                let mut shapes = Vec::with_capacity(fields.len());
                for field in fields {
                    projections.push(field.id.clone());
                    let shape = self.shape_for_type(&field.ty, storage, projections)?;
                    projections.pop();
                    shapes.push(FieldLiveness {
                        field: field.id.clone(),
                        field_index: field.index,
                        shape,
                    });
                }
                Ok(FieldLivenessShape::Record {
                    declaration: declaration.clone(),
                    fields: shapes,
                })
            }
            ResolvedTypeDeclarationKind::Variant { .. } => Err(plan_error(
                "droppable variant cleanup is outside the copy-only v1 slice",
            )),
        }
    }

    fn flags_under(&self, place: &CleanupPlace) -> Vec<LivenessFlagId> {
        self.leaves
            .iter()
            .filter_map(|(flag, metadata)| {
                (metadata.place.storage == place.storage
                    && metadata.place.projections.starts_with(&place.projections))
                .then_some(*flag)
            })
            .collect()
    }

    fn place_from_hir(&self, place: &Place) -> Result<CleanupPlace, Diagnostic> {
        let storage = if place.root == self.function.result_id {
            StorageId::ProvisionalResult
        } else {
            StorageId::Value(place.root.clone())
        };
        let mut resolved = CleanupPlace::whole(storage);
        if !self.storage_to_slot.contains_key(&resolved.storage) {
            return Err(plan_error(format!(
                "owned place `{}` has no cleanup storage",
                place.root
            )));
        }
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => resolved.projections.push(field.clone()),
                PlaceProjection::VariantField { .. } => {
                    return Err(plan_error(
                        "variant field reached cleanup planning before variant support",
                    ));
                }
            }
        }
        if self.flags_under(&resolved).is_empty() {
            return Err(plan_error(format!(
                "owned place `{}` has no droppable leaf",
                place.root
            )));
        }
        Ok(resolved)
    }

    fn expression_slot(
        &mut self,
        expression: &ResolvedExpr,
        region: CleanupRegionId,
    ) -> Result<Option<CleanupPlace>, Diagnostic> {
        if expression.ownership != OwnershipMode::Own || !self.needs_drop(&expression.ty)? {
            return Ok(None);
        }
        let storage = StorageId::Temporary(expression.id.clone());
        self.assign_slot(&storage, region)?;
        Ok(Some(CleanupPlace::whole(storage)))
    }

    fn binding_slot(
        &mut self,
        binding: &crate::hir::ResolvedBinding,
        region: CleanupRegionId,
    ) -> Result<Option<CleanupPlace>, Diagnostic> {
        if binding.ownership != OwnershipMode::Own || !self.needs_drop(&binding.ty)? {
            return Ok(None);
        }
        let storage = StorageId::Value(binding.id.clone());
        self.assign_slot(&storage, region)?;
        Ok(Some(CleanupPlace::whole(storage)))
    }

    fn call_argument_slot(
        &mut self,
        call: &ResolvedExpr,
        parameter_index: usize,
        value: &ResolvedExpr,
        region: CleanupRegionId,
    ) -> Result<CleanupPlace, Diagnostic> {
        let parameter_index =
            u32::try_from(parameter_index).map_err(|_| plan_error("too many call arguments"))?;
        let storage = StorageId::CallArgument {
            call: call.id.clone(),
            parameter_index,
            value_expression: value.id.clone(),
        };
        self.add_supplemental_slot(storage.clone(), value.ty.clone(), region)?;
        Ok(CleanupPlace::whole(storage))
    }

    fn push_transition(&mut self, block: BlockId, transition: CleanupTransition) {
        self.blocks[block.0 as usize].transitions.push(transition);
    }

    fn transfer(
        &mut self,
        block: BlockId,
        at: ExpressionId,
        source: CleanupPlace,
        destination: CleanupPlace,
        state: &mut FlowState,
        normalize_complete_aggregate: bool,
    ) -> Result<(), Diagnostic> {
        let source_flags = self.flags_under(&source);
        let destination_flags = self.flags_under(&destination);
        if source_flags.len() != destination_flags.len() || source_flags.is_empty() {
            return Err(plan_error(format!(
                "transfer at `{at}` has incompatible cleanup shapes"
            )));
        }
        if source_flags.iter().any(|flag| !state.is_live(*flag)) {
            return Err(plan_error(format!(
                "transfer at `{at}` reads a non-live cleanup place"
            )));
        }
        if destination_flags.iter().any(|flag| state.is_live(*flag)) {
            return Err(plan_error(format!(
                "transfer at `{at}` initializes a live cleanup place"
            )));
        }

        let source_set = source_flags.iter().copied().collect::<BTreeSet<_>>();
        let source_history = state
            .live_order
            .iter()
            .filter(|flag| source_set.contains(flag))
            .copied()
            .collect::<Vec<_>>();
        state.remove(&source_set);

        let destination_order = if normalize_complete_aggregate {
            destination_flags
        } else {
            let mut mapped = Vec::with_capacity(source_history.len());
            for source_flag in source_history {
                let source_leaf = &self.leaves[&source_flag].place;
                let relative = source_leaf
                    .projections
                    .strip_prefix(source.projections.as_slice())
                    .ok_or_else(|| plan_error("invalid cleanup transfer source prefix"))?;
                let expected = destination
                    .projections
                    .iter()
                    .chain(relative)
                    .cloned()
                    .collect::<Vec<_>>();
                let destination_flag = destination_flags
                    .iter()
                    .find(|flag| self.leaves[flag].place.projections == expected)
                    .copied()
                    .ok_or_else(|| plan_error("cleanup transfer leaf mismatch"))?;
                mapped.push(destination_flag);
            }
            mapped
        };
        state.append_distinct(destination_order);
        self.push_transition(
            block,
            CleanupTransition::Transfer {
                at,
                source,
                destination,
            },
        );
        Ok(())
    }

    fn initialize(
        &mut self,
        block: BlockId,
        at: ExpressionId,
        destination: CleanupPlace,
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        let flags = self.flags_under(&destination);
        if flags.iter().any(|flag| state.is_live(*flag)) {
            return Err(plan_error("cleanup initialization targets a live place"));
        }
        state.append_distinct(flags);
        self.push_transition(block, CleanupTransition::Initialize { at, destination });
        Ok(())
    }

    fn canonicalize_complete_aggregate(
        &self,
        place: &CleanupPlace,
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        let flags = self.flags_under(place);
        if flags.iter().any(|flag| !state.is_live(*flag)) {
            return Err(plan_error(
                "cannot normalize an incompletely initialized aggregate",
            ));
        }
        let set = flags.iter().copied().collect::<BTreeSet<_>>();
        state.remove(&set);
        state.append_distinct(flags);
        Ok(())
    }

    fn new_block(&mut self, region: CleanupRegionId) -> Result<BlockId, Diagnostic> {
        let index =
            u32::try_from(self.blocks.len()).map_err(|_| plan_error("too many cleanup blocks"))?;
        let id = BlockId(index);
        self.blocks.push(OpenBlock {
            id,
            region,
            transitions: Vec::new(),
            terminator: None,
        });
        Ok(id)
    }

    fn new_edge(
        &mut self,
        from: BlockId,
        to: BlockId,
        condition: EdgeCondition,
    ) -> Result<EdgeId, Diagnostic> {
        let index =
            u32::try_from(self.edges.len()).map_err(|_| plan_error("too many cleanup edges"))?;
        let id = EdgeId(index);
        self.edges.push(CleanupEdge {
            id,
            from,
            to,
            condition,
        });
        Ok(id)
    }

    fn terminate(
        &mut self,
        block: BlockId,
        terminator: CleanupTerminator,
    ) -> Result<(), Diagnostic> {
        let current = &mut self.blocks[block.0 as usize].terminator;
        if current.replace(terminator).is_some() {
            return Err(plan_error(format!(
                "cleanup block {} has two terminators",
                block.0
            )));
        }
        Ok(())
    }

    fn add_status_source(
        &mut self,
        id: StatusSourceId,
        producer: StatusProducer,
    ) -> Result<(), Diagnostic> {
        if self.status_sources.iter().any(|source| source.id == id) {
            return Err(plan_error(format!(
                "duplicate cleanup status source for `{}`",
                id.expression
            )));
        }
        self.status_sources.push(StatusSource { id, producer });
        Ok(())
    }

    fn finalizers_for(
        &self,
        state: &FlowState,
        included: impl Fn(&CleanupPlace) -> bool,
    ) -> Vec<FinalizeAction> {
        state
            .live_order
            .iter()
            .rev()
            .filter_map(|flag| {
                let metadata = &self.leaves[flag];
                included(&metadata.place).then(|| FinalizeAction {
                    source: metadata.place.clone(),
                    lifecycle_id: metadata.lifecycle.clone(),
                    guard_flag: *flag,
                })
            })
            .collect()
    }

    fn region_chain(&self, mut region: CleanupRegionId) -> Vec<CleanupRegionId> {
        let mut chain = Vec::new();
        loop {
            chain.push(region);
            let Some(parent) = self.regions[region.0 as usize].parent else {
                break;
            };
            region = parent;
        }
        chain
    }

    fn emit_exit(
        &mut self,
        from: BlockId,
        leaves_regions: Vec<CleanupRegionId>,
        finalize_in_order: Vec<FinalizeAction>,
        continuation: ExitContinuation,
    ) -> Result<ExitTargetId, Diagnostic> {
        let index =
            u32::try_from(self.exits.len()).map_err(|_| plan_error("too many cleanup exits"))?;
        let id = ExitTargetId(index);
        self.exits.push(ExitTarget {
            id,
            from,
            leaves_regions,
            finalize_in_order,
            continuation,
        });
        self.terminate(from, CleanupTerminator::Exit(id))?;
        Ok(id)
    }

    fn emit_failure(
        &mut self,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
        source: StatusSourceId,
    ) -> Result<(), Diagnostic> {
        self.push_transition(
            block,
            CleanupTransition::SelectFailure {
                source: source.clone(),
            },
        );
        let finalizers = self.finalizers_for(&state, |_| true);
        self.emit_exit(
            block,
            self.region_chain(region),
            finalizers,
            ExitContinuation::ReturnFailure { source },
        )?;
        Ok(())
    }

    /// Lower one Bounded While-Loops v1 statement.
    ///
    /// The admission profile guarantees the condition and body contain only
    /// Copy-scalar operations, so the loop contributes no cleanup slots,
    /// transfers, or finalizers of its own: every failure exit inside the
    /// loop finalizes exactly what was live on loop entry. The plan therefore
    /// linearizes one admitted iteration — condition evaluation branches on
    /// its Boolean result into a single body pass or the loop continuation,
    /// and the builder fail-closes if the body pass could ever change owned
    /// liveness (which would make a single pass unrepresentative).
    fn lower_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let entry_state = state.clone();
        let evaluated_condition = self.lower_expr(condition, block, state, region)?;
        if evaluated_condition.owned_source.is_some() {
            return Err(plan_error(
                "while condition owns a value, which no admitted program can express",
            ));
        }
        let body_entry = self.new_block(region)?;
        let after = self.new_block(region)?;
        let true_edge = self.new_edge(
            evaluated_condition.block,
            body_entry,
            EdgeCondition::BooleanResult(condition.id.clone(), true),
        )?;
        let false_edge = self.new_edge(
            evaluated_condition.block,
            after,
            EdgeCondition::BooleanResult(condition.id.clone(), false),
        )?;
        self.terminate(
            evaluated_condition.block,
            CleanupTerminator::Branch(vec![true_edge, false_edge]),
        )?;

        // The body is an ordinary checked block; lowering it once yields the
        // exact per-iteration ownership events of any iteration count.
        let evaluated_body =
            self.lower_expr(body, body_entry, evaluated_condition.state.clone(), region)?;
        if evaluated_body.state != evaluated_condition.state
            || evaluated_body.owned_source.is_some()
        {
            return Err(plan_error(
                "while loop body changes owned liveness, which the Bounded While-Loops v1 admission profile forbids",
            ));
        }
        let join_edge = self.new_edge(evaluated_body.block, after, EdgeCondition::Always)?;
        self.terminate(evaluated_body.block, CleanupTerminator::Goto(join_edge))?;
        Ok(EvalResult {
            block: after,
            state: entry_state,
            owned_source: None,
        })
    }

    /// Refutable Match v1 recursive-reference twin: one linearized pass
    /// whose Boolean joins mirror the while model, with fail-closed
    /// owned-liveness equality at the join.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn lower_scalar_match(
        &mut self,
        expression: &ResolvedExpr,
        scrutinee: &ResolvedExpr,
        arms: &[ResolvedMatchArm],
        decision_start: BlockId,
        branch_state: FlowState,
        region: CleanupRegionId,
        destination: Option<CleanupPlace>,
    ) -> Result<EvalResult, Diagnostic> {
        if arms.is_empty() {
            return Err(plan_error("refutable match has no arms"));
        }
        let entry_state = branch_state.clone();
        let mut decision = decision_start;
        let mut arm_results = Vec::with_capacity(arms.len());
        for (index, arm) in arms.iter().enumerate() {
            let final_arm = index + 1 == arms.len();
            let arm_entry = self.new_block(region)?;
            if final_arm {
                let edge = self.new_edge(decision, arm_entry, EdgeCondition::Always)?;
                self.terminate(decision, CleanupTerminator::Goto(edge))?;
            } else {
                let next_decision = self.new_block(region)?;
                let selected = self.new_edge(
                    decision,
                    arm_entry,
                    EdgeCondition::ArmSelected {
                        scrutinee: scrutinee.id.clone(),
                        arm: u32::try_from(index).map_err(|_| plan_error("too many match arms"))?,
                        selected: true,
                    },
                )?;
                let rejected = self.new_edge(
                    decision,
                    next_decision,
                    EdgeCondition::ArmSelected {
                        scrutinee: scrutinee.id.clone(),
                        arm: u32::try_from(index).map_err(|_| plan_error("too many match arms"))?,
                        selected: false,
                    },
                )?;
                self.terminate(
                    decision,
                    CleanupTerminator::Branch(vec![selected, rejected]),
                )?;
                decision = next_decision;
            }
            let value_block = if let Some(guard) = &arm.guard {
                let evaluated_guard = self.lower_expr_recursive_reference(
                    guard.as_ref(),
                    arm_entry,
                    branch_state.clone(),
                    region,
                )?;
                if evaluated_guard.owned_source.is_some() {
                    return Err(plan_error(
                        "scalar match guard owns a value, which no admitted program can express",
                    ));
                }
                let value_entry = self.new_block(region)?;
                let true_edge = self.new_edge(
                    evaluated_guard.block,
                    value_entry,
                    EdgeCondition::BooleanResult(guard.id.clone(), true),
                )?;
                let false_edge = self.new_edge(
                    evaluated_guard.block,
                    decision,
                    EdgeCondition::BooleanResult(guard.id.clone(), false),
                )?;
                self.terminate(
                    evaluated_guard.block,
                    CleanupTerminator::Branch(vec![true_edge, false_edge]),
                )?;
                value_entry
            } else {
                arm_entry
            };
            let mut result = self.lower_expr_recursive_reference(
                &arm.value,
                value_block,
                branch_state.clone(),
                region,
            )?;
            if let Some(destination) = destination.clone() {
                let source = result
                    .owned_source
                    .take()
                    .ok_or_else(|| plan_error("owned scalar match arm has no cleanup source"))?;
                self.transfer(
                    result.block,
                    expression.id.clone(),
                    source,
                    destination,
                    &mut result.state,
                    true,
                )?;
            }
            arm_results.push(result);
        }
        let mut arm_results = arm_results.into_iter();
        let first = arm_results
            .next()
            .ok_or_else(|| plan_error("refutable match produced no arm result"))?;
        let mut merged_state = first.state.clone();
        let mut completed = vec![first];
        for result in arm_results {
            merged_state = self.merge_states(&merged_state, &result.state)?;
            completed.push(result);
        }
        if merged_state != entry_state {
            return Err(plan_error(
                "refutable match changes owned liveness, which the Refutable Match v1 \
                 admission profile forbids",
            ));
        }
        let join = self.new_block(region)?;
        for result in completed {
            let edge = self.new_edge(result.block, join, EdgeCondition::Always)?;
            self.terminate(result.block, CleanupTerminator::Goto(edge))?;
        }
        Ok(EvalResult {
            block: join,
            state: merged_state,
            owned_source: None,
        })
    }

    fn split_status(
        &mut self,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
        source: StatusSourceId,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        let success = self.new_block(region)?;
        let failure = self.new_block(region)?;
        let success_edge =
            self.new_edge(block, success, EdgeCondition::StatusZero(source.clone()))?;
        let failure_edge =
            self.new_edge(block, failure, EdgeCondition::StatusNonzero(source.clone()))?;
        self.terminate(
            block,
            CleanupTerminator::Branch(vec![success_edge, failure_edge]),
        )?;
        self.emit_failure(failure, state.clone(), region, source)?;
        Ok((success, state))
    }

    fn lower_contract(
        &mut self,
        contract: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
        phase: ContractPhase,
        ordinal: usize,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        let source = StatusSourceId {
            expression: contract.id.clone(),
            lane: StatusLane::ContractFalse,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::ContractFalse {
                phase,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| plan_error("too many function contracts"))?,
            },
        )?;
        let success = self.new_block(region)?;
        let failure = self.new_block(region)?;
        let success_edge = self.new_edge(
            block,
            success,
            EdgeCondition::BooleanResult(contract.id.clone(), true),
        )?;
        let failure_edge = self.new_edge(
            block,
            failure,
            EdgeCondition::BooleanResult(contract.id.clone(), false),
        )?;
        self.terminate(
            block,
            CleanupTerminator::Branch(vec![success_edge, failure_edge]),
        )?;
        self.emit_failure(failure, state.clone(), region, source)?;
        Ok((success, state))
    }

    fn lower_contract_expression(
        &mut self,
        contract: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        parent: CleanupRegionId,
        phase: ContractPhase,
        ordinal: usize,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        let region = self.new_region(parent)?;
        let entry = self.new_block(region)?;
        let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
        self.terminate(block, CleanupTerminator::Goto(edge))?;
        let evaluated = self.lower_expr(contract, entry, state, region)?;
        let (success, state) = self.lower_contract(
            contract,
            evaluated.block,
            evaluated.state,
            region,
            phase,
            ordinal,
        )?;
        self.exit_scope(success, state, region)
    }

    fn checked_source(
        &mut self,
        expression: &ResolvedExpr,
        operation: CheckedOperation,
        normalized_cases: Vec<StatusCase>,
    ) -> Result<StatusSourceId, Diagnostic> {
        let source = StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::OperationFailure,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::CheckedArithmetic {
                operation,
                normalized_cases,
            },
        )?;
        Ok(source)
    }

    fn new_region(&mut self, parent: CleanupRegionId) -> Result<CleanupRegionId, Diagnostic> {
        let index = u32::try_from(self.regions.len())
            .map_err(|_| plan_error("too many cleanup regions"))?;
        let id = CleanupRegionId(index);
        self.regions.push(CleanupRegion {
            id,
            parent: Some(parent),
            slots: Vec::new(),
            normal_scope_end: UNRESOLVED_EXIT,
        });
        Ok(id)
    }

    fn exit_scope(
        &mut self,
        block: BlockId,
        mut state: FlowState,
        region: CleanupRegionId,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        let parent = self.regions[region.0 as usize]
            .parent
            .ok_or_else(|| plan_error("cannot continue after root cleanup region"))?;
        let region_storages = self.regions[region.0 as usize]
            .slots
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let finalized_flags = state
            .live_order
            .iter()
            .filter(|flag| region_storages.contains(&self.leaves[flag].place.storage))
            .copied()
            .collect::<BTreeSet<_>>();
        let finalizers =
            self.finalizers_for(&state, |place| region_storages.contains(&place.storage));
        state.remove(&finalized_flags);

        let continuation_block = self.new_block(parent)?;
        let edge = self.new_edge(block, continuation_block, EdgeCondition::Always)?;
        let exit = self.emit_exit(
            block,
            vec![region],
            finalizers,
            ExitContinuation::Continue(edge),
        )?;
        let normal_end = &mut self.regions[region.0 as usize].normal_scope_end;
        if *normal_end != UNRESOLVED_EXIT {
            return Err(plan_error("cleanup region has multiple normal scope ends"));
        }
        *normal_end = exit;
        Ok((continuation_block, state))
    }

    fn merge_states(&self, left: &FlowState, right: &FlowState) -> Result<FlowState, Diagnostic> {
        let flags = left
            .live_order
            .iter()
            .chain(&right.live_order)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut successors = flags
            .iter()
            .map(|flag| (*flag, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut indegree = flags
            .iter()
            .map(|flag| (*flag, 0_u32))
            .collect::<BTreeMap<_, _>>();
        for history in [&left.live_order, &right.live_order] {
            for pair in history.windows(2) {
                if successors
                    .get_mut(&pair[0])
                    .expect("joined flag is indexed")
                    .insert(pair[1])
                {
                    *indegree.get_mut(&pair[1]).expect("joined flag is indexed") += 1;
                }
            }
        }
        let mut ready = indegree
            .iter()
            .filter_map(|(flag, degree)| (*degree == 0).then_some(*flag))
            .collect::<BTreeSet<_>>();
        let mut live_order = Vec::with_capacity(flags.len());
        while let Some(flag) = ready.pop_first() {
            live_order.push(flag);
            for successor in successors[&flag].iter().copied() {
                let degree = indegree
                    .get_mut(&successor)
                    .expect("joined flag is indexed");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(successor);
                }
            }
        }
        if live_order.len() != flags.len() {
            return Err(plan_error(
                "branch join has conflicting cleanup initialization histories",
            ));
        }
        Ok(FlowState { live_order })
    }

    fn finish_success(
        &mut self,
        block: BlockId,
        state: FlowState,
        root: CleanupRegionId,
        result: CleanupResultSource,
    ) -> Result<(), Diagnostic> {
        let finalizers = self.finalizers_for(&state, |place| {
            place.storage != StorageId::ProvisionalResult
        });
        let exit = self.emit_exit(
            block,
            vec![root],
            finalizers,
            ExitContinuation::CommitResult { source: result },
        )?;
        self.regions[root.0 as usize].normal_scope_end = exit;
        Ok(())
    }

    fn consume_place(
        &self,
        place: &CleanupPlace,
        state: &mut FlowState,
        at: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        let flags = self.flags_under(place);
        if flags.is_empty() || flags.iter().any(|flag| !state.is_live(*flag)) {
            return Err(plan_error(format!(
                "call commit at `{at}` consumes a non-live argument epoch"
            )));
        }
        state.remove(&flags.into_iter().collect());
        Ok(())
    }

    fn lower_root_body(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        root: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let ResolvedExprKind::Block { statements, tail } = &expression.kind else {
            return self.lower_expr(expression, block, state, root);
        };

        // The function-body lexical scope is the root cleanup region.  Its
        // locals remain alive through `ensures`; only the final success/failure
        // epilogue may destroy them.
        let destination = self.expression_slot(expression, root)?;
        let mut current = block;
        let mut current_state = state;
        for statement in statements {
            // Field Mutation v1 stores replace one scalar Copy field; they
            // lower their RHS like an initializer and add no cleanup
            // structure and never transfer into a binding slot.
            if let ResolvedStatement::Assign {
                field: Some(_),
                value,
                ..
            } = statement
            {
                let evaluated = self.lower_expr(value, current, current_state, root)?;
                current = evaluated.block;
                current_state = evaluated.state;
                continue;
            }
            let (binding, value) = match statement {
                ResolvedStatement::Let { binding, value, .. }
                | ResolvedStatement::Assign { binding, value, .. } => (binding, value),
                // Unsafe boundaries bind nothing: their ordinary block body
                // lowers like any nested block expression and owns nothing at
                // this level.
                ResolvedStatement::Unsafe { body, .. } => {
                    let evaluated = self.lower_expr(body, current, current_state, root)?;
                    current = evaluated.block;
                    current_state = evaluated.state;
                    continue;
                }
                // Bounded While-Loops v1: linearize one admitted iteration.
                ResolvedStatement::While {
                    condition, body, ..
                } => {
                    let evaluated =
                        self.lower_while(condition, body, current, current_state, root)?;
                    current = evaluated.block;
                    current_state = evaluated.state;
                    continue;
                }
            };
            let evaluated = self.lower_expr(value, current, current_state, root)?;
            current = evaluated.block;
            current_state = evaluated.state;
            if let Some(binding_place) = self.binding_slot(binding, root)? {
                let source = evaluated.owned_source.ok_or_else(|| {
                    plan_error(format!(
                        "owned binding `{}` has no value source",
                        binding.id
                    ))
                })?;
                self.transfer(
                    current,
                    value.id.clone(),
                    source,
                    binding_place,
                    &mut current_state,
                    true,
                )?;
            }
        }
        let tail = self.lower_expr(tail, current, current_state, root)?;
        current = tail.block;
        current_state = tail.state;
        if let Some(destination) = destination.clone() {
            let source = tail
                .owned_source
                .ok_or_else(|| plan_error("owned root block tail has no cleanup source"))?;
            self.transfer(
                current,
                expression.id.clone(),
                source,
                destination,
                &mut current_state,
                true,
            )?;
        }
        Ok(EvalResult {
            block: current,
            state: current_state,
            owned_source: destination,
        })
    }

    fn lower_expr(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        self.lower_expr_iterative(expression, block, state, region)
    }

    #[cfg(test)]
    fn assert_lowering_oracle(
        actual_builder: &Self,
        expected_builder: &Self,
        actual: &Result<EvalResult, Diagnostic>,
        expected: &Result<EvalResult, Diagnostic>,
        expression: &ResolvedExpr,
    ) {
        match (actual, expected) {
            (Ok(actual), Ok(expected)) => assert_eq!(
                actual, expected,
                "cleanup lowering result differs at {}",
                expression.id
            ),
            (Err(actual), Err(expected)) => {
                assert_eq!(actual.code, expected.code);
                assert_eq!(actual.severity, expected.severity);
                assert_eq!(actual.message, expected.message);
                assert_eq!(actual.path, expected.path);
                assert_eq!(actual.span, expected.span);
                assert_eq!(actual.help, expected.help);
            }
            (actual, expected) => panic!(
                "cleanup lowering outcome differs at {}: actual={actual:?} expected={expected:?}",
                expression.id
            ),
        }
        assert_eq!(actual_builder.slots, expected_builder.slots);
        assert_eq!(
            actual_builder.storage_to_slot,
            expected_builder.storage_to_slot
        );
        assert_eq!(
            actual_builder.inventory_storage,
            expected_builder.inventory_storage
        );
        assert_eq!(actual_builder.leaves, expected_builder.leaves);
        assert_eq!(actual_builder.next_flag, expected_builder.next_flag);
        assert_eq!(
            actual_builder.status_sources,
            expected_builder.status_sources
        );
        assert_eq!(actual_builder.blocks, expected_builder.blocks);
        assert_eq!(actual_builder.edges, expected_builder.edges);
        assert_eq!(actual_builder.regions, expected_builder.regions);
        assert_eq!(actual_builder.exits, expected_builder.exits);
        assert_eq!(actual_builder.entry_state, expected_builder.entry_state);
        assert_eq!(actual_builder.initial_state, expected_builder.initial_state);
        assert_eq!(
            actual_builder.pending_try_residuals,
            expected_builder.pending_try_residuals
        );
        assert_eq!(actual_builder.schema, expected_builder.schema);
    }

    fn lower_expr_iterative(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        enum Frame<'e> {
            RestoreRegion(CleanupRegionId),
            Enter {
                expression: &'e ResolvedExpr,
                block: BlockId,
                state: FlowState,
            },
            Unary {
                expression: &'e ResolvedExpr,
                op: UnaryOp,
            },
            BinaryLeft {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                right: &'e ResolvedExpr,
            },
            BinaryRight {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
            },
            LazyAfterLeft {
                operation: BinaryOp,
                left_id: ExpressionId,
                right: &'e ResolvedExpr,
            },
            LazyAfterRight {
                left_state: FlowState,
                skip: BlockId,
            },
            IfAfterCondition {
                expression: &'e ResolvedExpr,
                then_branch: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
            },
            IfAfterThen {
                expression: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                else_entry: BlockId,
                condition_state: FlowState,
                destination: Option<CleanupPlace>,
            },
            IfAfterElse {
                expression: &'e ResolvedExpr,
                then_result: EvalResult,
                destination: Option<CleanupPlace>,
            },
            Project {
                expression: &'e ResolvedExpr,
                field: &'e DeclarationId,
            },
            UpcastAfterSource,
            NativeNext {
                args: &'e [ResolvedExpr],
                index: usize,
                flow: EvalResult,
            },
            NativeAfterArg {
                args: &'e [ResolvedExpr],
                index: usize,
            },
            HostCommandNext {
                expression: &'e ResolvedExpr,
                operation: crate::hir::ResolvedHostCommandOperation,
                args: &'e [ResolvedExpr],
                index: usize,
                flow: EvalResult,
            },
            HostCommandAfterArg {
                expression: &'e ResolvedExpr,
                operation: crate::hir::ResolvedHostCommandOperation,
                args: &'e [ResolvedExpr],
                index: usize,
            },
            ByteRangeAfterSource {
                expression: &'e ResolvedExpr,
                start: &'e ResolvedExpr,
                end: &'e ResolvedExpr,
            },
            ByteRangeAfterStart {
                expression: &'e ResolvedExpr,
                end: &'e ResolvedExpr,
            },
            ByteRangeAfterEnd {
                expression: &'e ResolvedExpr,
            },
            CallNext {
                expression: &'e ResolvedExpr,
                callee: &'e DeclarationId,
                args: &'e [ResolvedExpr],
                params: Vec<crate::hir::ResolvedParam>,
                index: usize,
                flow: EvalResult,
                commits: Vec<CallArgumentTransfer>,
            },
            CallAfterArg {
                expression: &'e ResolvedExpr,
                callee: &'e DeclarationId,
                args: &'e [ResolvedExpr],
                params: Vec<crate::hir::ResolvedParam>,
                index: usize,
                commits: Vec<CallArgumentTransfer>,
            },
            BlockNext {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                flow: EvalResult,
                child_region: CleanupRegionId,
                destination: Option<CleanupPlace>,
            },
            BlockAfterStatement {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                child_region: CleanupRegionId,
                destination: Option<CleanupPlace>,
            },
            BlockAfterTail {
                expression: &'e ResolvedExpr,
                child_region: CleanupRegionId,
                destination: Option<CleanupPlace>,
            },
            RecordNext {
                expression: &'e ResolvedExpr,
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                index: usize,
                flow: EvalResult,
                destination: Option<CleanupPlace>,
            },
            RecordAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                index: usize,
                destination: Option<CleanupPlace>,
            },
            VariantNext {
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                index: usize,
                flow: EvalResult,
            },
            VariantAfterField {
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                index: usize,
            },
            TryAfterOperand {
                expression: &'e ResolvedExpr,
                operand: &'e ResolvedExpr,
                result: &'e DeclarationId,
                ok_case: &'e DeclarationId,
                ok_field: &'e DeclarationId,
                err_case: &'e DeclarationId,
                err_field: &'e DeclarationId,
                residual_type: &'e ResolvedType,
            },
            TryOptionAfterOperand {
                expression: &'e ResolvedExpr,
                operand: &'e ResolvedExpr,
                option: &'e DeclarationId,
                some_case: &'e DeclarationId,
                some_field: &'e DeclarationId,
                none_case: &'e DeclarationId,
                residual_type: &'e ResolvedType,
            },
            MatchAfterScrutinee {
                expression: &'e ResolvedExpr,
                scrutinee: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
            },
            MatchRecordAfterArm,
            MatchNext {
                scrutinee: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                decision: BlockId,
                branch_state: FlowState,
                arm_results: Vec<EvalResult>,
            },
            MatchAfterArm {
                scrutinee: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                decision: BlockId,
                branch_state: FlowState,
                arm_results: Vec<EvalResult>,
            },
            /// Refutable Match v1 decision chain over a Copy-scalar
            /// scrutinee: one linearized pass whose Boolean joins mirror the
            /// while model. Owned-liveness equality is fail-closed.
            ScalarMatchNext {
                expression: &'e ResolvedExpr,
                scrutinee: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                decision: BlockId,
                branch_state: FlowState,
                arm_results: Vec<EvalResult>,
                destination: Option<CleanupPlace>,
            },
            ScalarMatchAfterGuard {
                expression: &'e ResolvedExpr,
                scrutinee: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                decision: BlockId,
                branch_state: FlowState,
                arm_results: Vec<EvalResult>,
                destination: Option<CleanupPlace>,
            },
            ScalarMatchAfterArm {
                expression: &'e ResolvedExpr,
                scrutinee: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                decision: BlockId,
                branch_state: FlowState,
                arm_results: Vec<EvalResult>,
                destination: Option<CleanupPlace>,
            },
            UpdateAfterBase {
                expression: &'e ResolvedExpr,
                record: &'e DeclarationId,
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                destination: Option<CleanupPlace>,
                update_region: CleanupRegionId,
            },
            UpdateNext {
                expression: &'e ResolvedExpr,
                record: &'e DeclarationId,
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                index: usize,
                flow: EvalResult,
                destination: Option<CleanupPlace>,
                update_region: CleanupRegionId,
                staged_base: Option<CleanupPlace>,
                replaced: BTreeSet<DeclarationId>,
            },
            UpdateAfterField {
                expression: &'e ResolvedExpr,
                record: &'e DeclarationId,
                fields: &'e [crate::hir::ResolvedFieldInitializer],
                index: usize,
                destination: Option<CleanupPlace>,
                update_region: CleanupRegionId,
                staged_base: Option<CleanupPlace>,
                replaced: BTreeSet<DeclarationId>,
            },
        }
        const { assert!(std::mem::size_of::<Frame<'static>>() == 344) };
        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame<'_>) -> usize {
            let destination = |place: &Option<CleanupPlace>| {
                place.as_ref().map_or(0, cleanup_place_owned_capacity)
            };
            let results = |values: &Vec<EvalResult>| {
                values.capacity() * std::mem::size_of::<EvalResult>()
                    + values.iter().map(eval_result_owned_capacity).sum::<usize>()
            };
            match frame {
                Frame::Enter { state, .. }
                | Frame::LazyAfterRight {
                    left_state: state, ..
                } => flow_state_owned_capacity(state),
                Frame::LazyAfterLeft { left_id, .. } => left_id.as_str().len(),
                Frame::IfAfterThen {
                    condition_state,
                    destination: place,
                    ..
                } => flow_state_owned_capacity(condition_state) + destination(place),
                Frame::IfAfterElse {
                    then_result,
                    destination: place,
                    ..
                } => eval_result_owned_capacity(then_result) + destination(place),
                Frame::NativeNext { flow, .. }
                | Frame::HostCommandNext { flow, .. }
                | Frame::VariantNext { flow, .. } => eval_result_owned_capacity(flow),
                Frame::BlockNext {
                    flow,
                    destination: place,
                    ..
                }
                | Frame::RecordNext {
                    flow,
                    destination: place,
                    ..
                } => eval_result_owned_capacity(flow) + destination(place),
                Frame::CallNext {
                    params,
                    flow,
                    commits,
                    ..
                } => {
                    params.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>()
                        + params
                            .iter()
                            .map(resolved_param_owned_capacity)
                            .sum::<usize>()
                        + eval_result_owned_capacity(flow)
                        + commits.capacity() * std::mem::size_of::<CallArgumentTransfer>()
                        + commits
                            .iter()
                            .map(|commit| cleanup_place_owned_capacity(&commit.source))
                            .sum::<usize>()
                }
                Frame::CallAfterArg {
                    params, commits, ..
                } => {
                    params.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>()
                        + params
                            .iter()
                            .map(resolved_param_owned_capacity)
                            .sum::<usize>()
                        + commits.capacity() * std::mem::size_of::<CallArgumentTransfer>()
                        + commits
                            .iter()
                            .map(|commit| cleanup_place_owned_capacity(&commit.source))
                            .sum::<usize>()
                }
                Frame::MatchNext {
                    branch_state,
                    arm_results,
                    ..
                }
                | Frame::MatchAfterArm {
                    branch_state,
                    arm_results,
                    ..
                } => flow_state_owned_capacity(branch_state) + results(arm_results),
                Frame::ScalarMatchNext {
                    branch_state,
                    arm_results,
                    destination: place,
                    ..
                }
                | Frame::ScalarMatchAfterGuard {
                    branch_state,
                    arm_results,
                    destination: place,
                    ..
                }
                | Frame::ScalarMatchAfterArm {
                    branch_state,
                    arm_results,
                    destination: place,
                    ..
                } => {
                    flow_state_owned_capacity(branch_state)
                        + results(arm_results)
                        + destination(place)
                }
                Frame::UpdateNext {
                    flow,
                    destination: place,
                    staged_base,
                    replaced,
                    ..
                } => {
                    eval_result_owned_capacity(flow)
                        + destination(place)
                        + destination(staged_base)
                        + replaced.len()
                            * (std::mem::size_of::<DeclarationId>()
                                + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + replaced.iter().map(|id| id.as_str().len()).sum::<usize>()
                }
                Frame::UpdateAfterField {
                    destination: place,
                    staged_base,
                    replaced,
                    ..
                } => {
                    destination(place)
                        + destination(staged_base)
                        + replaced.len()
                            * (std::mem::size_of::<DeclarationId>()
                                + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + replaced.iter().map(|id| id.as_str().len()).sum::<usize>()
                }
                Frame::BlockAfterStatement {
                    destination: place, ..
                }
                | Frame::BlockAfterTail {
                    destination: place, ..
                }
                | Frame::RecordAfterField {
                    destination: place, ..
                }
                | Frame::UpdateAfterBase {
                    destination: place, ..
                } => destination(place),
                _ => 0,
            }
        }
        let mut frames = vec![Frame::Enter {
            expression,
            block,
            state,
        }];
        let mut results = Vec::new();
        let mut active_region = region;
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_lower_capacity_high_water(
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + results.capacity() * std::mem::size_of::<EvalResult>()
                    + self.blocks.capacity() * std::mem::size_of::<OpenBlock>()
                    + self.edges.capacity() * std::mem::size_of::<CleanupEdge>()
                    + self.regions.capacity() * std::mem::size_of::<CleanupRegion>()
                    + self.exits.capacity() * std::mem::size_of::<ExitTarget>()
                    + self.status_sources.capacity() * std::mem::size_of::<StatusSource>()
                    + builder_nested_capacity(self)
                    + frames.iter().map(frame_owned_capacity).sum::<usize>()
                    + frame_owned_capacity(&frame)
                    + results
                        .iter()
                        .map(eval_result_owned_capacity)
                        .sum::<usize>(),
            );
            match frame {
                Frame::RestoreRegion(restored) => active_region = restored,
                Frame::Enter {
                    expression,
                    block,
                    state,
                } => match &expression.kind {
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
                    | ResolvedExprKind::String(_) => results.push(EvalResult {
                        block,
                        state,
                        owned_source: None,
                    }),
                    ResolvedExprKind::BorrowPlace { .. } => results.push(EvalResult {
                        block,
                        state,
                        owned_source: None,
                    }),
                    ResolvedExprKind::ByteRange {
                        operation,
                        source,
                        start,
                        end,
                    } => {
                        if operation.as_str() != crate::byte_ops::RANGE_ID {
                            return Err(plan_error(
                                "byte range carries an unknown operation identity",
                            ));
                        }
                        self.schema = CLEANUP_PLAN_SCHEMA_V4;
                        frames.push(Frame::ByteRangeAfterSource {
                            expression,
                            start,
                            end,
                        });
                        frames.push(Frame::Enter {
                            expression: source,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::Place(place) => {
                        let owned_source = if expression.ownership == OwnershipMode::Own
                            && self.needs_drop(&expression.ty)?
                        {
                            Some(self.place_from_hir(place)?)
                        } else {
                            None
                        };
                        results.push(EvalResult {
                            block,
                            state,
                            owned_source,
                        });
                    }
                    ResolvedExprKind::Unary { op, value } => {
                        frames.push(Frame::Unary {
                            expression,
                            op: *op,
                        });
                        frames.push(Frame::Enter {
                            expression: value,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::Binary { op, left, right }
                        if !matches!(op, BinaryOp::And | BinaryOp::Or) =>
                    {
                        frames.push(Frame::BinaryLeft {
                            expression,
                            op: *op,
                            right,
                        });
                        frames.push(Frame::Enter {
                            expression: left,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::Binary { op, left, right }
                        if matches!(op, BinaryOp::And | BinaryOp::Or) =>
                    {
                        frames.push(Frame::LazyAfterLeft {
                            operation: *op,
                            left_id: left.id.clone(),
                            right,
                        });
                        frames.push(Frame::Enter {
                            expression: left,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::Binary { .. } => {
                        return Err(plan_error(
                            "cleanup lowering received an unknown binary operator",
                        ));
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frames.push(Frame::IfAfterCondition {
                            expression,
                            then_branch,
                            else_branch,
                        });
                        frames.push(Frame::Enter {
                            expression: condition,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::Project { base, field } => {
                        frames.push(Frame::Project { expression, field });
                        frames.push(Frame::Enter {
                            expression: base,
                            block,
                            state,
                        });
                    }
                    // Class Inheritance v1: an upcast consumes its source, so
                    // the surrounding transfer moves the source's inherited
                    // leaves; the child-declared suffix is checked inert at
                    // resolution and contributes no liveness here.
                    ResolvedExprKind::Upcast { source } => {
                        frames.push(Frame::UpcastAfterSource);
                        frames.push(Frame::Enter {
                            expression: source,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        frames.push(Frame::NativeNext {
                            args: &call.args,
                            index: 0,
                            flow: EvalResult {
                                block,
                                state,
                                owned_source: None,
                            },
                        })
                    }
                    ResolvedExprKind::HostCommandCall(call) => {
                        frames.push(Frame::HostCommandNext {
                            expression,
                            operation: call.operation,
                            args: &call.args,
                            index: 0,
                            flow: EvalResult {
                                block,
                                state,
                                owned_source: None,
                            },
                        });
                    }
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        args,
                        ..
                    } => {
                        let params = if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                            // Compiler-owned string operations carry their
                            // reserved identity instead of an authored
                            // declaration; their synthetic parameters drive the
                            // ordinary argument transfer machinery.
                            if instance.is_some() {
                                return Err(plan_error(format!(
                                    "string operation call `{}` must be monomorphic",
                                    expression.id
                                )));
                            }
                            if args.len() != op.arity() {
                                return Err(plan_error(format!(
                                    "cleanup call `{}` has inconsistent arity",
                                    expression.id
                                )));
                            }
                            crate::string_ops::resolved_params(op)
                        } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                            if instance.is_some() {
                                return Err(plan_error(format!(
                                    "borrowed str operation call `{}` must be monomorphic",
                                    expression.id
                                )));
                            }
                            if args.len() != op.arity() {
                                return Err(plan_error(format!(
                                    "cleanup call `{}` has inconsistent arity",
                                    expression.id
                                )));
                            }
                            crate::str_ops::resolved_params(op)
                        } else if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                            if instance.is_some() {
                                return Err(plan_error(format!(
                                    "borrowed byte operation call `{}` must be monomorphic",
                                    expression.id
                                )));
                            }
                            if args.len() != op.arity() {
                                return Err(plan_error(format!(
                                    "cleanup call `{}` has inconsistent arity",
                                    expression.id
                                )));
                            }
                            crate::byte_ops::resolved_params(op)
                        } else if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
                            if instance.is_some() || args.len() != op.arity() {
                                return Err(plan_error(format!(
                                    "cleanup host I/O call `{}` has inconsistent shape",
                                    expression.id
                                )));
                            }
                            crate::host_io_ops::resolved_params(op)
                        } else {
                            let target = self
                                .program
                                .resolve_call_target(callee, instance.as_ref())
                                .ok_or_else(|| {
                                    plan_error(format!("unknown cleanup call target `{callee}`"))
                                })?;
                            if target.params.len() != args.len() {
                                return Err(plan_error(format!(
                                    "cleanup call `{}` has inconsistent arity",
                                    expression.id
                                )));
                            }
                            target.params.clone()
                        };
                        frames.push(Frame::CallNext {
                            expression,
                            callee,
                            args,
                            params,
                            index: 0,
                            flow: EvalResult {
                                block,
                                state,
                                owned_source: None,
                            },
                            commits: Vec::with_capacity(args.len()),
                        });
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        let destination = self.expression_slot(expression, active_region)?;
                        let child_region = self.new_region(active_region)?;
                        let entry = self.new_block(child_region)?;
                        let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
                        self.terminate(block, CleanupTerminator::Goto(edge))?;
                        frames.push(Frame::BlockNext {
                            expression,
                            statements,
                            tail,
                            index: 0,
                            flow: EvalResult {
                                block: entry,
                                state,
                                owned_source: None,
                            },
                            child_region,
                            destination,
                        });
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. } => {
                        let destination = self.expression_slot(expression, active_region)?;
                        frames.push(Frame::RecordNext {
                            expression,
                            fields,
                            index: 0,
                            flow: EvalResult {
                                block,
                                state,
                                owned_source: None,
                            },
                            destination,
                        });
                    }
                    ResolvedExprKind::ConstructVariant { fields, .. } => {
                        frames.push(Frame::VariantNext {
                            fields,
                            index: 0,
                            flow: EvalResult {
                                block,
                                state,
                                owned_source: None,
                            },
                        });
                    }
                    ResolvedExprKind::Try {
                        operand,
                        result,
                        ok_case,
                        ok_field,
                        err_case,
                        err_field,
                        residual_type,
                    } => {
                        self.check_try_metadata(
                            expression,
                            operand,
                            result,
                            ok_case,
                            ok_field,
                            err_case,
                            err_field,
                            residual_type,
                        )?;
                        frames.push(Frame::TryAfterOperand {
                            expression,
                            operand,
                            result,
                            ok_case,
                            ok_field,
                            err_case,
                            err_field,
                            residual_type,
                        });
                        frames.push(Frame::Enter {
                            expression: operand,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::TryOption {
                        operand,
                        option,
                        some_case,
                        some_field,
                        none_case,
                        residual_type,
                    } => {
                        self.check_try_option_metadata(
                            expression,
                            operand,
                            option,
                            some_case,
                            some_field,
                            none_case,
                            residual_type,
                        )?;
                        frames.push(Frame::TryOptionAfterOperand {
                            expression,
                            operand,
                            option,
                            some_case,
                            some_field,
                            none_case,
                            residual_type,
                        });
                        frames.push(Frame::Enter {
                            expression: operand,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::Match { scrutinee, arms } => {
                        if arms.is_empty() {
                            return Err(plan_error("copy-variant match has no arms"));
                        }
                        if self.needs_drop(&arms[0].value.ty)? {
                            return Err(plan_error(
                                "droppable match result reached the copy-only cleanup slice",
                            ));
                        }
                        frames.push(Frame::MatchAfterScrutinee {
                            expression,
                            scrutinee,
                            arms,
                        });
                        frames.push(Frame::Enter {
                            expression: scrutinee,
                            block,
                            state,
                        });
                    }
                    ResolvedExprKind::UpdateRecord {
                        base,
                        record,
                        fields,
                    } => {
                        let destination = self.expression_slot(expression, active_region)?;
                        let (entry, update_region) = if destination.is_some() {
                            let update_region = self.new_region(active_region)?;
                            let entry = self.new_block(update_region)?;
                            let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
                            self.terminate(block, CleanupTerminator::Goto(edge))?;
                            (entry, update_region)
                        } else {
                            (block, active_region)
                        };
                        frames.push(Frame::UpdateAfterBase {
                            expression,
                            record,
                            fields,
                            destination,
                            update_region,
                        });
                        if active_region != update_region {
                            frames.push(Frame::RestoreRegion(active_region));
                            active_region = update_region;
                        }
                        frames.push(Frame::Enter {
                            expression: base,
                            block: entry,
                            state,
                        });
                    }
                },
                Frame::Unary { expression, op } => {
                    let mut evaluated = results.pop().expect("unary result retained");
                    if op == UnaryOp::Neg {
                        let source = self.checked_source(
                            expression,
                            CheckedOperation::Neg,
                            vec![StatusCase::NegationOverflow],
                        )?;
                        let (block, state) = self.split_status(
                            evaluated.block,
                            evaluated.state,
                            active_region,
                            source,
                        )?;
                        evaluated = EvalResult {
                            block,
                            state,
                            owned_source: None,
                        };
                    } else {
                        evaluated.owned_source = None;
                    }
                    results.push(evaluated);
                }
                Frame::BinaryLeft {
                    expression,
                    op,
                    right,
                } => {
                    let left = results.pop().expect("binary left retained");
                    frames.push(Frame::BinaryRight { expression, op });
                    frames.push(Frame::Enter {
                        expression: right,
                        block: left.block,
                        state: left.state,
                    });
                }
                Frame::BinaryRight { expression, op } => {
                    let right = results.pop().expect("binary right retained");
                    let checked = match op {
                        BinaryOp::Add => {
                            Some((CheckedOperation::Add, vec![StatusCase::AddOverflow]))
                        }
                        BinaryOp::Sub => {
                            Some((CheckedOperation::Sub, vec![StatusCase::SubOverflow]))
                        }
                        BinaryOp::Mul => {
                            Some((CheckedOperation::Mul, vec![StatusCase::MulOverflow]))
                        }
                        BinaryOp::Div => Some((
                            CheckedOperation::Div,
                            vec![StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                        )),
                        BinaryOp::Rem => Some((
                            CheckedOperation::Rem,
                            vec![StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
                        )),
                        _ => None,
                    };
                    let (block, state) = if let Some((operation, cases)) = checked {
                        let source = self.checked_source(expression, operation, cases)?;
                        self.split_status(right.block, right.state, active_region, source)?
                    } else {
                        (right.block, right.state)
                    };
                    results.push(EvalResult {
                        block,
                        state,
                        owned_source: None,
                    });
                }
                Frame::LazyAfterLeft {
                    operation,
                    left_id,
                    right,
                } => {
                    let left = results.pop().expect("lazy left result retained");
                    let evaluate_right_when = operation == BinaryOp::And;
                    let evaluate = self.new_block(active_region)?;
                    let skip = self.new_block(active_region)?;
                    let evaluate_edge = self.new_edge(
                        left.block,
                        evaluate,
                        EdgeCondition::BooleanResult(left_id.clone(), evaluate_right_when),
                    )?;
                    let skip_edge = self.new_edge(
                        left.block,
                        skip,
                        EdgeCondition::BooleanResult(left_id, !evaluate_right_when),
                    )?;
                    self.terminate(
                        left.block,
                        CleanupTerminator::Branch(vec![evaluate_edge, skip_edge]),
                    )?;
                    frames.push(Frame::LazyAfterRight {
                        left_state: left.state.clone(),
                        skip,
                    });
                    frames.push(Frame::Enter {
                        expression: right,
                        block: evaluate,
                        state: left.state,
                    });
                }
                Frame::LazyAfterRight { left_state, skip } => {
                    let right = results.pop().expect("lazy right result retained");
                    let state = self.merge_states(&right.state, &left_state)?;
                    let join = self.new_block(active_region)?;
                    let evaluated_edge = self.new_edge(right.block, join, EdgeCondition::Always)?;
                    self.terminate(right.block, CleanupTerminator::Goto(evaluated_edge))?;
                    let skipped_edge = self.new_edge(skip, join, EdgeCondition::Always)?;
                    self.terminate(skip, CleanupTerminator::Goto(skipped_edge))?;
                    results.push(EvalResult {
                        block: join,
                        state,
                        owned_source: None,
                    });
                }
                Frame::IfAfterCondition {
                    expression,
                    then_branch,
                    else_branch,
                } => {
                    let condition = results.pop().expect("if condition result retained");
                    let destination = self.expression_slot(expression, active_region)?;
                    let then_entry = self.new_block(active_region)?;
                    let else_entry = self.new_block(active_region)?;
                    let then_edge = self.new_edge(
                        condition.block,
                        then_entry,
                        EdgeCondition::BooleanResult(condition_id(expression)?, true),
                    )?;
                    let else_edge = self.new_edge(
                        condition.block,
                        else_entry,
                        EdgeCondition::BooleanResult(condition_id(expression)?, false),
                    )?;
                    self.terminate(
                        condition.block,
                        CleanupTerminator::Branch(vec![then_edge, else_edge]),
                    )?;
                    frames.push(Frame::IfAfterThen {
                        expression,
                        else_branch,
                        else_entry,
                        condition_state: condition.state.clone(),
                        destination,
                    });
                    frames.push(Frame::Enter {
                        expression: then_branch,
                        block: then_entry,
                        state: condition.state,
                    });
                }
                Frame::IfAfterThen {
                    expression,
                    else_branch,
                    else_entry,
                    condition_state,
                    destination,
                } => {
                    let mut then_result = results.pop().expect("then result retained");
                    if let Some(destination) = destination.clone() {
                        let source = then_result
                            .owned_source
                            .take()
                            .ok_or_else(|| plan_error("owned then branch has no cleanup source"))?;
                        self.transfer(
                            then_result.block,
                            expression.id.clone(),
                            source,
                            destination,
                            &mut then_result.state,
                            true,
                        )?;
                    }
                    frames.push(Frame::IfAfterElse {
                        expression,
                        then_result,
                        destination,
                    });
                    frames.push(Frame::Enter {
                        expression: else_branch,
                        block: else_entry,
                        state: condition_state,
                    });
                }
                Frame::IfAfterElse {
                    expression,
                    then_result,
                    destination,
                } => {
                    let mut else_result = results.pop().expect("else result retained");
                    if let Some(destination) = destination.clone() {
                        let source = else_result
                            .owned_source
                            .take()
                            .ok_or_else(|| plan_error("owned else branch has no cleanup source"))?;
                        self.transfer(
                            else_result.block,
                            expression.id.clone(),
                            source,
                            destination,
                            &mut else_result.state,
                            true,
                        )?;
                    }
                    let state = self.merge_states(&then_result.state, &else_result.state)?;
                    let join = self.new_block(active_region)?;
                    let then_join =
                        self.new_edge(then_result.block, join, EdgeCondition::Always)?;
                    self.terminate(then_result.block, CleanupTerminator::Goto(then_join))?;
                    let else_join =
                        self.new_edge(else_result.block, join, EdgeCondition::Always)?;
                    self.terminate(else_result.block, CleanupTerminator::Goto(else_join))?;
                    results.push(EvalResult {
                        block: join,
                        state,
                        owned_source: destination,
                    });
                }
                // The upcast contributes no liveness of its own; the source
                // place stays the transfer source for the surrounding move.
                Frame::UpcastAfterSource { .. } => {
                    let source = results.pop().expect("upcast source retained");
                    results.push(source);
                }
                Frame::Project { expression, field } => {
                    let base = results.pop().expect("projection base retained");
                    let destination = self.expression_slot(expression, active_region)?;
                    let mut state = base.state;
                    if let Some(destination) = destination.clone() {
                        let source = base
                            .owned_source
                            .ok_or_else(|| {
                                plan_error("owned projection base has no cleanup source")
                            })?
                            .projected(field.clone());
                        self.transfer(
                            base.block,
                            expression.id.clone(),
                            source,
                            destination,
                            &mut state,
                            true,
                        )?;
                    }
                    results.push(EvalResult {
                        block: base.block,
                        state,
                        owned_source: destination,
                    });
                }
                Frame::NativeNext { args, index, flow } => {
                    if index == args.len() {
                        results.push(flow);
                    } else {
                        frames.push(Frame::NativeAfterArg { args, index });
                        frames.push(Frame::Enter {
                            expression: &args[index],
                            block: flow.block,
                            state: flow.state,
                        });
                    }
                }
                Frame::NativeAfterArg { args, index } => {
                    let evaluated = results.pop().expect("native argument result retained");
                    if evaluated.owned_source.is_some() {
                        return Err(plan_error(
                            "native Rust import received a non-scalar argument",
                        ));
                    }
                    frames.push(Frame::NativeNext {
                        args,
                        index: index + 1,
                        flow: evaluated,
                    });
                }
                Frame::HostCommandNext {
                    expression,
                    operation,
                    args,
                    index,
                    flow,
                } => {
                    if index < args.len() {
                        frames.push(Frame::HostCommandAfterArg {
                            expression,
                            operation,
                            args,
                            index,
                        });
                        frames.push(Frame::Enter {
                            expression: &args[index],
                            block: flow.block,
                            state: flow.state,
                        });
                        continue;
                    }
                    self.push_transition(
                        flow.block,
                        CleanupTransition::CallCommit {
                            call: expression.id.clone(),
                            arguments: Vec::new(),
                        },
                    );
                    let state = flow.state;
                    let (block, mut state) = if crate::command_io_ops::failure(operation)
                        == crate::command_io_ops::CommandIoFailure::Status
                    {
                        let source = StatusSourceId {
                            expression: expression.id.clone(),
                            lane: StatusLane::OperationFailure,
                        };
                        self.add_status_source(
                            source.clone(),
                            StatusProducer::PropagatedCall {
                                callee: DeclarationId::new(crate::command_io_ops::id(operation)),
                            },
                        )?;
                        self.split_status(flow.block, state, active_region, source)?
                    } else {
                        (flow.block, state)
                    };
                    let destination = self.expression_slot(expression, active_region)?;
                    if let Some(destination) = destination.clone() {
                        self.initialize(block, expression.id.clone(), destination, &mut state)?;
                    }
                    results.push(EvalResult {
                        block,
                        state,
                        owned_source: destination,
                    });
                }
                Frame::HostCommandAfterArg {
                    expression,
                    operation,
                    args,
                    index,
                } => {
                    let evaluated = results
                        .pop()
                        .expect("host-command argument result retained");
                    if evaluated.owned_source.is_some() {
                        return Err(plan_error(
                            "host-command operation received an owned argument",
                        ));
                    }
                    frames.push(Frame::HostCommandNext {
                        expression,
                        operation,
                        args,
                        index: index + 1,
                        flow: evaluated,
                    });
                }
                Frame::ByteRangeAfterSource {
                    expression,
                    start,
                    end,
                } => {
                    let evaluated = results.pop().expect("byte-range source result retained");
                    if evaluated.owned_source.is_some() {
                        return Err(plan_error("byte range received an owned source"));
                    }
                    frames.push(Frame::ByteRangeAfterStart { expression, end });
                    frames.push(Frame::Enter {
                        expression: start,
                        block: evaluated.block,
                        state: evaluated.state,
                    });
                }
                Frame::ByteRangeAfterStart { expression, end } => {
                    let evaluated = results.pop().expect("byte-range start result retained");
                    if evaluated.owned_source.is_some() {
                        return Err(plan_error("byte range received an owned start"));
                    }
                    frames.push(Frame::ByteRangeAfterEnd { expression });
                    frames.push(Frame::Enter {
                        expression: end,
                        block: evaluated.block,
                        state: evaluated.state,
                    });
                }
                Frame::ByteRangeAfterEnd { expression } => {
                    let evaluated = results.pop().expect("byte-range end result retained");
                    if evaluated.owned_source.is_some() {
                        return Err(plan_error("byte range received an owned end"));
                    }
                    self.push_transition(
                        evaluated.block,
                        CleanupTransition::CallCommit {
                            call: expression.id.clone(),
                            arguments: Vec::new(),
                        },
                    );
                    let source = StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    };
                    self.add_status_source(
                        source.clone(),
                        StatusProducer::PropagatedCall {
                            callee: DeclarationId::new(crate::byte_ops::RANGE_ID),
                        },
                    )?;
                    let (block, state) =
                        self.split_status(evaluated.block, evaluated.state, active_region, source)?;
                    results.push(EvalResult {
                        block,
                        state,
                        owned_source: None,
                    });
                }
                Frame::CallNext {
                    expression,
                    callee,
                    args,
                    params,
                    index,
                    flow,
                    commits,
                } => {
                    if index == args.len() {
                        let mut state = flow.state;
                        for commit in &commits {
                            self.consume_place(&commit.source, &mut state, &expression.id)?;
                        }
                        self.push_transition(
                            flow.block,
                            CleanupTransition::CallCommit {
                                call: expression.id.clone(),
                                arguments: commits,
                            },
                        );
                        if crate::byte_ops::by_id(callee.as_str()).is_some()
                            || crate::host_io_ops::by_id(callee.as_str()).is_some()
                        {
                            let destination = self.expression_slot(expression, active_region)?;
                            if let Some(destination) = destination.clone() {
                                self.initialize(
                                    flow.block,
                                    expression.id.clone(),
                                    destination,
                                    &mut state,
                                )?;
                            }
                            results.push(EvalResult {
                                block: flow.block,
                                state,
                                owned_source: destination,
                            });
                            continue;
                        }
                        let source = StatusSourceId {
                            expression: expression.id.clone(),
                            lane: StatusLane::OperationFailure,
                        };
                        self.add_status_source(
                            source.clone(),
                            StatusProducer::PropagatedCall {
                                callee: callee.clone(),
                            },
                        )?;
                        let (success, mut success_state) =
                            self.split_status(flow.block, state, active_region, source)?;
                        let destination = self.expression_slot(expression, active_region)?;
                        if let Some(destination) = destination.clone() {
                            self.initialize(
                                success,
                                expression.id.clone(),
                                destination,
                                &mut success_state,
                            )?;
                        }
                        results.push(EvalResult {
                            block: success,
                            state: success_state,
                            owned_source: destination,
                        });
                    } else {
                        let argument = &args[index];
                        frames.push(Frame::CallAfterArg {
                            expression,
                            callee,
                            args,
                            params,
                            index,
                            commits,
                        });
                        frames.push(Frame::Enter {
                            expression: argument,
                            block: flow.block,
                            state: flow.state,
                        });
                    }
                }
                Frame::CallAfterArg {
                    expression,
                    callee,
                    args,
                    params,
                    index,
                    mut commits,
                } => {
                    let argument = &args[index];
                    let evaluated = results.pop().expect("call argument result retained");
                    let mut state = evaluated.state;
                    if params[index].ownership == OwnershipMode::Own
                        && self.needs_drop(&params[index].ty)?
                    {
                        let source = evaluated.owned_source.ok_or_else(|| {
                            plan_error(format!(
                                "owned call argument {} at `{}` has no cleanup source",
                                index, expression.id
                            ))
                        })?;
                        let epoch =
                            self.call_argument_slot(expression, index, argument, active_region)?;
                        self.transfer(
                            evaluated.block,
                            argument.id.clone(),
                            source,
                            epoch.clone(),
                            &mut state,
                            true,
                        )?;
                        commits.push(CallArgumentTransfer {
                            parameter_index: u32::try_from(index)
                                .map_err(|_| plan_error("too many call arguments"))?,
                            source: epoch,
                        });
                    }
                    frames.push(Frame::CallNext {
                        expression,
                        callee,
                        args,
                        params,
                        index: index + 1,
                        flow: EvalResult {
                            block: evaluated.block,
                            state,
                            owned_source: None,
                        },
                        commits,
                    });
                }
                Frame::BlockNext {
                    expression,
                    statements,
                    tail,
                    index,
                    flow,
                    child_region,
                    destination,
                } => {
                    if index < statements.len() {
                        if let ResolvedStatement::While {
                            condition, body, ..
                        } = &statements[index]
                        {
                            // Bounded While-Loops v1: linearize one admitted
                            // iteration synchronously; its continuation feeds
                            // the ordinary after-statement bookkeeping.
                            if active_region != child_region {
                                frames.push(Frame::RestoreRegion(active_region));
                                active_region = child_region;
                            }
                            let evaluated = self.lower_while(
                                condition,
                                body,
                                flow.block,
                                flow.state,
                                child_region,
                            )?;
                            frames.push(Frame::BlockAfterStatement {
                                expression,
                                statements,
                                tail,
                                index,
                                child_region,
                                destination,
                            });
                            results.push(EvalResult {
                                block: evaluated.block,
                                state: evaluated.state,
                                owned_source: None,
                            });
                        } else {
                            frames.push(Frame::BlockAfterStatement {
                                expression,
                                statements,
                                tail,
                                index,
                                child_region,
                                destination,
                            });
                            if active_region != child_region {
                                frames.push(Frame::RestoreRegion(active_region));
                                active_region = child_region;
                            }
                            frames.push(Frame::Enter {
                                expression: statements[index].value(),
                                block: flow.block,
                                state: flow.state,
                            });
                        }
                    } else {
                        frames.push(Frame::BlockAfterTail {
                            expression,
                            child_region,
                            destination,
                        });
                        if active_region != child_region {
                            frames.push(Frame::RestoreRegion(active_region));
                            active_region = child_region;
                        }
                        frames.push(Frame::Enter {
                            expression: tail,
                            block: flow.block,
                            state: flow.state,
                        });
                    }
                }
                Frame::BlockAfterStatement {
                    expression,
                    statements,
                    tail,
                    index,
                    child_region,
                    destination,
                } => {
                    let evaluated = results.pop().expect("block statement result retained");
                    let mut state = evaluated.state;
                    // Field Mutation v1 stores bind nothing: only plain lets
                    // and whole-binding assignments transfer into slots.
                    let binds_whole_value = matches!(
                        &statements[index],
                        ResolvedStatement::Let { .. }
                            | ResolvedStatement::Assign { field: None, .. }
                    );
                    if binds_whole_value {
                        if let ResolvedStatement::Let { binding, .. }
                        | ResolvedStatement::Assign { binding, .. } = &statements[index]
                        {
                            if let Some(binding_place) = self.binding_slot(binding, child_region)? {
                                let value = &statements[index].value();
                                let source = evaluated.owned_source.ok_or_else(|| {
                                    plan_error(format!(
                                        "owned binding `{}` has no cleanup source",
                                        binding.id
                                    ))
                                })?;
                                self.transfer(
                                    evaluated.block,
                                    value.id.clone(),
                                    source,
                                    binding_place,
                                    &mut state,
                                    true,
                                )?;
                            }
                        }
                    }
                    frames.push(Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        flow: EvalResult {
                            block: evaluated.block,
                            state,
                            owned_source: None,
                        },
                        child_region,
                        destination,
                    });
                }
                Frame::BlockAfterTail {
                    expression,
                    child_region,
                    destination,
                } => {
                    let evaluated = results.pop().expect("block tail result retained");
                    let mut state = evaluated.state;
                    if let Some(destination) = destination.clone() {
                        let source = evaluated
                            .owned_source
                            .ok_or_else(|| plan_error("owned block tail has no cleanup source"))?;
                        self.transfer(
                            evaluated.block,
                            expression.id.clone(),
                            source,
                            destination,
                            &mut state,
                            true,
                        )?;
                    }
                    let (block, state) = self.exit_scope(evaluated.block, state, child_region)?;
                    results.push(EvalResult {
                        block,
                        state,
                        owned_source: destination,
                    });
                }
                Frame::RecordNext {
                    expression,
                    fields,
                    index,
                    flow,
                    destination,
                } => {
                    if index == fields.len() {
                        let mut state = flow.state;
                        if let Some(destination) = &destination {
                            self.canonicalize_complete_aggregate(destination, &mut state)?;
                        }
                        results.push(EvalResult {
                            block: flow.block,
                            state,
                            owned_source: destination,
                        });
                    } else {
                        frames.push(Frame::RecordAfterField {
                            expression,
                            fields,
                            index,
                            destination,
                        });
                        frames.push(Frame::Enter {
                            expression: &fields[index].value,
                            block: flow.block,
                            state: flow.state,
                        });
                    }
                }
                Frame::RecordAfterField {
                    expression,
                    fields,
                    index,
                    destination,
                } => {
                    let initializer = &fields[index];
                    let evaluated = results.pop().expect("record field result retained");
                    let mut state = evaluated.state;
                    if initializer.value.ownership == OwnershipMode::Own
                        && self.needs_drop(&initializer.value.ty)?
                    {
                        let source = evaluated.owned_source.ok_or_else(|| {
                            plan_error(format!(
                                "record field `{}` has no cleanup source",
                                initializer.field
                            ))
                        })?;
                        let field_destination = destination
                            .as_ref()
                            .ok_or_else(|| {
                                plan_error("droppable record constructor has no cleanup slot")
                            })?
                            .projected(initializer.field.clone());
                        self.transfer(
                            evaluated.block,
                            initializer.value.id.clone(),
                            source,
                            field_destination,
                            &mut state,
                            false,
                        )?;
                    }
                    frames.push(Frame::RecordNext {
                        expression,
                        fields,
                        index: index + 1,
                        flow: EvalResult {
                            block: evaluated.block,
                            state,
                            owned_source: None,
                        },
                        destination,
                    });
                }
                Frame::VariantNext {
                    fields,
                    index,
                    flow,
                } => {
                    if index == fields.len() {
                        results.push(EvalResult {
                            block: flow.block,
                            state: flow.state,
                            owned_source: None,
                        });
                    } else {
                        frames.push(Frame::VariantAfterField { fields, index });
                        frames.push(Frame::Enter {
                            expression: &fields[index].value,
                            block: flow.block,
                            state: flow.state,
                        });
                    }
                }
                Frame::VariantAfterField { fields, index } => {
                    let evaluated = results.pop().expect("variant field result retained");
                    let field = &fields[index];
                    if field.value.ownership == OwnershipMode::Own
                        && self.needs_drop(&field.value.ty)?
                    {
                        return Err(plan_error(
                            "droppable variant payload reached the copy-only cleanup slice",
                        ));
                    }
                    frames.push(Frame::VariantNext {
                        fields,
                        index: index + 1,
                        flow: EvalResult {
                            block: evaluated.block,
                            state: evaluated.state,
                            owned_source: None,
                        },
                    });
                }
                Frame::TryAfterOperand {
                    expression,
                    operand,
                    result,
                    ok_case,
                    ok_field,
                    err_case,
                    err_field,
                    residual_type,
                } => {
                    let evaluated = results.pop().expect("try operand result retained");
                    results.push(self.finish_try(
                        expression,
                        operand,
                        result,
                        ok_case,
                        ok_field,
                        err_case,
                        err_field,
                        residual_type,
                        evaluated,
                        active_region,
                    )?);
                }
                Frame::TryOptionAfterOperand {
                    expression,
                    operand,
                    option,
                    some_case,
                    some_field,
                    none_case,
                    residual_type,
                } => {
                    let evaluated = results.pop().expect("option try operand result retained");
                    results.push(self.finish_try_option(
                        expression,
                        operand,
                        option,
                        some_case,
                        some_field,
                        none_case,
                        residual_type,
                        evaluated,
                        active_region,
                    )?);
                }
                Frame::MatchAfterScrutinee {
                    expression,
                    scrutinee,
                    arms,
                } => {
                    let scrutinee_result = results.pop().expect("match scrutinee result retained");
                    if scrutinee_result.owned_source.is_some() {
                        return Err(plan_error(
                            "droppable match scrutinee reached the copy-only cleanup slice",
                        ));
                    }
                    // Refutable Match v1: Copy-scalar scrutinees lower to the
                    // literal/guard decision chain; aggregates keep the
                    // pre-feature variant/record lowering below.
                    if matches!(
                        scrutinee.ty,
                        ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::U8
                            | ResolvedType::Char
                            | ResolvedType::Bool
                    ) {
                        let destination = self.expression_slot(expression, active_region)?;
                        frames.push(Frame::ScalarMatchNext {
                            expression,
                            scrutinee,
                            arms,
                            index: 0,
                            decision: scrutinee_result.block,
                            branch_state: scrutinee_result.state,
                            arm_results: Vec::with_capacity(arms.len()),
                            destination,
                        });
                        continue;
                    }
                    let is_record = match &scrutinee.ty {
                        ResolvedType::Nominal { declaration, .. } => self
                            .program
                            .declarations
                            .declaration(declaration)
                            .is_some_and(|item| item.kind == DeclarationKind::Record),
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
                        | ResolvedType::String
                        | ResolvedType::Bytes
                        | ResolvedType::Str
                        | ResolvedType::SliceU8
                        | ResolvedType::TypeParameter { .. } => false,
                    };
                    if is_record {
                        let [arm] = arms else {
                            return Err(plan_error(
                                "irrefutable record match must have exactly one arm",
                            ));
                        };
                        if matches!(&arm.pattern, ResolvedMatchPattern::Variant { .. }) {
                            return Err(plan_error("variant pattern has a record match scrutinee"));
                        }
                        frames.push(Frame::MatchRecordAfterArm);
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            block: scrutinee_result.block,
                            state: scrutinee_result.state,
                        });
                    } else {
                        frames.push(Frame::MatchNext {
                            scrutinee,
                            arms,
                            index: 0,
                            decision: scrutinee_result.block,
                            branch_state: scrutinee_result.state,
                            arm_results: Vec::with_capacity(arms.len()),
                        });
                    }
                }
                Frame::MatchRecordAfterArm => {
                    let result = results.pop().expect("record match arm result retained");
                    if result.owned_source.is_some() {
                        return Err(plan_error(
                            "droppable record match arm reached the copy-only cleanup slice",
                        ));
                    }
                    results.push(result);
                }
                Frame::MatchNext {
                    scrutinee,
                    arms,
                    index,
                    mut decision,
                    branch_state,
                    arm_results,
                } => {
                    if index == arms.len() {
                        let mut arm_results = arm_results.into_iter();
                        let first = arm_results.next().ok_or_else(|| {
                            plan_error("copy-variant match produced no arm result")
                        })?;
                        let mut merged_state = first.state.clone();
                        let mut completed = vec![first];
                        for result in arm_results {
                            merged_state = self.merge_states(&merged_state, &result.state)?;
                            completed.push(result);
                        }
                        let join = self.new_block(active_region)?;
                        for result in completed {
                            let edge = self.new_edge(result.block, join, EdgeCondition::Always)?;
                            self.terminate(result.block, CleanupTerminator::Goto(edge))?;
                        }
                        results.push(EvalResult {
                            block: join,
                            state: merged_state,
                            owned_source: None,
                        });
                    } else {
                        let arm = &arms[index];
                        let final_arm = index + 1 == arms.len();
                        let arm_entry = self.new_block(active_region)?;
                        if final_arm {
                            let edge = self.new_edge(decision, arm_entry, EdgeCondition::Always)?;
                            self.terminate(decision, CleanupTerminator::Goto(edge))?;
                        } else {
                            let ResolvedMatchPattern::Variant { case, .. } = &arm.pattern else {
                                return Err(plan_error(
                                    "wildcard match arm must be the final exhaustive arm",
                                ));
                            };
                            let next_decision = self.new_block(active_region)?;
                            let selected = self.new_edge(
                                decision,
                                arm_entry,
                                EdgeCondition::VariantCase {
                                    scrutinee: scrutinee.id.clone(),
                                    case: case.clone(),
                                    matches: true,
                                },
                            )?;
                            let rejected = self.new_edge(
                                decision,
                                next_decision,
                                EdgeCondition::VariantCase {
                                    scrutinee: scrutinee.id.clone(),
                                    case: case.clone(),
                                    matches: false,
                                },
                            )?;
                            self.terminate(
                                decision,
                                CleanupTerminator::Branch(vec![selected, rejected]),
                            )?;
                            decision = next_decision;
                        }
                        frames.push(Frame::MatchAfterArm {
                            scrutinee,
                            arms,
                            index,
                            decision,
                            branch_state: branch_state.clone(),
                            arm_results,
                        });
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            block: arm_entry,
                            state: branch_state,
                        });
                    }
                }
                Frame::MatchAfterArm {
                    scrutinee,
                    arms,
                    index,
                    decision,
                    branch_state,
                    mut arm_results,
                } => {
                    let result = results.pop().expect("match arm result retained");
                    if result.owned_source.is_some() {
                        return Err(plan_error(
                            "droppable match arm reached the copy-only cleanup slice",
                        ));
                    }
                    arm_results.push(result);
                    frames.push(Frame::MatchNext {
                        scrutinee,
                        arms,
                        index: index + 1,
                        decision,
                        branch_state,
                        arm_results,
                    });
                }
                Frame::ScalarMatchNext {
                    expression,
                    scrutinee,
                    arms,
                    index,
                    mut decision,
                    branch_state,
                    arm_results,
                    destination,
                } => {
                    if index == arms.len() {
                        let entry_state = branch_state;
                        let mut arm_results = arm_results.into_iter();
                        let first = arm_results
                            .next()
                            .ok_or_else(|| plan_error("refutable match produced no arm result"))?;
                        let mut merged_state = first.state.clone();
                        let mut completed = vec![first];
                        for result in arm_results {
                            merged_state = self.merge_states(&merged_state, &result.state)?;
                            completed.push(result);
                        }
                        // Copy-scalar admission makes every path observe the
                        // same owned liveness as the decision entry; anything
                        // else means a non-admitted shape reached lowering.
                        if merged_state != entry_state {
                            return Err(plan_error(
                                "refutable match changes owned liveness, which the \
                                 Refutable Match v1 admission profile forbids",
                            ));
                        }
                        let join = self.new_block(active_region)?;
                        for result in completed {
                            let edge = self.new_edge(result.block, join, EdgeCondition::Always)?;
                            self.terminate(result.block, CleanupTerminator::Goto(edge))?;
                        }
                        results.push(EvalResult {
                            block: join,
                            state: merged_state,
                            owned_source: None,
                        });
                    } else {
                        let arm = &arms[index];
                        let final_arm = index + 1 == arms.len();
                        let arm_entry = self.new_block(active_region)?;
                        if final_arm {
                            // The resolver guarantees one trailing
                            // irrefutable guard-free catch-all, so the final
                            // decision falls through unconditionally.
                            let edge = self.new_edge(decision, arm_entry, EdgeCondition::Always)?;
                            self.terminate(decision, CleanupTerminator::Goto(edge))?;
                        } else {
                            // Every earlier arm — including irrefutable
                            // bindings — authenticates as one conditional
                            // decision so the plan stays total.
                            let next_decision = self.new_block(active_region)?;
                            let selected = self.new_edge(
                                decision,
                                arm_entry,
                                EdgeCondition::ArmSelected {
                                    scrutinee: scrutinee.id.clone(),
                                    arm: u32::try_from(index)
                                        .map_err(|_| plan_error("too many match arms"))?,
                                    selected: true,
                                },
                            )?;
                            let rejected = self.new_edge(
                                decision,
                                next_decision,
                                EdgeCondition::ArmSelected {
                                    scrutinee: scrutinee.id.clone(),
                                    arm: u32::try_from(index)
                                        .map_err(|_| plan_error("too many match arms"))?,
                                    selected: false,
                                },
                            )?;
                            self.terminate(
                                decision,
                                CleanupTerminator::Branch(vec![selected, rejected]),
                            )?;
                            decision = next_decision;
                        }
                        if let Some(guard) = &arm.guard {
                            frames.push(Frame::ScalarMatchAfterGuard {
                                expression,
                                scrutinee,
                                arms,
                                index,
                                decision,
                                branch_state: branch_state.clone(),
                                arm_results,
                                destination,
                            });
                            frames.push(Frame::Enter {
                                expression: guard.as_ref(),
                                block: arm_entry,
                                state: branch_state,
                            });
                        } else {
                            frames.push(Frame::ScalarMatchAfterArm {
                                expression,
                                scrutinee,
                                arms,
                                index,
                                decision,
                                branch_state: branch_state.clone(),
                                arm_results,
                                destination,
                            });
                            frames.push(Frame::Enter {
                                expression: &arm.value,
                                block: arm_entry,
                                state: branch_state,
                            });
                        }
                    }
                }
                Frame::ScalarMatchAfterGuard {
                    expression,
                    scrutinee,
                    arms,
                    index,
                    decision,
                    branch_state,
                    arm_results,
                    destination,
                } => {
                    // The guard is an ordinary bool expression evaluated once
                    // after the pattern matched; its Boolean join routes to
                    // this arm's value or falls through to the next decision.
                    let guard = results.pop().expect("scalar match guard retained");
                    if guard.owned_source.is_some() {
                        return Err(plan_error(
                            "scalar match guard owns a value, which no admitted program can express",
                        ));
                    }
                    let arm = &arms[index];
                    let Some(guard_expr) = &arm.guard else {
                        return Err(plan_error("scalar match guard continuation lost its guard"));
                    };
                    let value_entry = self.new_block(active_region)?;
                    let true_edge = self.new_edge(
                        guard.block,
                        value_entry,
                        EdgeCondition::BooleanResult(guard_expr.id.clone(), true),
                    )?;
                    let false_edge = self.new_edge(
                        guard.block,
                        decision,
                        EdgeCondition::BooleanResult(guard_expr.id.clone(), false),
                    )?;
                    self.terminate(
                        guard.block,
                        CleanupTerminator::Branch(vec![true_edge, false_edge]),
                    )?;
                    frames.push(Frame::ScalarMatchAfterArm {
                        expression,
                        scrutinee,
                        arms,
                        index,
                        decision,
                        branch_state: branch_state.clone(),
                        arm_results,
                        destination,
                    });
                    frames.push(Frame::Enter {
                        expression: &arm.value,
                        block: value_entry,
                        state: branch_state,
                    });
                }
                Frame::ScalarMatchAfterArm {
                    expression,
                    scrutinee,
                    arms,
                    index,
                    decision,
                    branch_state,
                    mut arm_results,
                    destination,
                } => {
                    let mut result = results.pop().expect("scalar match arm value retained");
                    if let Some(destination) = destination.clone() {
                        let source = result.owned_source.take().ok_or_else(|| {
                            plan_error("owned scalar match arm has no cleanup source")
                        })?;
                        self.transfer(
                            result.block,
                            expression.id.clone(),
                            source,
                            destination,
                            &mut result.state,
                            true,
                        )?;
                    }
                    arm_results.push(result);
                    frames.push(Frame::ScalarMatchNext {
                        expression,
                        scrutinee,
                        arms,
                        index: index + 1,
                        decision,
                        branch_state,
                        arm_results,
                        destination,
                    });
                }
                Frame::UpdateAfterBase {
                    expression,
                    record,
                    fields,
                    destination,
                    update_region,
                } => {
                    let mut evaluated = results.pop().expect("update base result retained");
                    let staged_base = if destination.is_some() {
                        let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                            unreachable!("update continuation retains update expression");
                        };
                        let staged_base =
                            CleanupPlace::whole(StorageId::Temporary(base.id.clone()));
                        self.assign_slot(&staged_base.storage, update_region)?;
                        let base_source = evaluated.owned_source.clone().ok_or_else(|| {
                            plan_error("owned record update base has no cleanup source")
                        })?;
                        if base_source != staged_base {
                            self.transfer(
                                evaluated.block,
                                base.id.clone(),
                                base_source,
                                staged_base.clone(),
                                &mut evaluated.state,
                                true,
                            )?;
                        }
                        Some(staged_base)
                    } else {
                        None
                    };
                    frames.push(Frame::UpdateNext {
                        expression,
                        record,
                        fields,
                        index: 0,
                        flow: evaluated,
                        destination,
                        update_region,
                        staged_base,
                        replaced: BTreeSet::new(),
                    });
                }
                Frame::UpdateNext {
                    expression,
                    record,
                    fields,
                    index,
                    flow,
                    destination,
                    update_region,
                    staged_base,
                    replaced,
                } => {
                    if index == fields.len() {
                        if let Some(destination) = destination {
                            let staged_base =
                                staged_base.expect("droppable update staged its base");
                            let declarations = self
                                .program
                                .declarations
                                .record_fields(record)
                                .ok_or_else(|| {
                                    plan_error(format!(
                                        "record update has unknown record `{record}`"
                                    ))
                                })?
                                .to_vec();
                            let mut state = flow.state;
                            for field in declarations {
                                if replaced.contains(&field.id) || !self.needs_drop(&field.ty)? {
                                    continue;
                                }
                                self.transfer(
                                    flow.block,
                                    expression.id.clone(),
                                    staged_base.projected(field.id.clone()),
                                    destination.projected(field.id),
                                    &mut state,
                                    false,
                                )?;
                            }
                            let (block, mut state) =
                                self.exit_scope(flow.block, state, update_region)?;
                            self.canonicalize_complete_aggregate(&destination, &mut state)?;
                            results.push(EvalResult {
                                block,
                                state,
                                owned_source: Some(destination),
                            });
                        } else {
                            results.push(EvalResult {
                                block: flow.block,
                                state: flow.state,
                                owned_source: None,
                            });
                        }
                    } else {
                        if replaced.contains(&fields[index].field) {
                            return Err(plan_error(format!(
                                "record update repeats field `{}`",
                                fields[index].field
                            )));
                        }
                        frames.push(Frame::UpdateAfterField {
                            expression,
                            record,
                            fields,
                            index,
                            destination,
                            update_region,
                            staged_base,
                            replaced,
                        });
                        if active_region != update_region {
                            frames.push(Frame::RestoreRegion(active_region));
                            active_region = update_region;
                        }
                        frames.push(Frame::Enter {
                            expression: &fields[index].value,
                            block: flow.block,
                            state: flow.state,
                        });
                    }
                }
                Frame::UpdateAfterField {
                    expression,
                    record,
                    fields,
                    index,
                    destination,
                    update_region,
                    staged_base,
                    mut replaced,
                } => {
                    let initializer = &fields[index];
                    let inserted = replaced.insert(initializer.field.clone());
                    debug_assert!(inserted);
                    let mut evaluated = results.pop().expect("update field result retained");
                    if let Some(destination) = &destination {
                        if initializer.value.ownership == OwnershipMode::Own
                            && self.needs_drop(&initializer.value.ty)?
                        {
                            let source = evaluated.owned_source.clone().ok_or_else(|| {
                                plan_error(format!(
                                    "record replacement field `{}` has no cleanup source",
                                    initializer.field
                                ))
                            })?;
                            self.transfer(
                                evaluated.block,
                                initializer.value.id.clone(),
                                source,
                                destination.projected(initializer.field.clone()),
                                &mut evaluated.state,
                                false,
                            )?;
                        }
                    }
                    frames.push(Frame::UpdateNext {
                        expression,
                        record,
                        fields,
                        index: index + 1,
                        flow: evaluated,
                        destination,
                        update_region,
                        staged_base,
                        replaced,
                    });
                }
            }
        }
        if results.len() != 1 {
            return Err(plan_error(
                "iterative cleanup lowering lost its root result",
            ));
        }
        results
            .pop()
            .ok_or_else(|| plan_error("iterative cleanup lowering produced no result"))
    }

    #[cfg(test)]
    fn lower_expr_recursive_reference(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        if matches!(expression.kind, ResolvedExprKind::Unary { .. }) {
            let mut unary = Vec::new();
            let mut leaf = expression;
            while let ResolvedExprKind::Unary { value, .. } = &leaf.kind {
                unary.push(leaf);
                leaf = value;
            }
            let mut evaluated = self.lower_expr_recursive_reference(leaf, block, state, region)?;
            for expression in unary.into_iter().rev() {
                let ResolvedExprKind::Unary { op, .. } = &expression.kind else {
                    unreachable!("unary chain contains only unary expressions");
                };
                if *op == UnaryOp::Neg {
                    let source = self.checked_source(
                        expression,
                        CheckedOperation::Neg,
                        vec![StatusCase::NegationOverflow],
                    )?;
                    let (block, state) =
                        self.split_status(evaluated.block, evaluated.state, region, source)?;
                    evaluated = EvalResult {
                        block,
                        state,
                        owned_source: None,
                    };
                } else {
                    evaluated.owned_source = None;
                }
            }
            return Ok(evaluated);
        }
        match &expression.kind {
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
            | ResolvedExprKind::String(_) => Ok(EvalResult {
                block,
                state,
                owned_source: None,
            }),
            ResolvedExprKind::Place(place) => {
                let owned_source = if expression.ownership == OwnershipMode::Own
                    && self.needs_drop(&expression.ty)?
                {
                    Some(self.place_from_hir(place)?)
                } else {
                    None
                };
                Ok(EvalResult {
                    block,
                    state,
                    owned_source,
                })
            }
            ResolvedExprKind::BorrowPlace { .. } => Ok(EvalResult {
                block,
                state,
                owned_source: None,
            }),
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(plan_error(
                        "byte range carries an unknown operation identity",
                    ));
                }
                self.schema = CLEANUP_PLAN_SCHEMA_V4;
                let mut evaluated =
                    self.lower_expr_recursive_reference(source, block, state, region)?;
                if evaluated.owned_source.is_some() {
                    return Err(plan_error("byte range received an owned source"));
                }
                evaluated = self.lower_expr_recursive_reference(
                    start,
                    evaluated.block,
                    evaluated.state,
                    region,
                )?;
                if evaluated.owned_source.is_some() {
                    return Err(plan_error("byte range received an owned start"));
                }
                evaluated = self.lower_expr_recursive_reference(
                    end,
                    evaluated.block,
                    evaluated.state,
                    region,
                )?;
                if evaluated.owned_source.is_some() {
                    return Err(plan_error("byte range received an owned end"));
                }
                self.push_transition(
                    evaluated.block,
                    CleanupTransition::CallCommit {
                        call: expression.id.clone(),
                        arguments: Vec::new(),
                    },
                );
                let status = StatusSourceId {
                    expression: expression.id.clone(),
                    lane: StatusLane::OperationFailure,
                };
                self.add_status_source(
                    status.clone(),
                    StatusProducer::PropagatedCall {
                        callee: DeclarationId::new(crate::byte_ops::RANGE_ID),
                    },
                )?;
                let (block, state) =
                    self.split_status(evaluated.block, evaluated.state, region, status)?;
                Ok(EvalResult {
                    block,
                    state,
                    owned_source: None,
                })
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => self.lower_call(
                expression,
                callee,
                instance.as_ref(),
                args,
                (block, state, region),
            ),
            ResolvedExprKind::NativeRustImportCall(call) => {
                let mut current_block = block;
                let mut current_state = state;
                for argument in &call.args {
                    let evaluated = self.lower_expr_recursive_reference(
                        argument,
                        current_block,
                        current_state,
                        region,
                    )?;
                    if evaluated.owned_source.is_some() {
                        return Err(plan_error(
                            "native Rust import received a non-scalar argument",
                        ));
                    }
                    current_block = evaluated.block;
                    current_state = evaluated.state;
                }
                Ok(EvalResult {
                    block: current_block,
                    state: current_state,
                    owned_source: None,
                })
            }
            ResolvedExprKind::HostCommandCall(call) => {
                let callee = DeclarationId::new(crate::command_io_ops::id(call.operation));
                self.lower_call(
                    expression,
                    &callee,
                    None,
                    &call.args,
                    (block, state, region),
                )
            }
            ResolvedExprKind::Unary { .. } => unreachable!("unary chain handled above"),
            ResolvedExprKind::Binary { op, left, right }
                if matches!(op, BinaryOp::And | BinaryOp::Or) =>
            {
                self.lower_lazy(expression, *op, left, right, block, state, region)
            }
            ResolvedExprKind::Binary { op, left, right } => {
                let left = self.lower_expr_recursive_reference(left, block, state, region)?;
                let right =
                    self.lower_expr_recursive_reference(right, left.block, left.state, region)?;
                let checked = match op {
                    BinaryOp::Add => Some((CheckedOperation::Add, vec![StatusCase::AddOverflow])),
                    BinaryOp::Sub => Some((CheckedOperation::Sub, vec![StatusCase::SubOverflow])),
                    BinaryOp::Mul => Some((CheckedOperation::Mul, vec![StatusCase::MulOverflow])),
                    BinaryOp::Div => Some((
                        CheckedOperation::Div,
                        vec![StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                    )),
                    BinaryOp::Rem => Some((
                        CheckedOperation::Rem,
                        vec![StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
                    )),
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => None,
                };
                let (block, state) = if let Some((operation, cases)) = checked {
                    let source = self.checked_source(expression, operation, cases)?;
                    self.split_status(right.block, right.state, region, source)?
                } else {
                    (right.block, right.state)
                };
                Ok(EvalResult {
                    block,
                    state,
                    owned_source: None,
                })
            }
            ResolvedExprKind::Block { statements, tail } => {
                self.lower_block(expression, statements, tail, block, state, region)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.lower_if(
                expression,
                condition,
                then_branch,
                else_branch,
                block,
                state,
                region,
            ),
            ResolvedExprKind::ConstructRecord { fields, .. } => {
                self.lower_record(expression, fields, block, state, region)
            }
            ResolvedExprKind::ConstructVariant { fields, .. } => {
                self.lower_copy_variant(fields, block, state, region)
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => self.lower_try(
                expression,
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
                block,
                state,
                region,
            ),
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => self.lower_try_option(
                expression,
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
                block,
                state,
                region,
            ),
            ResolvedExprKind::Match { scrutinee, arms } => {
                self.lower_match(expression, scrutinee, arms, block, state, region)
            }
            ResolvedExprKind::UpdateRecord { .. } => {
                self.lower_update_record(expression, block, state, region)
            }
            ResolvedExprKind::Project { base, field } => {
                let base = self.lower_expr_recursive_reference(base, block, state, region)?;
                let destination = self.expression_slot(expression, region)?;
                let mut state = base.state;
                if let Some(destination) = destination.clone() {
                    let source = base
                        .owned_source
                        .ok_or_else(|| plan_error("owned projection base has no cleanup source"))?
                        .projected(field.clone());
                    self.transfer(
                        base.block,
                        expression.id.clone(),
                        source,
                        destination,
                        &mut state,
                        true,
                    )?;
                }
                Ok(EvalResult {
                    block: base.block,
                    state,
                    owned_source: destination,
                })
            }
            // Class Inheritance v1: the upcast itself is transparent; its
            // consumed source remains the surrounding transfer's source.
            ResolvedExprKind::Upcast { source } => {
                self.lower_expr_recursive_reference(source, block, state, region)
            }
        }
    }

    #[cfg(test)]
    fn lower_call(
        &mut self,
        expression: &ResolvedExpr,
        callee: &DeclarationId,
        instance: Option<&crate::hir::FunctionInstanceId>,
        args: &[ResolvedExpr],
        flow: (BlockId, FlowState, CleanupRegionId),
    ) -> Result<EvalResult, Diagnostic> {
        let (block, state, region) = flow;
        let params = if instance.is_none() {
            if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                crate::string_ops::resolved_params(op)
            } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                crate::str_ops::resolved_params(op)
            } else if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                crate::byte_ops::resolved_params(op)
            } else if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
                crate::host_io_ops::resolved_params(op)
            } else if let Some(op) = crate::command_io_ops::by_id(callee.as_str()) {
                crate::command_io_ops::resolved_params(op)
            } else {
                let target = self
                    .program
                    .resolve_call_target(callee, instance)
                    .ok_or_else(|| plan_error(format!("unknown cleanup call target `{callee}`")))?;
                target.params.clone()
            }
        } else {
            let target = self
                .program
                .resolve_call_target(callee, instance)
                .ok_or_else(|| plan_error(format!("unknown cleanup call target `{callee}`")))?;
            target.params.clone()
        };
        if params.len() != args.len() {
            return Err(plan_error(format!(
                "cleanup call `{}` has inconsistent arity",
                expression.id
            )));
        }
        let mut current = block;
        let mut current_state = state;
        let mut commits = Vec::new();

        for (index, (argument, parameter)) in args.iter().zip(&params).enumerate() {
            let evaluated =
                self.lower_expr_recursive_reference(argument, current, current_state, region)?;
            current = evaluated.block;
            current_state = evaluated.state;
            if parameter.ownership == OwnershipMode::Own && self.needs_drop(&parameter.ty)? {
                let source = evaluated.owned_source.ok_or_else(|| {
                    plan_error(format!(
                        "owned call argument {} at `{}` has no cleanup source",
                        index, expression.id
                    ))
                })?;
                let epoch = self.call_argument_slot(expression, index, argument, region)?;
                self.transfer(
                    current,
                    argument.id.clone(),
                    source,
                    epoch.clone(),
                    &mut current_state,
                    true,
                )?;
                commits.push(CallArgumentTransfer {
                    parameter_index: u32::try_from(index)
                        .map_err(|_| plan_error("too many call arguments"))?,
                    source: epoch,
                });
            }
        }

        // This is the only caller-to-callee ownership boundary.  The
        // transition contains every and only owned parameter epoch in signature
        // order; once emitted, even a nonzero call status cannot restore them.
        for commit in &commits {
            self.consume_place(&commit.source, &mut current_state, &expression.id)?;
        }
        self.push_transition(
            current,
            CleanupTransition::CallCommit {
                call: expression.id.clone(),
                arguments: commits,
            },
        );

        if crate::byte_ops::by_id(callee.as_str()).is_some()
            || crate::host_io_ops::by_id(callee.as_str()).is_some()
            || crate::command_io_ops::by_id(callee.as_str()).is_some_and(|op| {
                crate::command_io_ops::failure(op)
                    == crate::command_io_ops::CommandIoFailure::Infallible
            })
        {
            let destination = self.expression_slot(expression, region)?;
            if let Some(destination) = destination.clone() {
                self.initialize(
                    current,
                    expression.id.clone(),
                    destination,
                    &mut current_state,
                )?;
            }
            return Ok(EvalResult {
                block: current,
                state: current_state,
                owned_source: destination,
            });
        }

        let source = StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::OperationFailure,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::PropagatedCall {
                callee: callee.clone(),
            },
        )?;
        let (success, mut success_state) =
            self.split_status(current, current_state, region, source)?;
        let destination = self.expression_slot(expression, region)?;
        if let Some(destination) = destination.clone() {
            // Caller result/out storage remains uninitialized until the
            // propagated status is known to be zero.
            self.initialize(
                success,
                expression.id.clone(),
                destination,
                &mut success_state,
            )?;
        }
        Ok(EvalResult {
            block: success,
            state: success_state,
            owned_source: destination,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn lower_lazy(
        &mut self,
        _expression: &ResolvedExpr,
        operation: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let left_expression_id = left.id.clone();
        let left = self.lower_expr_recursive_reference(left, block, state, region)?;
        let evaluate_right_when = operation == BinaryOp::And;
        let evaluate = self.new_block(region)?;
        let skip = self.new_block(region)?;
        let evaluate_edge = self.new_edge(
            left.block,
            evaluate,
            EdgeCondition::BooleanResult(left_expression_id.clone(), evaluate_right_when),
        )?;
        let skip_edge = self.new_edge(
            left.block,
            skip,
            EdgeCondition::BooleanResult(left_expression_id, !evaluate_right_when),
        )?;
        self.terminate(
            left.block,
            CleanupTerminator::Branch(vec![evaluate_edge, skip_edge]),
        )?;

        let evaluated_right =
            self.lower_expr_recursive_reference(right, evaluate, left.state.clone(), region)?;
        let joined_state = self.merge_states(&evaluated_right.state, &left.state)?;
        let join = self.new_block(region)?;
        let evaluated_edge = self.new_edge(evaluated_right.block, join, EdgeCondition::Always)?;
        self.terminate(
            evaluated_right.block,
            CleanupTerminator::Goto(evaluated_edge),
        )?;
        let skipped_edge = self.new_edge(skip, join, EdgeCondition::Always)?;
        self.terminate(skip, CleanupTerminator::Goto(skipped_edge))?;
        Ok(EvalResult {
            block: join,
            state: joined_state,
            owned_source: None,
        })
    }

    #[cfg(test)]
    fn lower_block(
        &mut self,
        expression: &ResolvedExpr,
        statements: &[ResolvedStatement],
        tail: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        parent: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        // The block expression's completed value belongs to the surrounding
        // region; locals and intermediate temporaries belong to the child.
        let destination = self.expression_slot(expression, parent)?;
        let region = self.new_region(parent)?;
        let entry = self.new_block(region)?;
        let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
        self.terminate(block, CleanupTerminator::Goto(edge))?;

        let mut current = entry;
        let mut current_state = state;
        for statement in statements {
            // Field Mutation v1 stores lower their RHS like an initializer
            // and never transfer into a binding slot.
            if let ResolvedStatement::Assign {
                field: Some(_),
                value,
                ..
            } = statement
            {
                let evaluated =
                    self.lower_expr_recursive_reference(value, current, current_state, region)?;
                current = evaluated.block;
                current_state = evaluated.state;
                continue;
            }
            let (binding, value) = match statement {
                ResolvedStatement::Let { binding, value, .. }
                | ResolvedStatement::Assign { binding, value, .. } => (binding, value),
                // Unsafe boundaries bind nothing: their ordinary block body lowers
                // like any nested block expression.
                ResolvedStatement::Unsafe { body, .. } => {
                    let evaluated =
                        self.lower_expr_recursive_reference(body, current, current_state, region)?;
                    current = evaluated.block;
                    current_state = evaluated.state;
                    continue;
                }
                // Bounded While-Loops v1: linearize one admitted iteration.
                ResolvedStatement::While {
                    condition, body, ..
                } => {
                    let evaluated =
                        self.lower_while(condition, body, current, current_state, region)?;
                    current = evaluated.block;
                    current_state = evaluated.state;
                    continue;
                }
            };
            let evaluated =
                self.lower_expr_recursive_reference(value, current, current_state, region)?;
            current = evaluated.block;
            current_state = evaluated.state;
            if let Some(binding_place) = self.binding_slot(binding, region)? {
                let source = evaluated.owned_source.ok_or_else(|| {
                    plan_error(format!(
                        "owned binding `{}` has no cleanup source",
                        binding.id
                    ))
                })?;
                self.transfer(
                    current,
                    value.id.clone(),
                    source,
                    binding_place,
                    &mut current_state,
                    true,
                )?;
            }
        }
        let evaluated_tail =
            self.lower_expr_recursive_reference(tail, current, current_state, region)?;
        current = evaluated_tail.block;
        current_state = evaluated_tail.state;
        if let Some(destination) = destination.clone() {
            let source = evaluated_tail
                .owned_source
                .ok_or_else(|| plan_error("owned block tail has no cleanup source"))?;
            self.transfer(
                current,
                expression.id.clone(),
                source,
                destination,
                &mut current_state,
                true,
            )?;
        }
        let (continuation, state) = self.exit_scope(current, current_state, region)?;
        Ok(EvalResult {
            block: continuation,
            state,
            owned_source: destination,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn lower_if(
        &mut self,
        expression: &ResolvedExpr,
        condition: &ResolvedExpr,
        then_branch: &ResolvedExpr,
        else_branch: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let destination = self.expression_slot(expression, region)?;
        let condition = self.lower_expr_recursive_reference(condition, block, state, region)?;
        let then_entry = self.new_block(region)?;
        let else_entry = self.new_block(region)?;
        let then_edge = self.new_edge(
            condition.block,
            then_entry,
            EdgeCondition::BooleanResult(condition_id(expression)?, true),
        )?;
        let else_edge = self.new_edge(
            condition.block,
            else_entry,
            EdgeCondition::BooleanResult(condition_id(expression)?, false),
        )?;
        self.terminate(
            condition.block,
            CleanupTerminator::Branch(vec![then_edge, else_edge]),
        )?;

        let mut then_result = self.lower_expr_recursive_reference(
            then_branch,
            then_entry,
            condition.state.clone(),
            region,
        )?;
        if let Some(destination) = destination.clone() {
            let source = then_result
                .owned_source
                .take()
                .ok_or_else(|| plan_error("owned then branch has no cleanup source"))?;
            self.transfer(
                then_result.block,
                expression.id.clone(),
                source,
                destination,
                &mut then_result.state,
                true,
            )?;
        }

        let mut else_result =
            self.lower_expr_recursive_reference(else_branch, else_entry, condition.state, region)?;
        if let Some(destination) = destination.clone() {
            let source = else_result
                .owned_source
                .take()
                .ok_or_else(|| plan_error("owned else branch has no cleanup source"))?;
            self.transfer(
                else_result.block,
                expression.id.clone(),
                source,
                destination,
                &mut else_result.state,
                true,
            )?;
        }

        let state = self.merge_states(&then_result.state, &else_result.state)?;
        let join = self.new_block(region)?;
        let then_join = self.new_edge(then_result.block, join, EdgeCondition::Always)?;
        self.terminate(then_result.block, CleanupTerminator::Goto(then_join))?;
        let else_join = self.new_edge(else_result.block, join, EdgeCondition::Always)?;
        self.terminate(else_result.block, CleanupTerminator::Goto(else_join))?;
        Ok(EvalResult {
            block: join,
            state,
            owned_source: destination,
        })
    }

    #[cfg(test)]
    fn lower_record(
        &mut self,
        expression: &ResolvedExpr,
        fields: &[crate::hir::ResolvedFieldInitializer],
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let destination = self.expression_slot(expression, region)?;
        let mut current = block;
        let mut current_state = state;
        for initializer in fields {
            let evaluated = self.lower_expr_recursive_reference(
                &initializer.value,
                current,
                current_state,
                region,
            )?;
            current = evaluated.block;
            current_state = evaluated.state;
            if initializer.value.ownership == OwnershipMode::Own
                && self.needs_drop(&initializer.value.ty)?
            {
                let source = evaluated.owned_source.ok_or_else(|| {
                    plan_error(format!(
                        "record field `{}` has no cleanup source",
                        initializer.field
                    ))
                })?;
                let field_destination = destination
                    .as_ref()
                    .ok_or_else(|| plan_error("droppable record constructor has no cleanup slot"))?
                    .projected(initializer.field.clone());
                self.transfer(
                    current,
                    initializer.value.id.clone(),
                    source,
                    field_destination,
                    &mut current_state,
                    false,
                )?;
            }
        }

        // While construction is partial, `live_order` is actual initializer
        // completion order and failure reverses it.  At the successful
        // whole-aggregate boundary, history is intentionally normalized to
        // recursive declaration order as specified by cleanup-plan v1.
        if let Some(destination) = &destination {
            self.canonicalize_complete_aggregate(destination, &mut current_state)?;
        }
        Ok(EvalResult {
            block: current,
            state: current_state,
            owned_source: destination,
        })
    }

    #[cfg(test)]
    fn lower_copy_variant(
        &mut self,
        fields: &[crate::hir::ResolvedFieldInitializer],
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let mut evaluated = EvalResult {
            block,
            state,
            owned_source: None,
        };
        for field in fields {
            evaluated = self.lower_expr_recursive_reference(
                &field.value,
                evaluated.block,
                evaluated.state,
                region,
            )?;
            if field.value.ownership == OwnershipMode::Own && self.needs_drop(&field.value.ty)? {
                return Err(plan_error(
                    "droppable variant payload reached the copy-only cleanup slice",
                ));
            }
        }
        evaluated.owned_source = None;
        Ok(evaluated)
    }

    #[allow(clippy::too_many_arguments)]
    fn check_try_metadata(
        &self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        result: &DeclarationId,
        ok_case: &DeclarationId,
        ok_field: &DeclarationId,
        err_case: &DeclarationId,
        err_field: &DeclarationId,
        residual_type: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        if result.as_str() != prelude::RESULT_ID
            || ok_case.as_str() != prelude::RESULT_OK_ID
            || ok_field.as_str() != prelude::RESULT_OK_VALUE_ID
            || err_case.as_str() != prelude::RESULT_ERR_ID
            || err_field.as_str() != prelude::RESULT_ERR_ERROR_ID
        {
            return Err(plan_error(
                "postfix `?` does not authenticate the ordinary Result prelude",
            ));
        }
        for id in [result, ok_case, ok_field, err_case, err_field] {
            let declaration = self
                .program
                .declarations
                .declaration(id)
                .ok_or_else(|| plan_error(format!("postfix `?` references unknown `{id}`")))?;
            if declaration.identity_origin != IdentityOrigin::CompilerOwned {
                return Err(plan_error(format!(
                    "postfix `?` reference `{id}` is not compiler-owned"
                )));
            }
        }
        let source_arguments = result_arguments(&operand.ty, result)?;
        let target_arguments = result_arguments(residual_type, result)?;
        if source_arguments.len() != 2
            || target_arguments.len() != 2
            || source_arguments
                .iter()
                .chain(target_arguments.iter())
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            || expression.ty != source_arguments[0]
            || source_arguments[1] != target_arguments[1]
            || residual_type != &self.function.return_type
        {
            return Err(plan_error(
                "postfix `?` has inconsistent source, value, residual, or function types",
            ));
        }
        for ty in [&operand.ty, residual_type] {
            let facts = self
                .program
                .declarations
                .type_facts(ty)
                .ok_or_else(|| plan_error("postfix `?` Result instance has no type facts"))?;
            if !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop {
                return Err(plan_error(
                    "postfix `?` reached cleanup planning outside the Copy Result slice",
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_try(
        &mut self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        result: &DeclarationId,
        ok_case: &DeclarationId,
        ok_field: &DeclarationId,
        err_case: &DeclarationId,
        err_field: &DeclarationId,
        residual_type: &ResolvedType,
        evaluated: EvalResult,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        if evaluated.owned_source.is_some() {
            return Err(plan_error(
                "postfix `?` operand reached the Copy slice with cleanup storage",
            ));
        }
        let success = self.new_block(region)?;
        let residual = self.new_block(region)?;
        let success_edge = self.new_edge(
            evaluated.block,
            success,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: ok_case.clone(),
                matches: true,
            },
        )?;
        let residual_edge = self.new_edge(
            evaluated.block,
            residual,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: ok_case.clone(),
                matches: false,
            },
        )?;
        self.terminate(
            evaluated.block,
            CleanupTerminator::Branch(vec![success_edge, residual_edge]),
        )?;
        self.push_transition(
            residual,
            CleanupTransition::StageCopyResult {
                source: StagedCopyResultSource::TryResidual {
                    expression: expression.id.clone(),
                    operand: operand.id.clone(),
                    source_instance: operand.ty.clone(),
                    target_instance: residual_type.clone(),
                    result: result.clone(),
                    ok_case: ok_case.clone(),
                    ok_field: ok_field.clone(),
                    err_case: err_case.clone(),
                    err_field: err_field.clone(),
                },
            },
        );
        self.pending_try_residuals.push(PendingTryResidual {
            block: residual,
            state: evaluated.state.clone(),
            region,
        });
        Ok(EvalResult {
            block: success,
            state: evaluated.state,
            owned_source: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn check_try_option_metadata(
        &mut self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        option: &DeclarationId,
        some_case: &DeclarationId,
        some_field: &DeclarationId,
        none_case: &DeclarationId,
        residual_type: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        if self.schema == CLEANUP_PLAN_SCHEMA_V2 {
            self.schema = CLEANUP_PLAN_SCHEMA_V3;
        }
        if option.as_str() != prelude::OPTION_ID
            || some_case.as_str() != prelude::OPTION_SOME_ID
            || some_field.as_str() != prelude::OPTION_SOME_VALUE_ID
            || none_case.as_str() != prelude::OPTION_NONE_ID
        {
            return Err(plan_error(
                "Option postfix `?` does not authenticate the ordinary Option prelude",
            ));
        }
        for id in [option, some_case, some_field, none_case] {
            let declaration = self.program.declarations.declaration(id).ok_or_else(|| {
                plan_error(format!("Option postfix `?` references unknown `{id}`"))
            })?;
            if declaration.identity_origin != IdentityOrigin::CompilerOwned {
                return Err(plan_error(format!(
                    "Option postfix `?` reference `{id}` is not compiler-owned"
                )));
            }
        }
        let source_arguments = option_arguments(&operand.ty, option)?;
        let target_arguments = option_arguments(residual_type, option)?;
        if source_arguments.len() != 1
            || target_arguments.len() != 1
            || source_arguments
                .iter()
                .chain(target_arguments.iter())
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            || expression.ty != source_arguments[0]
            || residual_type != &self.function.return_type
        {
            return Err(plan_error(
                "Option postfix `?` has inconsistent source, value, residual, or function types",
            ));
        }
        for ty in [&operand.ty, residual_type] {
            let facts = self
                .program
                .declarations
                .type_facts(ty)
                .ok_or_else(|| plan_error("Option postfix `?` instance has no type facts"))?;
            if !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop {
                return Err(plan_error(
                    "Option postfix `?` reached cleanup planning outside the Copy Option slice",
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_try_option(
        &mut self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        option: &DeclarationId,
        some_case: &DeclarationId,
        some_field: &DeclarationId,
        none_case: &DeclarationId,
        residual_type: &ResolvedType,
        evaluated: EvalResult,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        if evaluated.owned_source.is_some() {
            return Err(plan_error(
                "Option postfix `?` operand reached the Copy slice with cleanup storage",
            ));
        }
        let success = self.new_block(region)?;
        let residual = self.new_block(region)?;
        let success_edge = self.new_edge(
            evaluated.block,
            success,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: some_case.clone(),
                matches: true,
            },
        )?;
        let residual_edge = self.new_edge(
            evaluated.block,
            residual,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: some_case.clone(),
                matches: false,
            },
        )?;
        self.terminate(
            evaluated.block,
            CleanupTerminator::Branch(vec![success_edge, residual_edge]),
        )?;
        self.push_transition(
            residual,
            CleanupTransition::StageCopyResult {
                source: StagedCopyResultSource::TryOptionNone {
                    expression: expression.id.clone(),
                    operand: operand.id.clone(),
                    source_instance: operand.ty.clone(),
                    target_instance: residual_type.clone(),
                    option: option.clone(),
                    some_case: some_case.clone(),
                    some_field: some_field.clone(),
                    none_case: none_case.clone(),
                },
            },
        );
        self.pending_try_residuals.push(PendingTryResidual {
            block: residual,
            state: evaluated.state.clone(),
            region,
        });
        Ok(EvalResult {
            block: success,
            state: evaluated.state,
            owned_source: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn lower_try(
        &mut self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        result: &DeclarationId,
        ok_case: &DeclarationId,
        ok_field: &DeclarationId,
        err_case: &DeclarationId,
        err_field: &DeclarationId,
        residual_type: &ResolvedType,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        if result.as_str() != prelude::RESULT_ID
            || ok_case.as_str() != prelude::RESULT_OK_ID
            || ok_field.as_str() != prelude::RESULT_OK_VALUE_ID
            || err_case.as_str() != prelude::RESULT_ERR_ID
            || err_field.as_str() != prelude::RESULT_ERR_ERROR_ID
        {
            return Err(plan_error(
                "postfix `?` does not authenticate the ordinary Result prelude",
            ));
        }
        for id in [result, ok_case, ok_field, err_case, err_field] {
            let declaration = self
                .program
                .declarations
                .declaration(id)
                .ok_or_else(|| plan_error(format!("postfix `?` references unknown `{id}`")))?;
            if declaration.identity_origin != IdentityOrigin::CompilerOwned {
                return Err(plan_error(format!(
                    "postfix `?` reference `{id}` is not compiler-owned"
                )));
            }
        }
        let source_arguments = result_arguments(&operand.ty, result)?;
        let target_arguments = result_arguments(residual_type, result)?;
        if source_arguments.len() != 2
            || target_arguments.len() != 2
            || source_arguments
                .iter()
                .chain(target_arguments.iter())
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            || expression.ty != source_arguments[0]
            || source_arguments[1] != target_arguments[1]
            || residual_type != &self.function.return_type
        {
            return Err(plan_error(
                "postfix `?` has inconsistent source, value, residual, or function types",
            ));
        }
        for ty in [&operand.ty, residual_type] {
            let facts = self
                .program
                .declarations
                .type_facts(ty)
                .ok_or_else(|| plan_error("postfix `?` Result instance has no type facts"))?;
            if !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop {
                return Err(plan_error(
                    "postfix `?` reached cleanup planning outside the Copy Result slice",
                ));
            }
        }

        let evaluated = self.lower_expr_recursive_reference(operand, block, state, region)?;
        if evaluated.owned_source.is_some() {
            return Err(plan_error(
                "postfix `?` operand reached the Copy slice with cleanup storage",
            ));
        }
        let success = self.new_block(region)?;
        let residual = self.new_block(region)?;
        let success_edge = self.new_edge(
            evaluated.block,
            success,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: ok_case.clone(),
                matches: true,
            },
        )?;
        let residual_edge = self.new_edge(
            evaluated.block,
            residual,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: ok_case.clone(),
                matches: false,
            },
        )?;
        self.terminate(
            evaluated.block,
            CleanupTerminator::Branch(vec![success_edge, residual_edge]),
        )?;
        self.push_transition(
            residual,
            CleanupTransition::StageCopyResult {
                source: StagedCopyResultSource::TryResidual {
                    expression: expression.id.clone(),
                    operand: operand.id.clone(),
                    source_instance: operand.ty.clone(),
                    target_instance: residual_type.clone(),
                    result: result.clone(),
                    ok_case: ok_case.clone(),
                    ok_field: ok_field.clone(),
                    err_case: err_case.clone(),
                    err_field: err_field.clone(),
                },
            },
        );
        self.pending_try_residuals.push(PendingTryResidual {
            block: residual,
            state: evaluated.state.clone(),
            region,
        });
        Ok(EvalResult {
            block: success,
            state: evaluated.state,
            owned_source: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn lower_try_option(
        &mut self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        option: &DeclarationId,
        some_case: &DeclarationId,
        some_field: &DeclarationId,
        none_case: &DeclarationId,
        residual_type: &ResolvedType,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        if self.schema == CLEANUP_PLAN_SCHEMA_V2 {
            self.schema = CLEANUP_PLAN_SCHEMA_V3;
        }
        if option.as_str() != prelude::OPTION_ID
            || some_case.as_str() != prelude::OPTION_SOME_ID
            || some_field.as_str() != prelude::OPTION_SOME_VALUE_ID
            || none_case.as_str() != prelude::OPTION_NONE_ID
        {
            return Err(plan_error(
                "Option postfix `?` does not authenticate the ordinary Option prelude",
            ));
        }
        for id in [option, some_case, some_field, none_case] {
            let declaration = self.program.declarations.declaration(id).ok_or_else(|| {
                plan_error(format!("Option postfix `?` references unknown `{id}`"))
            })?;
            if declaration.identity_origin != IdentityOrigin::CompilerOwned {
                return Err(plan_error(format!(
                    "Option postfix `?` reference `{id}` is not compiler-owned"
                )));
            }
        }
        let source_arguments = option_arguments(&operand.ty, option)?;
        let target_arguments = option_arguments(residual_type, option)?;
        if source_arguments.len() != 1
            || target_arguments.len() != 1
            || source_arguments
                .iter()
                .chain(target_arguments.iter())
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            || expression.ty != source_arguments[0]
            || residual_type != &self.function.return_type
        {
            return Err(plan_error(
                "Option postfix `?` has inconsistent source, value, residual, or function types",
            ));
        }
        for ty in [&operand.ty, residual_type] {
            let facts = self
                .program
                .declarations
                .type_facts(ty)
                .ok_or_else(|| plan_error("Option postfix `?` instance has no type facts"))?;
            if !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop {
                return Err(plan_error(
                    "Option postfix `?` reached cleanup planning outside the Copy Option slice",
                ));
            }
        }

        let evaluated = self.lower_expr_recursive_reference(operand, block, state, region)?;
        if evaluated.owned_source.is_some() {
            return Err(plan_error(
                "Option postfix `?` operand reached the Copy slice with cleanup storage",
            ));
        }
        let success = self.new_block(region)?;
        let residual = self.new_block(region)?;
        let success_edge = self.new_edge(
            evaluated.block,
            success,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: some_case.clone(),
                matches: true,
            },
        )?;
        let residual_edge = self.new_edge(
            evaluated.block,
            residual,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: some_case.clone(),
                matches: false,
            },
        )?;
        self.terminate(
            evaluated.block,
            CleanupTerminator::Branch(vec![success_edge, residual_edge]),
        )?;
        self.push_transition(
            residual,
            CleanupTransition::StageCopyResult {
                source: StagedCopyResultSource::TryOptionNone {
                    expression: expression.id.clone(),
                    operand: operand.id.clone(),
                    source_instance: operand.ty.clone(),
                    target_instance: residual_type.clone(),
                    option: option.clone(),
                    some_case: some_case.clone(),
                    some_field: some_field.clone(),
                    none_case: none_case.clone(),
                },
            },
        );
        self.pending_try_residuals.push(PendingTryResidual {
            block: residual,
            state: evaluated.state.clone(),
            region,
        });
        Ok(EvalResult {
            block: success,
            state: evaluated.state,
            owned_source: None,
        })
    }

    #[cfg(test)]
    fn lower_match(
        &mut self,
        expression: &ResolvedExpr,
        scrutinee: &ResolvedExpr,
        arms: &[ResolvedMatchArm],
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        if arms.is_empty() {
            return Err(plan_error("copy-variant match has no arms"));
        }
        if self.needs_drop(&arms[0].value.ty)? {
            return Err(plan_error(
                "droppable match result reached the copy-only cleanup slice",
            ));
        }

        let scrutinee_result =
            self.lower_expr_recursive_reference(scrutinee, block, state, region)?;
        if scrutinee_result.owned_source.is_some() {
            return Err(plan_error(
                "droppable match scrutinee reached the copy-only cleanup slice",
            ));
        }
        // Refutable Match v1: recursive-reference twin of the scalar
        // decision chain.
        if matches!(
            scrutinee.ty,
            ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::Char
                | ResolvedType::Bool
        ) {
            let destination = self.expression_slot(expression, region)?;
            return self.lower_scalar_match(
                expression,
                scrutinee,
                arms,
                scrutinee_result.block,
                scrutinee_result.state,
                region,
                destination,
            );
        }

        let is_record = match &scrutinee.ty {
            ResolvedType::Nominal { declaration, .. } => self
                .program
                .declarations
                .declaration(declaration)
                .is_some_and(|item| item.kind == DeclarationKind::Record),
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
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::TypeParameter { .. } => false,
        };
        if is_record {
            let [arm] = arms else {
                return Err(plan_error(
                    "irrefutable record match must have exactly one arm",
                ));
            };
            if matches!(&arm.pattern, ResolvedMatchPattern::Variant { .. }) {
                return Err(plan_error("variant pattern has a record match scrutinee"));
            }
            let result = self.lower_expr_recursive_reference(
                &arm.value,
                scrutinee_result.block,
                scrutinee_result.state,
                region,
            )?;
            if result.owned_source.is_some() {
                return Err(plan_error(
                    "droppable record match arm reached the copy-only cleanup slice",
                ));
            }
            return Ok(result);
        }

        let mut decision = scrutinee_result.block;
        let branch_state = scrutinee_result.state;
        let mut arm_results = Vec::with_capacity(arms.len());
        for (index, arm) in arms.iter().enumerate() {
            let final_arm = index + 1 == arms.len();
            let arm_entry = self.new_block(region)?;
            if final_arm {
                let edge = self.new_edge(decision, arm_entry, EdgeCondition::Always)?;
                self.terminate(decision, CleanupTerminator::Goto(edge))?;
            } else {
                let ResolvedMatchPattern::Variant { case, .. } = &arm.pattern else {
                    return Err(plan_error(
                        "wildcard match arm must be the final exhaustive arm",
                    ));
                };
                let next_decision = self.new_block(region)?;
                let selected = self.new_edge(
                    decision,
                    arm_entry,
                    EdgeCondition::VariantCase {
                        scrutinee: scrutinee.id.clone(),
                        case: case.clone(),
                        matches: true,
                    },
                )?;
                let rejected = self.new_edge(
                    decision,
                    next_decision,
                    EdgeCondition::VariantCase {
                        scrutinee: scrutinee.id.clone(),
                        case: case.clone(),
                        matches: false,
                    },
                )?;
                self.terminate(
                    decision,
                    CleanupTerminator::Branch(vec![selected, rejected]),
                )?;
                decision = next_decision;
            }

            let result = self.lower_expr_recursive_reference(
                &arm.value,
                arm_entry,
                branch_state.clone(),
                region,
            )?;
            if result.owned_source.is_some() {
                return Err(plan_error(
                    "droppable match arm reached the copy-only cleanup slice",
                ));
            }
            arm_results.push(result);
        }

        let mut merged_state = arm_results[0].state.clone();
        for result in &arm_results[1..] {
            merged_state = self.merge_states(&merged_state, &result.state)?;
        }
        let join = self.new_block(region)?;
        for result in arm_results {
            let edge = self.new_edge(result.block, join, EdgeCondition::Always)?;
            self.terminate(result.block, CleanupTerminator::Goto(edge))?;
        }
        Ok(EvalResult {
            block: join,
            state: merged_state,
            owned_source: None,
        })
    }

    #[cfg(test)]
    fn lower_update_record(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } = &expression.kind
        else {
            return Err(plan_error(
                "record-update lowering received another expression",
            ));
        };
        let destination = self.expression_slot(expression, region)?;

        // A record without droppable leaves has no cleanup state. Evaluation
        // order still matters, so walk base then replacements in their authored
        // order and leave physical value movement to the backend layout lane.
        let Some(destination) = destination else {
            let mut evaluated = self.lower_expr_recursive_reference(base, block, state, region)?;
            for initializer in fields {
                evaluated = self.lower_expr_recursive_reference(
                    &initializer.value,
                    evaluated.block,
                    evaluated.state,
                    region,
                )?;
            }
            return Ok(EvalResult {
                block: evaluated.block,
                state: evaluated.state,
                owned_source: None,
            });
        };

        // The base epoch is isolated so the same reverse cleanup handles both
        // failure and successful disposal of displaced values.  The completed
        // destination belongs to the parent region and therefore survives the
        // child region's normal exit.
        let update_region = self.new_region(region)?;
        let entry = self.new_block(update_region)?;
        let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
        self.terminate(block, CleanupTerminator::Goto(edge))?;

        let mut evaluated =
            self.lower_expr_recursive_reference(base, entry, state, update_region)?;
        let staged_base = CleanupPlace::whole(StorageId::Temporary(base.id.clone()));
        self.assign_slot(&staged_base.storage, update_region)?;
        let base_source = evaluated
            .owned_source
            .clone()
            .ok_or_else(|| plan_error("owned record update base has no cleanup source"))?;
        if base_source != staged_base {
            self.transfer(
                evaluated.block,
                base.id.clone(),
                base_source,
                staged_base.clone(),
                &mut evaluated.state,
                true,
            )?;
        }

        let mut replaced = BTreeSet::new();
        for initializer in fields {
            if !replaced.insert(initializer.field.clone()) {
                return Err(plan_error(format!(
                    "record update repeats field `{}`",
                    initializer.field
                )));
            }
            evaluated = self.lower_expr_recursive_reference(
                &initializer.value,
                evaluated.block,
                evaluated.state,
                update_region,
            )?;
            if initializer.value.ownership == OwnershipMode::Own
                && self.needs_drop(&initializer.value.ty)?
            {
                let source = evaluated.owned_source.clone().ok_or_else(|| {
                    plan_error(format!(
                        "record replacement field `{}` has no cleanup source",
                        initializer.field
                    ))
                })?;
                self.transfer(
                    evaluated.block,
                    initializer.value.id.clone(),
                    source,
                    destination.projected(initializer.field.clone()),
                    &mut evaluated.state,
                    false,
                )?;
            }
        }

        let declarations = self
            .program
            .declarations
            .record_fields(record)
            .ok_or_else(|| plan_error(format!("record update has unknown record `{record}`")))?
            .to_vec();
        for field in declarations {
            if replaced.contains(&field.id) || !self.needs_drop(&field.ty)? {
                continue;
            }
            self.transfer(
                evaluated.block,
                expression.id.clone(),
                staged_base.projected(field.id.clone()),
                destination.projected(field.id),
                &mut evaluated.state,
                false,
            )?;
        }

        let (block, mut state) =
            self.exit_scope(evaluated.block, evaluated.state, update_region)?;
        self.canonicalize_complete_aggregate(&destination, &mut state)?;
        Ok(EvalResult {
            block,
            state,
            owned_source: Some(destination),
        })
    }
}

fn result_arguments<'a>(
    ty: &'a ResolvedType,
    result: &DeclarationId,
) -> Result<&'a [ResolvedType], Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(plan_error("postfix `?` operand is not a nominal Result"));
    };
    if declaration != result {
        return Err(plan_error(
            "postfix `?` operand or residual is not the authenticated Result",
        ));
    }
    Ok(arguments)
}

fn option_arguments<'a>(
    ty: &'a ResolvedType,
    option: &DeclarationId,
) -> Result<&'a [ResolvedType], Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(plan_error(
            "Option postfix `?` operand is not a nominal Option",
        ));
    };
    if declaration != option {
        return Err(plan_error(
            "Option postfix `?` operand or residual is not the authenticated Option",
        ));
    }
    Ok(arguments)
}

fn condition_id(expression: &ResolvedExpr) -> Result<ExpressionId, Diagnostic> {
    let ResolvedExprKind::If { condition, .. } = &expression.kind else {
        return Err(plan_error(
            "cleanup if lowering received a non-if expression",
        ));
    };
    Ok(condition.id.clone())
}

fn plan_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", format!("cleanup plan: {}", message.into()))
}

#[cfg(test)]
mod iterative_lowering_tests {
    use std::path::Path;

    use sha2::{Digest, Sha256};

    use super::assert_expression_lowering_oracle;
    use crate::{hir, parse};

    #[test]
    fn iterative_lowering_private_frame_sizes_stay_within_capacity_formula() {
        assert!(
            std::mem::size_of::<super::EvalResult>() <= super::CLEANUP_EVAL_RESULT_SIZE_CEILING
        );
    }

    #[test]
    fn fallible_byte_range_selects_v4_and_keeps_an_exact_status_source() {
        let source = r#"
module test.cleanup_byte_range;
@id("window.len")
fn window_len(value: borrow Slice<u8>, start: usize, end: usize) -> usize {
  byte_len(byte_range(value, start, end))
}
@id("main") fn main() -> i64 { 0 }
"#;
        let program = hir::resolve(
            &parse(source, Path::new("cleanup-byte-range.spx")).expect("source parses"),
        )
        .expect("source resolves");
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "window.len")
            .expect("range function is retained");
        assert_expression_lowering_oracle(&program, function, &function.body);
        assert_eq!(
            function.cleanup_plan.schema,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V4
        );
        assert!(function.cleanup_plan.status_sources.iter().any(|source| {
            matches!(
                &source.producer,
                crate::cleanup_plan::StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == crate::byte_ops::RANGE_ID
            )
        }));
    }

    #[test]
    fn iterative_lowering_matches_recursive_reference_for_every_resolved_body() {
        let source = r#"
module test.cleanup_lowering_oracle;
permit { host.echo }
@id("choice")
variant Choice {
  @id("choice.a") A { @id("choice.a.v") v: i64, },
  @id("choice.b") B,
}
@id("pair")
record Pair {
  @id("pair.a") a: i64,
  @id("pair.b") b: i64,
}
@id("host.echo.interface")
interface HostEcho permits { host.echo } {
  @id("host.echo") import rust fn host_echo(value: i64) -> i64
    effects { host.echo }
    failure status "host.echo.v1";
}
@id("callee") fn callee(a: i64, b: i64) -> i64 { a + b }
@id("identity") fn identity<T>(value: T) -> T { value }
@id("option_use") fn option_use(value: Option<i64>) -> Option<bool> {
  let checked = value?;
  Option<bool>::Some { value: checked > 0 }
}
@id("result_use") fn result_use(value: Result<i64, bool>) -> Result<bool, bool> {
  let checked = value?;
  Result<bool, bool>::Ok { value: checked > 0 }
}
@id("exercise") fn exercise(flag: bool, choice: Choice, pair: Pair) -> i64
  uses { host.echo }
{
  let x = callee(1, 2);
  let native = host_echo(identity<i64>(x));
  let rebuilt = if flag && true { Choice::A { v: Pair { a: native, b: 3 }.a } } else { choice };
  let y = pair with { b: 4 }.b;
  match rebuilt { Choice::A { v } => y + v, Choice::B {} => -y, }
}
@id("main") fn main() -> i64 { 0 }
"#;
        crate::cleanup::reset_capacity_high_water();
        let program =
            hir::resolve(&parse(source, Path::new("cleanup-lowering-oracle.spx")).unwrap())
                .unwrap();
        super::reset_lower_capacity_high_water();
        for function in &program.functions {
            assert_expression_lowering_oracle(&program, function, &function.body);
        }
        assert!(super::lower_capacity_high_water() > 0);
        assert!(crate::cleanup::capacity_high_water() > 0);
    }

    #[test]
    fn inventory_and_cleanup_capacity_cover_owned_hostile_families() {
        let source = include_str!("../../tests/fixtures/native_rust_hir_capacity.spx");
        let parsed = parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
        let canonical = crate::format::canonical(&parsed);
        assert_eq!(
            format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(canonical.as_bytes()))
            ),
            "sha256:2a012464bb1bdb624a79972d558fe837f6d55a9cd9f40d2ead16bfbba615f316",
            "shared canonical hostile identity drifted"
        );
        crate::cleanup::reset_capacity_high_water();
        super::reset_lower_capacity_high_water();
        let resolved = hir::resolve(&parsed).unwrap();
        assert!(resolved.functions.iter().any(|function| {
            function.cleanup.slots.iter().any(|slot| {
                matches!(
                    slot.origin,
                    crate::cleanup::CleanupStorageOrigin::ProvisionalResult { .. }
                )
            })
        }));
        assert!(resolved
            .functions
            .iter()
            .any(|function| function.params.len() == 8));
        for identity in [
            "choice.nested",
            "scalar.nested-wide",
            "generic.calls",
            "pair.update",
            "token.call",
            "option.use",
            "result.use",
            "host.use",
        ] {
            assert!(
                resolved
                    .functions
                    .iter()
                    .any(|function| function.id.as_str() == identity),
                "hostile family `{identity}` was not resolved"
            );
        }
        assert_eq!(source.matches("generic_identity<").count(), 7);
        assert_eq!(resolved.function_instances.len(), 2);
        assert!(
            resolved
                .functions
                .iter()
                .find(|function| function.id.as_str() == "choice.nested")
                .unwrap()
                .cleanup
                .flags
                .len()
                >= 8
        );
        assert!(resolved.functions.iter().any(|function| {
            function
                .cleanup_plan
                .blocks
                .iter()
                .any(|block| matches!(block.terminator, crate::cleanup_plan::CleanupTerminator::Branch(ref edges) if edges.len() >= 2))
        }));
        let transitions = resolved
            .functions
            .iter()
            .flat_map(|function| &function.cleanup_plan.blocks)
            .flat_map(|block| &block.transitions)
            .collect::<Vec<_>>();
        assert!(transitions.iter().any(|transition| matches!(
            transition,
            crate::cleanup_plan::CleanupTransition::CallCommit { arguments, .. }
                if arguments.len() >= 2
        )));
        assert!(transitions.iter().any(|transition| matches!(
            transition,
            crate::cleanup_plan::CleanupTransition::Transfer { source, destination, .. }
                if !source.projections.is_empty() || !destination.projections.is_empty()
        )));
        assert!(transitions.iter().any(|transition| matches!(
            transition,
            crate::cleanup_plan::CleanupTransition::StageCopyResult {
                source: crate::cleanup_plan::StagedCopyResultSource::TryResidual { .. },
            }
        )));
        assert!(transitions.iter().any(|transition| matches!(
            transition,
            crate::cleanup_plan::CleanupTransition::StageCopyResult {
                source: crate::cleanup_plan::StagedCopyResultSource::TryOptionNone { .. },
            }
        )));
        assert!(resolved
            .functions
            .iter()
            .any(|function| !function.cleanup_plan.status_sources.is_empty()));
        assert!(resolved.functions.iter().any(|function| {
            !function.cleanup_plan.regions.is_empty()
                && !function.cleanup_plan.edges.is_empty()
                && !function.cleanup_plan.exits.is_empty()
        }));
        let actual = [
            crate::cleanup::capacity_high_water(),
            super::lower_capacity_high_water(),
        ];
        assert_eq!(
            actual,
            [8_968, 137_286],
            "inventory/lowering owned-capacity high-water pins drifted"
        );
        assert!(actual[0] <= 6_492_084);
        assert!(actual[1] <= 6_821_908);
    }

    #[test]
    fn long_identity_cleanup_dag_owned_census_covers_many_deep_roots() {
        use std::fmt::Write as _;

        fn long_id(family: &str, index: usize) -> String {
            format!("{family}.{index:03}.{}", "x".repeat(160))
        }

        let mut source = String::from("module cleanup.long_ids;\n");
        writeln!(
            source,
            "@id(\"{}\") resource R0 {{ @id(\"{}\") drop trivial; }}",
            long_id("resource", 0),
            long_id("lifecycle", 0)
        )
        .unwrap();
        for index in 1..514 {
            writeln!(
                source,
                "@id(\"{}\") record R{index} {{ @id(\"{}\") value: R{}, }}",
                long_id("record", index),
                long_id("field", index),
                index - 1
            )
            .unwrap();
        }
        let parameters = (0..8)
            .map(|index| format!("p{index}: own R513"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            source,
            "@id(\"{}\") fn consume({parameters}) -> i64 {{ 0 }}",
            long_id("consume", 0)
        )
        .unwrap();
        source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");

        let parsed = parse(&source, Path::new("cleanup-long-ids.spx")).unwrap();
        let canonical = crate::format::canonical(&parsed);
        let canonical_digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(canonical.as_bytes()))
        );
        assert!(canonical.len() < 1_048_576);
        crate::cleanup::reset_capacity_high_water();
        super::reset_lower_capacity_high_water();
        let resolved = hir::resolve(&parsed).unwrap();
        let function = resolved
            .functions
            .iter()
            .find(|function| function.name == "consume")
            .unwrap();
        assert_eq!(function.params.len(), 8);
        assert_eq!(function.cleanup.slots.len(), 8);
        assert_eq!(function.cleanup_plan.slots.len(), 8);

        let mut maximum_shape_depth = 0usize;
        let mut pending = function
            .cleanup_plan
            .slots
            .iter()
            .map(|slot| (&slot.field_liveness_shape, 1usize))
            .collect::<Vec<_>>();
        while let Some((shape, depth)) = pending.pop() {
            maximum_shape_depth = maximum_shape_depth.max(depth);
            if let crate::cleanup::FieldLivenessShape::Record { fields, .. } = shape {
                pending.extend(fields.iter().map(|field| (&field.shape, depth + 1)));
            }
        }
        assert_eq!(maximum_shape_depth, 514);

        let inventory_owned =
            crate::private_capacity_contract::cleanup_inventory_owned_capacity(&function.cleanup)
                .unwrap();
        let plan_owned =
            crate::private_capacity_contract::cleanup_plan_owned_capacity(&function.cleanup_plan)
                .unwrap();
        let inventory_water = crate::cleanup::capacity_high_water();
        let lower_water = super::lower_capacity_high_water();
        assert!(inventory_owned > 8 * 514 * 160);
        assert!(plan_owned > 8 * 514 * 160);
        assert!(inventory_water >= inventory_owned);
        assert!(lower_water >= plan_owned);
        assert_eq!(canonical_digest.len(), 71);
    }
}
