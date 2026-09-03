//! Native Rust import admission and the Graph v25 projection.
//!
//! Graph v25 is selected exactly when a program declares a native Rust
//! import. No earlier schema can represent one, so every program without such
//! a declaration keeps the schema it already selected and its previously
//! emitted bytes.
//!
//! The module Graph projects these declarations and their calls. The agent
//! context, review, impact, and target-evidence projections stay closed: each
//! omits import nodes by construction, so admitting a native Rust import there
//! would silently drop authenticated meaning rather than represent it.

use crate::ast::Program;
use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedImportResultKind, ResolvedInterface, ResolvedProgram};

/// The schema selected by any program declaring a native Rust import.
pub(crate) const NATIVE_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v25";

const CLOSED_PROJECTION: &str =
    "Native Rust import declarations are outside the agent, review, impact, and evidence Graph projections";

/// Reports whether any interface declares a native Rust import.
///
/// This is the exact Graph v25 selection predicate. It reads declarations
/// rather than call sites so that a declared but uncalled import still selects
/// the schema that can represent its result type.
pub(crate) fn declares_native_rust_import(interfaces: &[ResolvedInterface]) -> bool {
    interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
}

/// The projected spelling of an import result type.
pub(crate) fn result_text(kind: &ResolvedImportResultKind) -> &'static str {
    match kind {
        ResolvedImportResultKind::Unit => "unit",
        ResolvedImportResultKind::I64 => "i64",
        ResolvedImportResultKind::Bool => "bool",
    }
}

/// Closes a resolved program out of the projections that cannot represent a
/// native Rust import.
pub(crate) fn reject_native_rust_imports(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if declares_native_rust_import(&program.interfaces) {
        Err(Diagnostic::io("SPX-G218", CLOSED_PROJECTION))
    } else {
        Ok(())
    }
}

/// The same closure before resolution, so an unresolvable program cannot be
/// mistaken for an admitted one.
pub(crate) fn reject_source_native_rust_imports(program: &Program) -> Result<(), Vec<Diagnostic>> {
    if program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
    {
        Err(vec![Diagnostic::io("SPX-G218", CLOSED_PROJECTION)])
    } else {
        Ok(())
    }
}

/// Closes an import node. Graph v25 records whether the declaration is a
/// native Rust import; every earlier schema closes the node unchanged, so its
/// previously emitted bytes are preserved exactly.
pub(crate) fn append_import_tail(output: &mut CappedString, schema: &str, native_rust: bool) {
    if schema == NATIVE_RUST_IMPORT_SCHEMA {
        output.push_str(",\"native_rust\":");
        output.push_str(if native_rust { "true" } else { "false" });
    }
    output.push('}');
}
