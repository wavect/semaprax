//! SEMAPRAX development-harness provider host.
//!
//! Host-only crate specified by `docs/HARNESS-PROVIDER-V1.md`. It never links
//! into the compiler: the compiler is a service invoked through an explicit
//! executable path, and every external provider runs as a separate adapter
//! process negotiated through typed capability contracts.

pub mod diag;
pub mod json;

pub mod contract;
pub mod profile;
pub mod host;
pub mod workflow;
pub mod context;
pub mod command_view;
pub mod decision;
pub mod endpoint;
pub mod skills;
pub mod bridge;
pub mod observe;
pub mod conformance;
pub mod bench;

pub mod cli;
