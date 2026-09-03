//! Candidate-bound rebind of an exact base-image function reference.
//!
//! This is a pure navigation projection. It derives both images from the
//! immutable candidate, authenticates the exact base reference through the
//! ordinary image resolver, and returns the existing conservative rebind
//! report inside exact candidate provenance.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectSemanticImage, IMAGE_FUNCTION_REFERENCE_REBIND_SCHEMA,
    MAX_IMAGE_FUNCTION_REFERENCE_REBIND_BYTES,
};

use super::ProjectCandidate;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_SCHEMA: &str =
    "semaprax.project-candidate-function-reference-rebind.v1";
pub const MAX_PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_BYTES: usize =
    MAX_IMAGE_FUNCTION_REFERENCE_REBIND_BYTES + 128 * 1024;

const NONCLAIMS: &[&str] = &[
    "stable_identity_survival_is_not_semantic_or_behavioral_equivalence",
    "accepted_rebind_is_not_candidate_migration_or_approval",
    "destination_reference_still_requires_normal_exact_image_resolution",
    "no_source_execution_retention_or_publication_authority",
    "no_revision_ancestry_external_consumer_or_compatibility_claim",
];

impl ProjectCandidate {
    /// Rebind one canonical reference from this candidate's exact base image
    /// to its independently derived final image. The underlying image rebind
    /// remains the sole selector and provenance authority.
    pub fn rebind_function_reference(
        &self,
        expected_candidate: &str,
        reference_bytes: &[u8],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let base_image =
            ProjectSemanticImage::derive(Arc::clone(&self.base), self.base.project_revision())?;
        let candidate_image = ProjectSemanticImage::derive(
            Arc::clone(&self.revision),
            self.revision.project_revision(),
        )?;
        let report = candidate_image.rebind_function_reference(
            candidate_image.image_digest(),
            &base_image,
            base_image.image_digest(),
            reference_bytes,
        )?;
        let rebind: Value = serde_json::from_str(&report)
            .map_err(|_| invalid("function reference rebind report is not compiler JSON"))?;
        if rebind.get("schema").and_then(Value::as_str)
            != Some(IMAGE_FUNCTION_REFERENCE_REBIND_SCHEMA)
        {
            return Err(invalid(
                "function reference rebind report has an unexpected compiler schema",
            ));
        }
        if rebind
            .get("source_image")
            .and_then(|value| value.get("image_revision"))
            != Some(&json!(base_image.image_digest()))
            || rebind
                .get("destination_image")
                .and_then(|value| value.get("image_revision"))
                != Some(&json!(candidate_image.image_digest()))
        {
            return Err(invalid(
                "function reference rebind report is not bound to candidate images",
            ));
        }

        super::super::image::render(
            json!({
                "schema": PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_SCHEMA,
                "candidate_revision": self.candidate_digest(),
                "base_project_revision": self.base.project_revision(),
                "project_revision": self.revision.project_revision(),
                "base_workspace_revision": self.base.workspace_revision(),
                "workspace_revision": self.revision.workspace_revision(),
                "base_image_revision": base_image.image_digest(),
                "image_revision": candidate_image.image_digest(),
                "rebind": rebind,
                "verification": "exact_candidate_selection_and_independently_derived_base_and_final_images_then_base_reference_resolution_and_unique_explicit_destination_identity",
                "candidate_retained": false,
                "source_authority": false,
                "execution": false,
                "publication_authority": false,
                "nonclaims": NONCLAIMS,
            }),
            false,
            MAX_PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_BYTES,
        )
        .map_err(|_| capacity("candidate function reference rebind exceeds its byte bound"))
    }
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G490", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G491", message)]
}
