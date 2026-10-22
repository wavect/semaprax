//! Selected path execution failure data; it grants no authority.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum Kind {
    LoopEntryHasNoCertificate,
    LoopCertificateEntryOrderDiffersFromTheReplayedEntryOwnership,
    LoopIsEnteredWithTwoDifferentOwnershipStates,
    LatchCheckOnANonLatchExit,
    LoopLatchPrecedesItsLoopEntryState,
    LoopIterationDoesNotRestoreItsExactEntryOwnershipState,
    LoopBreakNamesAnUnknownLoop,
    LoopBreakHasNoCertificate,
    OwningBreakRetainsALiveHiddenStep,
    LoopBreakPrecedesItsLoopEntryState,
    LoopBreakDoesNotRestoreItsExactSurroundingOwnershipState,
    OwningBreakConditionalStepHasUnexpectedRenewalAuthority,
    ConditionalEntryCaseDoesNotNameExactCaseQualifiedLeaves,
    TooManyCleanupPredecessors,
    ReachableCleanupBlockHasNoIncomingOwnershipState,
    OwningBreakFragmentDoesNotRejoinItsParentWithExactOwnershipState,
    OwningBreakConditionalStepHasUnexpectedRenewalHistory,
    LoopLatchOutsideTheLexicalProfile,
    TooManyTerminalCleanupPaths,
    TooManySuccessfulCleanupPaths,
    CleanupPredecessorCountUnderflowed,
    CleanupCfgHasNoReplayableTerminalSuccessPath,
    CleanupJoinHasNoIncomingStates,
    CleanupJoinHasConflictingInitializationHistories,
    CleanupTransitionOccursAfterResultPublication,
    OrdinaryCleanupTransitionOccursAfterFailureSelection,
    OperationFailureIsNotSelectedBeforeAnotherTransition,
    SelectedPrimitiveReplayHasConditionalAggregateState,
    InitializeTransitionInitializesAnEmptyOrLiveCleanupPlace,
    FailureSelectionIsNotWriteOnceDuringPathReplay,
    CopyAggregateResultIsStagedMoreThanOnceOnOnePath,
    CopyAggregateResultStagingCarriesResourceLiveness,
    SelectedPrimitiveReplayExcludesConditionalAggregateTransitions,
    CleanupTransferDoesNotHaveDisjointEqualSizeOwnershipEpochs,
    CleanupTransferReadsADeadSourceLeaf,
    CleanupTransferInitializesALiveDestinationLeaf,
    TransferSourceIsNotAProjectionPrefix,
    TransferDestinationProjectionDepthOverflows,
    CleanupTransferLeafShapesDoNotCorrespond,
    CleanupTransferChangesALeafLifecycleOrAliasesItsDestination,
    CleanupTransferHasNoMappedOwnershipLeaves,
    ConditionalCleanupTransferOmitsACaseLeaf,
    CleanupTransferSourceHistoryIsIncomplete,
    CleanupTransferHistoryLengthOverflowed,
    CleanupExitFollowsPublication,
    SelectedPrimitiveExitHasConditionalOwnership,
    ExitDoesNotPreserveExactConditionalVariantFinalizers,
    CleanupExitRetainsALiveNonResultLeafInARegionItLeaves,
    ConditionalEdgeFollowsPendingSelectedOrPublishedState,
    NormalCleanupContinuationCarriesAFailureState,
    LoopLatchCarriesAFailureStagedResultOrReturnState,
    FunctionReturnCarriesAFailureStagedResultOrEarlierReturnState,
    OwnedFunctionReturnHasAnIncompleteProvisionalResult,
    FailureReturnChangesStatusOrRetainsLiveOwnership,
    ResultCommitFollowsAPendingOrSelectedFailure,
    OrdinaryScalarResultCommitDoesNotPublishTheFunctionBody,
    ReturnDomainCommitsAResultOtherThanItsHandedOffSource,
    ScalarResultCommitsBeforeNonResultCleanupCompletes,
    CopyResultCommitHasNoAuthenticatedStagedProducer,
    NonTryScalarResultCarriesAStagedCopyAggregate,
    ConditionalOwnedResultRetainsNonResultOwnership,
    OwnedResultCommitHasIncompleteResultOrRemainingNonResultOwnership,
    ResultPublicationLeavesCleanupOwnershipLive,
    UnitReturnFollowsFailureOrIncompleteCleanup,
    RenewalIsOutsideAnExactAuthenticatedSameOwnerAssignment,
    AuthenticatedRenewalUsesAnOrdinaryTransfer,
    RenewalReservationRequiresOneLiveUnreservedOwnerLeaf,
    RenewalHasNoReservation,
    RenewalDestinationIsNotOneOwnerLeaf,
    StringReplacementAliasesItsNewOwner,
    RenewalChangedUnrelatedCleanupInitializationHistory,
    SelectedAppendLacksCurrentFunctionCensus,
    SelectedAppendLookupOverflow,
    StringAppendReservesItsOwnerTwice,
    StringAppendPublishesWithoutAReservedOwner,
    AppendPublicationLengthOverflow,
    AppendPublicationWorkOverflow,
    EntryStateInitializesAnEmptyOrLiveCleanupPlace,
    OrdinaryRenewalDestinationComparisonOverflowed,
}
impl Kind {
    fn detail(self) -> &'static str {
        match self {
            Self::LoopEntryHasNoCertificate => "loop entry has no certificate",
            Self::LoopCertificateEntryOrderDiffersFromTheReplayedEntryOwnership => {
                "loop certificate entry order differs from the replayed entry ownership"
            }
            Self::LoopIsEnteredWithTwoDifferentOwnershipStates => {
                "loop is entered with two different ownership states"
            }
            Self::LatchCheckOnANonLatchExit => "latch check on a non-latch exit",
            Self::LoopLatchPrecedesItsLoopEntryState => "loop latch precedes its loop entry state",
            Self::LoopIterationDoesNotRestoreItsExactEntryOwnershipState => {
                "loop iteration does not restore its exact entry ownership state"
            }
            Self::LoopBreakNamesAnUnknownLoop => "loop break names an unknown loop",
            Self::LoopBreakHasNoCertificate => "loop break has no certificate",
            Self::OwningBreakRetainsALiveHiddenStep => "owning break retains a live hidden Step",
            Self::LoopBreakPrecedesItsLoopEntryState => "loop break precedes its loop entry state",
            Self::LoopBreakDoesNotRestoreItsExactSurroundingOwnershipState => {
                "loop break does not restore its exact surrounding ownership state"
            }
            Self::OwningBreakConditionalStepHasUnexpectedRenewalAuthority => {
                "owning Break conditional Step has unexpected renewal authority"
            }
            Self::ConditionalEntryCaseDoesNotNameExactCaseQualifiedLeaves => {
                "conditional entry case does not name exact case-qualified leaves"
            }
            Self::TooManyCleanupPredecessors => "too many cleanup predecessors",
            Self::ReachableCleanupBlockHasNoIncomingOwnershipState => {
                "reachable cleanup block has no incoming ownership state"
            }
            Self::OwningBreakFragmentDoesNotRejoinItsParentWithExactOwnershipState => {
                "owning break fragment does not rejoin its parent with exact ownership state"
            }
            Self::OwningBreakConditionalStepHasUnexpectedRenewalHistory => {
                "owning Break conditional Step has unexpected renewal history"
            }
            Self::LoopLatchOutsideTheLexicalProfile => "loop latch outside the lexical profile",
            Self::TooManyTerminalCleanupPaths => "too many terminal cleanup paths",
            Self::TooManySuccessfulCleanupPaths => "too many successful cleanup paths",
            Self::CleanupPredecessorCountUnderflowed => "cleanup predecessor count underflowed",
            Self::CleanupCfgHasNoReplayableTerminalSuccessPath => {
                "cleanup CFG has no replayable terminal success path"
            }
            Self::CleanupJoinHasNoIncomingStates => "cleanup join has no incoming states",
            Self::CleanupJoinHasConflictingInitializationHistories => {
                "cleanup join has conflicting initialization histories"
            }
            Self::CleanupTransitionOccursAfterResultPublication => {
                "cleanup transition occurs after result publication"
            }
            Self::OrdinaryCleanupTransitionOccursAfterFailureSelection => {
                "ordinary cleanup transition occurs after failure selection"
            }
            Self::OperationFailureIsNotSelectedBeforeAnotherTransition => {
                "operation failure is not selected before another transition"
            }
            Self::SelectedPrimitiveReplayHasConditionalAggregateState => {
                "selected primitive replay has conditional aggregate state"
            }
            Self::InitializeTransitionInitializesAnEmptyOrLiveCleanupPlace => {
                "initialize transition initializes an empty or live cleanup place"
            }
            Self::FailureSelectionIsNotWriteOnceDuringPathReplay => {
                "failure selection is not write-once during path replay"
            }
            Self::CopyAggregateResultIsStagedMoreThanOnceOnOnePath => {
                "Copy aggregate result is staged more than once on one path"
            }
            Self::CopyAggregateResultStagingCarriesResourceLiveness => {
                "Copy aggregate result staging carries resource liveness"
            }
            Self::SelectedPrimitiveReplayExcludesConditionalAggregateTransitions => {
                "selected primitive replay excludes conditional aggregate transitions"
            }
            Self::CleanupTransferDoesNotHaveDisjointEqualSizeOwnershipEpochs => {
                "cleanup transfer does not have disjoint equal-size ownership epochs"
            }
            Self::CleanupTransferReadsADeadSourceLeaf => {
                "cleanup transfer reads a dead source leaf"
            }
            Self::CleanupTransferInitializesALiveDestinationLeaf => {
                "cleanup transfer initializes a live destination leaf"
            }
            Self::TransferSourceIsNotAProjectionPrefix => {
                "transfer source is not a projection prefix"
            }
            Self::TransferDestinationProjectionDepthOverflows => {
                "transfer destination projection depth overflows"
            }
            Self::CleanupTransferLeafShapesDoNotCorrespond => {
                "cleanup transfer leaf shapes do not correspond"
            }
            Self::CleanupTransferChangesALeafLifecycleOrAliasesItsDestination => {
                "cleanup transfer changes a leaf lifecycle or aliases its destination"
            }
            Self::CleanupTransferHasNoMappedOwnershipLeaves => {
                "cleanup transfer has no mapped ownership leaves"
            }
            Self::ConditionalCleanupTransferOmitsACaseLeaf => {
                "conditional cleanup transfer omits a case leaf"
            }
            Self::CleanupTransferSourceHistoryIsIncomplete => {
                "cleanup transfer source history is incomplete"
            }
            Self::CleanupTransferHistoryLengthOverflowed => {
                "cleanup transfer history length overflowed"
            }
            Self::CleanupExitFollowsPublication => "cleanup exit follows publication",
            Self::SelectedPrimitiveExitHasConditionalOwnership => {
                "selected primitive exit has conditional ownership"
            }
            Self::ExitDoesNotPreserveExactConditionalVariantFinalizers => {
                "exit does not preserve exact conditional variant finalizers"
            }
            Self::CleanupExitRetainsALiveNonResultLeafInARegionItLeaves => {
                "cleanup exit retains a live non-result leaf in a region it leaves"
            }
            Self::ConditionalEdgeFollowsPendingSelectedOrPublishedState => {
                "conditional edge follows pending, selected, or published state"
            }
            Self::NormalCleanupContinuationCarriesAFailureState => {
                "normal cleanup continuation carries a failure state"
            }
            Self::LoopLatchCarriesAFailureStagedResultOrReturnState => {
                "loop latch carries a failure, staged-result or return state"
            }
            Self::FunctionReturnCarriesAFailureStagedResultOrEarlierReturnState => {
                "function return carries a failure, staged-result or earlier return state"
            }
            Self::OwnedFunctionReturnHasAnIncompleteProvisionalResult => {
                "owned function return has an incomplete provisional result"
            }
            Self::FailureReturnChangesStatusOrRetainsLiveOwnership => {
                "failure return changes status or retains live ownership"
            }
            Self::ResultCommitFollowsAPendingOrSelectedFailure => {
                "result commit follows a pending or selected failure"
            }
            Self::OrdinaryScalarResultCommitDoesNotPublishTheFunctionBody => {
                "ordinary scalar result commit does not publish the function body"
            }
            Self::ReturnDomainCommitsAResultOtherThanItsHandedOffSource => {
                "return domain commits a result other than its handed-off source"
            }
            Self::ScalarResultCommitsBeforeNonResultCleanupCompletes => {
                "scalar result commits before non-result cleanup completes"
            }
            Self::CopyResultCommitHasNoAuthenticatedStagedProducer => {
                "Copy Result commit has no authenticated staged producer"
            }
            Self::NonTryScalarResultCarriesAStagedCopyAggregate => {
                "non-try scalar result carries a staged Copy aggregate"
            }
            Self::ConditionalOwnedResultRetainsNonResultOwnership => {
                "conditional owned result retains non-result ownership"
            }
            Self::OwnedResultCommitHasIncompleteResultOrRemainingNonResultOwnership => {
                "owned result commit has incomplete result or remaining non-result ownership"
            }
            Self::ResultPublicationLeavesCleanupOwnershipLive => {
                "result publication leaves cleanup ownership live"
            }
            Self::UnitReturnFollowsFailureOrIncompleteCleanup => {
                "unit return follows failure or incomplete cleanup"
            }
            Self::RenewalIsOutsideAnExactAuthenticatedSameOwnerAssignment => {
                "renewal is outside an exact authenticated same-owner assignment"
            }
            Self::AuthenticatedRenewalUsesAnOrdinaryTransfer => {
                "authenticated renewal uses an ordinary transfer"
            }
            Self::RenewalReservationRequiresOneLiveUnreservedOwnerLeaf => {
                "renewal reservation requires one live unreserved owner leaf"
            }
            Self::RenewalHasNoReservation => "renewal has no reservation",
            Self::RenewalDestinationIsNotOneOwnerLeaf => {
                "renewal destination is not one owner leaf"
            }
            Self::StringReplacementAliasesItsNewOwner => "String replacement aliases its new owner",
            Self::RenewalChangedUnrelatedCleanupInitializationHistory => {
                "renewal changed unrelated cleanup initialization history"
            }
            Self::SelectedAppendLacksCurrentFunctionCensus => {
                "selected append lacks current function census"
            }
            Self::SelectedAppendLookupOverflow => "selected append lookup overflow",
            Self::StringAppendReservesItsOwnerTwice => "string append reserves its owner twice",
            Self::StringAppendPublishesWithoutAReservedOwner => {
                "string append publishes without a reserved owner"
            }
            Self::AppendPublicationLengthOverflow => "append publication length overflow",
            Self::AppendPublicationWorkOverflow => "append publication work overflow",
            Self::OrdinaryRenewalDestinationComparisonOverflowed => {
                "ordinary renewal destination comparison overflowed"
            }
            Self::EntryStateInitializesAnEmptyOrLiveCleanupPlace => {
                "entry state initializes an empty or live cleanup place"
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum Reason {
    Finite(Kind),
    BlockControl(BlockId),
    BlockHistory(BlockId),
    NormalFlow(BlockId),
    ExitOrder(crate::cleanup_plan::ExitTargetId),
}
impl Reason {
    pub(in crate::cleanup_plan::replay) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        match self {
 Self::Finite(kind)=>replay_error(function,kind.detail()),
 Self::BlockControl(block)=>replay_error(function,format_args!("cleanup join at block {} has incompatible control states",block.0)),
 Self::BlockHistory(block)=>replay_error(function,format_args!("cleanup join at block {} has conflicting initialization histories",block.0)),
 Self::NormalFlow(block)=>replay_error(function,format_args!("block {} continues ordinary control flow with failure or publication state",block.0)),
 Self::ExitOrder(exit)=>replay_error(function,format_args!("exit {} does not finalize its live region leaves in reverse initialization order",exit.0)),
 }
    }
}
