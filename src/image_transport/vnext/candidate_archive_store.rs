//! Startup-selected persistence of one already retained complete candidate.
//!
//! Request bytes select only the exact retained candidate identity. The store
//! root is held before frames, and an optional retention checkpoint receives
//! only the typed receipt after immutable archive publication succeeds.

use std::path::Path;

use serde_json::{json, Value};

use crate::candidate_archive_store::{CandidateArchiveStore, CandidateArchiveStoreReceipt};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectCandidateArchive;
use crate::project_transport::codec::RequestId;
use crate::semantic_retention_lifecycle::SuccessfulRetentionReceipt;

use super::{
    failure, response, text, Action, Method, Operation, Parameter, ParameterKind, VNextSession,
    REVISION,
};

#[path = "candidate_archive_store_schemas.rs"]
mod schemas;

pub(super) fn schema_documents(capabilities: &Value) -> std::collections::BTreeMap<String, Value> {
    schemas::documents(capabilities)
}

pub const IMAGE_CANDIDATE_ARCHIVE_STORE_SCHEMA: &str = "semaprax.image-candidate-archive-store.v1";

const METHOD: Method = Method {
    name: "candidate/archive-store",
    operation: Operation::VNext(Action::CandidateArchiveStore),
    parameters: &[
        REVISION,
        Parameter {
            name: "candidate_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: false,
    payload_schema: IMAGE_CANDIDATE_ARCHIVE_STORE_SCHEMA,
};

pub(super) const fn method() -> &'static Method {
    &METHOD
}

impl VNextSession {
    /// Select and hold one explicit private immutable archive store before any
    /// frame. Requests never carry or replace its path.
    pub fn with_candidate_archive_store(mut self, root: &Path) -> Result<Self, Vec<Diagnostic>> {
        if self.started
            || self.terminal
            || self.package_attachment_closed
            || self.candidate_archive_store.is_some()
            || !self.policy.candidate_prepare
        {
            return Err(failure(
                "SPX-G500",
                "candidate archive store must be selected once before protocol input with candidate preparation enabled",
            ));
        }
        self.candidate_archive_store = Some(CandidateArchiveStore::open(root)?);
        Ok(self)
    }

    pub(super) fn candidate_archive_store_request(
        &mut self,
        id: &RequestId,
        params: &serde_json::Map<String, Value>,
    ) -> Vec<u8> {
        let prepared = self.snapshot.with_authenticated_request(|_| {
            let candidate = self
                .registry
                .candidate(text(params, "candidate_revision"))?;
            ProjectCandidateArchive::prepare(candidate, text(params, "candidate_revision"))
        });
        let archive = match prepared {
            Ok(archive) => archive,
            Err(errors) => return super::error_response(id, &errors),
        };
        let receipt = match self
            .candidate_archive_store
            .as_ref()
            .expect("method is selected only with its startup store")
            .persist(&archive)
        {
            Ok(receipt) => receipt,
            Err(errors) => return super::error_response(id, &errors),
        };

        let retention = self.checkpoint_candidate_archive_receipt(&receipt);
        let payload = json!({
            "schema":IMAGE_CANDIDATE_ARCHIVE_STORE_SCHEMA,
            "image_revision":self.image.image_digest(),
            "candidate_revision":receipt.candidate_digest(),
            "archive_digest":receipt.archive_digest(),
            "base_project_revision":receipt.base_revision(),
            "stored_bytes":receipt.stored_bytes(),
            "store_status":"immutable_archive_stored",
            "retention_lifecycle":retention,
            "source_authority":false,
            "approval_authority":false,
            "publication_authority":false,
            "restore_authority":false,
            "gc_authority":false,
            "nonclaims":[
                "archive_store_success_does_not_make_the_candidate_current",
                "retention_checkpoint_failure_does_not_undo_or_deny_archive_store_success",
                "request_contains_no_store_or_registry_path_policy_or_authority",
                "no_restore_delete_gc_approval_source_write_or_publication_operation",
            ],
        });
        response(id, &self.image, payload)
    }

    fn checkpoint_candidate_archive_receipt(
        &mut self,
        receipt: &CandidateArchiveStoreReceipt,
    ) -> Value {
        let Some(coordinator) = self.retention_lifecycle.as_mut() else {
            return json!({
                "selected":false,
                "outcome":null,
                "status":"not_selected_before_frames",
            });
        };
        let receipts = [SuccessfulRetentionReceipt::Candidate(receipt)];
        self.retention_lifecycle_outcome = Some(coordinator.checkpoint(&receipts));
        let outcome = self
            .retention_lifecycle_outcome
            .as_ref()
            .expect("retention outcome stored immediately before projection");
        let value: Value = serde_json::from_str(outcome.to_json())
            .expect("retention lifecycle always retains canonical JSON");
        json!({
            "selected":true,
            "outcome":value,
            "status":"checkpoint_outcome_returned",
        })
    }
}
