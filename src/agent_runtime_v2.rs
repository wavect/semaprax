//! Direct retained-Project binding to typed iterative Agent execution.
//! Runtime v1 profile bytes remain a checked compatibility carrier; this path
//! never instantiates the Runtime v1 action loop or converts typed calls to it.
pub use crate::execution_revision::typed::{
    bind_agent_runtime_v2, bind_linked_agent_runtime_v2, AgentRuntimeV2,
    AgentRuntimeV2DurableEvidence, AgentRuntimeV2Evidence,
};

pub mod checkpoint;

pub use crate::execution_revision::typed::{
    migrate_suspended_agent_runtime_v2, resume_migrated_agent_runtime_v2,
    AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure, DurableMigrationFailure,
    MigratedAgentRuntimeV2, ResumedMigratedAgentRuntimeV2,
};
