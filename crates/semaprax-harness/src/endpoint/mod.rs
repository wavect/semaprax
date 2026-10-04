//! Local model endpoint and gateway catalog adoption (HP-12).
//! Spec: docs/HARNESS-ENDPOINTS-V1.md. Diagnostics letter `L`.

pub mod adopt;
pub mod catalog;
mod cli;
pub mod ownership;
pub mod probe;
pub mod types;
pub mod usage;

pub use adopt::{adopt, probe_protocols, AdoptRequest};
pub use catalog::{
    AttemptOwner, BindingStatus, Capabilities, Catalog, CatalogModel, EndpointRecord, LogicalModel,
};
pub use ownership::{
    check_policy, litellm_config_snippet, AttemptOwnership, Balancing, Disclosure, EndpointPolicy,
    GatewayFallbacks, GatewayRetries,
};
pub use probe::{ProbeClient, Target};
pub use types::{EndpointKind, ModelIdentity, Protocol, ProtocolVerdict, Verdict};
pub use usage::{assess_reply, assess_stream, parse_usage, CallAssessment, Outcome, UsageEvidence};

pub fn cli_endpoints(args: &[String], env: &crate::cli::Environment) -> crate::cli::Outcome {
    cli::run(args, env)
}
