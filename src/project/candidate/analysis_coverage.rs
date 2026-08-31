//! Explicit analysis boundaries over one fully admitted candidate revision.
//! The derived semantic image is invocation-local evidence only.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectSemanticImage, IMAGE_ANALYSIS_COVERAGE_SCHEMA, MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
};

use super::ProjectCandidate;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA: &str =
    "semaprax.project-candidate-analysis-coverage.v1";
pub const MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES: usize = MAX_IMAGE_ANALYSIS_COVERAGE_BYTES;

impl ProjectCandidate {
    /// Describe the retained facts and blind spots of this exact admitted
    /// candidate without retaining an image or granting source authority.
    pub fn analysis_coverage(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let image = ProjectSemanticImage::derive(
            Arc::clone(&self.revision),
            self.revision.project_revision(),
        )?;
        let report = image.analysis_coverage(image.image_digest())?;
        let mut value: Value = serde_json::from_str(&report)
            .map_err(|_| invalid("candidate analysis coverage report is not compiler JSON"))?;
        let object = value.as_object_mut().ok_or_else(|| {
            invalid("candidate analysis coverage report is not a compiler object")
        })?;
        if object.get("schema").and_then(Value::as_str) != Some(IMAGE_ANALYSIS_COVERAGE_SCHEMA) {
            return Err(invalid(
                "candidate analysis coverage report has an unexpected compiler schema",
            ));
        }
        object.insert(
            "schema".into(),
            json!(PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA),
        );
        object.insert("candidate_revision".into(), json!(self.candidate_digest()));
        object.insert(
            "base_project_revision".into(),
            json!(self.base.project_revision()),
        );
        object.insert("candidate_retained".into(), json!(false));
        object.insert("publication_authority".into(), json!(false));
        super::super::image::render(value, false, MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES)
            .map_err(|_| capacity("candidate analysis coverage report exceeds its byte bound"))
    }
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G219", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G220", message)]
}
