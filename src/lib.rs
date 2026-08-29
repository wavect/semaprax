#![allow(clippy::result_large_err)]

//! SEMAPRAX v0.1 compiler library.
//!
//! The source projection is for humans. Semantic graphs and the bounded
//! project transport are the agent-facing interfaces.

pub mod abi_report;
pub mod agent_economics;
pub mod agent_runtime;
pub mod agent_transport;
pub(crate) mod aggregate_layout;
pub mod arc_zones;
pub mod ast;
pub(crate) mod bounded_output;
pub(crate) mod byte_data_capacity;
pub(crate) mod byte_ops;
pub mod c_header;
pub(crate) mod call_index;
pub mod capability_manifest;
pub mod cleanup;
pub mod cleanup_plan;
pub mod codegen;
pub(crate) mod command_io_ops;
pub(crate) mod command_profile;
pub mod conformance;
pub mod cxx_shim;
pub mod diagnostic;
#[doc(hidden)]
pub mod digest_hex;
pub mod economic_agent;
pub mod format;
pub mod freestanding_object;
pub mod graph;
pub mod hir;
pub mod hosted_interpreter;
pub mod hygienic;
pub mod impact;
pub mod interpreter;
pub mod lexer;
pub mod loan_plan;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub(crate) mod native_settlement;
pub mod openapi;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub mod owned_resource_corpus;
pub mod package_lock;
pub mod package_report;
pub mod package_report_v2;
pub mod parser;
pub mod patch;
pub mod patch_evidence;
pub mod plugin_manifest;
#[allow(dead_code, reason = "path-included by the unpublished native builder")]
mod private_capacity_contract;
pub mod project;
pub mod project_revision_store;
#[doc(hidden)]
pub mod project_transport;
pub mod properties;
pub mod protocol_check;
pub mod quality_route;
pub mod region_report;
pub mod repair;
pub mod review;
pub mod runtime_status;
pub mod scoped_tasks;
pub mod semantic_trace;
pub mod simd_report;
pub(crate) mod str_ops;
pub(crate) mod string_ops;
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
pub(crate) mod variant_layout;
pub mod verify;
pub mod wasm;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
#[doc(hidden)]
pub mod wit_component;
pub mod workspace;
pub mod workspace_patch_evidence;

mod graph_cleanup;
mod graph_loan;
pub(crate) mod host_io_ops;
mod host_ownership;
mod prelude;
pub mod semantic_workspace;
pub mod semantic_workspace_change;
pub mod semantic_workspace_operations;
pub mod semantic_workspace_structural_change;
mod source_verify;
pub mod workspace_analysis;
pub mod workspace_graph;

use std::path::Path;

use ast::Program;
use diagnostic::Diagnostic;

pub fn parse(source: &str, path: impl AsRef<Path>) -> Result<Program, Diagnostic> {
    parser::Parser::new(source, path.as_ref()).and_then(parser::Parser::parse)
}

pub fn check(source: &str, path: impl AsRef<Path>) -> Result<Program, Vec<Diagnostic>> {
    let program = parse(source, path).map_err(|error| vec![error])?;
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
