//! Candidate-bound explorer projection over either retained candidate side.
use super::ProjectCandidate;
use crate::project::{
    ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerSide, ExplorerView,
    ProjectSemanticImage,
};

type Result<T> = std::result::Result<T, Vec<crate::diagnostic::Diagnostic>>;

/// One exact candidate side retained for a bounded explorer request.
///
/// The image owns the derived index and is intentionally held across the
/// summary and its pages. Transport entry points still create a fresh view for
/// each independent request; callers that assemble a complete offline report
/// can retain this view for the report's selected side.
pub(crate) struct CandidateExplorerView<'a> {
    candidate: &'a ProjectCandidate,
    side: ExplorerSide,
    image: &'a ProjectSemanticImage,
}

impl ProjectCandidate {
    pub(crate) fn explorer_view(
        &self,
        expected_candidate: &str,
        side: ExplorerSide,
    ) -> Result<CandidateExplorerView<'_>> {
        self.require_candidate(expected_candidate)?;
        let image = match side {
            ExplorerSide::Base => self
                .base_explorer_image_cache
                .get_or_init(|| {
                    ProjectSemanticImage::derive(
                        std::sync::Arc::clone(&self.base),
                        self.base.project_revision(),
                    )
                })
                .as_ref()
                .map_err(Clone::clone)?,
            ExplorerSide::Candidate => self
                .candidate_explorer_image_cache
                .get_or_init(|| {
                    ProjectSemanticImage::derive(
                        std::sync::Arc::clone(&self.revision),
                        self.revision.project_revision(),
                    )
                })
                .as_ref()
                .map_err(Clone::clone)?,
            ExplorerSide::Current => {
                return Err(vec![crate::diagnostic::Diagnostic::io(
                    "SPX-G326",
                    "candidate explorer requires base or candidate side",
                )])
            }
        };
        Ok(CandidateExplorerView {
            candidate: self,
            side,
            image,
        })
    }

    pub fn explorer_summary(
        &self,
        expected_candidate: &str,
        side: ExplorerSide,
        mode: ExplorerMode,
        target: Option<&str>,
        query: ExplorerQuery,
    ) -> Result<String> {
        self.explorer_view(expected_candidate, side)?
            .summary(mode, target, query)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn explorer_page(
        &self,
        expected_candidate: &str,
        side: ExplorerSide,
        mode: ExplorerMode,
        target: Option<&str>,
        query: ExplorerQuery,
        view: ExplorerView,
        handle: &str,
        cursor: Option<&str>,
        options: ExplorerPageOptions,
    ) -> Result<String> {
        self.explorer_view(expected_candidate, side)?
            .page(mode, target, query, view, handle, cursor, options)
    }
}

impl CandidateExplorerView<'_> {
    fn subject(&self) -> crate::project::semantic_explorer::ExplorerSubject<'_> {
        crate::project::semantic_explorer::ExplorerSubject {
            image_digest: self.image.image_digest(),
            candidate_digest: Some(self.candidate.candidate_digest()),
            side: self.side,
            revision: self.image.revision(),
        }
    }

    pub(crate) fn summary(
        &self,
        mode: ExplorerMode,
        target: Option<&str>,
        query: ExplorerQuery,
    ) -> Result<String> {
        crate::project::semantic_explorer::summary(self.subject(), mode, target, query)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn page(
        &self,
        mode: ExplorerMode,
        target: Option<&str>,
        query: ExplorerQuery,
        view: ExplorerView,
        handle: &str,
        cursor: Option<&str>,
        options: ExplorerPageOptions,
    ) -> Result<String> {
        crate::project::semantic_explorer::page(
            self.subject(),
            mode,
            target,
            query,
            view,
            handle,
            cursor,
            options,
        )
    }
}
