//! Per-project configuration, frozen lock, trust store and provider resolution (HP-02).
//!
//! Diagnostics are `SPX-HPB001..`: 001-008 configuration, 010-014 lock,
//! 020-024 machine-local state and adoption, 030-034 trust, 040-042
//! resolution, 050 usage. Repository files only *request*; authority exists
//! only as a [`crate::host::grant::Grant`] issued by [`trust::grant_for`].

pub mod adopt;
pub mod builtin;
pub mod config;
pub mod installations;
pub mod lock;
pub mod resolve;
pub mod status;
pub mod trust;

mod cli;

pub use config::{CapabilityConfig, HarnessConfig, Mode};
pub use installations::{CurrentDigests, Installation, LocalState};
pub use resolve::{
    Binding, BindingState, Resolution, ResolvedLaunch, ResolvedProfile, UpstreamBinding,
};
pub use trust::{check_grant_current, grant_for};

use crate::cli::{Environment, Outcome};
use crate::diag::HarnessResult;
use std::path::Path;

/// Load the project configuration and machine-local state and resolve strictly
/// (an unmet `required` capability is an error). The one entry point HP-03/HP-04
/// use to obtain bindings and launch views; it executes nothing.
pub fn resolve_project(env: &Environment, project: &Path) -> HarnessResult<Resolution> {
    let config = HarnessConfig::load(project)?;
    let state = LocalState::load(env)?;
    resolve::resolve(&config, &state)
}

pub fn cli_status(args: &[String], env: &Environment) -> Outcome {
    cli::status_verb(args, env)
}

pub fn cli_explain(args: &[String], env: &Environment) -> Outcome {
    cli::explain_verb(args, env)
}

pub fn cli_resolve(args: &[String], env: &Environment) -> Outcome {
    cli::resolve_verb(args, env)
}

pub fn cli_adopt(args: &[String], env: &Environment) -> Outcome {
    cli::adopt_verb(args, env)
}

pub fn cli_trust(args: &[String], env: &Environment) -> Outcome {
    cli::trust_verb(args, env)
}

pub fn cli_revoke(args: &[String], env: &Environment) -> Outcome {
    cli::revoke_verb(args, env)
}

pub fn cli_inspect(args: &[String], env: &Environment) -> Outcome {
    cli::inspect_verb(args, env)
}
