//! Authoritative command results versus model-facing views (HP-08/HP-09 host
//! side). The host executes a command exactly once and owns its status,
//! streams and digests; a `command.view` provider may only shape the text a
//! model sees. See `docs/HARNESS-COMMAND-VIEW-V1.md`. Diagnostics `SPX-HPH...`
//! (command view) and `SPX-HPI...` (automatic provider use).

pub mod cli;
pub mod executor;
pub mod guard;
pub mod intent;
pub mod lineage;
pub mod measure;
pub mod policy;
pub mod recover;
pub mod result;
pub mod retention;
pub mod run;
pub mod view;
pub mod wrapper;

pub use measure::{Measurement, ViewTokenizer};
pub use recover::{recover, recover_by_id, Recovered};
pub use result::{CommandResult, Envelope, ModelView};
pub use run::{execute, ExecOptions, ExecReport};

pub fn cli_exec(args: &[String], env: &crate::cli::Environment) -> crate::cli::Outcome {
    cli::exec(args, env)
}

pub fn cli_recover(args: &[String], env: &crate::cli::Environment) -> crate::cli::Outcome {
    cli::recover(args, env)
}
