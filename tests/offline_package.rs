//! Offline package resolution, locking and reporting, plus the ABI, C header,
//! release-packaging and release-archive product gates.

#[path = "offline_package/abi_report.rs"]
mod abi_report;
#[path = "offline_package/build.rs"]
mod build;
#[path = "offline_package/c_header_emission.rs"]
mod c_header_emission;
#[path = "offline_package/ci_release_gate.rs"]
mod ci_release_gate;
#[path = "offline_package/effect_free_wasm_build.rs"]
mod effect_free_wasm_build;
#[path = "offline_package/linked_scalar_wasm_build.rs"]
mod linked_scalar_wasm_build;
#[path = "offline_package/lock.rs"]
mod lock;
#[path = "offline_package/multi_source_capsule.rs"]
mod multi_source_capsule;
#[path = "offline_package/ranges.rs"]
mod ranges;
#[path = "offline_package/release_packaging_unix.rs"]
mod release_packaging_unix;
#[path = "offline_package/release_packaging_windows.rs"]
mod release_packaging_windows;
#[path = "offline_package/release_workflow.rs"]
mod release_workflow;
#[path = "offline_package/report.rs"]
mod report;
#[path = "offline_package/resolution_snapshot.rs"]
mod resolution_snapshot;
#[path = "offline_package/resolver.rs"]
mod resolver;
#[path = "offline_package/resolver_cli.rs"]
mod resolver_cli;
#[path = "offline_package/semantic_graph.rs"]
mod semantic_graph;
#[path = "offline_package/standalone.rs"]
mod standalone;
