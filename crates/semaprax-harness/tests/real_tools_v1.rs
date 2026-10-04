//! Provisioned real-tool evidence for the harness (ignored by default).
//! Each test names the environment variables it requires and refuses, rather
//! than skips, when they are absent:
//!   SEMAPRAX_COMPILER  absolute path to a built `semaprax` executable
//!   HARNESS_GRAFT      absolute path to a pinned `graft` executable
//!   HARNESS_GRAPHIFY   absolute path to a pinned `graphify` executable
//!   HARNESS_RTK        absolute path to a pinned `rtk` executable
//!   HARNESS_NODE / HARNESS_PYTHON  adapter runtimes
//! Run: cargo test -p semaprax-harness --test real_tools_v1 -- --ignored

#[path = "harness_v1/support.rs"]
mod support;

#[path = "real_tools_v1/decision_local.rs"]
mod decision_local;
#[path = "real_tools_v1/endpoints.rs"]
mod endpoints;
#[path = "real_tools_v1/external_host.rs"]
mod external_host;
#[path = "real_tools_v1/graft.rs"]
mod graft;
#[path = "real_tools_v1/graphify.rs"]
mod graphify;
#[path = "real_tools_v1/rtk.rs"]
mod rtk;
#[path = "real_tools_v1/setup_dist.rs"]
mod setup_dist;
#[path = "real_tools_v1/workflow_compiler.rs"]
mod workflow_compiler;
