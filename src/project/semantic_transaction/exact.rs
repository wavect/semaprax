//! Additive exact-context selection for Universal Semantic Transaction v1.
//!
//! Candidate ProgramRoot v2 derivation is deliberately absent: Project Lock
//! verification is snapshot-bound, so an in-memory candidate cannot yet
//! freshly replay every external fact without weakening admission.

use std::sync::Arc;

use super::super::ExactProgramContext;
use super::{SemanticTransaction, SemanticTransactionArtifacts};
use crate::diagnostic::Diagnostic;

impl SemanticTransaction {
    /// Select an exact enriched ProgramRoot v2 base, then run the unchanged v1
    /// transaction validation over its retained Project revision.
    pub fn validate_exact(
        &self,
        context: Arc<ExactProgramContext>,
        expected_workspace_revision: &str,
        expected_program_root_v2_digest: &str,
    ) -> Result<SemanticTransactionArtifacts, Vec<Diagnostic>> {
        let program_root_v2 = context
            .select(expected_workspace_revision, expected_program_root_v2_digest)?
            .clone();
        let mut artifacts = self.validate(Arc::clone(context.revision()))?;
        artifacts.base_program_root_v2 = Some(program_root_v2);
        Ok(artifacts)
    }
}
