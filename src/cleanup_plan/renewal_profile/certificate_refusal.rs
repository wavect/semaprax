//! Closed lexical-certificate failure data; no validation or publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CertificateRefusal {
    LexicalProfileFunctionHasNoLexicalControlCertificate,
    LexicalControlCertificateUsesAnUnknownSchema,
    LexicalExitCertificateNamesAnUnknownResumeBlock,
    LoopCertificateNamesAnUnknownRegionOrBlock,
    LoopCertificateCarriesASharedLatchEntry,
    ResumeCertificateNamesAnUnknownBlock,
    LoopCertificatesDoNotCoverExactlyTheReachableHirLoops,
    LoopCertificateDisagreesWithItsTypedHirLoop,
    LoopCertificateConditionIsRepeated,
    LoopCertificateBlocksDoNotMatchTheConditionBranchAndLoopRegion,
    LoopConditionEntryDoesNotDominateItsConditionBranch,
    ExitCertificatesDoNotCoverExactlyTheReachableHirExits,
    ExitCertificateDisagreesWithItsTypedHirExit,
    ExitCertificateNamesAnUnknownExit,
    ExitCertificateNamesARepeatedExit,
    FunctionReturnHasNoOperand,
    FunctionReturnExitDisagreesWithItsOperandResultSourceOrResume,
    LoopBreakNamesAnUnknownLoop,
    LoopBreakExitDisagreesWithItsLoopJoin,
    LoopContinueNamesAnUnknownLoop,
    LoopContinueExitNamesAnotherTarget,
    ContinueExitsShareOneGeneratedFragment,
    ContinueResumeDisagreesWithItsGeneratedAdvancement,
    ExitCertificateKindDisagreesWithItsCleanupContinuation,
    LoopCertificateHasNoNormalIterationEnd,
    LoopIterationEndIsNotItsOwnUncertifiedLatchExit,
    LoopWithANormalEndpointCarriesTheAbruptOnlyIterationSentinel,
    ResumeCertificateEntryIsRepeated,
    ResumeCertificatesDoNotCoverExactlyTheCertifiedExitEntries,
    ResumeCertificateIsNotItsEntryFollowedByItsExactReachableBlocks,
    LexicalProofDomainsOverlap,
    OwningBreakFragmentDoesNotEndInExactlyOneParentJoin,
    OwningBreakFragmentDoesNotRejoinItsWrapperSParentExactly,
    TerminalLexicalFragmentHasAnOutgoingEdge,
    CleanupBlockBelongsToNoLexicalProofDomain,
    AnEdgeEntersALexicalResumeOutsideItsCertifiedExit,
    OwningBreakFragmentHasNoOwningLatch,
    LatchFragmentEndsAtAnotherLoopSLatch,
    ExitContinuationIsNotAdmittedInItsLexicalProofDomain,
    OrdinarySuccessExitsDisagreeWithTheBodySNormalCompletion,
    ResumeProofDomainDoesNotEndInItsExactTerminalExit,
    FunctionSuccessExitsAreNotTheExactOrdinaryAndReturnCommits,
    NormalIterationEndsAreNotOnePerCompletingLoop,
    AbruptOnlyRegionListIsInvalidOrRepeated,
    ProvisionalNormalScopeEndDiffersFromTheAbruptOnlyCertificate,
    RegionNormalScopeEndIsNotItsOwnScopeExit,
    RegionNormalScopeEndCrossesItsTypedEnsuresReturnDomains,
    AbruptOnlyRegionHasANormalScopeExit,
    AbruptOnlyRegionFlowEscapesWithoutALexicalExit,
    RootRegionHasNoNormalSuccessExit,
    LoopConditionHasRepeatedBooleanEdges,
    LoopConditionHasNoComplementaryBooleanBranch,
}
impl CertificateRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::LexicalProfileFunctionHasNoLexicalControlCertificate => {
                "lexical-profile function has no lexical-control certificate"
            }
            Self::LexicalControlCertificateUsesAnUnknownSchema => {
                "lexical-control certificate uses an unknown schema"
            }
            Self::LexicalExitCertificateNamesAnUnknownResumeBlock => {
                "lexical exit certificate names an unknown resume block"
            }
            Self::LoopCertificateNamesAnUnknownRegionOrBlock => {
                "loop certificate names an unknown region or block"
            }
            Self::LoopCertificateCarriesASharedLatchEntry => {
                "loop certificate carries a shared latch entry"
            }
            Self::ResumeCertificateNamesAnUnknownBlock => {
                "resume certificate names an unknown block"
            }
            Self::LoopCertificatesDoNotCoverExactlyTheReachableHirLoops => {
                "loop certificates do not cover exactly the reachable HIR loops"
            }
            Self::LoopCertificateDisagreesWithItsTypedHirLoop => {
                "loop certificate disagrees with its typed HIR loop"
            }
            Self::LoopCertificateConditionIsRepeated => "loop certificate condition is repeated",
            Self::LoopCertificateBlocksDoNotMatchTheConditionBranchAndLoopRegion => {
                "loop certificate blocks do not match the condition branch and loop region"
            }
            Self::LoopConditionEntryDoesNotDominateItsConditionBranch => {
                "loop condition entry does not dominate its condition branch"
            }
            Self::ExitCertificatesDoNotCoverExactlyTheReachableHirExits => {
                "exit certificates do not cover exactly the reachable HIR exits"
            }
            Self::ExitCertificateDisagreesWithItsTypedHirExit => {
                "exit certificate disagrees with its typed HIR exit"
            }
            Self::ExitCertificateNamesAnUnknownExit => "exit certificate names an unknown exit",
            Self::ExitCertificateNamesARepeatedExit => "exit certificate names a repeated exit",
            Self::FunctionReturnHasNoOperand => "function return has no operand",
            Self::FunctionReturnExitDisagreesWithItsOperandResultSourceOrResume => {
                "function return exit disagrees with its operand, result source or resume"
            }
            Self::LoopBreakNamesAnUnknownLoop => "loop break names an unknown loop",
            Self::LoopBreakExitDisagreesWithItsLoopJoin => {
                "loop break exit disagrees with its loop join"
            }
            Self::LoopContinueNamesAnUnknownLoop => "loop continue names an unknown loop",
            Self::LoopContinueExitNamesAnotherTarget => "loop continue exit names another target",
            Self::ContinueExitsShareOneGeneratedFragment => {
                "continue exits share one generated fragment"
            }
            Self::ContinueResumeDisagreesWithItsGeneratedAdvancement => {
                "continue resume disagrees with its generated advancement"
            }
            Self::ExitCertificateKindDisagreesWithItsCleanupContinuation => {
                "exit certificate kind disagrees with its cleanup continuation"
            }
            Self::LoopCertificateHasNoNormalIterationEnd => {
                "loop certificate has no normal iteration end"
            }
            Self::LoopIterationEndIsNotItsOwnUncertifiedLatchExit => {
                "loop iteration end is not its own uncertified latch exit"
            }
            Self::LoopWithANormalEndpointCarriesTheAbruptOnlyIterationSentinel => {
                "loop with a normal endpoint carries the abrupt-only iteration sentinel"
            }
            Self::ResumeCertificateEntryIsRepeated => "resume certificate entry is repeated",
            Self::ResumeCertificatesDoNotCoverExactlyTheCertifiedExitEntries => {
                "resume certificates do not cover exactly the certified exit entries"
            }
            Self::ResumeCertificateIsNotItsEntryFollowedByItsExactReachableBlocks => {
                "resume certificate is not its entry followed by its exact reachable blocks"
            }
            Self::LexicalProofDomainsOverlap => "lexical proof domains overlap",
            Self::OwningBreakFragmentDoesNotEndInExactlyOneParentJoin => {
                "owning break fragment does not end in exactly one parent join"
            }
            Self::OwningBreakFragmentDoesNotRejoinItsWrapperSParentExactly => {
                "owning break fragment does not rejoin its wrapper's parent exactly"
            }
            Self::TerminalLexicalFragmentHasAnOutgoingEdge => {
                "terminal lexical fragment has an outgoing edge"
            }
            Self::CleanupBlockBelongsToNoLexicalProofDomain => {
                "cleanup block belongs to no lexical proof domain"
            }
            Self::AnEdgeEntersALexicalResumeOutsideItsCertifiedExit => {
                "an edge enters a lexical resume outside its certified exit"
            }
            Self::OwningBreakFragmentHasNoOwningLatch => {
                "owning break fragment has no owning latch"
            }
            Self::LatchFragmentEndsAtAnotherLoopSLatch => {
                "latch fragment ends at another loop's latch"
            }
            Self::ExitContinuationIsNotAdmittedInItsLexicalProofDomain => {
                "exit continuation is not admitted in its lexical proof domain"
            }
            Self::OrdinarySuccessExitsDisagreeWithTheBodySNormalCompletion => {
                "ordinary success exits disagree with the body's normal completion"
            }
            Self::ResumeProofDomainDoesNotEndInItsExactTerminalExit => {
                "resume proof domain does not end in its exact terminal exit"
            }
            Self::FunctionSuccessExitsAreNotTheExactOrdinaryAndReturnCommits => {
                "function success exits are not the exact ordinary and return commits"
            }
            Self::NormalIterationEndsAreNotOnePerCompletingLoop => {
                "normal iteration ends are not one per completing loop"
            }
            Self::AbruptOnlyRegionListIsInvalidOrRepeated => {
                "abrupt-only region list is invalid or repeated"
            }
            Self::ProvisionalNormalScopeEndDiffersFromTheAbruptOnlyCertificate => {
                "provisional normal scope end differs from the abrupt-only certificate"
            }
            Self::RegionNormalScopeEndIsNotItsOwnScopeExit => {
                "region normal scope end is not its own scope exit"
            }
            Self::RegionNormalScopeEndCrossesItsTypedEnsuresReturnDomains => {
                "region normal scope end crosses its typed ensures Return domains"
            }
            Self::AbruptOnlyRegionHasANormalScopeExit => {
                "abrupt-only region has a normal scope exit"
            }
            Self::AbruptOnlyRegionFlowEscapesWithoutALexicalExit => {
                "abrupt-only region flow escapes without a lexical exit"
            }
            Self::RootRegionHasNoNormalSuccessExit => "root region has no normal success exit",
            Self::LoopConditionHasRepeatedBooleanEdges => {
                "loop condition has repeated Boolean edges"
            }
            Self::LoopConditionHasNoComplementaryBooleanBranch => {
                "loop condition has no complementary Boolean branch"
            }
        }
    }
}
