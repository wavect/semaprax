//! Bounded adapter host lifecycle and permission boundary (HP-03).
//!
//! Host-only: nothing here is linked into the compiler, and ordinary
//! `check`/`build` never reaches this module. See `docs/HARNESS-HOST-V1.md`.
//! Diagnostics are `SPX-HPC001..`.

pub mod budget;
pub mod grant;
pub mod http;
pub mod isolation;
pub mod launch;
pub mod lifecycle;
pub mod manager;
pub mod process;
pub mod rpc;

#[cfg(test)]
mod tests;

pub use budget::{BudgetLedger, HostBudget};
pub use http::{ApprovedEndpoint, Credential, HttpClient, HttpLimits, HttpResponse};
pub use isolation::{IsolationBackend, IsolationMode, IsolationRequest, NetworkPolicy};
pub use launch::LaunchSpec;
pub use lifecycle::{AdapterState, CancelToken, InvocationClass, Outcome};
pub use manager::{AdapterHandle, AdapterManager, HostConfig};
