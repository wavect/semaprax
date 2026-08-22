#![allow(clippy::result_large_err)]

//! SEMAPRAX v0.1 compiler library.
//!
//! The source projection is for humans. The semantic graph is the agent API.

pub mod agent_economics;
pub mod agent_runtime;
pub mod agent_transport;
pub(crate) mod aggregate_layout;
pub mod ast;
pub(crate) mod bounded_output;
pub(crate) mod call_index;
pub mod cleanup;
pub mod cleanup_plan;
pub mod codegen;
pub mod conformance;
pub mod diagnostic;
#[doc(hidden)]
pub mod digest_hex;
pub mod economic_agent;
pub mod format;
pub mod graph;
pub mod hir;
pub mod impact;
pub mod lexer;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub(crate) mod native_settlement;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub mod owned_resource_corpus;
pub mod parser;
pub mod patch;
pub mod patch_evidence;
#[allow(dead_code, reason = "path-included by the unpublished native builder")]
mod private_capacity_contract;
pub mod project;
pub mod properties;
pub mod quality_route;
pub mod repair;
pub mod review;
pub mod runtime_status;
pub mod semantic_trace;
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
pub(crate) mod variant_layout;
pub mod verify;
pub mod wasm;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
#[doc(hidden)]
pub mod wit_component;
pub mod workspace;
pub mod workspace_patch_evidence;

mod graph_cleanup;
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
