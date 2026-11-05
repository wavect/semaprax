#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! Finite closed labels used only to bound owned diagnostic capacity.
//! They grant no source/budget authority and record no fabricated refusal.
use crate::bounded_output::ReservationStage;
pub(super) const STAGES: &[ReservationStage] = &[
    ReservationStage::ResolvedCore,
    ReservationStage::CompactValidation,
    ReservationStage::UncachedSyntheticPeak,
    ReservationStage::AuthoredModuleValidation,
    ReservationStage::FrontendLookup,
    ReservationStage::SelectiveResolution,
    ReservationStage::FrontendRetention,
    ReservationStage::ImportedVecInstances,
    ReservationStage::CorePrebound,
    ReservationStage::ExpectedEdges,
    ReservationStage::ResolvedCallEdges,
    ReservationStage::StubSignatures,
    ReservationStage::DeclarationFacts,
    ReservationStage::RetainedFacts,
    ReservationStage::CoreDirectories,
    ReservationStage::EdgeProof,
    ReservationStage::SyntheticModule,
    ReservationStage::CachedValidation,
    ReservationStage::FreshResolution,
    ReservationStage::HirLowering,
    ReservationStage::HirLoanPlans,
    ReservationStage::HirInstanceLoanPlans,
    ReservationStage::HirCleanupInventories,
    ReservationStage::HirInstanceCleanupInventories,
    ReservationStage::HirCleanupPlans,
    ReservationStage::HirYieldProfiles,
    ReservationStage::HirValidation,
    ReservationStage::CanonicalLoanStorage,
    ReservationStage::LoanCandidateInventory,
    ReservationStage::LoanCfgDiscovery,
    ReservationStage::LoanCfgPoints,
    ReservationStage::LoanCfgEdges,
    ReservationStage::LoanCanonicalAppend,
    ReservationStage::LoanOrderedDirectoryNode,
    ReservationStage::LoanVectorBacking,
    ReservationStage::RetainedModule,
    ReservationStage::LoanIdentityInventory,
    ReservationStage::RetainedLoanScratch,
    ReservationStage::RetainedLoanSidecars,
];
const _: () = {
    assert!(STAGES.len() == ReservationStage::RetainedLoanSidecars as usize + 1);
};
pub(super) fn longest() -> &'static str {
    STAGES
        .iter()
        .map(|stage| stage.label())
        .chain(["unknown"])
        .max_by_key(|label| label.len())
        .expect("unknown always present")
}
