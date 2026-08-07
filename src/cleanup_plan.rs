//! Target-neutral, replay-validated cleanup control flow.
//!
//! A [`CleanupPlan`] is executable ownership metadata: backends consume its
//! blocks, edges, regions, status lanes, and exits instead of rediscovering
//! cleanup from syntax.  Construction and validation are pure and
//! deterministic.  This module deliberately does not enable resource lowering.

mod build;
mod validate;

pub(crate) use build::build_plan;
pub(crate) use validate::validate_program;

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::hir::{DeclarationId, ExpressionId, ResolvedType, ValueId};

pub const CLEANUP_PLAN_SCHEMA_V1: &str = "semaprax.cleanup-plan.v1";

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
            schema: CLEANUP_PLAN_SCHEMA_V1,
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
}
