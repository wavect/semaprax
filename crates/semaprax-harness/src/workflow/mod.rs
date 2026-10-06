//! Automatic provider composition in the compiler-assisted development
//! workflow (HP-04). See `docs/HARNESS-WORKFLOW-V1.md`. Diagnostics `SPX-HPD001..`.
//!
//! The compiler is a service ([`compiler::CompilerService`]); providers are
//! reached only through the adapter host; publication is the compiler's own
//! route under a preexisting host policy.

mod acquire;
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
pub mod context_target;
mod cost_ladder;
pub mod decision_open;
pub mod decision_reuse;
pub mod feedback;
pub mod generation;
pub mod journal;
pub mod lineage;
pub mod phases;
pub mod pipeline;
pub mod policy;
pub mod prompt_render;
pub mod report;
pub mod route_explain;
mod route_signals;
pub mod routing;
pub mod session;
mod session_repair;
pub mod snapshot;
pub mod spend;
mod spend_dispatch;
pub mod stages;
pub mod tokenizers;
mod updates_hook;

pub use cli::{cli_run, open_model, run_with, OpenedModel, RunOptions};
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
