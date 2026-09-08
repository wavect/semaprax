//! Runtime producers selected from one immutable semantic service generation.
//!
//! Receipts describe retained facts. Replaying a receipt reselects compiler
//! state; it cannot import a root, invocation, observation, or host authority.

use std::sync::Arc;

use crate::diagnostic::Diagnostic;
use crate::execution_revision::{ExecutionRoot, ProgramRootRef};
use serde_json::json;

use super::{ProjectRevision, SemanticWorkspaceService, SemanticWorkspaceSnapshot};

mod runtime;
pub use runtime::{
    WorkspaceExecution, WorkspaceExecutionEvidence, WORKSPACE_RUNTIME_ASSOCIATION_SCHEMA,
    WORKSPACE_RUNTIME_EVIDENCE_SCHEMA,
};

pub const WORKSPACE_EXECUTION_BINDING_SCHEMA: &str = "semaprax.workspace-execution-binding.v1";
pub const MAX_WORKSPACE_EXECUTION_BINDING_BYTES: usize = 16_384;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Selects an existing root family without accepting caller-supplied root data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceExecutionRootVersion {
    V1,
    V2,
    V3,
}

impl WorkspaceExecutionRootVersion {
    fn schema(self) -> &'static str {
        match self {
            Self::V1 => "semaprax.program-root.v1",
            Self::V2 => "semaprax.program-root.v2",
            Self::V3 => "semaprax.program-root.v3",
        }
    }
}

/// A retained generation and exact root choice. Clone shares immutable state;
/// creating or cloning this value does not invoke a handler or open a store.
#[derive(Clone)]
pub struct WorkspaceExecutionBinding {
    snapshot: SemanticWorkspaceSnapshot,
    version: WorkspaceExecutionRootVersion,
    workspace_revision: String,
    association: ExecutionRoot,
}

impl WorkspaceExecutionBinding {
    pub fn select(
        service: &SemanticWorkspaceService,
        version: WorkspaceExecutionRootVersion,
        expected_workspace_revision: &str,
        expected_program_root_digest: &str,
    ) -> Result<Self> {
        let snapshot =
            match version {
                WorkspaceExecutionRootVersion::V1 => service.snapshot(expected_workspace_revision),
                WorkspaceExecutionRootVersion::V2 => service
                    .snapshot_exact(expected_workspace_revision, expected_program_root_digest),
                WorkspaceExecutionRootVersion::V3 => service
                    .snapshot_exact_v2(expected_workspace_revision, expected_program_root_digest),
            }
            .map_err(|_| refused("workspace or ProgramRoot selection is not current"))?;
        let program = selected_root(&snapshot, version);
        if program.digest() != expected_program_root_digest {
            return Err(refused("workspace ProgramRoot digest differs"));
        }
        let association = associate(
            WORKSPACE_EXECUTION_BINDING_SCHEMA,
            json!({
                "authority": false,
                "project_revision": snapshot.generation().revision().project_revision(),
                "base_workspace_revision": snapshot.workspace_revision(),
                "selected_workspace_revision": expected_workspace_revision,
                "program_root_schema": version.schema(),
                "program_root": program.digest(),
                "semantic_image": snapshot.generation().image().image_digest(),
            }),
        );
        if association.canonical_json().len() > MAX_WORKSPACE_EXECUTION_BINDING_BYTES {
            return Err(refused("workspace execution receipt capacity"));
        }
        Ok(Self {
            snapshot,
            version,
            workspace_revision: expected_workspace_revision.to_owned(),
            association,
        })
    }

    /// Reselect current retained state and exact-compare its independently
    /// produced receipt. Even a self-consistent remint remains untrusted input.
    pub fn replay(
        service: &SemanticWorkspaceService,
        version: WorkspaceExecutionRootVersion,
        expected_workspace_revision: &str,
        expected_program_root_digest: &str,
        expected_association_digest: &str,
        receipt: &str,
    ) -> Result<Self> {
        if receipt.len() > MAX_WORKSPACE_EXECUTION_BINDING_BYTES {
            return Err(refused("workspace execution receipt capacity"));
        }
        let binding = Self::select(
            service,
            version,
            expected_workspace_revision,
            expected_program_root_digest,
        )?;
        if binding.association.digest() != expected_association_digest
            || binding.association.canonical_json() != receipt
        {
            return Err(refused(
                "workspace execution receipt differs from retained generation",
            ));
        }
        Ok(binding)
    }

    pub fn association_root(&self) -> &ExecutionRoot {
        &self.association
    }

    pub fn root_version(&self) -> WorkspaceExecutionRootVersion {
        self.version
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn image_digest(&self) -> &str {
        self.snapshot.generation().image().image_digest()
    }

    pub fn project_revision(&self) -> &Arc<ProjectRevision> {
        self.snapshot.generation().revision()
    }

    pub fn program_root(&self) -> ProgramRootRef<'_> {
        selected_root(&self.snapshot, self.version)
    }

    /// Historical bindings remain valid, but cannot be described as current
    /// after this service adopts a different selected generation.
    pub fn require_current(&self, service: &SemanticWorkspaceService) -> Result<()> {
        let current = Self::select(
            service,
            self.version,
            self.workspace_revision(),
            self.program_root().digest(),
        )?;
        if current.association != self.association {
            return Err(refused("workspace execution generation differs"));
        }
        Ok(())
    }
}

fn selected_root(
    snapshot: &SemanticWorkspaceSnapshot,
    version: WorkspaceExecutionRootVersion,
) -> ProgramRootRef<'_> {
    match version {
        WorkspaceExecutionRootVersion::V1 => ProgramRootRef::V1(snapshot.program_root()),
        WorkspaceExecutionRootVersion::V2 => ProgramRootRef::V2(
            snapshot
                .program_root_v2()
                .expect("exact v2 snapshot retained its root"),
        ),
        WorkspaceExecutionRootVersion::V3 => ProgramRootRef::V3(
            snapshot
                .program_root_v3()
                .expect("exact v3 snapshot retained its root"),
        ),
    }
}

pub(super) fn associate(schema: &str, facts: serde_json::Value) -> ExecutionRoot {
    crate::execution_revision::root(schema, facts)
}

pub(super) fn refused(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G583",
        format!("Workspace execution association rejected: {detail}"),
    )]
}
