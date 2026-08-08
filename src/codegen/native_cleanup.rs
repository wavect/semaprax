//! Bounded indexing for the first native resource-cleanup slice.
//!
//! This module does not emit C and does not widen native feature support.  It
//! classifies one already-validated resolved function and retains references to
//! the attached cleanup plan in canonical vector order.  Unsupported shapes
//! fail with `SPX-B104`; callers must not repair or reconstruct cleanup from
//! HIR when classification fails.
//!
//! A `Continue` exit is accepted only for the compiler's canonical checked
//! success path: it leaves a contiguous chain of empty regions through one
//! uniquely owned unconditional edge. It cannot finalize, transfer, or cross
//! a resource-owning region.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the classifier lands before its gated emitter call site"
    )
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::ast::BinaryOp;
use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{
    BlockId, CleanupBlock, CleanupEdge, CleanupPlace, CleanupResultSource, CleanupSlot,
    CleanupTerminator, CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget,
    ExitTargetId, FinalizeAction, StatusSource, StorageId, CLEANUP_PLAN_SCHEMA_V1,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind,
};

/// One direct resource leaf, in canonical cleanup-slot order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCleanupLeaf<'a> {
    pub(crate) flag: LivenessFlagId,
    pub(crate) lifecycle_id: &'a DeclarationId,
    pub(crate) place: CleanupPlace,
}

/// One cleanup slot and its single direct resource leaf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCleanupSlot<'a> {
    pub(crate) slot: &'a CleanupSlot,
    pub(crate) leaf: NativeCleanupLeaf<'a>,
}

/// A block reference whose transition slice retains canonical execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCleanupBlock<'a> {
    pub(crate) block: &'a CleanupBlock,
    pub(crate) transitions: &'a [CleanupTransition],
}

/// An exit reference whose finalizer slice retains canonical execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCleanupExit<'a> {
    pub(crate) exit: &'a ExitTarget,
    pub(crate) finalizers: &'a [FinalizeAction],
}

/// Exact lowering index for the intentionally small first native cleanup slice.
///
/// Vectors are never sorted. Lookup maps contain only positions into those
/// vectors and therefore cannot change transition, finalizer, slot, or entry
/// order.
#[derive(Clone, Debug)]
pub(crate) struct NativeCleanupIndex<'a> {
    admission: NativeCleanupAdmission,
    canonical_function: &'a ResolvedFunction,
    function_id: &'a DeclarationId,
    entry: BlockId,
    slots: Vec<NativeCleanupSlot<'a>>,
    leaves: Vec<NativeCleanupLeaf<'a>>,
    live_owned_parameters: &'a [CleanupPlace],
    status_sources: &'a [StatusSource],
    regions: &'a [crate::cleanup_plan::CleanupRegion],
    blocks: Vec<NativeCleanupBlock<'a>>,
    edges: &'a [CleanupEdge],
    exits: Vec<NativeCleanupExit<'a>>,
    slot_positions: BTreeMap<StorageId, usize>,
    leaf_positions: BTreeMap<LivenessFlagId, usize>,
    block_positions: BTreeMap<BlockId, usize>,
    edge_positions: BTreeMap<EdgeId, usize>,
    exit_positions: BTreeMap<ExitTargetId, usize>,
}

/// Private shared proof that cleanup classification admitted one immutable
/// index. Clones preserve the proof without exposing forgeable numeric state.
#[derive(Clone, Debug)]
pub(super) struct NativeCleanupAdmission(Arc<()>);

impl PartialEq for NativeCleanupAdmission {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other)
    }
}

impl Eq for NativeCleanupAdmission {}

impl NativeCleanupAdmission {
    pub(super) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<'a> NativeCleanupIndex<'a> {
    /// Prove that this index retains references into this exact canonical HIR
    /// function, rather than a detached clone with reused stable identities.
    pub(crate) fn belongs_to(&self, function: &ResolvedFunction) -> bool {
        std::ptr::eq(self.canonical_function, function)
    }

    pub(super) fn admission(&self) -> NativeCleanupAdmission {
        self.admission.clone()
    }

    pub(crate) fn function_id(&self) -> &DeclarationId {
        self.function_id
    }

    pub(crate) fn entry(&self) -> BlockId {
        self.entry
    }

    pub(crate) fn slots(&self) -> &[NativeCleanupSlot<'a>] {
        &self.slots
    }

    pub(crate) fn leaves(&self) -> &[NativeCleanupLeaf<'a>] {
        &self.leaves
    }

    pub(crate) fn live_owned_parameters(&self) -> &[CleanupPlace] {
        self.live_owned_parameters
    }

    pub(crate) fn status_sources(&self) -> &[StatusSource] {
        self.status_sources
    }

    pub(crate) fn regions(&self) -> &[crate::cleanup_plan::CleanupRegion] {
        self.regions
    }

    pub(crate) fn blocks(&self) -> &[NativeCleanupBlock<'a>] {
        &self.blocks
    }

    pub(crate) fn edges(&self) -> &[CleanupEdge] {
        self.edges
    }

    pub(crate) fn exits(&self) -> &[NativeCleanupExit<'a>] {
        &self.exits
    }

    pub(crate) fn slot(&self, storage: &StorageId) -> Option<&NativeCleanupSlot<'a>> {
        self.slot_positions
            .get(storage)
            .map(|position| &self.slots[*position])
    }

    pub(crate) fn leaf(&self, flag: LivenessFlagId) -> Option<&NativeCleanupLeaf<'a>> {
        self.leaf_positions
            .get(&flag)
            .map(|position| &self.leaves[*position])
    }

    pub(crate) fn block(&self, id: BlockId) -> Option<&NativeCleanupBlock<'a>> {
        self.block_positions
            .get(&id)
            .map(|position| &self.blocks[*position])
    }

    pub(crate) fn edge(&self, id: EdgeId) -> Option<&'a CleanupEdge> {
        self.edge_positions
            .get(&id)
            .map(|position| &self.edges[*position])
    }

    pub(crate) fn exit(&self, id: ExitTargetId) -> Option<&NativeCleanupExit<'a>> {
        self.exit_positions
            .get(&id)
            .map(|position| &self.exits[*position])
    }
}

/// Classify and index one validated function without widening the native gate.
pub(crate) fn classify<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
) -> Result<NativeCleanupIndex<'a>, Diagnostic> {
    if function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V1 {
        return Err(unsupported(
            function,
            format!(
                "uses cleanup schema `{}` instead of `{CLEANUP_PLAN_SCHEMA_V1}`",
                function.cleanup_plan.schema
            ),
        ));
    }

    validate_program_types(program, function)?;
    validate_function_types(program, function)?;
    validate_expression(program, function, &function.body)?;
    for contract in function.requires.iter().chain(&function.ensures) {
        validate_expression(program, function, contract)?;
    }

    let plan = &function.cleanup_plan;
    let mut slots = Vec::with_capacity(plan.slots.len());
    let mut leaves = Vec::with_capacity(plan.slots.len());
    let mut slot_positions = BTreeMap::new();
    let mut leaf_positions = BTreeMap::new();

    for slot in &plan.slots {
        let expected_lifecycle =
            direct_resource_lifecycle(program, function, &slot.ty, "cleanup slot")?;
        let FieldLivenessShape::Leaf { flag, lifecycle } = &slot.field_liveness_shape else {
            let detail = match &slot.field_liveness_shape {
                FieldLivenessShape::NoDrop => "has a no-drop cleanup shape",
                FieldLivenessShape::Record { .. } => "has a projected record cleanup shape",
                FieldLivenessShape::Leaf { .. } => unreachable!(),
            };
            return Err(unsupported(
                function,
                format!("cleanup slot {} {detail}", slot.id.0),
            ));
        };
        if lifecycle != expected_lifecycle {
            return Err(unsupported(
                function,
                format!(
                    "cleanup slot {} lifecycle `{lifecycle}` disagrees with resource type `{}` lifecycle `{expected_lifecycle}`",
                    slot.id.0,
                    slot.ty.identity_key()
                ),
            ));
        }
        validate_trivial_lifecycle(program, function, lifecycle)?;
        let place = CleanupPlace {
            storage: slot.storage.clone(),
            projections: Vec::new(),
        };
        let leaf = NativeCleanupLeaf {
            flag: *flag,
            lifecycle_id: lifecycle,
            place,
        };
        if slot_positions
            .insert(slot.storage.clone(), slots.len())
            .is_some()
        {
            return Err(unsupported(function, "repeats cleanup storage"));
        }
        if leaf_positions.insert(*flag, leaves.len()).is_some() {
            return Err(unsupported(
                function,
                format!("repeats cleanup flag {}", flag.0),
            ));
        }
        leaves.push(leaf.clone());
        slots.push(NativeCleanupSlot { slot, leaf });
    }

    for place in &plan.entry_state.live_owned_parameters {
        validate_place(function, place, &slot_positions, "owned entry place")?;
    }

    let mut blocks = Vec::with_capacity(plan.blocks.len());
    let mut block_positions = BTreeMap::new();
    for block in &plan.blocks {
        if block_positions.insert(block.id, blocks.len()).is_some() {
            return Err(unsupported(
                function,
                format!("repeats cleanup block {}", block.id.0),
            ));
        }
        for transition in &block.transitions {
            validate_transition(function, transition, &slot_positions, &slots)?;
        }
        blocks.push(NativeCleanupBlock {
            block,
            transitions: &block.transitions,
        });
    }

    let mut edge_positions = BTreeMap::new();
    for (position, edge) in plan.edges.iter().enumerate() {
        if edge_positions.insert(edge.id, position).is_some() {
            return Err(unsupported(
                function,
                format!("repeats cleanup edge {}", edge.id.0),
            ));
        }
    }

    let mut exits = Vec::with_capacity(plan.exits.len());
    let mut exit_positions = BTreeMap::new();
    for exit in &plan.exits {
        if exit_positions.insert(exit.id, exits.len()).is_some() {
            return Err(unsupported(
                function,
                format!("repeats cleanup exit {}", exit.id.0),
            ));
        }
        for action in &exit.finalize_in_order {
            validate_place(
                function,
                &action.source,
                &slot_positions,
                "finalizer source",
            )?;
            let Some(position) = leaf_positions.get(&action.guard_flag) else {
                return Err(unsupported(
                    function,
                    format!("finalizer uses unknown flag {}", action.guard_flag.0),
                ));
            };
            let leaf = &leaves[*position];
            if leaf.place != action.source || leaf.lifecycle_id != &action.lifecycle_id {
                return Err(unsupported(
                    function,
                    format!(
                        "finalizer for flag {} disagrees with its direct resource leaf",
                        action.guard_flag.0
                    ),
                ));
            }
        }
        if let ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage },
        } = &exit.continuation
        {
            validate_place(function, storage, &slot_positions, "owned result source")?;
            if storage.storage != StorageId::ProvisionalResult {
                return Err(unsupported(
                    function,
                    "publishes an owned result from non-provisional storage",
                ));
            }
        }
        exits.push(NativeCleanupExit {
            exit,
            finalizers: &exit.finalize_in_order,
        });
    }

    validate_control_references(
        function,
        &blocks,
        &plan.edges,
        &edge_positions,
        &exits,
        &exit_positions,
    )?;
    validate_bounded_continuations(function, plan)?;

    if !block_positions.contains_key(&plan.entry) {
        return Err(unsupported(
            function,
            format!("uses unknown entry block {}", plan.entry.0),
        ));
    }

    Ok(NativeCleanupIndex {
        admission: NativeCleanupAdmission(Arc::new(())),
        canonical_function: function,
        function_id: &function.id,
        entry: plan.entry,
        slots,
        leaves,
        live_owned_parameters: &plan.entry_state.live_owned_parameters,
        status_sources: &plan.status_sources,
        regions: &plan.regions,
        blocks,
        edges: &plan.edges,
        exits,
        slot_positions,
        leaf_positions,
        block_positions,
        edge_positions,
        exit_positions,
    })
}

fn validate_bounded_continuations(
    function: &ResolvedFunction,
    plan: &crate::cleanup_plan::CleanupPlan,
) -> Result<(), Diagnostic> {
    for exit in &plan.exits {
        let ExitContinuation::Continue(edge_id) = exit.continuation else {
            continue;
        };
        let reject = |detail: &str| {
            unsupported(
                function,
                format!(
                    "cleanup continuation exit {} {detail}; only the canonical empty-region success continuation is supported",
                    exit.id.0
                ),
            )
        };
        if !exit.finalize_in_order.is_empty() {
            return Err(reject("performs finalization"));
        }
        if exit.leaves_regions.is_empty() {
            return Err(reject("does not leave a region"));
        }
        let source = plan
            .blocks
            .iter()
            .find(|block| block.id == exit.from)
            .ok_or_else(|| reject("has an unknown source block"))?;
        if !source.transitions.is_empty() || source.terminator != CleanupTerminator::Exit(exit.id) {
            return Err(reject("changes state before continuing"));
        }
        let edge = plan
            .edges
            .iter()
            .find(|edge| edge.id == edge_id)
            .ok_or_else(|| reject("references an unknown edge"))?;
        if edge.from != exit.from || !matches!(edge.condition, EdgeCondition::Always) {
            return Err(reject("does not own one unconditional edge"));
        }

        let incoming = plan
            .edges
            .iter()
            .filter(|candidate| candidate.to == source.id)
            .collect::<Vec<_>>();
        if incoming.len() != 1
            || !matches!(
                incoming[0].condition,
                EdgeCondition::BooleanResult(_, true) | EdgeCondition::StatusZero(_)
            )
        {
            return Err(reject("is not reached by one successful checked branch"));
        }

        let mut expected_region = Some(source.region);
        for region_id in &exit.leaves_regions {
            if expected_region != Some(*region_id) {
                return Err(reject("does not leave one contiguous region chain"));
            }
            let region = plan
                .regions
                .iter()
                .find(|region| region.id == *region_id)
                .ok_or_else(|| reject("references an unknown region"))?;
            if !region.slots.is_empty() || region.normal_scope_end != exit.id {
                return Err(reject("leaves a resource-owning or non-normal region"));
            }
            expected_region = region.parent;
        }
        let Some(parent_region) = expected_region else {
            return Err(reject("escapes the root region"));
        };
        let target = plan
            .blocks
            .iter()
            .find(|block| block.id == edge.to)
            .ok_or_else(|| reject("targets an unknown block"))?;
        if target.region != parent_region {
            return Err(reject("does not enter the immediate surviving region"));
        }
        if plan.edges.iter().filter(|candidate| candidate.to == target.id).count() != 1
            || plan
                .exits
                .iter()
                .filter(|candidate| {
                    matches!(candidate.continuation, ExitContinuation::Continue(id) if id == edge_id)
                })
                .count()
                != 1
        {
            return Err(reject("does not have a unique continuation target"));
        }
    }
    Ok(())
}

fn validate_program_types(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    for declaration in &program.types {
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { .. } => {
                return Err(unsupported(
                    function,
                    format!("does not support record declaration `{}`", declaration.id),
                ));
            }
            ResolvedTypeDeclarationKind::Resource { drop } => {
                if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                    return Err(unsupported(
                        function,
                        format!(
                            "does not support imported lifecycle `{}` bound to `{import}`",
                            drop.id
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_function_types(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    for parameter in &function.params {
        validate_supported_type(program, function, &parameter.ty, "parameter")?;
    }
    validate_supported_type(program, function, &function.return_type, "result")
}

fn validate_supported_type(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    context: &str,
) -> Result<(), Diagnostic> {
    match ty {
        ResolvedType::I64 | ResolvedType::Bool => Ok(()),
        ResolvedType::TypeParameter { .. } => Err(unsupported(
            function,
            format!(
                "does not support a generic {context} type `{}`",
                ty.identity_key()
            ),
        )),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            if !arguments.is_empty() {
                return Err(unsupported(
                    function,
                    format!(
                        "does not support generic nominal type `{}`",
                        ty.identity_key()
                    ),
                ));
            }
            let item = program
                .types
                .iter()
                .find(|item| item.id == *declaration)
                .ok_or_else(|| {
                    unsupported(function, format!("references unknown type `{declaration}`"))
                })?;
            match &item.kind {
                ResolvedTypeDeclarationKind::Resource { drop } => {
                    validate_trivial_drop(function, &drop.id, &drop.kind)
                }
                ResolvedTypeDeclarationKind::Record { .. } => Err(unsupported(
                    function,
                    format!("uses record type `{declaration}`"),
                )),
            }
        }
    }
}

fn direct_resource_lifecycle<'a>(
    program: &'a ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    context: &str,
) -> Result<&'a DeclarationId, Diagnostic> {
    validate_supported_type(program, function, ty, context)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(unsupported(
            function,
            format!("{context} is not a direct resource type"),
        ));
    };
    if !arguments.is_empty() {
        return Err(unsupported(
            function,
            format!("{context} uses generic resource arguments"),
        ));
    }
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| unsupported(function, format!("references unknown type `{declaration}`")))?;
    match &item.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => Ok(&drop.id),
        ResolvedTypeDeclarationKind::Record { .. } => Err(unsupported(
            function,
            format!("{context} `{declaration}` is not an opaque resource"),
        )),
    }
}

fn validate_trivial_lifecycle(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    lifecycle: &DeclarationId,
) -> Result<(), Diagnostic> {
    let mut resolved = None;
    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        if drop.id == *lifecycle && resolved.replace(&drop.kind).is_some() {
            return Err(unsupported(
                function,
                format!("lifecycle `{lifecycle}` resolves more than once"),
            ));
        }
    }
    let kind = resolved.ok_or_else(|| {
        unsupported(
            function,
            format!("references unknown lifecycle `{lifecycle}`"),
        )
    })?;
    validate_trivial_drop(function, lifecycle, kind)
}

fn validate_trivial_drop(
    function: &ResolvedFunction,
    lifecycle: &DeclarationId,
    kind: &ResolvedResourceDropKind,
) -> Result<(), Diagnostic> {
    match kind {
        ResolvedResourceDropKind::Trivial => Ok(()),
        ResolvedResourceDropKind::Imported { import, .. } => Err(unsupported(
            function,
            format!("does not support imported lifecycle `{lifecycle}` bound to `{import}`"),
        )),
    }
}

fn validate_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    validate_supported_type(program, function, &expression.ty, "expression")?;
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
        ResolvedExprKind::Place(place) => {
            if !place.projections.is_empty() {
                return Err(unsupported(
                    function,
                    format!("uses projected place expression `{}`", expression.id),
                ));
            }
        }
        ResolvedExprKind::Call { callee, .. } => {
            return Err(unsupported(
                function,
                format!(
                    "does not support call execution `{}` to `{callee}` while native cleanup conformance is single-frame",
                    expression.id
                ),
            ));
        }
        ResolvedExprKind::Unary { value, .. } => {
            validate_expression(program, function, value)?;
        }
        ResolvedExprKind::Binary { op, left, right } => {
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                return Err(unsupported(
                    function,
                    format!(
                        "does not support lazy boolean expression `{}` in the first native cleanup slice",
                        expression.id
                    ),
                ));
            }
            if expression_contains_resource(program, function, left)?
                || expression_contains_resource(program, function, right)?
            {
                return Err(unsupported(
                    function,
                    format!(
                        "does not support resource-valued binary operands in expression `{}`",
                        expression.id
                    ),
                ));
            }
            validate_expression(program, function, left)?;
            validate_expression(program, function, right)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                validate_supported_type(program, function, &binding.ty, "binding")?;
                validate_expression(program, function, value)?;
            }
            validate_expression(program, function, tail)?;
        }
        ResolvedExprKind::If { .. } => {
            return Err(unsupported(
                function,
                format!(
                    "does not support conditional expression `{}` in the first native cleanup slice",
                    expression.id
                ),
            ));
        }
        ResolvedExprKind::ConstructRecord { .. } | ResolvedExprKind::Project { .. } => {
            return Err(unsupported(
                function,
                format!(
                    "uses projected or constructed expression `{}`",
                    expression.id
                ),
            ));
        }
    }
    Ok(())
}

fn expression_contains_resource(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<bool, Diagnostic> {
    program
        .declarations
        .type_facts(&expression.ty)
        .map(|facts| facts.contains_resource)
        .ok_or_else(|| {
            unsupported(
                function,
                format!(
                    "cannot resolve type facts for binary operand `{}`",
                    expression.id
                ),
            )
        })
}

fn validate_transition(
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    slots: &BTreeMap<StorageId, usize>,
    indexed_slots: &[NativeCleanupSlot<'_>],
) -> Result<(), Diagnostic> {
    match transition {
        CleanupTransition::Initialize { at, .. } => Err(unsupported(
            function,
            format!(
                "does not support initialize transition `{at}` without a physical payload source"
            ),
        )),
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            validate_place(function, source, slots, "transfer source")?;
            validate_place(function, destination, slots, "transfer destination")?;
            let source_slot = &indexed_slots[*slots
                .get(&source.storage)
                .expect("validated transfer source is indexed")];
            let destination_slot = &indexed_slots[*slots
                .get(&destination.storage)
                .expect("validated transfer destination is indexed")];
            if source_slot.slot.ty != destination_slot.slot.ty {
                return Err(unsupported(
                    function,
                    format!(
                        "transfer `{at}` changes resource type from `{}` to `{}`",
                        source_slot.slot.ty.identity_key(),
                        destination_slot.slot.ty.identity_key()
                    ),
                ));
            }
            Ok(())
        }
        CleanupTransition::CallCommit { call, .. } => Err(unsupported(
            function,
            format!(
                "does not support call-commit transition `{call}` while native cleanup conformance is single-frame"
            ),
        )),
        CleanupTransition::SelectFailure { .. } => Ok(()),
    }
}

fn validate_place(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    slots: &BTreeMap<StorageId, usize>,
    context: &str,
) -> Result<(), Diagnostic> {
    if !place.projections.is_empty() {
        return Err(unsupported(
            function,
            format!("{context} uses field projections"),
        ));
    }
    if !slots.contains_key(&place.storage) {
        return Err(unsupported(
            function,
            format!("{context} references unindexed storage"),
        ));
    }
    Ok(())
}

fn validate_control_references(
    function: &ResolvedFunction,
    blocks: &[NativeCleanupBlock<'_>],
    edges: &[CleanupEdge],
    edge_positions: &BTreeMap<EdgeId, usize>,
    exits: &[NativeCleanupExit<'_>],
    exit_positions: &BTreeMap<ExitTargetId, usize>,
) -> Result<(), Diagnostic> {
    for block in blocks {
        match &block.block.terminator {
            CleanupTerminator::Goto(edge) => {
                validate_owned_edge(function, block.block.id, *edge, edges, edge_positions)?;
            }
            CleanupTerminator::Branch(branches) => {
                for edge in branches {
                    validate_owned_edge(function, block.block.id, *edge, edges, edge_positions)?;
                }
            }
            CleanupTerminator::Exit(exit) => {
                let Some(position) = exit_positions.get(exit) else {
                    return Err(unsupported(
                        function,
                        format!(
                            "block {} references unknown exit {}",
                            block.block.id.0, exit.0
                        ),
                    ));
                };
                if exits[*position].exit.from != block.block.id {
                    return Err(unsupported(
                        function,
                        format!("exit {} has the wrong owning block", exit.0),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_owned_edge(
    function: &ResolvedFunction,
    owner: BlockId,
    edge: EdgeId,
    edges: &[CleanupEdge],
    edge_positions: &BTreeMap<EdgeId, usize>,
) -> Result<(), Diagnostic> {
    let Some(position) = edge_positions.get(&edge) else {
        return Err(unsupported(
            function,
            format!("block {} references unknown edge {}", owner.0, edge.0),
        ));
    };
    if edges[*position].from != owner {
        return Err(unsupported(
            function,
            format!("edge {} has the wrong owning block", edge.0),
        ));
    }
    Ok(())
}

fn unsupported(function: &ResolvedFunction, detail: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!(
            "native cleanup first slice for function `{}` {}",
            function.id,
            detail.into()
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::cleanup_plan::ExitContinuation;
    use crate::hir::{self, DeclarationId, OwnershipMode, ResolvedType};
    use crate::parse;

    use super::*;

    const SUPPORTED: &str = r#"module test.native_cleanup_index;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.contract-failure")
fn contract_failure(value: own Token) -> i64 requires false { 0 }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64 requires number >= 0 { number + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn resolve(source: &str) -> ResolvedProgram {
        let parsed = parse(source, Path::new("native-cleanup-index.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap()
    }

    #[test]
    fn supported_direct_resource_indexes_preserve_exact_order() {
        let first = resolve(SUPPORTED);
        let second = resolve(SUPPORTED);

        for id in [
            "token.discard",
            "token.discard-two",
            "token.contract-failure",
            "token.checked",
        ] {
            let first_index = classify(&first, function(&first, id)).unwrap();
            let second_index = classify(&second, function(&second, id)).unwrap();
            assert_eq!(first_index.function_id.as_str(), id);
            assert_eq!(second_index.function_id.as_str(), id);
            assert_eq!(first_index.entry, second_index.entry);
            assert_eq!(first_index.slots, second_index.slots);
            assert_eq!(first_index.leaves, second_index.leaves);
            assert_eq!(first_index.blocks, second_index.blocks);
            assert_eq!(first_index.edges, second_index.edges);
            assert_eq!(first_index.exits, second_index.exits);
        }

        let discard = classify(&first, function(&first, "token.discard")).unwrap();
        assert_eq!(discard.slots.len(), 1);
        assert_eq!(discard.live_owned_parameters.len(), 1);
        assert_eq!(discard.leaves[0].flag, LivenessFlagId(0));
        assert_eq!(discard.leaves[0].lifecycle_id.as_str(), "token.drop");
        assert_eq!(
            discard.slot(&discard.leaves[0].place.storage),
            Some(&discard.slots[0])
        );
        assert_eq!(discard.leaf(LivenessFlagId(0)), Some(&discard.leaves[0]));
        assert!(discard.block(discard.entry).is_some());
        assert!(discard
            .edges
            .iter()
            .all(|edge| discard.edge(edge.id).is_some()));
        assert!(discard
            .exits
            .iter()
            .all(|exit| discard.exit(exit.exit.id).is_some()));

        let two = classify(&first, function(&first, "token.discard-two")).unwrap();
        let success = two
            .exits
            .iter()
            .find(|exit| {
                matches!(
                    exit.exit.continuation,
                    ExitContinuation::CommitResult { .. }
                )
            })
            .unwrap();
        assert_eq!(
            success
                .finalizers
                .iter()
                .map(|action| action.guard_flag)
                .collect::<Vec<_>>(),
            [LivenessFlagId(1), LivenessFlagId(0)]
        );

        let failure = classify(&first, function(&first, "token.contract-failure")).unwrap();
        assert!(failure.status_sources.iter().any(|source| {
            matches!(
                source.producer,
                crate::cleanup_plan::StatusProducer::ContractFalse { .. }
            )
        }));
        assert!(failure
            .exits
            .iter()
            .filter(|exit| {
                matches!(
                    exit.exit.continuation,
                    ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnFailure { .. }
                )
            })
            .all(|exit| {
                exit.finalizers
                    .iter()
                    .map(|action| action.guard_flag)
                    .eq([LivenessFlagId(0)])
            }));

        let checked = classify(&first, function(&first, "token.checked")).unwrap();
        assert!(checked
            .blocks
            .iter()
            .any(|block| matches!(block.block.terminator, CleanupTerminator::Branch(_))));
        assert!(checked.status_sources.iter().any(|source| matches!(
            source.producer,
            crate::cleanup_plan::StatusProducer::CheckedArithmetic { .. }
        )));
        assert!(checked.status_sources.iter().any(|source| matches!(
            source.producer,
            crate::cleanup_plan::StatusProducer::ContractFalse { .. }
        )));
    }

    #[test]
    fn conditional_and_lazy_control_flow_are_rejected_without_reconstruction() {
        let conditional = resolve(
            r#"module test.native_cleanup_if;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("token.choose") fn choose(value: own Token, condition: bool) -> i64 {
    if condition { 1 } else { 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let diagnostic =
            classify(&conditional, function(&conditional, "token.choose")).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("conditional expression"));

        let lazy = resolve(
            r#"module test.native_cleanup_lazy;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("token.lazy") fn lazy(value: own Token, condition: bool) -> bool {
    condition && true
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let diagnostic = classify(&lazy, function(&lazy, "token.lazy")).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("lazy boolean expression"));
    }

    #[test]
    fn resource_valued_binary_operands_are_rejected_even_in_hostile_hir() {
        let program = resolve(SUPPORTED);
        let mut hostile = function(&program, "token.discard").clone();
        let parameter = &hostile.params[0];
        let operand = ResolvedExpr {
            id: hostile.body.id.clone(),
            ty: parameter.ty.clone(),
            ownership: OwnershipMode::Borrow,
            kind: ResolvedExprKind::Place(crate::hir::Place {
                root: parameter.id.clone(),
                projections: Vec::new(),
            }),
            span: hostile.body.span,
        };
        hostile.body.ty = ResolvedType::Bool;
        hostile.body.ownership = OwnershipMode::Value;
        hostile.body.kind = ResolvedExprKind::Binary {
            op: BinaryOp::Eq,
            left: Box::new(operand.clone()),
            right: Box::new(operand),
        };

        let diagnostic = classify(&program, &hostile).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains("resource-valued binary operands"));
    }

    #[test]
    fn initialize_and_cleanup_bearing_continue_are_rejected_by_the_classifier() {
        let program = resolve(SUPPORTED);
        let mut initialize = function(&program, "token.discard").clone();
        initialize.cleanup_plan.blocks[0]
            .transitions
            .push(CleanupTransition::Initialize {
                at: initialize.body.id.clone(),
                destination: initialize.cleanup_plan.entry_state.live_owned_parameters[0].clone(),
            });
        let diagnostic = classify(&program, &initialize).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("initialize transition"));
        assert!(diagnostic.message.contains("physical payload source"));

        let mut continuation = function(&program, "token.contract-failure").clone();
        let continue_position = continuation
            .cleanup_plan
            .exits
            .iter()
            .position(|exit| matches!(exit.continuation, ExitContinuation::Continue(_)))
            .expect("compiler contract continuation");
        let finalizer = continuation
            .cleanup_plan
            .exits
            .iter()
            .flat_map(|exit| &exit.finalize_in_order)
            .next()
            .expect("terminal cleanup")
            .clone();
        continuation.cleanup_plan.exits[continue_position]
            .finalize_in_order
            .push(finalizer);
        let diagnostic = classify(&program, &continuation).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("performs finalization"));
        assert!(diagnostic.message.contains("canonical empty-region"));

        let mut conditional = function(&program, "token.contract-failure").clone();
        let continuation_edge = conditional
            .cleanup_plan
            .exits
            .iter()
            .find_map(|exit| match exit.continuation {
                ExitContinuation::Continue(edge) => Some(edge),
                _ => None,
            })
            .expect("compiler contract continuation");
        let hostile_condition = conditional
            .cleanup_plan
            .edges
            .iter()
            .find(|edge| !matches!(edge.condition, EdgeCondition::Always))
            .expect("contract branch")
            .condition
            .clone();
        conditional
            .cleanup_plan
            .edges
            .iter_mut()
            .find(|edge| edge.id == continuation_edge)
            .expect("continuation edge")
            .condition = hostile_condition;
        let diagnostic = classify(&program, &conditional).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains("does not own one unconditional edge"));
    }

    #[test]
    fn records_are_rejected_precisely() {
        let program = resolve(
            r#"module test.native_cleanup_record;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("box.type") record Box { @id("box.value") value: Token, }
@id("box.discard") fn discard(value: own Box) -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let diagnostic = classify(&program, function(&program, "box.discard")).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert_eq!(
            diagnostic.message,
            "native cleanup first slice for function `box.discard` does not support record declaration `box.type`"
        );
    }

    #[test]
    fn imported_lifecycles_are_rejected_precisely() {
        let program = resolve(
            r#"module test.native_cleanup_import;
permit { io.release }
@id("file.type") resource File { @id("file.drop") drop import "file.finalize"; }
@id("file.host") interface FileHost permits { io.release } {
    @id("file.finalize") import fn finalize(file: own File) -> unit
        effects { io.release } failure infallible consumes file always;
}
@id("file.discard") fn discard(value: own File) -> i64 uses { io.release } { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let diagnostic = classify(&program, function(&program, "file.discard")).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains("imported lifecycle `file.drop`"));
        assert!(diagnostic.message.contains("`file.finalize`"));
    }

    #[test]
    fn resource_bearing_calls_are_rejected_precisely() {
        let program = resolve(
            r#"module test.native_cleanup_call;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("token.consume") fn consume(value: own Token) -> i64 { 0 }
@id("token.forward") fn forward(value: own Token) -> i64 { consume(value) }
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let diagnostic = classify(&program, function(&program, "token.forward")).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("call execution"));
        assert!(diagnostic.message.contains("`token.consume`"));
        assert!(diagnostic.message.contains("single-frame"));
    }

    #[test]
    fn scalar_calls_from_resource_owning_functions_are_rejected_precisely() {
        let program = resolve(
            r#"module test.native_cleanup_scalar_call;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("scalar.helper") fn helper() -> i64 { 7 }
@id("token.holding") fn holding(value: own Token) -> i64 { helper() }
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let diagnostic = classify(&program, function(&program, "token.holding")).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("call execution"));
        assert!(diagnostic.message.contains("`scalar.helper`"));
        assert!(diagnostic.message.contains("single-frame"));
    }

    #[test]
    fn empty_call_commit_transitions_are_rejected_without_repair() {
        let program = resolve(SUPPORTED);
        let mut hostile = function(&program, "token.discard").clone();
        let hostile_call = hostile.body.id.clone();
        hostile.cleanup_plan.blocks[0]
            .transitions
            .push(CleanupTransition::CallCommit {
                call: hostile_call.clone(),
                arguments: Vec::new(),
            });

        let diagnostic = classify(&program, &hostile).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains(&format!("call-commit transition `{hostile_call}`")));
        assert!(diagnostic.message.contains("single-frame"));
    }

    #[test]
    fn projected_cleanup_places_are_rejected_without_repair() {
        let program = resolve(SUPPORTED);
        let mut hostile = function(&program, "token.discard").clone();
        hostile.cleanup_plan.entry_state.live_owned_parameters[0]
            .projections
            .push(DeclarationId::new("hostile.field"));

        let diagnostic = classify(&program, &hostile).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains("owned entry place uses field projections"));
    }

    #[test]
    fn generic_cleanup_slots_are_rejected_without_repair() {
        let program = resolve(SUPPORTED);
        let mut hostile = function(&program, "token.discard").clone();
        hostile.cleanup_plan.slots[0].ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("token.type"),
            arguments: vec![ResolvedType::I64],
        };

        let diagnostic = classify(&program, &hostile).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("generic nominal type"));
    }

    #[test]
    fn forged_slot_lifecycle_and_transfer_type_mismatches_are_rejected() {
        let program = resolve(
            r#"module test.native_cleanup_type_identity;
@id("alpha.type") resource Alpha { @id("alpha.drop") drop trivial; }
@id("beta.type") resource Beta { @id("beta.drop") drop trivial; }
@id("alpha.identity") fn identity(value: own Alpha) -> Alpha { value }
@id("app.main") fn main() -> i64 { 0 }
"#,
        );
        let original = function(&program, "alpha.identity");

        let mut lifecycle_mismatch = original.clone();
        let FieldLivenessShape::Leaf { lifecycle, .. } =
            &mut lifecycle_mismatch.cleanup_plan.slots[0].field_liveness_shape
        else {
            panic!("direct resource slot must have one leaf");
        };
        *lifecycle = DeclarationId::new("beta.drop");
        let diagnostic = classify(&program, &lifecycle_mismatch).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("lifecycle `beta.drop`"));
        assert!(diagnostic.message.contains("lifecycle `alpha.drop`"));

        let mut transfer_mismatch = original.clone();
        let temporary = transfer_mismatch
            .cleanup_plan
            .slots
            .iter_mut()
            .find(|slot| matches!(slot.storage, StorageId::Temporary(_)))
            .expect("owned identity has body temporary storage");
        temporary.ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("beta.type"),
            arguments: Vec::new(),
        };
        let FieldLivenessShape::Leaf { lifecycle, .. } = &mut temporary.field_liveness_shape else {
            panic!("direct resource temporary must have one leaf");
        };
        *lifecycle = DeclarationId::new("beta.drop");
        let diagnostic = classify(&program, &transfer_mismatch).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("changes resource type"));
        assert!(diagnostic.message.contains("alpha.type"));
        assert!(diagnostic.message.contains("beta.type"));
    }
}
