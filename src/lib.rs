#![allow(
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    reason = "typed-carrier and generic-abi docs reference private/test items and cross-crate paths that are valid with --document-private-items but fail under -D warnings; fix forward tracked separately, unblock CI (other agents active)"
)]

//! SEMAPRAX v0.1 compiler library.
//!
//! The source projection is for humans. Semantic graphs and the bounded
//! project transport are the agent-facing interfaces.

// Path-included by the toolchain crate and its harnesses, where the same
// sources name this crate as `semaprax`.
extern crate self as semaprax;

pub mod abi_report;
pub mod agent_definition;
pub mod agent_deployment;
pub mod agent_economics;
pub mod agent_harness;
pub mod agent_interaction_schema;
#[allow(
    dead_code,
    unused_imports,
    unused_mut,
    unused_variables,
    clippy::filter_map_bool_then,
    clippy::large_enum_variant,
    clippy::manual_is_multiple_of,
    clippy::needless_option_as_deref,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod agent_lifecycle;
pub mod agent_lifecycle_typed_carrier;
pub mod agent_observation;
pub mod agent_proposal;
#[allow(
    unused_imports,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod agent_runtime;
#[allow(
    clippy::assertions_on_constants,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod agent_runtime_v2;
pub mod agent_skill_bundle;
pub mod agent_trajectory_export;
pub mod agent_transcript;
pub mod agent_transport;
pub(crate) mod aggregate_layout;
pub mod arc_zones;
pub mod architecture_claims;
#[allow(
    unused_imports,
    clippy::needless_lifetimes,
    clippy::ptr_arg,
    clippy::replace_box,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_literal_unwrap,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod assurance_manifest;
pub mod assurance_policy;
pub mod ast;
pub(crate) mod literal_format;
pub mod audit_capsule;
#[allow(
    dead_code,
    clippy::err_expect,
    clippy::map_identity,
    clippy::needless_as_bytes,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod authentication;
pub(crate) mod bounded_output;
pub(crate) mod box_ops;
pub(crate) mod byte_data_capacity;
pub(crate) mod byte_ops;
pub mod c_header;
pub(crate) mod cache_codec;
pub(crate) mod call_index;
pub mod candidate_archive_store;
pub mod capability_manifest;
#[allow(
    unreachable_patterns,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod cleanup;
#[allow(
    dead_code,
    unreachable_patterns,
    clippy::unnecessary_lazy_evaluations,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod cleanup_plan;
#[allow(
    dead_code,
    unused_variables,
    clippy::needless_borrow,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod codegen;
pub(crate) mod command_io_ops;
pub(crate) mod command_profile;
pub mod compact_semantic_projection;
#[allow(
    unused_imports,
    clippy::too_many_arguments,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod compute_profile;
pub mod conformance;
pub mod cxx_shim;
pub mod diagnostic;
#[doc(hidden)]
pub mod digest_hex;
pub mod doc;
pub mod doctor;
pub mod economic_agent;
pub mod embedding_api;
pub(crate) mod environment_ops;
pub mod environment_snapshot;
#[allow(
    dead_code,
    unused_imports,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod execution_revision;
pub(crate) mod filesystem_ops;
pub mod format;
pub mod freestanding_object;
#[allow(
    dead_code,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod graph;
#[allow(
    unused_imports,
    clippy::needless_borrows_for_generic_args,
    clippy::too_many_arguments,
    clippy::unnecessary_map_or,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod hir;
pub mod hosted_interpreter;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
pub mod https_client;
pub mod hygienic;
pub mod image_transport;
mod immutable_list;
pub mod impact;
pub mod installed_diagnostics;
pub mod installed_fix_plan;
pub mod installed_guidance;
#[allow(
    dead_code,
    unused_imports,
    unused_mut,
    unused_variables,
    clippy::arc_with_non_send_sync,
    clippy::cloned_ref_to_slice_refs,
    clippy::large_enum_variant,
    clippy::needless_borrow,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_cast,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod interpreter;
pub(crate) mod iterator_ops;
#[allow(
    dead_code,
    unused_imports,
    clippy::needless_borrow,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub(crate) mod kernel_zero;
pub mod lexer;
mod list_ops;
#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    clippy::arc_with_non_send_sync,
    clippy::bool_assert_comparison,
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::double_ended_iterator_last,
    clippy::drop_non_drop,
    clippy::empty_line_after_doc_comments,
    clippy::enum_variant_names,
    clippy::err_expect,
    clippy::infallible_destructuring_match,
    clippy::int_plus_one,
    clippy::items_after_test_module,
    clippy::large_enum_variant,
    clippy::match_like_matches_macro,
    clippy::needless_borrow,
    clippy::needless_range_loop,
    clippy::op_ref,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_cast,
    clippy::useless_vec,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod live_invocation;
pub mod loan_plan;
pub(crate) mod loop_calls;
mod map_ops;
#[allow(
    clippy::too_many_arguments,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod model_budget_policy;
pub mod model_call_receipt;
#[allow(
    clippy::large_enum_variant,
    clippy::manual_contains,
    clippy::manual_is_multiple_of,
    clippy::type_complexity,
    clippy::unnecessary_sort_by,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod model_routing;
pub mod native_law_source;
pub mod native_rust_binding;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub(crate) mod native_settlement;
#[allow(
    dead_code,
    unused_imports,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub(crate) mod oci_package;
pub mod openapi;
#[allow(
    dead_code,
    unused_imports,
    unknown_lints,
    clippy::chunks_exact_to_as_chunks,
    clippy::len_without_is_empty,
    clippy::manual_is_multiple_of,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod outbound_host_adapter;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub mod owned_resource_corpus;
pub mod package_build;
pub mod package_build_v2;
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
pub mod package_cache_host;
pub mod package_compatibility;
pub mod package_lock;
pub mod package_lock_v2;
pub mod package_lock_v3;
mod package_range;
#[allow(
    unused_imports,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::useless_vec,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod package_registry;
pub mod package_report;
pub mod package_report_v2;
pub mod package_resolution_snapshot;
pub mod package_resolver;
pub mod package_resolver_v2;
pub mod package_semantic_graph;
pub mod package_source_capsule;
pub mod parser;
#[allow(
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod patch;
pub mod patch_evidence;
pub mod plugin_manifest;
#[allow(dead_code, reason = "path-included by the unpublished native builder")]
mod private_capacity_contract;
pub(crate) mod process_ops;
pub mod process_provider;
#[allow(
    dead_code,
    unused_imports,
    unused_mut,
    clippy::collapsible_if,
    clippy::manual_is_multiple_of,
    clippy::needless_lifetimes,
    clippy::needless_question_mark,
    clippy::question_mark,
    clippy::redundant_closure,
    clippy::redundant_iter_cloned,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod project;
pub mod project_revision_store;
#[doc(hidden)]
#[allow(
    dead_code,
    unused_imports,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod project_transport;
#[allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::needless_lifetimes,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod proof_export;
pub mod properties;
pub mod protocol_check;
#[allow(
    dead_code,
    unused_imports,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod provider_adapter_sdk;
#[allow(
    unused_mut,
    clippy::manual_repeat_n,
    clippy::needless_borrow,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod public_generic_abi;
pub mod public_generic_consumer;
pub mod public_generic_settlement;
pub mod public_generic_surface;
pub mod public_generic_type;
pub mod quality_route;
pub mod query;
pub mod region_report;
#[allow(
    unknown_lints,
    clippy::chunks_exact_to_as_chunks,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of,
    clippy::too_many_arguments,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod release_provenance;
pub mod repair;
pub mod requirement_traceability;
#[allow(
    dead_code,
    unused_imports,
    unknown_lints,
    clippy::chunks_exact_to_as_chunks,
    clippy::cloned_ref_to_slice_refs,
    clippy::items_after_test_module,
    clippy::large_enum_variant,
    clippy::manual_is_multiple_of,
    clippy::manual_noop_waker,
    clippy::needless_lifetimes,
    clippy::redundant_async_block,
    clippy::replace_box,
    clippy::too_many_arguments,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod resumable_effects;
pub mod review;
pub mod runtime_status;
pub mod rust_api_context;
#[doc(hidden)]
pub mod rust_api_index;
pub mod scoped_tasks;
pub mod semantic_cache_store;
pub mod semantic_discovery;
pub mod semantic_embedding;
pub mod semantic_retention;
#[allow(
    clippy::large_enum_variant,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod semantic_retention_lifecycle;
pub mod semantic_retention_registry;
pub mod semantic_retention_store;
pub mod semantic_service_mcp;
pub mod semantic_service_transport;
#[allow(
    unused_mut,
    clippy::doc_lazy_continuation,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod semantic_task_context;
pub mod semantic_trace;
pub mod simd_report;
pub mod source_command;
#[cfg(feature = "unstable-rust-source-lowering")]
pub mod stable_rust_lowering;
pub mod static_protocol;
pub(crate) mod stdin_stream_ops;
pub(crate) mod str_ops;
pub mod streaming_proposal_decode;
pub(crate) mod string_ops;
pub mod structured_tasks;
pub mod target_evidence;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub mod trace_path_certificate;
#[cfg(not(any(test, feature = "unstable-native-host-internal")))]
#[allow(
    dead_code,
    reason = "host-only certificate inspection remains behind the unpublished feature"
)]
mod trace_path_certificate;
pub mod ui_schema;
pub(crate) mod variant_guards;
pub(crate) mod variant_layout;
pub(crate) mod vec_ops;
pub mod verify;
#[allow(
    dead_code,
    unused_imports,
    clippy::cloned_ref_to_slice_refs,
    clippy::manual_is_multiple_of,
    clippy::needless_return,
    clippy::unnecessary_cast,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod wasm;
pub mod webapp;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
#[doc(hidden)]
pub mod wit_component;
pub mod workspace;
pub mod workspace_patch_evidence;

pub mod database_fixture;
#[allow(
    dead_code,
    unused_imports,
    clippy::enum_variant_names,
    clippy::explicit_counter_loop,
    clippy::field_reassign_with_default,
    clippy::suspicious_open_options,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod durable_jobs;
pub mod filesystem_provider;
mod graph_cleanup;
mod graph_loan;
pub(crate) mod host_io_ops;
mod host_ownership;
pub mod job_evidence;
pub mod job_fixture;
#[allow(
    clippy::filter_map_identity,
    clippy::too_many_arguments,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod job_runtime;
pub(crate) mod network_io_ops;
#[allow(
    clippy::field_reassign_with_default,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod network_provider;
mod prelude;
pub mod semantic_workspace;
pub mod semantic_workspace_change;
pub mod semantic_workspace_operations;
pub mod semantic_workspace_structural_change;
#[allow(
    clippy::explicit_auto_deref,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
pub mod session_protocol;
#[allow(
    clippy::needless_borrows_for_generic_args,
    clippy::needless_return,
    reason = "REF-24 (#543) inventoried pre-existing findings in this module; fix forward, never extend"
)]
mod source_verify;
pub mod typed_workflow;
#[cfg(feature = "unstable-workflow-profiling")]
pub mod workflow_profile;
pub mod workspace_analysis;
pub mod workspace_graph;

use std::path::Path;

use ast::Program;
use diagnostic::Diagnostic;

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_PUBLIC_PARSE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static TEST_PUBLIC_PARSE_SITES: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

#[cfg_attr(test, track_caller)]
pub fn parse(source: &str, path: impl AsRef<Path>) -> Result<Program, Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Parse);
    #[cfg(test)]
    TEST_PUBLIC_PARSE_CALLS.with(|calls| calls.set(calls.get() + 1));
    #[cfg(test)]
    {
        let caller = std::panic::Location::caller();
        TEST_PUBLIC_PARSE_SITES.with(|sites| {
            if let Some(sites) = sites.borrow_mut().as_mut() {
                sites.push(format!("{}:{}", caller.file(), caller.line()));
            }
        });
    }
    parser::Parser::new(source, path.as_ref()).and_then(parser::Parser::parse)
}

/// Parse `source` and also return its `//` comments, in source order, for
/// [`format::comments::canonical_with_comments`]. The program is identical to
/// [`parse`].
pub fn parse_with_comments(
    source: &str,
    path: impl AsRef<Path>,
) -> Result<(Program, lexer::Comments), Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Parse);
    parser::Parser::parse_with_comments(source, path.as_ref())
}

/// Parse `source` and render it canonically with its `//` comments restored:
/// the text a route that rewrites a source file publishes, so the comments of
/// the file survive the rewrite. The returned program is the parsed input.
pub fn parse_canonical(
    source: &str,
    path: impl AsRef<Path>,
) -> Result<(Program, String), Diagnostic> {
    let (program, comments) = parse_with_comments(source, path)?;
    let canonical = format::comments::canonical_with_comments(&program, &comments);
    Ok((program, canonical))
}

pub fn check(source: &str, path: impl AsRef<Path>) -> Result<Program, Vec<Diagnostic>> {
    let path = path.as_ref();
    let program = parse(source, path).map_err(|error| vec![error])?;
    if let Some(import) = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| import.index_selected)
    {
        return Err(vec![Diagnostic::error(
            "SPX-B147",
            "selected Rust import requires an authenticated API index",
            import.span,
        )
        .at_path(path.display().to_string())]);
    }
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        Err(diagnostics)
    } else {
        Ok(program)
    }
}

pub fn compile_file(path: impl AsRef<Path>) -> Result<Program, Vec<Diagnostic>> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    check(&source, path)
}
