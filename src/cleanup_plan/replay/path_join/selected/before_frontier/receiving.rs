//! Literal lexical caller argument/result role banks, no authority or storage.
use super::*;
pub(super) type SuccessorsCall<'a> = (
    &'a ResolvedFunction,
    BlockId,
    &'a mut ReplayBudget,
    Result<Successors, Error>,
);
pub(super) type ValidateBindingSelectedCall<'a> = (
    &'a ResolvedFunction,
    &'a ExpressionId,
    &'a CleanupPlace,
    &'a crate::cleanup_plan::ReplayRenewalBindings<'a>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type RejectUnmarkedFinishCall<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a ExpressionId,
    &'a CleanupPlace,
    Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type ReserveCall<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a ExpressionId,
    &'a CleanupPlace,
    &'a mut PathState,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type RenewCall<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a ExpressionId,
    (&'a CleanupPlace, &'a CleanupPlace),
    &'a mut PathState,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type ValidateJoinCompatibilityCall<'a> = (
    &'a ResolvedFunction,
    std::collections::btree_set::Range<'a, Box<PathState>>,
    &'a PathState,
    BlockId,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type ValidateControlGroupWithBudgetCall<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<Box<PathState>>,
    BlockId,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type JoinedOwnershipStateCall<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a BTreeSet<Box<PathState>>,
    &'a mut ReplayBudget,
    Result<(Vec<LivenessFlagId>, Vec<ReplayConditionalVariant>), Error>,
);
pub(super) type JoinLiveOrdersCall<'a> = (
    &'a ResolvedFunction,
    &'a [LivenessFlagId],
    &'a [LivenessFlagId],
    &'a mut ReplayBudget,
    Result<Vec<LivenessFlagId>, Error>,
);
pub(super) type ExecuteCall<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a CleanupTransition,
    &'a mut PathState,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type TransferCall<'a> = (
    &'a ResolvedFunction,
    &'a mut PathState,
    &'a CleanupPlace,
    &'a CleanupPlace,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type ExitCall<'a> = (
    &'a ResolvedFunction,
    &'a ExitTarget,
    PathState,
    &'a BTreeMap<StorageId, CleanupRegionId>,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a BTreeSet<StorageId>,
    &'a mut ReplayBudget,
    Result<ExitFlow, Error>,
);
pub(super) type ReserveAppendCall<'a> = (
    &'a dyn ReplayContext,
    &'a ResolvedFunction,
    &'a ExpressionId,
    &'a CleanupPlace,
    &'a CleanupPlace,
    &'a mut PathState,
    &'a mut ReplayBudget,
    Result<Option<Vec<LivenessFlagId>>, Error>,
);
pub(super) type PublishAppendCall<'a> = (
    &'a ResolvedFunction,
    Option<Vec<LivenessFlagId>>,
    &'a mut PathState,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type AppendFlagsCall<'a> = (
    &'a ResolvedFunction,
    &'a mut PathState,
    Vec<LivenessFlagId>,
    Kind,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type StateForEdgeCall<'a> = (
    &'a ResolvedFunction,
    PathState,
    &'a EdgeCondition,
    &'a BTreeMap<ExpressionId, StatusSourceId>,
    &'a mut ReplayBudget,
    Result<PathState, Error>,
);
pub(super) type FinishExitCall<'a> = (
    &'a ResolvedFunction,
    &'a ExitTarget,
    PathState,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<ExitFlow, Error>,
);
pub(super) type RequireNormalFlowStateCall<'a> = (
    &'a ResolvedFunction,
    &'a PathState,
    BlockId,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type CaptureLoopEntryCall<'a> = (
    &'a ResolvedFunction,
    &'a LexicalAuth,
    &'a EdgeCondition,
    &'a PathState,
    &'a mut BTreeMap<usize, PathState>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type CheckLatchCall<'a> = (
    &'a ResolvedFunction,
    &'a LexicalAuth,
    &'a ExitTarget,
    &'a PathState,
    &'a BTreeMap<usize, PathState>,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type CheckBreakCall<'a> = (
    &'a ResolvedFunction,
    &'a LexicalAuth,
    &'a ExitTarget,
    &'a PathState,
    &'a BTreeMap<usize, PathState>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
pub(super) type PlaceCall<'a> = (
    &'a ResolvedFunction,
    &'a CleanupPlace,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<Vec<LivenessFlagId>, Error>,
);

pub(super) type WalkBody<'a> = (
    &'a lexical_cert::LexicalAuth,
    &'a ExpressionId,
    &'a crate::cleanup_plan::CleanupPlan,
    BTreeMap<StorageId, CleanupRegionId>,
    BTreeMap<usize, PathState>,
    BTreeSet<StorageId>,
    BTreeMap<ExpressionId, StatusSourceId>,
    &'a StatusSource,
    std::slice::Iter<'a, StatusSource>,
    PathState,
    &'a CleanupPlace,
    std::slice::Iter<'a, CleanupPlace>,
    Vec<LivenessFlagId>,
    &'a crate::cleanup_plan::ConditionalVariantEntry,
    Vec<(DeclarationId, Vec<LivenessFlagId>)>,
    &'a crate::cleanup_plan::ConditionalVariantCase,
    Vec<LivenessFlagId>,
    &'a CleanupPlace,
    Vec<LivenessFlagId>,
    Vec<BTreeSet<Box<PathState>>>,
    Vec<usize>,
    &'a crate::cleanup_plan::CleanupBlock,
    BlockId,
    VecDeque<BlockId>,
    usize,
    usize,
    branch_join::BranchJoins,
    Option<BlockId>,
    BlockId,
    BTreeSet<Box<PathState>>,
    usize,
    &'a BTreeSet<Box<PathState>>,
    &'a crate::cleanup_plan::CleanupBlock,
    bool,
    PathState,
    std::collections::btree_set::IntoIter<Box<PathState>>,
    &'a lexical_cert::LexicalAuth,
    &'a CleanupTransition,
    std::slice::Iter<'a, CleanupTransition>,
    &'a crate::cleanup_plan::CleanupEdge,
    PathState,
    &'a crate::cleanup_plan::CleanupEdge,
    PathState,
    PathState,
    &'a lexical_cert::LexicalAuth,
    &'a ExitTarget,
    &'a BTreeSet<StorageId>,
    ExitFlow,
    PathState,
    EdgeId,
    BlockId,
    &'a ExpressionId,
    &'a mut usize,
    BTreeMap<ControlKey, BTreeSet<Box<PathState>>>,
    Box<PathState>,
    ControlKey,
    &'a mut BTreeSet<Box<PathState>>,
    &'a crate::cleanup_plan::CleanupBlock,
    BTreeSet<Box<PathState>>,
    bool,
    Vec<LivenessFlagId>,
    Vec<ReplayConditionalVariant>,
    PathState,
    &'a lexical_cert::LexicalAuth,
    &'a CleanupTransition,
    std::slice::Iter<'a, CleanupTransition>,
    &'a crate::cleanup_plan::CleanupEdge,
    PathState,
    &'a crate::cleanup_plan::CleanupEdge,
    PathState,
    PathState,
    &'a lexical_cert::LexicalAuth,
    &'a ExitTarget,
    &'a BTreeSet<StorageId>,
    ExitFlow,
    PathState,
    EdgeId,
    BlockId,
    &'a ExpressionId,
    &'a mut usize,
    std::collections::btree_map::IntoValues<ControlKey, BTreeSet<Box<PathState>>>,
    std::collections::btree_set::IntoIter<Box<PathState>>,
    Successors,
    Successors,
    Successors,
);

pub(super) type CancelEntriesNext<'a> = (
    &'a mut std::collections::btree_map::Values<
        'a,
        crate::cleanup_plan::ExitTargetId,
        ExpressionId,
    >,
    Option<&'a ExpressionId>,
    &'a ExpressionId,
);

pub(super) type EntryPlacesNext<'a> = (
    &'a mut std::slice::Iter<'a, CleanupPlace>,
    Option<&'a CleanupPlace>,
    &'a CleanupPlace,
);

pub(super) type ConditionalEntriesNext<'a> = (
    &'a mut std::slice::Iter<'a, crate::cleanup_plan::ConditionalVariantEntry>,
    Option<&'a crate::cleanup_plan::ConditionalVariantEntry>,
    &'a crate::cleanup_plan::ConditionalVariantEntry,
);

pub(super) type ConditionalCasesNext<'a> = (
    &'a mut std::slice::Iter<'a, crate::cleanup_plan::ConditionalVariantCase>,
    Option<&'a crate::cleanup_plan::ConditionalVariantCase>,
    &'a crate::cleanup_plan::ConditionalVariantCase,
);

pub(super) type ConditionalPlacesNext<'a> = (
    &'a mut std::slice::Iter<'a, CleanupPlace>,
    Option<&'a CleanupPlace>,
    &'a CleanupPlace,
);

pub(super) type PlanBlocksNext<'a> = (
    &'a mut std::slice::Iter<'a, crate::cleanup_plan::CleanupBlock>,
    Option<&'a crate::cleanup_plan::CleanupBlock>,
    &'a crate::cleanup_plan::CleanupBlock,
);

pub(super) type BlockTransitionsNext<'a> = (
    &'a mut std::slice::Iter<'a, CleanupTransition>,
    Option<&'a CleanupTransition>,
    &'a CleanupTransition,
);

pub(super) type BranchEdgesNext<'a> = (
    &'a mut std::slice::Iter<'a, EdgeId>,
    Option<&'a EdgeId>,
    &'a EdgeId,
);

pub(super) type OwnedStatesNext<'a> = (
    &'a mut std::collections::btree_set::IntoIter<Box<PathState>>,
    Option<Box<PathState>>,
    Box<PathState>,
);

pub(super) type ControlGroupsNext<'a> = (
    &'a mut std::collections::btree_map::IntoValues<
        super::walk::ControlKey,
        BTreeSet<Box<PathState>>,
    >,
    Option<BTreeSet<Box<PathState>>>,
    BTreeSet<Box<PathState>>,
);

pub(super) type SuccessorEntriesNext<'a> = (&'a mut Successors, Option<BlockId>, BlockId);

pub(super) type CallArgumentsNext<'a> = (
    &'a mut std::slice::Iter<'a, crate::cleanup_plan::CallArgumentTransfer>,
    Option<&'a crate::cleanup_plan::CallArgumentTransfer>,
    &'a crate::cleanup_plan::CallArgumentTransfer,
);

pub(super) type RenewalEntriesNext<'a> = (
    &'a mut std::collections::btree_map::ValuesMut<
        'a,
        ExpressionId,
        crate::cleanup_plan::renewal_history::RenewalHistory,
    >,
    Option<&'a mut crate::cleanup_plan::renewal_history::RenewalHistory>,
    &'a mut crate::cleanup_plan::renewal_history::RenewalHistory,
);

pub(super) type SourceFlagEntriesNext<'a> = (
    &'a mut std::slice::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    &'a LivenessFlagId,
);

pub(super) type LeafEntriesNext<'a> = (
    &'a mut std::collections::btree_map::Iter<'a, LivenessFlagId, super::super::super::Leaf>,
    Option<(&'a LivenessFlagId, &'a super::super::super::Leaf)>,
    &'a LivenessFlagId,
    &'a super::super::super::Leaf,
);

pub(super) type VariantCaseEntriesNext<'a> = (
    &'a mut std::slice::IterMut<'a, (DeclarationId, Vec<LivenessFlagId>)>,
    Option<&'a mut (DeclarationId, Vec<LivenessFlagId>)>,
    &'a mut Vec<LivenessFlagId>,
);

pub(super) type VariantFlagEntriesNext<'a> = (
    &'a mut std::slice::IterMut<'a, LivenessFlagId>,
    Option<&'a mut LivenessFlagId>,
    &'a mut LivenessFlagId,
);

pub(super) type ArgumentFlagsNext<'a> = (
    &'a mut std::vec::IntoIter<LivenessFlagId>,
    Option<LivenessFlagId>,
    LivenessFlagId,
);

pub(super) type GroupStateEntriesNext<'a> = (
    &'a mut std::collections::btree_set::Iter<'a, Box<PathState>>,
    Option<&'a Box<PathState>>,
    &'a Box<PathState>,
);

pub(super) type UnionFlagEntriesNext<'a> = (
    &'a mut std::collections::btree_set::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    &'a LivenessFlagId,
);

pub(super) type JoinHistoriesNext<'a> = (
    &'a mut std::array::IntoIter<&'a [LivenessFlagId], 2>,
    Option<&'a [LivenessFlagId]>,
    &'a [LivenessFlagId],
);

pub(super) type JoinPairsNext<'a> = (
    &'a mut std::slice::Windows<'a, LivenessFlagId>,
    Option<&'a [LivenessFlagId]>,
    &'a [LivenessFlagId],
);

pub(super) type JoinDegreesNext<'a> = (
    &'a mut std::collections::btree_map::Iter<'a, LivenessFlagId, u32>,
    Option<(&'a LivenessFlagId, &'a u32)>,
    &'a LivenessFlagId,
    &'a u32,
);

pub(super) type JoinSuccessorsNext<'a> = (
    &'a mut std::collections::btree_set::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    &'a LivenessFlagId,
);

pub(super) type RenewalUnionFlagsNext<'a> = (
    &'a mut std::collections::btree_set::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    &'a LivenessFlagId,
);

pub(super) type RenewalHistoriesNext<'a> = (
    &'a mut std::vec::IntoIter<&'a Vec<LivenessFlagId>>,
    Option<&'a Vec<LivenessFlagId>>,
    &'a Vec<LivenessFlagId>,
);

pub(super) type RenewalPairsNext<'a> = (
    &'a mut std::slice::Windows<'a, LivenessFlagId>,
    Option<&'a [LivenessFlagId]>,
    &'a [LivenessFlagId],
);

pub(super) type RenewalDegreesNext<'a> = (
    &'a mut std::collections::btree_map::Iter<'a, LivenessFlagId, usize>,
    Option<(&'a LivenessFlagId, &'a usize)>,
    &'a LivenessFlagId,
    &'a usize,
);

pub(super) type RenewalSuccessorsNext<'a> = (
    &'a mut std::collections::btree_set::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    &'a LivenessFlagId,
);

pub(super) type ContractSourcesNext<'a> = (
    &'a mut std::slice::Iter<'a, StatusSource>,
    Option<&'a StatusSource>,
    &'a StatusSource,
);
pub(super) type JoinedFlags<'a> = std::iter::Copied<
    std::iter::Chain<std::slice::Iter<'a, LivenessFlagId>, std::slice::Iter<'a, LivenessFlagId>>,
>;
pub(super) type JoinedFlagsNext<'a> = (
    &'a mut JoinedFlags<'a>,
    Option<LivenessFlagId>,
    LivenessFlagId,
);
pub(super) type HistoryReferencesNext<'a> = (
    &'a mut std::slice::Iter<'a, &'a Vec<LivenessFlagId>>,
    Option<&'a &'a Vec<LivenessFlagId>>,
    &'a &'a Vec<LivenessFlagId>,
);
pub(super) type HistoryFlagsNext<'a> = (
    &'a mut std::slice::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    LivenessFlagId,
);

pub(super) type FlagSetInsert<'a> = (
    &'a ResolvedFunction,
    &'a mut BTreeSet<LivenessFlagId>,
    LivenessFlagId,
    &'a mut ReplayBudget,
    &'a str,
    Result<bool, Diagnostic>,
);

pub(super) type SuccessorMapInsert<'a> = (
    &'a ResolvedFunction,
    &'a mut BTreeMap<LivenessFlagId, BTreeSet<LivenessFlagId>>,
    LivenessFlagId,
    BTreeSet<LivenessFlagId>,
    &'a mut ReplayBudget,
    &'a str,
    Result<(), Diagnostic>,
);

pub(super) type DegreeMapInsert<'a> = (
    &'a ResolvedFunction,
    &'a mut BTreeMap<LivenessFlagId, u32>,
    LivenessFlagId,
    u32,
    &'a mut ReplayBudget,
    &'a str,
    Result<(), Diagnostic>,
);

pub(super) type RenewalDegreeMapInsert<'a> = (
    &'a ResolvedFunction,
    &'a mut BTreeMap<LivenessFlagId, usize>,
    LivenessFlagId,
    usize,
    &'a mut ReplayBudget,
    &'a str,
    Result<(), Diagnostic>,
);

pub(super) type OwnershipClone<'a> = (
    &'a ResolvedFunction,
    &'a PathState,
    &'a mut ReplayBudget,
    Result<(), Diagnostic>,
);

pub(super) type StateClone<'a> = (
    &'a ResolvedFunction,
    &'a PathState,
    &'a mut ReplayBudget,
    &'a str,
    Result<(), Diagnostic>,
);

pub(super) type StorageRegions<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<BTreeMap<StorageId, CleanupRegionId>, Diagnostic>,
);

pub(super) type ControlClone<'a> = (
    &'a ResolvedFunction,
    &'a PathState,
    &'a mut ReplayBudget,
    Result<(), Diagnostic>,
);

pub(super) type TransitionCharge<'a> = (
    &'a ResolvedFunction,
    &'a CleanupTransition,
    &'a PathState,
    &'a mut ReplayBudget,
    Result<(), Error>,
);

pub(super) type BranchNormalize<'a> = (
    &'a mut branch_join::BranchJoins,
    &'a dyn ReplayContext,
    &'a ResolvedFunction,
    BlockId,
    bool,
    Vec<ReplayConditionalVariant>,
    &'a mut ReplayBudget,
    Result<Vec<ReplayConditionalVariant>, Diagnostic>,
);

pub(super) type BoundaryCheck<'a> = (
    &'a ResolvedFunction,
    &'a lexical_cert::LexicalAuth,
    BlockId,
    &'a PathState,
    &'a BTreeSet<StorageId>,
    &'a BTreeMap<StorageId, CleanupRegionId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<(), Error>,
);

pub(super) type ContractMapInsert<'a> = (
    &'a ResolvedFunction,
    &'a mut BTreeMap<ExpressionId, StatusSourceId>,
    ExpressionId,
    StatusSourceId,
    &'a mut ReplayBudget,
    &'a str,
    Result<(), Diagnostic>,
);

pub(super) type NodeReserve<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    &'a str,
    Result<(), Diagnostic>,
);

pub(super) type ConditionalJoin<'a> = (
    &'a dyn ReplayContext,
    &'a ResolvedFunction,
    &'a [ReplayConditionalVariant],
    &'a [ReplayConditionalVariant],
    &'a mut ReplayBudget,
    Result<Vec<ReplayConditionalVariant>, Diagnostic>,
);
