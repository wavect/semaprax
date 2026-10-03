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
pub struct CandidateExplorerView<'a> {
    candidate: &'a ProjectCandidate,
    side: ExplorerSide,
    image: &'a ProjectSemanticImage,
}

impl ProjectCandidate {
    pub fn explorer_view(
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

    pub fn summary(
        &self,
        mode: ExplorerMode,
        target: Option<&str>,
        query: ExplorerQuery,
    ) -> Result<String> {
        crate::project::semantic_explorer::summary(self.subject(), mode, target, query)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn page(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{with_authenticated_project, SemanticChange};
    use serde_json::{json, Value};

    fn open_candidate() -> ProjectCandidate {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/calculator-project/semaprax.toml");
        with_authenticated_project(&manifest, |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }

    #[test]
    fn explorer_images_are_reused_per_side_and_fresh_after_a_candidate_change() {
        let candidate = open_candidate();
        assert!(candidate.base_explorer_image_cache.get().is_none());
        assert!(candidate.candidate_explorer_image_cache.get().is_none());

        let first = candidate
            .explorer_view(candidate.candidate_digest(), ExplorerSide::Base)
            .unwrap();
        let first_image = first.image as *const ProjectSemanticImage;
        let query = ExplorerQuery::default();
        let summary: Value =
            serde_json::from_str(&first.summary(ExplorerMode::Overview, None, query).unwrap())
                .unwrap();
        let inventory = summary["inventories"].as_array().unwrap().first().unwrap();
        let view = ExplorerView::parse(inventory["view"].as_str().unwrap()).unwrap();
        first
            .page(
                ExplorerMode::Overview,
                None,
                query,
                view,
                inventory["handle"].as_str().unwrap(),
                None,
                ExplorerPageOptions::default(),
            )
            .unwrap();
        let repeated = candidate
            .explorer_view(candidate.candidate_digest(), ExplorerSide::Base)
            .unwrap();
        assert!(std::ptr::eq(first.image, repeated.image));
        assert_eq!(first_image, repeated.image as *const ProjectSemanticImage);
        assert!(candidate.base_explorer_image_cache.get().is_some());

        let candidate_side = candidate
            .explorer_view(candidate.candidate_digest(), ExplorerSide::Candidate)
            .unwrap();
        assert!(!std::ptr::eq(first.image, candidate_side.image));
        assert!(candidate.candidate_explorer_image_cache.get().is_some());
        assert!(candidate
            .explorer_view(candidate.candidate_digest(), ExplorerSide::Current)
            .is_err());

        let change = SemanticChange::new(
            candidate.revision().project_revision(),
            &json!({
                "kind": "rename_declaration",
                "target": "calculator.add",
                "name": "sum",
            }),
        )
        .unwrap();
        let changed = candidate
            .apply(candidate.candidate_digest(), &change)
            .unwrap();
        assert!(changed.base_explorer_image_cache.get().is_none());
        assert!(changed.candidate_explorer_image_cache.get().is_none());
        let changed_view = changed
            .explorer_view(changed.candidate_digest(), ExplorerSide::Candidate)
            .unwrap();
        assert_ne!(
            first_image, changed_view.image as *const ProjectSemanticImage,
            "a new immutable candidate must not inherit a predecessor image cache"
        );
    }
}
