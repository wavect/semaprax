//! Workspace regressions: the semantic workspace, its session CLI, and the
//! workspace graph.
//!
//! One harness binary for the workspace subject. Each module below was its own
//! integration test binary, and every one statically linked the whole compiler,
//! so the family cost thirteen executables to express one subject. The modules
//! stay independent: each owns a distinct temporary fixture root and asserts
//! only over its own workspace.
//!
//! Two workspace files are deliberately not modules here:
//!
//!   - `workspace_mcp_cli_v1` is named with `--test` by
//!     `scripts/graph-operational-client-mcp-evidence.py`.
//!   - `semantic_workspace_transaction_v1` re-invokes its own binary through
//!     `current_exe` with `--exact`, which only holds while it is its own test
//!     target. Its hostile companion does not, and is a module here.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "workspace/graph_phase_a_surface.rs"]
mod graph_phase_a_surface;
#[path = "workspace/semantic_change.rs"]
mod semantic_change;
#[path = "workspace/semantic_graph.rs"]
mod semantic_graph;
#[path = "workspace/semantic_image.rs"]
mod semantic_image;
#[path = "workspace/semantic_operations.rs"]
mod semantic_operations;
#[path = "workspace/semantic_patch_evidence.rs"]
mod semantic_patch_evidence;
#[path = "workspace/semantic_patch_evidence_apply.rs"]
mod semantic_patch_evidence_apply;
#[path = "workspace/semantic_patch_evidence_hostile.rs"]
mod semantic_patch_evidence_hostile;
#[path = "workspace/semantic_structural_change.rs"]
mod semantic_structural_change;
#[path = "workspace/semantic_transaction_hostile.rs"]
mod semantic_transaction_hostile;
#[path = "workspace/session_cli.rs"]
mod session_cli;
#[path = "workspace/session_read_batch_cli.rs"]
mod session_read_batch_cli;
#[path = "workspace/session_semantic_cache_cli.rs"]
mod session_semantic_cache_cli;
