//! Closed value-only dispatch over immutable subjects selected by the host.
use super::*;
use candidates::reads::ReadSubjects;

pub(super) fn supports(operation: Operation) -> bool {
    use candidates::diagnostics::Action as D;
    matches!(
        operation,
        Operation::Candidate(candidates::Action::Diagnostic(
            D::Summary
                | D::Query
                | D::RepairCatalog
                | D::Delta
                | D::DeltaCatalog
                | D::ProtocolConformance
                | D::InterfaceCatalog
        )) | Operation::VNext(
            Action::Targets
                | Action::InterfaceDelta
                | Action::ContractDelta
                | Action::OwnershipDelta
                | Action::SourceReview
                | Action::CandidateMergePreview
                | Action::CandidateAnalysisCoverage
                | Action::CandidateDependencySummary
                | Action::CandidateDependencyPage
                | Action::CandidateFunctionSummary
                | Action::CandidateFunctionFacet
                | Action::CandidateImpactSummary
                | Action::CandidateImpactPage
                | Action::FunctionReferenceExport
                | Action::FunctionReferenceResolve
                | Action::HoleSummary
                | Action::HolePage
                | Action::HoleFillSuggestions
                | Action::DraftExpressionCatalog
                | Action::CandidateCleanupDependencies
                | Action::ContractExpressionCatalog
                | Action::DraftRecoveryExport
                | Action::DraftArchiveExport
                | Action::SymbolDiagnostics
        )
    )
}

pub(super) fn prepare(
    operation: Operation,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    subjects: &ReadSubjects,
) -> Result<Value, Vec<Diagnostic>> {
    // The normal frame and batch coordinators check this before subject lookup.
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure("SPX-G282", "v5 expected image revision is stale"));
    }
    let candidate = || {
        subjects.candidate.as_deref().ok_or_else(|| {
            failure(
                "SPX-G224",
                "candidate handle is stale, discarded, or unknown",
            )
        })
    };
    let draft = || {
        subjects
            .draft
            .as_deref()
            .ok_or_else(|| failure("SPX-G232", "draft handle is stale, discarded, or unknown"))
    };
    match operation {
        Operation::VNext(Action::Targets) => projections::target_for_image(params, image),
        Operation::Candidate(candidates::Action::Diagnostic(action)) => {
            candidates::diagnostics::read_payload(action, params, image, subjects)
        }
        Operation::VNext(Action::InterfaceDelta) => {
            review_facets::interface_delta_for_candidate(params, image, candidate()?)
        }
        Operation::VNext(Action::ContractDelta) => {
            review_facets::contract_delta_for_candidate(params, image, candidate()?)
        }
        Operation::VNext(Action::OwnershipDelta) => {
            review_facets::ownership_delta_for_candidate(params, image, candidate()?)
        }
        Operation::VNext(Action::SourceReview) => {
            source_review::for_candidate(params, image, candidate()?)
        }
        Operation::VNext(Action::CandidateMergePreview) => {
            let candidate = candidate()?;
            let other = subjects.other.as_deref().ok_or_else(|| {
                failure(
                    "SPX-G224",
                    "other candidate handle is stale, discarded, or unknown",
                )
            })?;
            merge_preview::for_candidates(params, candidate, other)
        }
        Operation::VNext(Action::CandidateAnalysisCoverage) => {
            analysis_coverage::for_candidate(params, candidate()?)
        }
        Operation::VNext(
            action @ (Action::FunctionReferenceExport | Action::FunctionReferenceResolve),
        ) => function_reference::prepare(action, params, image),
        Operation::VNext(
            action @ (Action::HoleSummary | Action::HolePage | Action::DraftExpressionCatalog),
        ) => hole_navigation::for_draft(action, params, draft()?),
        Operation::VNext(Action::CandidateCleanupDependencies) => {
            cleanup_dependencies::for_candidate(params, image, candidate()?)
        }
        Operation::VNext(
            action @ (Action::CandidateDependencySummary | Action::CandidateDependencyPage),
        ) => candidate_dependency_navigation::for_candidate(action, params, candidate()?),
        Operation::VNext(
            action @ (Action::CandidateFunctionSummary | Action::CandidateFunctionFacet),
        ) => candidate_function_facets::for_candidate(action, params, candidate()?),
        Operation::VNext(
            action @ (Action::CandidateImpactSummary | Action::CandidateImpactPage),
        ) => candidate_impact_navigation::for_candidate(action, params, candidate()?),
        Operation::VNext(Action::HoleFillSuggestions) => {
            hole_suggestions::for_draft(params, draft()?)
        }
        Operation::VNext(Action::ContractExpressionCatalog) => {
            contract_holes::catalog_for_candidate(params, candidate()?)
        }
        Operation::VNext(Action::DraftRecoveryExport) => {
            draft_recovery::export_for_draft(params, draft()?)
        }
        Operation::VNext(Action::DraftArchiveExport) => {
            draft_archive::export_for_draft(params, image, draft()?)
        }
        Operation::VNext(Action::SymbolDiagnostics) => {
            symbol_diagnostics::for_subjects(params, image, subjects)
        }
        _ => Err(failure(
            "SPX-G294",
            "operation is not a detached immutable report",
        )),
    }
}
