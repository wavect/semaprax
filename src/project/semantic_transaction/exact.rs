//! Additive exact-context selection for Universal Semantic Transaction v1.
//!
//! Candidate ProgramRoot v2 derivation is deliberately absent: Project Lock
//! verification is snapshot-bound, so an in-memory candidate cannot yet
//! freshly replay every external fact without weakening admission.

use std::sync::Arc;

use super::super::ExactProgramContext;
use super::{
    capacity, stale, SemanticTransaction, SemanticTransactionArtifacts,
    MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES,
};
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

    /// Replay the frozen v1 transaction evidence only after selecting the
    /// exact enriched ProgramRoot v2 base. The wire artifacts remain v1 bytes;
    /// the selected root is retained only in the returned typed result.
    pub fn replay_exact(
        context: Arc<ExactProgramContext>,
        expected_workspace_revision: &str,
        expected_program_root_v2_digest: &str,
        transaction_bytes: &[u8],
        evidence_bytes: &[u8],
    ) -> Result<SemanticTransactionArtifacts, Vec<Diagnostic>> {
        context.select(expected_workspace_revision, expected_program_root_v2_digest)?;
        if evidence_bytes.len() > MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES {
            return Err(capacity(
                "semantic transaction evidence exceeds its byte limit",
            ));
        }
        let transaction = Self::from_json(transaction_bytes)?;
        let artifacts = transaction.validate_exact(
            context,
            expected_workspace_revision,
            expected_program_root_v2_digest,
        )?;
        if artifacts.evidence.as_bytes() != evidence_bytes {
            return Err(stale("semantic transaction evidence failed exact replay"));
        }
        Ok(artifacts)
    }
}
