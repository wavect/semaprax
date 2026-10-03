//! Builder-owned LAW-09 publication evidence. This stays distinct from a
//! core Project diagnostic and does not by itself satisfy protected LawSet.

use super::ProjectNativeRustSdkBundle;
use crate::diagnostic::Diagnostic;
use semaprax::project::{ForeignCallerCertificate, ProjectRevision};

fn mismatch(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-FL311", message)]
}

/// An exact conditional caller and guarded Project SDK publication retained
/// together. Only the guarded builder can populate the private bundle facts.
/// Foreign behavior remains the certificate's declared assumption.
#[derive(Clone, Debug)]
pub struct GuardedForeignCallerEvidence {
    bundle: ProjectNativeRustSdkBundle,
    caller: ForeignCallerCertificate,
}

impl ProjectNativeRustSdkBundle {
    pub fn bind_guarded_foreign_caller(
        &self,
        revision: &ProjectRevision,
        caller: ForeignCallerCertificate,
    ) -> Result<GuardedForeignCallerEvidence, Vec<Diagnostic>> {
        let Some(frontier) = &self.guarded_frontier else {
            return Err(mismatch(
                "Project SDK has no builder-retained foreign guard",
            ));
        };
        if frontier != caller.frontier()
            || self.project_revision != revision.project_revision()
            || self.workspace_revision != revision.workspace_revision()
            || self.sdk.manifest_digest() != caller.adapter_digest()
            || self.sdk.target_triple() != frontier.target()
        {
            return Err(mismatch(
                "guarded Project SDK publication differs from conditional caller",
            ));
        }
        caller.verify_published_guard(
            revision,
            self.sdk.output_directory(),
            self.sdk.manifest_digest(),
        )?;
        Ok(GuardedForeignCallerEvidence {
            bundle: self.clone(),
            caller,
        })
    }
}

impl GuardedForeignCallerEvidence {
    pub fn caller(&self) -> &ForeignCallerCertificate {
        &self.caller
    }

    pub fn manifest_digest(&self) -> &str {
        self.bundle.manifest_digest()
    }

    /// Recheck exact retained source and published package bytes at every
    /// consuming boundary; this grants no runtime or filesystem authority.
    pub fn replay(&self, revision: &ProjectRevision) -> Result<(), Vec<Diagnostic>> {
        let expected = self
            .bundle
            .bind_guarded_foreign_caller(revision, self.caller.clone())?;
        if expected.bundle != self.bundle || expected.caller != self.caller {
            return Err(mismatch("guarded foreign caller publication changed"));
        }
        Ok(())
    }
}
