//! Semantic-image agent-protocol regressions across the v1-v4 surfaces.
//!
//! One harness binary for the image protocol families that predate v5:
//! the v1 library and transport subjects, the v2 candidate protocol, the v3
//! candidate test protocol, and the v4 diagnostic protocol. Each module below
//! was its own integration test binary, and every one statically linked the
//! compiler, so the family cost twenty-two executables to express one
//! subject. The modules stay independent: each owns a distinct fixture root
//! and asserts only over its own session.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "image_protocol/analysis_coverage_v1.rs"]
mod analysis_coverage_v1;
#[path = "image_protocol/c_artifacts_v1.rs"]
mod c_artifacts_v1;
#[path = "image_protocol/candidate_test_transport_v3.rs"]
mod candidate_test_transport_v3;
#[path = "image_protocol/candidate_transport_v2.rs"]
mod candidate_transport_v2;
#[path = "image_protocol/cleanup_dependencies_v1.rs"]
mod cleanup_dependencies_v1;
#[path = "image_protocol/declaration_dependencies_v1.rs"]
mod declaration_dependencies_v1;
#[path = "image_protocol/dependency_navigation_v1.rs"]
mod dependency_navigation_v1;
#[path = "image_protocol/diagnostic_transport_v4.rs"]
mod diagnostic_transport_v4;
#[path = "image_protocol/facets_v1.rs"]
mod facets_v1;
#[path = "image_protocol/function_instances_v1.rs"]
mod function_instances_v1;
#[path = "image_protocol/function_reference_v1.rs"]
mod function_reference_v1;
#[path = "image_protocol/hir_relationships_v1.rs"]
mod hir_relationships_v1;
#[path = "image_protocol/openapi_artifacts_v1.rs"]
mod openapi_artifacts_v1;
#[path = "image_protocol/parallel_candidate_reads_v1.rs"]
mod parallel_candidate_reads_v1;
#[path = "image_protocol/parallel_reads_v1.rs"]
mod parallel_reads_v1;
#[path = "image_protocol/protocol_conformance_v1.rs"]
mod protocol_conformance_v1;
#[path = "image_protocol/read_batch_protocol_v1.rs"]
mod read_batch_protocol_v1;
#[path = "image_protocol/symbol_diagnostics_v1.rs"]
mod symbol_diagnostics_v1;
#[path = "image_protocol/target_artifacts_v1.rs"]
mod target_artifacts_v1;
#[path = "image_protocol/transport_v1.rs"]
mod transport_v1;
#[path = "image_protocol/workspace_archive_recovery_v1.rs"]
mod workspace_archive_recovery_v1;
#[path = "image_protocol/workspace_frontend_cache_v1.rs"]
mod workspace_frontend_cache_v1;
