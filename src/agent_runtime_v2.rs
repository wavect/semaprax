//! Direct retained-Project binding to typed iterative Agent execution.
//! Runtime v1 profile bytes remain a checked compatibility carrier; this path
//! never instantiates the Runtime v1 action loop or converts typed calls to it.
pub use crate::execution_revision::typed::{
    bind_agent_runtime_v2, bind_agent_runtime_v2_live, bind_linked_agent_runtime_v2,
    AgentRuntimeV2, AgentRuntimeV2DurableEvidence, AgentRuntimeV2DurableModelEvidence,
    AgentRuntimeV2DurableModelFailure, AgentRuntimeV2DurableModelWaitEvidence,
    AgentRuntimeV2Evidence, AgentRuntimeV2ModelEvidence, AgentRuntimeV2ModelFailure,
    SourceModelWaitBinding,
};
pub use crate::execution_revision::typed_repair::{
    OfflineRepairEnvelope, OfflineRepairHandler, OfflineRepairPreview, OfflineRepairRejection,
};

pub mod checkpoint;
pub mod live_smoke;
pub mod repair_approval;
pub mod source_model;

pub use source_model::{
    SourceModelAdapterIdentity, SourceModelAttemptEvidence, SourceModelBinding,
    SourceModelEvidence, SourceModelInvocationCapability, SourceModelPolicyBinding,
    SourceModelReservationEvidence,
};

pub use crate::execution_revision::typed::{
    migrate_suspended_agent_runtime_v2, resume_migrated_agent_runtime_v2,
    AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure, DurableMigrationFailure,
    MigratedAgentRuntimeV2, ResumedMigratedAgentRuntimeV2,
};

pub use live_smoke::{
    AuthorizedLiveRepairSmoke, LiveRepairSmokeOutcome, LiveRepairSmokePlan,
    LiveRepairSmokePreflight, LiveRepairSmokePrerequisite, LiveRepairSmokeRecord,
    LiveRepairSmokeTarget, LiveRepairSmokeUsage, OperatorLiveSmokeGrant,
    LIVE_REPAIR_SMOKE_GRANT_SCHEMA, LIVE_REPAIR_SMOKE_PLAN_SCHEMA,
    LIVE_REPAIR_SMOKE_PREFLIGHT_SCHEMA, LIVE_REPAIR_SMOKE_RECORD_SCHEMA,
    LIVE_REPAIR_SMOKE_TARGET_SCHEMA, MAX_LIVE_REPAIR_SMOKE_BYTES,
};

pub use repair_approval::{
    apply_approved_repair_publication, prepare_approved_repair_publication,
    RepairCandidateApproval, RepairCandidateReview, RepairValidationOutcome,
    RepairValidationStatus, REPAIR_CANDIDATE_APPROVAL_SCHEMA, REPAIR_CANDIDATE_REVIEW_SCHEMA,
};
