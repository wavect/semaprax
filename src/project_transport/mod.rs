//! Persistent, bounded Project Agent Transport v2 over stdio.
//!
//! The host binds one Project v1 manifest when the process starts. Requests
//! cannot redirect that authority to another path. Graph Agent Transport v1
//! remains a separate, unchanged protocol.

pub(crate) mod codec;
mod config;
pub(crate) mod framing;
mod sdk;
mod session;

pub use sdk::{
    generate_project_public_api_transport_client, project_public_api_transport_discovery,
    ProjectTransportClientLanguage, PROJECT_PUBLIC_API_DISCOVERY_SCHEMA,
};

use std::ffi::OsString;

/// The byte-preserved default read-only protocol.
pub const TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v2";
/// Explicit opt-in profile that adds one bounded Project rename transaction.
pub const PROJECT_RENAME_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v3";
/// Explicit opt-in profile for the bounded Project inspect/change/build workflow.
pub const PROJECT_WORKFLOW_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v4";
/// Explicit opt-in read-only profile for Project v8 API inspection and npm builds.
pub const PROJECT_OWNED_DATA_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v5";
/// Explicit opt-in read-only profile for Project v8-v11 public API inspection and npm builds.
pub const PROJECT_PUBLIC_API_TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v6";

/// Parse daemon startup authority and serve exactly one sequential stdio
/// session. Human-readable startup/I/O failures are returned to the tiny
/// binary for stderr; stdout is owned exclusively by JSON-RPC responses.
pub fn run_from_args(arguments: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let config = config::ServerConfig::parse(arguments)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    session::serve(stdin.lock(), stdout.lock(), config).map_err(|error| error.to_string())
}
