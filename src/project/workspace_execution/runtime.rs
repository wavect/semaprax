//! Consuming runtime producers bound to one immutable workspace selection.
//!
//! These associations confer no host authority. Runtime binders still verify
//! retained Agent/deployment facts, and runs use the caller's injected handlers.
use super::{associate, WorkspaceExecutionBinding};
use crate::agent_lifecycle::iterative::effects::{
    DurableTypedFailure, EffectBudget, EffectOperation, TypedEffectHandler,
};
use crate::agent_lifecycle::iterative::IterativeBudget;
use crate::agent_lifecycle::{AgentReadOperation, CheckpointStore, LifecycleBudget, LifecycleTask};
use crate::agent_runtime::AgentCancellation;
use crate::agent_runtime_v2::{
    bind_agent_runtime_v2, AgentRuntimeV2, AgentRuntimeV2DurableEvidence, AgentRuntimeV2Evidence,
};
use crate::diagnostic::Diagnostic;
use crate::execution_revision::iterative::{
    bind_iterative_execution_revision, IterativeExecutionEvidence, IterativeExecutionRevision,
};
use crate::execution_revision::{
    bind_execution_revision, ExecutionEvidence, ExecutionRevision, ExecutionRoot,
};
use crate::project::SemanticWorkspaceService;
use serde_json::json;

pub const WORKSPACE_RUNTIME_ASSOCIATION_SCHEMA: &str = "semaprax.workspace-runtime-association.v1";
pub const WORKSPACE_RUNTIME_EVIDENCE_SCHEMA: &str = "semaprax.workspace-runtime-evidence.v1";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// The runtime is private and consumed by a run; no arbitrary runtime can be
/// attached to a workspace association and no mutable runtime escape is exposed.
pub struct WorkspaceExecution<R> {
    binding: WorkspaceExecutionBinding,
    runtime: R,
    association: ExecutionRoot,
}
impl<R> WorkspaceExecution<R> {
    pub fn binding(&self) -> &WorkspaceExecutionBinding {
        &self.binding
    }
    pub fn association_root(&self) -> &ExecutionRoot {
        &self.association
    }
    /// Optional freshness guard. Ordinary `run` deliberately executes the
    /// retained immutable snapshot even after a workspace advances.
    pub fn require_current(&self, service: &SemanticWorkspaceService) -> Result<()> {
        self.binding.require_current(service)
    }
}

/// Only an actual consuming runtime run can produce this joined evidence.
pub struct WorkspaceExecutionEvidence<E> {
    binding: WorkspaceExecutionBinding,
    execution_association: ExecutionRoot,
    evidence: E,
    association: ExecutionRoot,
}
impl<E> WorkspaceExecutionEvidence<E> {
    pub fn binding(&self) -> &WorkspaceExecutionBinding {
        &self.binding
    }
    pub fn evidence(&self) -> &E {
        &self.evidence
    }
    pub fn association_root(&self) -> &ExecutionRoot {
        &self.association
    }
    pub fn execution_association_root(&self) -> &ExecutionRoot {
        &self.execution_association
    }
    /// Release the genuine producer evidence for an existing consuming API,
    /// such as typed migration. This does not mint a new workspace association.
    pub fn into_evidence(self) -> E {
        self.evidence
    }
}

impl WorkspaceExecutionBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn bind_once(
        &self,
        source_path: &str,
        agent_id: &str,
        deployment_source: &str,
        task: LifecycleTask,
        proposal: &str,
        budget: LifecycleBudget,
    ) -> Result<WorkspaceExecution<ExecutionRevision>> {
        let program = self.program_root();
        let runtime = bind_execution_revision(
            self.project_revision().clone(),
            program,
            program.digest(),
            source_path,
            agent_id,
            deployment_source,
            task,
            proposal,
            budget,
        )?;
        let association = self.join_runtime(
            "once",
            runtime.deployment_root(),
            runtime.instance_root(),
            runtime.execution_revision(),
        );
        Ok(WorkspaceExecution {
            binding: self.clone(),
            runtime,
            association,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn bind_iterative(
        &self,
        source_path: &str,
        agent_id: &str,
        step_type_id: &str,
        deployment_source: &str,
        task: LifecycleTask,
        proposals: &[String],
        budget: IterativeBudget,
    ) -> Result<WorkspaceExecution<IterativeExecutionRevision>> {
        let program = self.program_root();
        let runtime = bind_iterative_execution_revision(
            self.project_revision().clone(),
            program,
            program.digest(),
            source_path,
            agent_id,
            step_type_id,
            deployment_source,
            task,
            proposals,
            budget,
        )?;
        let association = self.join_runtime(
            "iterative",
            runtime.deployment_root(),
            runtime.instance_root(),
            runtime.execution_revision(),
        );
        Ok(WorkspaceExecution {
            binding: self.clone(),
            runtime,
            association,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn bind_typed(
        &self,
        source_path: &str,
        agent_id: &str,
        step_type_id: &str,
        selector_field_id: &str,
        operations: Vec<EffectOperation>,
        deployment_source: &str,
        task: LifecycleTask,
        proposals: &[String],
        budget: IterativeBudget,
        effects: EffectBudget,
    ) -> Result<WorkspaceExecution<AgentRuntimeV2>> {
        let program = self.program_root();
        let runtime = bind_agent_runtime_v2(
            self.project_revision().clone(),
            program,
            program.digest(),
            source_path,
            agent_id,
            step_type_id,
            selector_field_id,
            operations,
            deployment_source,
            task,
            proposals,
            budget,
            effects,
        )?;
        let association = self.join_runtime(
            "typed",
            runtime.deployment_root(),
            runtime.instance_root(),
            runtime.execution_revision(),
        );
        Ok(WorkspaceExecution {
            binding: self.clone(),
            runtime,
            association,
        })
    }
    fn join_runtime(
        &self,
        kind: &str,
        deployment: &ExecutionRoot,
        instance: &ExecutionRoot,
        execution: &ExecutionRoot,
    ) -> ExecutionRoot {
        associate(
            WORKSPACE_RUNTIME_ASSOCIATION_SCHEMA,
            json!({
                "workspace_binding": self.association_root().digest(),
                "runtime_kind": kind,
                "deployment_root": deployment.digest(),
                "instance_root": instance.digest(),
                "execution_revision": execution.digest(),
            }),
        )
    }
}

fn joined_evidence<E>(
    binding: WorkspaceExecutionBinding,
    execution_association: ExecutionRoot,
    actual_evidence_root: &ExecutionRoot,
    evidence: E,
) -> WorkspaceExecutionEvidence<E> {
    let association = associate(
        WORKSPACE_RUNTIME_EVIDENCE_SCHEMA,
        json!({
            "workspace_binding": binding.association_root().digest(),
            "workspace_execution": execution_association.digest(),
            "evidence_root": actual_evidence_root.digest(),
        }),
    );
    WorkspaceExecutionEvidence {
        binding,
        execution_association,
        evidence,
        association,
    }
}

impl WorkspaceExecution<ExecutionRevision> {
    pub fn run(
        self,
        handler: &mut dyn AgentReadOperation,
        cancellation: &AgentCancellation,
    ) -> Result<WorkspaceExecutionEvidence<ExecutionEvidence>> {
        let evidence = self.runtime.run(handler, cancellation)?;
        let root = evidence.evidence_root().clone();
        Ok(joined_evidence(
            self.binding,
            self.association,
            &root,
            evidence,
        ))
    }
}
impl WorkspaceExecution<IterativeExecutionRevision> {
    pub fn run(
        self,
        handler: &mut dyn AgentReadOperation,
        cancellation: &AgentCancellation,
    ) -> Result<WorkspaceExecutionEvidence<IterativeExecutionEvidence>> {
        let evidence = self.runtime.run(handler, cancellation)?;
        let root = evidence.evidence_root().clone();
        Ok(joined_evidence(
            self.binding,
            self.association,
            &root,
            evidence,
        ))
    }
}
impl WorkspaceExecution<AgentRuntimeV2> {
    pub fn run(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> Result<WorkspaceExecutionEvidence<AgentRuntimeV2Evidence>> {
        let evidence = self.runtime.run(handler, cancellation)?;
        let root = evidence.evidence_root().clone();
        Ok(joined_evidence(
            self.binding,
            self.association,
            &root,
            evidence,
        ))
    }
    /// The shared service borrow spans this call; a stale binding fails before
    /// executing a retained stage or injected host operation.
    pub fn run_current(
        self,
        service: &SemanticWorkspaceService,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> Result<WorkspaceExecutionEvidence<AgentRuntimeV2Evidence>> {
        self.require_current(service)?;
        self.run(handler, cancellation)
    }
    /// Trusted checkpoint snapshots and single-writer store authority have the
    /// exact underlying Runtime v2 contract. Its rich failure is returned intact,
    /// including any selected terminal run after a persistence failure.
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
    ) -> std::result::Result<
        WorkspaceExecutionEvidence<AgentRuntimeV2DurableEvidence>,
        DurableTypedFailure,
    > {
        let evidence = self.runtime.run_durable(
            handler,
            cancellation,
            retained_checkpoint,
            store,
            max_reserved_fuel,
        )?;
        let root = evidence.evidence_root().clone();
        Ok(joined_evidence(
            self.binding,
            self.association,
            &root,
            evidence,
        ))
    }
}

mod migration;
pub use migration::{
    prepare_workspace_migration, resume_workspace_migration, WorkspaceMigrationEvidence,
    WorkspaceMigrationExecution, WorkspaceMigrationFailure, WorkspaceSuspensionEvidence,
    MAX_WORKSPACE_MIGRATION_RECEIPT_BYTES, WORKSPACE_MIGRATION_ASSOCIATION_SCHEMA,
    WORKSPACE_MIGRATION_EVIDENCE_SCHEMA,
};
