//! Closed selected structural-control reasons; these values carry no authority.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum Kind {
    CleanupEntryBlockIsUnknown,
    CleanupEntryPlacesOverlapOrRepeatLivenessFlags,
    CleanupEdgeHasANonContiguousIdOrUnknownEndpoint,
    CleanupBlockHasANonContiguousIdOrUnknownRegion,
    CleanupTransferHasIncompatibleStructuralPlaces,
    SelectedStructuralReplayDoesNotSupportConditionalVariantOwnership,
    CallCommitDoesNotNameACallExpression,
    CallHasMoreThanOneAtomicCommit,
    CleanupBlockSelectsMoreThanOneFailureStatus,
    SelectedStructuralReplayDoesNotSupportResidualStaging,
    BodyCopyResultStageNamesAnotherExpression,
    GotoEdgeIsConditional,
    CleanupBranchMustOwnExactlyTwoDistinctEdges,
    CleanupBlockReferencesAnUnknownOrRepeatedExit,
    CleanupExitIsOwnedByTheWrongBlock,
    TooManyCallExpressions,
    CleanupPlanDoesNotContainExactlyOneAtomicCommitForEveryCall,
    CleanupTerminatorReferencesUnknownEdge,
    CleanupEdgeHasTheWrongOwnerOrIsReferencedRepeatedly,
    LexicalExitContinuationHasNoLexicalControlCertificate,
    CleanupExitHasANonContiguousIdOrUnknownSourceBlock,
    CleanupExitDoesNotLeaveTheExactSourceToTargetRegionChain,
    CleanupExitReferencesUnknownRegion,
    CleanupExitRegionsAreNotAParentChain,
    CleanupContinuationEdgeIsConditional,
    FunctionReturnContinuationEdgeIsConditional,
    LoopBreakContinuationEdgeIsConditional,
    CleanupReferenceNamesAnUnknownStatusSource,
    FailureExitDoesNotReturnItsUniquelySelectedStatus,
    ReturnunitIsInvalidForCurrentSourceFunctionReturnTypes,
    RegionNormalScopeExitDoesNotLeaveThatRegion,
    CleanupRegionTreeContainsACycle,
    CleanupRegionChainReferencesAnUnknownRegion,
    CleanupExitHasAnUnknownSourceBlock,
    SelectedPrimitiveExitsDoNotAdmitOwnedIteratorScopes,
    LoopLatchNamesAnUnknownLexicalLoop,
    LoopLatchExitDoesNotOriginateInsideItsLoopRegion,
    CleanupContinuationHasAnUnknownTarget,
    DroppableFunctionResultUsesAScalarResultCommit,
    ScalarResultCommitDoesNotPublishTheFunctionBody,
    OwnedResultCommitMustPublishTheWholeDroppableProvisionalResult,
    OwnedResultCommitHasNoProvisionalResultLeaves,
    CleanupCfgReferencesAnUnknownBlock,
    CleanupCfgReferencesAnUnknownExit,
    CleanupCfgReferencesAnUnknownEdge,
    CleanupCfgContainsACycle,
    CleanupCfgContainsAnUnreachableBlock,
    PropagatedCallStatusSourceDoesNotNameACallExpression,
    PropagatedCallStatusSourceNamesTheWrongCallee,
    CheckedArithmeticStatusCasesAreEmptyOrRepeated,
    StatusSourceLaneDoesNotMatchItsProducer,
    CleanupPlaceHasNoDroppableLeaf,
    RenewalIsOutsideAnExactAuthenticatedSameOwnerAssignment,
    BodyResultStagingReferencesAnotherExpression,
    StagedCopyResultTargetsAnotherConcreteFunctionResult,
    CleanupBranchConditionsAreNotComplementary,
    CleanupPlaceReferencesUnknownStorage,
    FinalizerGuardDoesNotNameItsExactCleanupLeaf,
    FinalizerLifecycleOrGuardIsInvalidOrRepeated,
    SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile,
    ConditionalFinalizerDoesNotBindItsExactVariantCasePath,
    TooManyCleanupEdges,
    TooManyCleanupBlocks,
    TooManyCleanupExits,
}
impl Kind {
    pub(super) fn detail(self) -> &'static str {
        match self {
            Self::CleanupEntryBlockIsUnknown => "cleanup entry block is unknown",
            Self::CleanupEntryPlacesOverlapOrRepeatLivenessFlags => {
                "cleanup entry places overlap or repeat liveness flags"
            }
            Self::CleanupEdgeHasANonContiguousIdOrUnknownEndpoint => {
                "cleanup edge has a non-contiguous ID or unknown endpoint"
            }
            Self::CleanupBlockHasANonContiguousIdOrUnknownRegion => {
                "cleanup block has a non-contiguous ID or unknown region"
            }
            Self::CleanupTransferHasIncompatibleStructuralPlaces => {
                "cleanup transfer has incompatible structural places"
            }
            Self::SelectedStructuralReplayDoesNotSupportConditionalVariantOwnership => {
                "selected structural replay does not support conditional variant ownership"
            }
            Self::CallCommitDoesNotNameACallExpression => {
                "call commit does not name a call expression"
            }
            Self::CallHasMoreThanOneAtomicCommit => "call has more than one atomic commit",
            Self::CleanupBlockSelectsMoreThanOneFailureStatus => {
                "cleanup block selects more than one failure status"
            }
            Self::SelectedStructuralReplayDoesNotSupportResidualStaging => {
                "selected structural replay does not support residual staging"
            }
            Self::BodyCopyResultStageNamesAnotherExpression => {
                "body Copy-result stage names another expression"
            }
            Self::GotoEdgeIsConditional => "goto edge is conditional",
            Self::CleanupBranchMustOwnExactlyTwoDistinctEdges => {
                "cleanup branch must own exactly two distinct edges"
            }
            Self::CleanupBlockReferencesAnUnknownOrRepeatedExit => {
                "cleanup block references an unknown or repeated exit"
            }
            Self::CleanupExitIsOwnedByTheWrongBlock => "cleanup exit is owned by the wrong block",
            Self::TooManyCallExpressions => "too many call expressions",
            Self::CleanupPlanDoesNotContainExactlyOneAtomicCommitForEveryCall => {
                "cleanup plan does not contain exactly one atomic commit for every call"
            }
            Self::CleanupTerminatorReferencesUnknownEdge => {
                "cleanup terminator references unknown edge"
            }
            Self::CleanupEdgeHasTheWrongOwnerOrIsReferencedRepeatedly => {
                "cleanup edge has the wrong owner or is referenced repeatedly"
            }
            Self::LexicalExitContinuationHasNoLexicalControlCertificate => {
                "lexical exit continuation has no lexical-control certificate"
            }
            Self::CleanupExitHasANonContiguousIdOrUnknownSourceBlock => {
                "cleanup exit has a non-contiguous ID or unknown source block"
            }
            Self::CleanupExitDoesNotLeaveTheExactSourceToTargetRegionChain => {
                "cleanup exit does not leave the exact source-to-target region chain"
            }
            Self::CleanupExitReferencesUnknownRegion => "cleanup exit references unknown region",
            Self::CleanupExitRegionsAreNotAParentChain => {
                "cleanup exit regions are not a parent chain"
            }
            Self::CleanupContinuationEdgeIsConditional => {
                "cleanup continuation edge is conditional"
            }
            Self::FunctionReturnContinuationEdgeIsConditional => {
                "function-return continuation edge is conditional"
            }
            Self::LoopBreakContinuationEdgeIsConditional => {
                "loop-break continuation edge is conditional"
            }
            Self::CleanupReferenceNamesAnUnknownStatusSource => {
                "cleanup reference names an unknown status source"
            }
            Self::FailureExitDoesNotReturnItsUniquelySelectedStatus => {
                "failure exit does not return its uniquely selected status"
            }
            Self::ReturnunitIsInvalidForCurrentSourceFunctionReturnTypes => {
                "ReturnUnit is invalid for current source-function return types"
            }
            Self::RegionNormalScopeExitDoesNotLeaveThatRegion => {
                "region normal-scope exit does not leave that region"
            }
            Self::CleanupRegionTreeContainsACycle => "cleanup region tree contains a cycle",
            Self::CleanupRegionChainReferencesAnUnknownRegion => {
                "cleanup region chain references an unknown region"
            }
            Self::CleanupExitHasAnUnknownSourceBlock => "cleanup exit has an unknown source block",
            Self::SelectedPrimitiveExitsDoNotAdmitOwnedIteratorScopes => {
                "selected primitive exits do not admit owned iterator scopes"
            }
            Self::LoopLatchNamesAnUnknownLexicalLoop => "loop latch names an unknown lexical loop",
            Self::LoopLatchExitDoesNotOriginateInsideItsLoopRegion => {
                "loop latch exit does not originate inside its loop region"
            }
            Self::CleanupContinuationHasAnUnknownTarget => {
                "cleanup continuation has an unknown target"
            }
            Self::DroppableFunctionResultUsesAScalarResultCommit => {
                "droppable function result uses a scalar result commit"
            }
            Self::ScalarResultCommitDoesNotPublishTheFunctionBody => {
                "scalar result commit does not publish the function body"
            }
            Self::OwnedResultCommitMustPublishTheWholeDroppableProvisionalResult => {
                "owned result commit must publish the whole droppable provisional result"
            }
            Self::OwnedResultCommitHasNoProvisionalResultLeaves => {
                "owned result commit has no provisional-result leaves"
            }
            Self::CleanupCfgReferencesAnUnknownBlock => "cleanup CFG references an unknown block",
            Self::CleanupCfgReferencesAnUnknownExit => "cleanup CFG references an unknown exit",
            Self::CleanupCfgReferencesAnUnknownEdge => "cleanup CFG references an unknown edge",
            Self::CleanupCfgContainsACycle => "cleanup CFG contains a cycle",
            Self::CleanupCfgContainsAnUnreachableBlock => {
                "cleanup CFG contains an unreachable block"
            }
            Self::PropagatedCallStatusSourceDoesNotNameACallExpression => {
                "propagated-call status source does not name a call expression"
            }
            Self::PropagatedCallStatusSourceNamesTheWrongCallee => {
                "propagated-call status source names the wrong callee"
            }
            Self::CheckedArithmeticStatusCasesAreEmptyOrRepeated => {
                "checked-arithmetic status cases are empty or repeated"
            }
            Self::StatusSourceLaneDoesNotMatchItsProducer => {
                "status source lane does not match its producer"
            }
            Self::CleanupPlaceHasNoDroppableLeaf => "cleanup place has no droppable leaf",
            Self::RenewalIsOutsideAnExactAuthenticatedSameOwnerAssignment => {
                "renewal is outside an exact authenticated same-owner assignment"
            }
            Self::BodyResultStagingReferencesAnotherExpression => {
                "body result staging references another expression"
            }
            Self::StagedCopyResultTargetsAnotherConcreteFunctionResult => {
                "staged Copy result targets another concrete function result"
            }
            Self::CleanupBranchConditionsAreNotComplementary => {
                "cleanup branch conditions are not complementary"
            }
            Self::CleanupPlaceReferencesUnknownStorage => {
                "cleanup place references unknown storage"
            }
            Self::FinalizerGuardDoesNotNameItsExactCleanupLeaf => {
                "finalizer guard does not name its exact cleanup leaf"
            }
            Self::FinalizerLifecycleOrGuardIsInvalidOrRepeated => {
                "finalizer lifecycle or guard is invalid or repeated"
            }
            Self::SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile => {
                "selected lexical boundary conditional ownership is outside the primitive profile"
            }
            Self::ConditionalFinalizerDoesNotBindItsExactVariantCasePath => {
                "conditional finalizer does not bind its exact variant case path"
            }
            Self::TooManyCleanupEdges => "too many cleanup edges",
            Self::TooManyCleanupBlocks => "too many cleanup blocks",
            Self::TooManyCleanupExits => "too many cleanup exits",
        }
    }
}
#[derive(Debug)]
pub(in crate::cleanup_plan::replay) enum Reason {
    Finite(Kind),
    UnknownExpression(ExpressionId),
    UnknownStatus(ExpressionId),
}
impl Reason {
    /// Explicit legacy observation outside the paid no-allocation cut.
    pub(in crate::cleanup_plan::replay) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        match self {
            Self::Finite(kind) => replay_error(function, kind.detail()),
            Self::UnknownExpression(id) => replay_error(
                function,
                format_args!("cleanup plan references unknown expression `{id}`"),
            ),
            Self::UnknownStatus(id) => replay_error(
                function,
                format_args!("cleanup plan references unknown status source `{id}`"),
            ),
        }
    }
}
