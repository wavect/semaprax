//! Closed boundary/region-chain failure data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundaryRefusal {
    ExecutionBoundariesHaveNoLexicalControlCertificate,
    ExecutionBoundariesDoNotCoverExactlyTheEntryAndEveryTypedPhaseLoop,
    PhaseLoopConditionRepeatsABooleanEdge,
    PhaseLoopLacksComplementaryBooleanEdges,
    PhaseLoopConditionCrossesItsExactProofDomain,
    ExecutionBoundaryDiffersFromItsTypedPhaseBlockOrCanonicalContour,
    ReturnPostconditionsHaveNoExactDomain,
    ReturnPostconditionsBelongToTheOrdinaryDomain,
    CleanupRegionTreeContainsACycle,
    CleanupRegionChainReferencesAnUnknownRegion,
}
impl BoundaryRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::ExecutionBoundariesHaveNoLexicalControlCertificate => {
                "execution boundaries have no lexical-control certificate"
            }
            Self::ExecutionBoundariesDoNotCoverExactlyTheEntryAndEveryTypedPhaseLoop => {
                "execution boundaries do not cover exactly the entry and every typed phase loop"
            }
            Self::PhaseLoopConditionRepeatsABooleanEdge => {
                "phase loop condition repeats a Boolean edge"
            }
            Self::PhaseLoopLacksComplementaryBooleanEdges => {
                "phase loop lacks complementary Boolean edges"
            }
            Self::PhaseLoopConditionCrossesItsExactProofDomain => {
                "phase loop condition crosses its exact proof domain"
            }
            Self::ExecutionBoundaryDiffersFromItsTypedPhaseBlockOrCanonicalContour => {
                "execution boundary differs from its typed phase, block or canonical contour"
            }
            Self::ReturnPostconditionsHaveNoExactDomain => {
                "return postconditions have no exact domain"
            }
            Self::ReturnPostconditionsBelongToTheOrdinaryDomain => {
                "return postconditions belong to the ordinary domain"
            }
            Self::CleanupRegionTreeContainsACycle => "cleanup region tree contains a cycle",
            Self::CleanupRegionChainReferencesAnUnknownRegion => {
                "cleanup region chain references an unknown region"
            }
        }
    }
}
