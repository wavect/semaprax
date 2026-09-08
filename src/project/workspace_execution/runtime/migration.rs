//! Consuming cross-generation migration associations over checked producers.
use super::super::{associate, refused};
use super::*;
use crate::agent_runtime_v2::{
    migrate_suspended_agent_runtime_v2, resume_migrated_agent_runtime_v2,
    AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure, DurableMigrationFailure,
    MigratedAgentRuntimeV2, ResumedMigratedAgentRuntimeV2,
};

pub const WORKSPACE_MIGRATION_ASSOCIATION_SCHEMA: &str =
    "semaprax.workspace-migration-association.v1";
pub const WORKSPACE_MIGRATION_EVIDENCE_SCHEMA: &str = "semaprax.workspace-migration-evidence.v1";
pub const MAX_WORKSPACE_MIGRATION_RECEIPT_BYTES: usize = 16_384;
type MigrationResult<T> = std::result::Result<T, WorkspaceMigrationFailure>;

#[derive(Debug)]
pub enum WorkspaceMigrationFailure {
    Association(Vec<Diagnostic>),
    Runtime(AgentRuntimeV2MigrationFailure),
    Durable(DurableMigrationFailure),
}
impl WorkspaceMigrationFailure {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Association(value) => value,
            Self::Runtime(value) => value.diagnostics(),
            Self::Durable(value) => value.diagnostics(),
        }
    }
}

/// Only durable producer evidence can supply a migration seed. The underlying
/// migration verifies its actual Suspend status and all retained cumulative work.
pub enum WorkspaceSuspensionEvidence {
    Ordinary(WorkspaceExecutionEvidence<AgentRuntimeV2DurableEvidence>),
    Migrated(WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>),
}
impl From<WorkspaceExecutionEvidence<AgentRuntimeV2DurableEvidence>>
    for WorkspaceSuspensionEvidence
{
    fn from(value: WorkspaceExecutionEvidence<AgentRuntimeV2DurableEvidence>) -> Self {
        Self::Ordinary(value)
    }
}
impl From<WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>>
    for WorkspaceSuspensionEvidence
{
    fn from(value: WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>) -> Self {
        Self::Migrated(value)
    }
}

struct Provenance {
    previous_binding: WorkspaceExecutionBinding,
    destination_binding: WorkspaceExecutionBinding,
    previous_execution: ExecutionRoot,
    destination_execution: ExecutionRoot,
}
/// Owns the real migrated runtime. There is no public construction or runtime
/// extraction path and a recovered runtime can only execute durably.
pub struct WorkspaceMigrationExecution<R> {
    provenance: Provenance,
    runtime: R,
    association: ExecutionRoot,
}
impl<R> WorkspaceMigrationExecution<R> {
    pub fn association_root(&self) -> &ExecutionRoot {
        &self.association
    }
    pub fn previous_binding(&self) -> &WorkspaceExecutionBinding {
        &self.provenance.previous_binding
    }
    pub fn destination_binding(&self) -> &WorkspaceExecutionBinding {
        &self.provenance.destination_binding
    }
    pub fn require_current(&self, service: &SemanticWorkspaceService) -> MigrationResult<()> {
        self.provenance
            .destination_binding
            .require_current(service)
            .map_err(WorkspaceMigrationFailure::Association)
    }
}
pub struct WorkspaceMigrationEvidence<E> {
    provenance: Provenance,
    migration_association: ExecutionRoot,
    evidence: E,
    association: ExecutionRoot,
}
impl<E> WorkspaceMigrationEvidence<E> {
    pub fn evidence(&self) -> &E {
        &self.evidence
    }
    pub fn association_root(&self) -> &ExecutionRoot {
        &self.association
    }
    pub fn migration_association_root(&self) -> &ExecutionRoot {
        &self.migration_association
    }
    pub fn destination_binding(&self) -> &WorkspaceExecutionBinding {
        &self.provenance.destination_binding
    }
    pub fn into_evidence(self) -> E {
        self.evidence
    }
}

fn receipt(provenance: &Provenance, migration: &ExecutionRoot) -> ExecutionRoot {
    associate(
        WORKSPACE_MIGRATION_ASSOCIATION_SCHEMA,
        json!({
            "previous_workspace_binding": provenance.previous_binding.association_root().digest(),
            "destination_workspace_binding": provenance.destination_binding.association_root().digest(),
            "previous_workspace_execution": provenance.previous_execution.digest(),
            "destination_workspace_execution": provenance.destination_execution.digest(),
            "migration_root": migration.digest(),
        }),
    )
}
fn finish<E>(
    provenance: Provenance,
    migration_association: ExecutionRoot,
    evidence_root: &ExecutionRoot,
    evidence: E,
) -> WorkspaceMigrationEvidence<E> {
    let association = associate(
        WORKSPACE_MIGRATION_EVIDENCE_SCHEMA,
        json!({
            "workspace_migration": migration_association.digest(),
            "evidence_root": evidence_root.digest(),
        }),
    );
    WorkspaceMigrationEvidence {
        provenance,
        migration_association,
        evidence,
        association,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_workspace_migration(
    previous: WorkspaceExecution<AgentRuntimeV2>,
    suspended: WorkspaceSuspensionEvidence,
    destination: WorkspaceExecution<AgentRuntimeV2>,
    migration_function: &str,
    max_steps: usize,
    max_fuel: u64,
) -> MigrationResult<WorkspaceMigrationExecution<MigratedAgentRuntimeV2>> {
    let (binding, execution) = match &suspended {
        WorkspaceSuspensionEvidence::Ordinary(evidence) => {
            (&evidence.binding, &evidence.execution_association)
        }
        WorkspaceSuspensionEvidence::Migrated(evidence) => (
            &evidence.provenance.destination_binding,
            &evidence.provenance.destination_execution,
        ),
    };
    if binding.association_root() != previous.binding.association_root()
        || execution != &previous.association
    {
        return Err(WorkspaceMigrationFailure::Association(refused(
            "migration suspension provenance differs from previous workspace execution",
        )));
    }
    let suspended = match suspended {
        WorkspaceSuspensionEvidence::Ordinary(evidence) => evidence.evidence,
        WorkspaceSuspensionEvidence::Migrated(evidence) => evidence.evidence,
    };
    let previous_revision = previous.runtime.execution_revision().digest().to_owned();
    let destination_revision = destination.runtime.execution_revision().digest().to_owned();
    let provenance = Provenance {
        previous_binding: previous.binding,
        destination_binding: destination.binding,
        previous_execution: previous.association,
        destination_execution: destination.association,
    };
    let runtime = migrate_suspended_agent_runtime_v2(
        previous.runtime,
        suspended,
        destination.runtime,
        &previous_revision,
        &destination_revision,
        migration_function,
        max_steps,
        max_fuel,
    )
    .map_err(WorkspaceMigrationFailure::Runtime)?;
    let association = receipt(&provenance, runtime.migration_root());
    Ok(WorkspaceMigrationExecution {
        provenance,
        runtime,
        association,
    })
}

/// `trusted_snapshot` and expected handoff digest retain the underlying trusted
/// single-writer store contract. Receipt fields are never parsed as authority.
#[allow(clippy::too_many_arguments)]
pub fn resume_workspace_migration(
    previous: WorkspaceExecution<AgentRuntimeV2>,
    destination: WorkspaceExecution<AgentRuntimeV2>,
    trusted_snapshot: &str,
    expected_handoff_digest: &str,
    expected_association_digest: &str,
    submitted_receipt: &str,
) -> MigrationResult<WorkspaceMigrationExecution<ResumedMigratedAgentRuntimeV2>> {
    if submitted_receipt.len() > MAX_WORKSPACE_MIGRATION_RECEIPT_BYTES {
        return Err(WorkspaceMigrationFailure::Association(refused(
            "migration receipt capacity",
        )));
    }
    let previous_revision = previous.runtime.execution_revision().digest().to_owned();
    let destination_revision = destination.runtime.execution_revision().digest().to_owned();
    let provenance = Provenance {
        previous_binding: previous.binding,
        destination_binding: destination.binding,
        previous_execution: previous.association,
        destination_execution: destination.association,
    };
    let runtime = resume_migrated_agent_runtime_v2(
        previous.runtime,
        destination.runtime,
        trusted_snapshot,
        expected_handoff_digest,
        &previous_revision,
        &destination_revision,
    )
    .map_err(WorkspaceMigrationFailure::Association)?;
    let association = receipt(&provenance, runtime.migration_root());
    if association.digest() != expected_association_digest
        || association.canonical_json() != submitted_receipt
    {
        return Err(WorkspaceMigrationFailure::Association(refused(
            "migration receipt differs from checked workspace producers",
        )));
    }
    Ok(WorkspaceMigrationExecution {
        provenance,
        runtime,
        association,
    })
}

impl WorkspaceMigrationExecution<MigratedAgentRuntimeV2> {
    pub fn handoff_digest(&self) -> MigrationResult<String> {
        self.runtime
            .handoff_digest()
            .map_err(WorkspaceMigrationFailure::Association)
    }
    pub fn run(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> MigrationResult<WorkspaceMigrationEvidence<AgentRuntimeV2MigrationEvidence>> {
        let evidence = self
            .runtime
            .run(handler, cancellation)
            .map_err(WorkspaceMigrationFailure::Runtime)?;
        let root = evidence.evidence_root().clone();
        Ok(finish(self.provenance, self.association, &root, evidence))
    }
    pub fn run_current(
        self,
        service: &SemanticWorkspaceService,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> MigrationResult<WorkspaceMigrationEvidence<AgentRuntimeV2MigrationEvidence>> {
        self.require_current(service)?;
        self.run(handler, cancellation)
    }
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> MigrationResult<WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>> {
        let evidence = self
            .runtime
            .run_durable(handler, cancellation, store)
            .map_err(WorkspaceMigrationFailure::Durable)?;
        let root = evidence.evidence_root().clone();
        Ok(finish(self.provenance, self.association, &root, evidence))
    }
    pub fn run_durable_current(
        self,
        service: &SemanticWorkspaceService,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> MigrationResult<WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>> {
        self.require_current(service)?;
        self.run_durable(handler, cancellation, store)
    }
}
impl WorkspaceMigrationExecution<ResumedMigratedAgentRuntimeV2> {
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> MigrationResult<WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>> {
        let evidence = self
            .runtime
            .run_durable(handler, cancellation, store)
            .map_err(WorkspaceMigrationFailure::Durable)?;
        let root = evidence.evidence_root().clone();
        Ok(finish(self.provenance, self.association, &root, evidence))
    }
    pub fn run_durable_current(
        self,
        service: &SemanticWorkspaceService,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> MigrationResult<WorkspaceMigrationEvidence<AgentRuntimeV2DurableEvidence>> {
        self.require_current(service)?;
        self.run_durable(handler, cancellation, store)
    }
}
