//! Canonical cleanup-plan construction from validated core HIR.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup::{
    CleanupStorageId as InventoryStorageId, FieldLiveness, FieldLivenessShape, LivenessFlagId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, OwnershipMode, Place, PlaceProjection, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind,
};

use super::{
    BlockId, CallArgumentTransfer, CheckedOperation, CleanupBlock, CleanupEdge, CleanupEntryState,
    CleanupPlace, CleanupPlan, CleanupRegion, CleanupRegionId, CleanupResultSource, CleanupSlot,
    CleanupSlotId, CleanupTerminator, CleanupTransition, ContractPhase, EdgeCondition, EdgeId,
    ExitContinuation, ExitTarget, ExitTargetId, FinalizeAction, StatusCase, StatusLane,
    StatusProducer, StatusSource, StatusSourceId, StorageId, CLEANUP_PLAN_SCHEMA_V1,
};

const UNRESOLVED_EXIT: ExitTargetId = ExitTargetId(u32::MAX);

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

struct EvalResult {
    block: BlockId,
    state: FlowState,
    owned_source: Option<CleanupPlace>,
}

struct OpenBlock {
    id: BlockId,
    region: CleanupRegionId,
    transitions: Vec<CleanupTransition>,
    terminator: Option<CleanupTerminator>,
}

#[derive(Clone)]
struct LeafMetadata {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

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
            schema: CLEANUP_PLAN_SCHEMA_V1,
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
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| facts.needs_drop)
            .ok_or_else(|| plan_error(format!("type `{}` has no cleanup facts", ty.identity_key())))
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
            ResolvedTypeDeclarationKind::Record { fields } => {
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
            let ResolvedStatement::Let { binding, value, .. } = statement;
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
        match &expression.kind {
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => Ok(EvalResult {
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
            ResolvedExprKind::Call { callee, args } => {
                self.lower_call(expression, callee, args, block, state, region)
            }
            ResolvedExprKind::Unary { op, value } => {
                let evaluated = self.lower_expr(value, block, state, region)?;
                let (block, state) = match op {
                    UnaryOp::Not => (evaluated.block, evaluated.state),
                    UnaryOp::Neg => {
                        let source = self.checked_source(
                            expression,
                            CheckedOperation::Neg,
                            vec![StatusCase::NegationOverflow],
                        )?;
                        self.split_status(evaluated.block, evaluated.state, region, source)?
                    }
                };
                Ok(EvalResult {
                    block,
                    state,
                    owned_source: None,
                })
            }
            ResolvedExprKind::Binary { op, left, right }
                if matches!(op, BinaryOp::And | BinaryOp::Or) =>
            {
                self.lower_lazy(expression, *op, left, right, block, state, region)
            }
            ResolvedExprKind::Binary { op, left, right } => {
                let left = self.lower_expr(left, block, state, region)?;
                let right = self.lower_expr(right, left.block, left.state, region)?;
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
            ResolvedExprKind::Project { base, field } => {
                let base = self.lower_expr(base, block, state, region)?;
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
        }
    }

    fn lower_call(
        &mut self,
        expression: &ResolvedExpr,
        callee: &DeclarationId,
        args: &[ResolvedExpr],
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let target = self
            .program
            .functions
            .iter()
            .find(|function| function.id == *callee)
            .ok_or_else(|| plan_error(format!("unknown cleanup call target `{callee}`")))?;
        if target.params.len() != args.len() {
            return Err(plan_error(format!(
                "cleanup call `{}` has inconsistent arity",
                expression.id
            )));
        }
        let params = target.params.clone();
        let mut current = block;
        let mut current_state = state;
        let mut commits = Vec::new();

        for (index, (argument, parameter)) in args.iter().zip(&params).enumerate() {
            let evaluated = self.lower_expr(argument, current, current_state, region)?;
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
        let left = self.lower_expr(left, block, state, region)?;
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

        let evaluated_right = self.lower_expr(right, evaluate, left.state.clone(), region)?;
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
            let ResolvedStatement::Let { binding, value, .. } = statement;
            let evaluated = self.lower_expr(value, current, current_state, region)?;
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
        let evaluated_tail = self.lower_expr(tail, current, current_state, region)?;
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
        let condition = self.lower_expr(condition, block, state, region)?;
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

        let mut then_result =
            self.lower_expr(then_branch, then_entry, condition.state.clone(), region)?;
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

        let mut else_result = self.lower_expr(else_branch, else_entry, condition.state, region)?;
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
            let evaluated = self.lower_expr(&initializer.value, current, current_state, region)?;
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
