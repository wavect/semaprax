//! Opt-in host boundary for protected law review and publication.
use super::{publication, ProjectCandidate};
use crate::assurance_manifest::law_set::{
    protected::{ProtectedLawBaseline, ProtectedLawReview, SpecificationChangeApproval},
    LawSet,
};
use crate::diagnostic::Diagnostic;
use std::path::Path;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectCandidate {
    /// Read-only law/assumption/evidence and transitive specification delta.
    pub fn protected_law_review(
        &self,
        baseline: &ProtectedLawBaseline,
        laws: &LawSet,
    ) -> Result<ProtectedLawReview> {
        baseline.review(
            self.base_revision(),
            self.revision(),
            laws,
            self.candidate_digest(),
        )
    }
}

/// The host must select this route whenever it configures a protected baseline.
/// Recheck after ordinary publication authority/lock acquisition and before
/// staging. Ordinary publication evidence carries no specification authority.
#[allow(clippy::too_many_arguments)]
pub fn apply_protected_law_publication(
    candidate: &ProjectCandidate,
    baseline: &ProtectedLawBaseline,
    laws: &LawSet,
    approval: Option<&SpecificationChangeApproval>,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
) -> Result<String> {
    publication::apply_with_law_gate(
        candidate,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        submitted_publication,
        || {
            candidate
                .protected_law_review(baseline, laws)?
                .require(approval)
        },
    )
}
