//! Candidate-bound explorer projection over either retained candidate side.
use super::ProjectCandidate;
use crate::project::{
    ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerSide, ExplorerView,
    ProjectSemanticImage,
};

type Result<T> = std::result::Result<T, Vec<crate::diagnostic::Diagnostic>>;

impl ProjectCandidate {
    pub fn explorer_summary(
        &self,
        expected_candidate: &str,
        side: ExplorerSide,
        mode: ExplorerMode,
        target: Option<&str>,
        query: ExplorerQuery,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let revision = match side {
            ExplorerSide::Base => &self.base,
            ExplorerSide::Candidate => &self.revision,
            ExplorerSide::Current => {
                return Err(vec![crate::diagnostic::Diagnostic::io(
                    "SPX-G326",
                    "candidate explorer requires base or candidate side",
                )])
            }
        };
        let image = ProjectSemanticImage::derive(
            std::sync::Arc::clone(revision),
            revision.project_revision(),
        )?;
        crate::project::semantic_explorer::summary(
            crate::project::semantic_explorer::ExplorerSubject {
                image_digest: image.image_digest(),
                candidate_digest: Some(self.candidate_digest()),
                side,
                revision,
            },
            mode,
            target,
            query,
        )
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
        self.require_candidate(expected_candidate)?;
        let revision = match side {
            ExplorerSide::Base => &self.base,
            ExplorerSide::Candidate => &self.revision,
            ExplorerSide::Current => {
                return Err(vec![crate::diagnostic::Diagnostic::io(
                    "SPX-G326",
                    "candidate explorer requires base or candidate side",
                )])
            }
        };
        let image = ProjectSemanticImage::derive(
            std::sync::Arc::clone(revision),
            revision.project_revision(),
        )?;
        crate::project::semantic_explorer::page(
            crate::project::semantic_explorer::ExplorerSubject {
                image_digest: image.image_digest(),
                candidate_digest: Some(self.candidate_digest()),
                side,
                revision,
            },
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
