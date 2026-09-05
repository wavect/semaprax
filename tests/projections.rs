//! Generated reports, schemas, manifests and foreign-surface projections: the
//! deterministic artifacts a verified program projects out of the semantic
//! graph, and the shims and descriptors that carry it across a boundary.
//!
//! One harness binary for the projection subject. Each module below was its own
//! integration test binary and every one statically linked the whole compiler,
//! so the family cost fourteen executables to express one subject. The modules
//! stay independent: each derives its own temporary artifact root from a
//! distinct literal prefix and asserts only over its own output.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "projections/capability_manifest.rs"]
mod capability_manifest;
#[path = "projections/cxx_package.rs"]
mod cxx_package;
#[path = "projections/cxx_shim_projection.rs"]
mod cxx_shim_projection;
#[path = "projections/doc_projection.rs"]
mod doc_projection;
#[path = "projections/fmt_comments.rs"]
mod fmt_comments;
#[path = "projections/freestanding_object.rs"]
mod freestanding_object;
#[path = "projections/hygienic_gen.rs"]
mod hygienic_gen;
#[path = "projections/openapi_generation.rs"]
mod openapi_generation;
#[path = "projections/plugin_manifest.rs"]
mod plugin_manifest;
#[path = "projections/portable_indexed_byte_data.rs"]
mod portable_indexed_byte_data;
#[path = "projections/protocol_projection.rs"]
mod protocol_projection;
#[path = "projections/public_api_descriptor.rs"]
mod public_api_descriptor;
#[path = "projections/region_report.rs"]
mod region_report;
#[path = "projections/simd_report.rs"]
mod simd_report;
#[path = "projections/static_protocol_conformance.rs"]
mod static_protocol_conformance;
#[path = "projections/ui_schema.rs"]
mod ui_schema;
#[path = "projections/unsafe_boundaries.rs"]
mod unsafe_boundaries;
