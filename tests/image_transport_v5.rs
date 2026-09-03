//! Semantic-image v5 transport regressions.
//!
//! One harness binary for the whole v5 transport surface. Each module below
//! was its own integration test binary, and every one statically linked the
//! compiler, so the family cost nineteen executables to express one subject.
//! The modules stay independent: each owns a distinct fixture root and
//! asserts only over its own session.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "image_transport_v5/analysis_artifact_evidence.rs"]
mod analysis_artifact_evidence;
mod analysis_boundary_attachments;
#[path = "image_transport_v5/artifact_delta.rs"]
mod artifact_delta;
#[path = "image_transport_v5/builtin_calls.rs"]
mod builtin_calls;
#[path = "image_transport_v5/c_artifacts.rs"]
mod c_artifacts;
#[path = "image_transport_v5/cleanup_dependencies.rs"]
mod cleanup_dependencies;
#[path = "image_transport_v5/contract_delta.rs"]
mod contract_delta;
#[path = "image_transport_v5/contract_holes.rs"]
mod contract_holes;
#[path = "image_transport_v5/declaration_dependencies.rs"]
mod declaration_dependencies;
#[path = "image_transport_v5/dependency_navigation.rs"]
mod dependency_navigation;
#[path = "image_transport_v5/deployment_contract_evidence.rs"]
mod deployment_contract_evidence;
#[path = "image_transport_v5/draft_archive.rs"]
mod draft_archive;
#[path = "image_transport_v5/draft_merge.rs"]
mod draft_merge;
#[path = "image_transport_v5/draft_rebase.rs"]
mod draft_rebase;
#[path = "image_transport_v5/draft_recovery.rs"]
mod draft_recovery;
#[path = "image_transport_v5/field_place.rs"]
mod field_place;
#[path = "image_transport_v5/function_reference.rs"]
mod function_reference;
#[path = "image_transport_v5/nominal_rename.rs"]
mod nominal_rename;
#[path = "image_transport_v5/openapi_artifacts.rs"]
mod openapi_artifacts;
#[path = "image_transport_v5/ownership_delta.rs"]
mod ownership_delta;
#[path = "image_transport_v5/test_tasks.rs"]
mod test_tasks;
#[path = "image_transport_v5/workspace.rs"]
mod workspace;
