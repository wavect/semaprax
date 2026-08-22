//! Target-neutral, replay-validated cleanup control flow.
//!
//! A [`CleanupPlan`] is executable ownership metadata: backends consume its
//! blocks, edges, regions, status lanes, and exits instead of rediscovering
//! cleanup from syntax.  Construction and validation are pure and
//! deterministic.  This module deliberately does not enable resource lowering.

mod build;
mod execute;
mod replay;
mod validate;

pub(crate) use build::build_plan;
pub use execute::{execute_for_conformance, CleanupExecutionError, CleanupScenario};
pub(crate) use validate::validate_program;

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::hir::{DeclarationId, ExpressionId, ResolvedType, ValueId};

pub const CLEANUP_PLAN_SCHEMA_V2: &str = "semaprax.cleanup-plan.v2";
pub const CLEANUP_PLAN_SCHEMA_V3: &str = "semaprax.cleanup-plan.v3";

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub u32);
    };
}

numeric_id!(BlockId);
numeric_id!(EdgeId);
numeric_id!(CleanupRegionId);
numeric_id!(ExitTargetId);
numeric_id!(CleanupSlotId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StatusLane {
    OperationFailure,
    ContractFalse,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatusSourceId {
    pub expression: ExpressionId,
    pub lane: StatusLane,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StorageId {
    Value(ValueId),
    Temporary(ExpressionId),
    /// Caller-owned argument storage.  In particular, evaluating an owned
    /// `Place` must transfer it here before later arguments can fail; the
    /// atomic call commit consumes these epochs together.
    CallArgument {
        call: ExpressionId,
        parameter_index: u32,
        value_expression: ExpressionId,
    },
    ProvisionalResult,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CleanupPlace {
    pub storage: StorageId,
    pub projections: Vec<DeclarationId>,
}

impl CleanupPlace {
    fn whole(storage: StorageId) -> Self {
        Self {
            storage,
            projections: Vec::new(),
        }
    }

    fn projected(&self, projection: DeclarationId) -> Self {
        let mut projections = self.projections.clone();
        projections.push(projection);
        Self {
            storage: self.storage.clone(),
            projections,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupSlot {
    pub id: CleanupSlotId,
    pub storage: StorageId,
    pub ty: ResolvedType,
    pub storage_index: u32,
    pub field_liveness_shape: FieldLivenessShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupEntryState {
    pub live_owned_parameters: Vec<CleanupPlace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArgumentTransfer {
    pub parameter_index: u32,
    pub source: CleanupPlace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupTransition {
    Initialize {
        at: ExpressionId,
        destination: CleanupPlace,
    },
    Transfer {
        at: ExpressionId,
        source: CleanupPlace,
        destination: CleanupPlace,
    },
    CallCommit {
        call: ExpressionId,
        arguments: Vec<CallArgumentTransfer>,
    },
    SelectFailure {
        source: StatusSourceId,
    },
    /// Stage one complete Copy aggregate in compiler-owned provisional result
    /// storage.  This is semantic proof data only: the public conformance trace
    /// has no aggregate value representation, and backends remain responsible
    /// for target-specific bytes after independently validating this plan.
    StageCopyResult {
        source: StagedCopyResultSource,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StagedCopyResultSource {
    /// The ordinary function-body value on the non-residual path.
    Body {
        expression: ExpressionId,
        instance: ResolvedType,
    },
    /// A compiler-synthesized outer `Result::Err` produced by postfix `?`.
    ///
    /// Source and target instances are both authenticated because sharing the
    /// same residual type does not imply sharing size, alignment, or layout.
    TryResidual {
        expression: ExpressionId,
        operand: ExpressionId,
        source_instance: ResolvedType,
        target_instance: ResolvedType,
        result: DeclarationId,
        ok_case: DeclarationId,
        ok_field: DeclarationId,
        err_case: DeclarationId,
        err_field: DeclarationId,
    },
    /// A compiler-synthesized outer `Option::None` produced by postfix `?`.
    ///
    /// `None` has no payload identity. Source and target instances remain
    /// explicit because their concrete layouts may differ.
    TryOptionNone {
        expression: ExpressionId,
        operand: ExpressionId,
        source_instance: ResolvedType,
        target_instance: ResolvedType,
        option: DeclarationId,
        some_case: DeclarationId,
        some_field: DeclarationId,
        none_case: DeclarationId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedOperation {
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StatusCase {
    AddOverflow,
    SubOverflow,
    MulOverflow,
    DivisionByZero,
    DivisionOverflow,
    RemainderByZero,
    RemainderOverflow,
    NegationOverflow,
}

impl StatusCase {
    /// Stable `semaprax.status.v1` codes for compiler-originated arithmetic
    /// failures.  Zero is reserved for success; imported domains normalize
    /// separately at their adapter boundary.
    pub const fn code(self) -> u32 {
        match self {
            Self::AddOverflow => 1,
            Self::SubOverflow => 2,
            Self::MulOverflow => 3,
            Self::DivisionByZero => 4,
            Self::DivisionOverflow => 5,
            Self::RemainderByZero => 6,
            Self::RemainderOverflow => 7,
            Self::NegationOverflow => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractPhase {
    Requires,
    Ensures,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatusProducer {
    PropagatedCall {
        callee: DeclarationId,
    },
    CheckedArithmetic {
        operation: CheckedOperation,
        normalized_cases: Vec<StatusCase>,
    },
    ContractFalse {
        phase: ContractPhase,
        ordinal: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusSource {
    pub id: StatusSourceId,
    pub producer: StatusProducer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupTerminator {
    Goto(EdgeId),
    Branch(Vec<EdgeId>),
    Exit(ExitTargetId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupBlock {
    pub id: BlockId,
    pub region: CleanupRegionId,
    pub transitions: Vec<CleanupTransition>,
    pub terminator: CleanupTerminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdgeCondition {
    Always,
    BooleanResult(ExpressionId, bool),
    VariantCase {
        scrutinee: ExpressionId,
        case: DeclarationId,
        matches: bool,
    },
    StatusZero(StatusSourceId),
    StatusNonzero(StatusSourceId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupEdge {
    pub id: EdgeId,
    pub from: BlockId,
    pub to: BlockId,
    pub condition: EdgeCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupRegion {
    pub id: CleanupRegionId,
    pub parent: Option<CleanupRegionId>,
    pub slots: Vec<StorageId>,
    pub normal_scope_end: ExitTargetId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizeAction {
    pub source: CleanupPlace,
    pub lifecycle_id: DeclarationId,
    pub guard_flag: LivenessFlagId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExitContinuation {
    Continue(EdgeId),
    CommitResult { source: CleanupResultSource },
    ReturnFailure { source: StatusSourceId },
    ReturnUnit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupResultSource {
    /// A scalar has no cleanup slot or liveness flag, but still commits exactly
    /// once from the evaluated body expression after contracts and cleanup.
    Scalar { expression: ExpressionId },
    /// An owned result remains guarded until the publication commit transfers
    /// it to the caller's previously uninitialized out-slot.
    Owned { storage: CleanupPlace },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitTarget {
    pub id: ExitTargetId,
    pub from: BlockId,
    pub leaves_regions: Vec<CleanupRegionId>,
    pub finalize_in_order: Vec<FinalizeAction>,
    pub continuation: ExitContinuation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupPlan {
    pub schema: &'static str,
    pub entry: BlockId,
    pub entry_state: CleanupEntryState,
    pub slots: Vec<CleanupSlot>,
    pub status_sources: Vec<StatusSource>,
    pub blocks: Vec<CleanupBlock>,
    pub edges: Vec<CleanupEdge>,
    pub regions: Vec<CleanupRegion>,
    pub exits: Vec<ExitTarget>,
}

impl CleanupPlan {
    /// Placeholder used only while resolving the mutually recursive
    /// `ResolvedFunction`/`CleanupPlan` boundary.  HIR validation must replace
    /// and reject it before any semantic consumer runs.
    pub(crate) fn unresolved() -> Self {
        Self {
            schema: CLEANUP_PLAN_SCHEMA_V2,
            entry: BlockId(0),
            entry_state: CleanupEntryState {
                live_owned_parameters: Vec::new(),
            },
            slots: Vec::new(),
            status_sources: Vec::new(),
            blocks: Vec::new(),
            edges: Vec::new(),
            regions: Vec::new(),
            exits: Vec::new(),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn owned_capacity_bytes(&self) -> Option<usize> {
        fn storage_bytes(storage: &StorageId) -> usize {
            match storage {
                StorageId::Value(value) => value.as_str().len(),
                StorageId::Temporary(expression) => expression.as_str().len(),
                StorageId::CallArgument {
                    call,
                    value_expression,
                    ..
                } => call
                    .as_str()
                    .len()
                    .saturating_add(value_expression.as_str().len()),
                StorageId::ProvisionalResult => 0,
            }
        }
        fn place_bytes(place: &CleanupPlace) -> Option<usize> {
            place
                .projections
                .iter()
                .try_fold(storage_bytes(&place.storage), |bytes, projection| {
                    bytes.checked_add(projection.as_str().len())
                })?
                .checked_add(place.projections.capacity() * std::mem::size_of::<DeclarationId>())
        }
        fn status_id_bytes(status: &StatusSourceId) -> usize {
            status.expression.as_str().len()
        }
        let mut total = self
            .entry_state
            .live_owned_parameters
            .capacity()
            .checked_mul(std::mem::size_of::<CleanupPlace>())?
            .checked_add(self.slots.capacity() * std::mem::size_of::<CleanupSlot>())?
            .checked_add(self.status_sources.capacity() * std::mem::size_of::<StatusSource>())?
            .checked_add(self.blocks.capacity() * std::mem::size_of::<CleanupBlock>())?
            .checked_add(self.edges.capacity() * std::mem::size_of::<CleanupEdge>())?
            .checked_add(self.regions.capacity() * std::mem::size_of::<CleanupRegion>())?
            .checked_add(self.exits.capacity() * std::mem::size_of::<ExitTarget>())?;
        for place in &self.entry_state.live_owned_parameters {
            total = total.checked_add(place_bytes(place)?)?;
        }
        for slot in &self.slots {
            total = total
                .checked_add(storage_bytes(&slot.storage))?
                .checked_add(resolved_type_owned_capacity(&slot.ty)?)?
                .checked_add(field_shape_bytes(&slot.field_liveness_shape)?)?;
        }
        for status in &self.status_sources {
            total = total.checked_add(status_id_bytes(&status.id))?;
            match &status.producer {
                StatusProducer::PropagatedCall { callee } => {
                    total = total.checked_add(callee.as_str().len())?;
                }
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } => {
                    total = total.checked_add(
                        normalized_cases.capacity() * std::mem::size_of::<StatusCase>(),
                    )?;
                }
                StatusProducer::ContractFalse { .. } => {}
            }
        }
        for block in &self.blocks {
            total = total.checked_add(
                block.transitions.capacity() * std::mem::size_of::<CleanupTransition>(),
            )?;
            for transition in &block.transitions {
                match transition {
                    CleanupTransition::Initialize { at, destination } => {
                        total = total
                            .checked_add(at.as_str().len())?
                            .checked_add(place_bytes(destination)?)?;
                    }
                    CleanupTransition::Transfer {
                        at,
                        source,
                        destination,
                    } => {
                        total = total
                            .checked_add(at.as_str().len())?
                            .checked_add(place_bytes(source)?)?
                            .checked_add(place_bytes(destination)?)?;
                    }
                    CleanupTransition::CallCommit { call, arguments } => {
                        total = total.checked_add(call.as_str().len())?.checked_add(
                            arguments.capacity() * std::mem::size_of::<CallArgumentTransfer>(),
                        )?;
                        for argument in arguments {
                            total = total.checked_add(place_bytes(&argument.source)?)?;
                        }
                    }
                    CleanupTransition::SelectFailure { source } => {
                        total = total.checked_add(status_id_bytes(source))?;
                    }
                    CleanupTransition::StageCopyResult { source } => {
                        total = total.checked_add(staged_result_bytes(source)?)?;
                    }
                }
            }
            if let CleanupTerminator::Branch(edges) = &block.terminator {
                total = total.checked_add(edges.capacity() * std::mem::size_of::<EdgeId>())?;
            }
        }
        for edge in &self.edges {
            total = total.checked_add(match &edge.condition {
                EdgeCondition::Always => 0,
                EdgeCondition::BooleanResult(expression, _) => expression.as_str().len(),
                EdgeCondition::VariantCase {
                    scrutinee, case, ..
                } => scrutinee.as_str().len().saturating_add(case.as_str().len()),
                EdgeCondition::StatusZero(status) | EdgeCondition::StatusNonzero(status) => {
                    status_id_bytes(status)
                }
            })?;
        }
        for region in &self.regions {
            total =
                total.checked_add(region.slots.capacity() * std::mem::size_of::<StorageId>())?;
            for storage in &region.slots {
                total = total.checked_add(storage_bytes(storage))?;
            }
        }
        for exit in &self.exits {
            total = total
                .checked_add(
                    exit.leaves_regions.capacity() * std::mem::size_of::<CleanupRegionId>(),
                )?
                .checked_add(
                    exit.finalize_in_order.capacity() * std::mem::size_of::<FinalizeAction>(),
                )?;
            for action in &exit.finalize_in_order {
                total = total
                    .checked_add(place_bytes(&action.source)?)?
                    .checked_add(action.lifecycle_id.as_str().len())?;
            }
            total = total.checked_add(match &exit.continuation {
                ExitContinuation::Continue(_) | ExitContinuation::ReturnUnit => 0,
                ExitContinuation::CommitResult { source } => match source {
                    CleanupResultSource::Scalar { expression } => expression.as_str().len(),
                    CleanupResultSource::Owned { storage } => place_bytes(storage)?,
                },
                ExitContinuation::ReturnFailure { source } => status_id_bytes(source),
            })?;
        }
        Some(total)
    }
}

#[allow(dead_code)]
fn resolved_type_owned_capacity(ty: &ResolvedType) -> Option<usize> {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::Char
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => Some(0),
        ResolvedType::TypeParameter { owner, .. } => Some(owner.as_str().len()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => arguments
            .iter()
            .try_fold(declaration.as_str().len(), |bytes, argument| {
                bytes.checked_add(resolved_type_owned_capacity(argument)?)
            })?
            .checked_add(arguments.capacity() * std::mem::size_of::<ResolvedType>()),
    }
}

#[allow(dead_code)]
fn field_shape_bytes(shape: &FieldLivenessShape) -> Option<usize> {
    match shape {
        FieldLivenessShape::NoDrop => Some(0),
        FieldLivenessShape::Leaf { lifecycle, .. } => Some(lifecycle.as_str().len()),
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => fields
            .iter()
            .try_fold(declaration.as_str().len(), |bytes, field| {
                bytes
                    .checked_add(field.field.as_str().len())?
                    .checked_add(field_shape_bytes(&field.shape)?)
            })?
            .checked_add(fields.capacity() * std::mem::size_of::<crate::cleanup::FieldLiveness>()),
    }
}

#[allow(dead_code)]
fn staged_result_bytes(source: &StagedCopyResultSource) -> Option<usize> {
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
