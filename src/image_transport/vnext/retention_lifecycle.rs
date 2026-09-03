//! Startup-selected retention accounting for successful out-of-band stores.
//!
//! V5 currently creates no immutable image/candidate/draft store receipt. The
//! embedding host therefore supplies only typed receipts returned by its own
//! successful store calls; request bytes cannot select a root or trigger this
//! route.

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::semantic_retention::RetentionPolicy;
use crate::semantic_retention_lifecycle::{
    RetentionLifecycleCoordinator, RetentionLifecycleOutcome, SuccessfulRetentionReceipt,
};

use super::{failure, VNextSession};

impl VNextSession {
    /// Attach one explicit private registry before accepting protocol input.
    /// The coordinator retains its authenticated root identity for the complete
    /// session lifetime; neither requests nor successful store receipts carry a
    /// path or policy.
    pub fn with_retention_lifecycle(
        mut self,
        root: &Path,
        policy: RetentionPolicy,
        expected_cursor: Option<&str>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if self.started
            || self.terminal
            || self.package_attachment_closed
            || self.retention_lifecycle.is_some()
        {
            return Err(failure(
                "SPX-G280",
                "retention lifecycle must be selected once before protocol input",
            ));
        }
        let lifecycle = RetentionLifecycleCoordinator::open(root, policy, expected_cursor)?;
        if self
            .candidate_archive_store
            .as_ref()
            .is_some_and(|store| store.held_root_identity() == lifecycle.held_root_identity())
        {
            return Err(failure(
                "SPX-G500",
                "candidate archive store and retention registry must hold distinct root identities",
            ));
        }
        self.retention_lifecycle = Some(lifecycle);
        Ok(self)
    }

    /// Account for a bounded batch of typed receipts after their immutable store
    /// operations have already succeeded. The returned outcome always keeps the
    /// store fact separate from registry stale/uncertain/poisoned status. This
    /// method never retries, rolls back, deletes, restores, or makes a subject
    /// current.
    pub fn checkpoint_successful_retention_receipts<'a>(
        &'a mut self,
        receipts: &[SuccessfulRetentionReceipt<'_>],
    ) -> Result<&'a RetentionLifecycleOutcome, Vec<Diagnostic>> {
        let coordinator = self.retention_lifecycle.as_mut().ok_or_else(|| {
            failure(
                "SPX-G280",
                "successful store receipt remains valid but this session has no retention lifecycle",
            )
        })?;
        self.retention_lifecycle_outcome = Some(coordinator.checkpoint(receipts));
        Ok(self
            .retention_lifecycle_outcome
            .as_ref()
            .expect("retention outcome stored immediately before borrowing"))
    }

    /// Latest explicit accountability outcome for this session. A report is not
    /// a subject-store receipt, GC approval, freshness observation, or authority.
    pub fn retention_lifecycle_outcome(&self) -> Option<&RetentionLifecycleOutcome> {
        self.retention_lifecycle_outcome.as_ref()
    }
}
