//! Automatic provider composition in the compiler-assisted development
//! workflow (HP-04). See `docs/HARNESS-WORKFLOW-V1.md`. Diagnostics `SPX-HPD001..`.
//!
//! The compiler is a service ([`compiler::CompilerService`]); providers are
//! reached only through the adapter host; publication is the compiler's own
//! route under a preexisting host policy.

pub mod adapter_config;
mod attempt;
mod b64;
pub mod broker_stage;
pub mod budget;
pub mod checks;
mod cli;
mod cli_apply;
pub mod compiler;
pub mod composition;
pub mod journal;
pub mod lineage;
pub mod pipeline;
pub mod policy;
pub mod report;
pub mod routing;
pub mod session;
mod session_repair;
pub mod snapshot;
pub mod stages;
pub mod tokenizers;
mod updates_hook;

pub use cli::{cli_run, run_with, RunOptions};
pub use cli_apply::cli_apply;
pub use compiler::{CompilerService, SubprocessCompiler};

pub use broker_stage::BrokerContext;
pub use checks::{CheckRun, CheckSpec, HostCommandChecks};
pub use compiler::{
    CandidatePreview, Capsule, CheckReport, CompilerDiagnostic, PublishError, PublishReceipt,
    SourceChange, TestReport,
};
pub use composition::{Composition, Interception, Slot, StageId};
pub use pipeline::{change_bytes, run, DecisionStage, RunConfig, SkillPromptUse, Stages};
pub use policy::{check_protected_facts, ApplyPolicy};
pub use report::{ProviderUse, Report};
pub use session::{apply_result, CancelFlag, SessionBounds};
pub use snapshot::Snapshot;
pub use stages::{CommandStage, ContextStage, ProposalStage, Task};
