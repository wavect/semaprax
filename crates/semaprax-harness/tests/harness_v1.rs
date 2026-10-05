//! Harness provider host integration harness (docs/HARNESS-PROVIDER-V1.md).
//! One module per work item; each owns its fixture-directory prefix.

#[path = "harness_v1/support.rs"]
mod support;

#[path = "harness_v1/bench.rs"]
mod bench;
#[path = "harness_v1/bridge.rs"]
mod bridge;
#[path = "harness_v1/command_view.rs"]
mod command_view;
#[path = "harness_v1/command_view_caveman.rs"]
mod command_view_caveman;
#[path = "harness_v1/conformance.rs"]
mod conformance;
#[path = "harness_v1/context.rs"]
mod context;
#[path = "harness_v1/contract.rs"]
mod contract;
#[path = "harness_v1/decision.rs"]
mod decision;
#[path = "harness_v1/endpoint.rs"]
mod endpoint;
#[path = "harness_v1/host.rs"]
mod host;
#[path = "harness_v1/m_ma.rs"]
mod m_ma;
#[path = "harness_v1/observe.rs"]
mod observe;
#[path = "harness_v1/profile.rs"]
mod profile;
#[path = "harness_v1/receipt.rs"]
mod receipt;
#[path = "harness_v1/skills.rs"]
mod skills;
#[path = "harness_v1/workflow.rs"]
mod workflow;
