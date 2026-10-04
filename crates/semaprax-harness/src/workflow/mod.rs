//! Automatic provider composition in the compiler-assisted development
//! workflow (HP-04). See `docs/HARNESS-WORKFLOW-V1.md`. Diagnostics `SPX-HPD001..`.
//!
//! The compiler is a service ([`compiler::CompilerService`]); providers are
//! reached only through the adapter host; publication is the compiler's own
//! route under a preexisting host policy.

mod b64;
mod cli;
pub mod compiler;
pub mod composition;
pub mod journal;
pub mod lineage;
pub mod pipeline;
pub mod policy;
pub mod report;
pub mod snapshot;
pub mod stages;

pub use cli::{cli_run, run_with, RunOptions};
pub use compiler::{CompilerService, SubprocessCompiler};

pub use compiler::{
    CandidatePreview, Capsule, CheckReport, CompilerDiagnostic, PublishError, PublishReceipt,
    SourceChange, TestReport,
};
pub use composition::{Composition, Interception, Slot, StageId};
pub use pipeline::{change_bytes, run, RunConfig, Stages};
pub use policy::{check_protected_facts, ApplyPolicy};
pub use report::{ProviderUse, Report};
pub use snapshot::Snapshot;
pub use stages::{CommandStage, ContextStage, ProposalStage, Task};
