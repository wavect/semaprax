//! Closed path-state boundary and finalizer failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathBoundaryRefusal {
    ExecutionBoundaryHasNoCertificate,
    ExecutionBoundaryIsReachedWithFailureOrPublicationState,
    ExecutionBoundaryCarriesAnotherPhaseSReturnedResult,
    SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile,
    ExecutionBoundaryFinalizersDifferFromTheReplayedLiveOwnershipContour,
    ExecutionBoundaryVariantHasNoRegion,
    CleanupPlaceReferencesUnknownStorage,
    CleanupPlaceHasNoDroppableLeaf,
    FinalizerGuardDoesNotNameItsExactCleanupLeaf,
    FinalizerLifecycleOrGuardIsInvalidOrRepeated,
    ConditionalFinalizerDoesNotBindItsExactVariantCasePath,
}
impl PathBoundaryRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::ExecutionBoundaryHasNoCertificate => "execution boundary has no certificate",
            Self::ExecutionBoundaryIsReachedWithFailureOrPublicationState => {
                "execution boundary is reached with failure or publication state"
            }
            Self::ExecutionBoundaryCarriesAnotherPhaseSReturnedResult => {
                "execution boundary carries another phase's returned result"
            }
            Self::SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile => {
                "selected lexical boundary conditional ownership is outside the primitive profile"
            }
            Self::ExecutionBoundaryFinalizersDifferFromTheReplayedLiveOwnershipContour => {
                "execution boundary finalizers differ from the replayed live ownership contour"
            }
            Self::ExecutionBoundaryVariantHasNoRegion => "execution boundary variant has no region",
            Self::CleanupPlaceReferencesUnknownStorage => {
                "cleanup place references unknown storage"
            }
            Self::CleanupPlaceHasNoDroppableLeaf => "cleanup place has no droppable leaf",
            Self::FinalizerGuardDoesNotNameItsExactCleanupLeaf => {
                "finalizer guard does not name its exact cleanup leaf"
            }
            Self::FinalizerLifecycleOrGuardIsInvalidOrRepeated => {
                "finalizer lifecycle or guard is invalid or repeated"
            }
            Self::ConditionalFinalizerDoesNotBindItsExactVariantCasePath => {
                "conditional finalizer does not bind its exact variant case path"
            }
        }
    }
}
