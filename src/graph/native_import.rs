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
pub(crate) const INDEXED_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v52";
pub(crate) const SELECTED_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v53";
pub(crate) const SELECTED_RUST_METHOD_SCHEMA: &str = "semaprax.graph.v54";
pub(crate) const SELECTED_RUST_RESULT_SCHEMA: &str = "semaprax.graph.v55";
pub(crate) const OWNED_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v56";
pub(crate) const STRING_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v57";
pub(crate) const CONTAINER_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v58";
pub(crate) const NESTED_CONTAINER_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v59";

pub(crate) const OWNER_ADMISSION_SCHEMA: &str = "semaprax.graph.v60";

pub(crate) fn selected_schema(
    interfaces: &[ResolvedInterface],
    functions: &[crate::hir::ResolvedFunction],
) -> Option<&'static str> {
    if functions.iter().any(|function| {
        function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V14
    }) {
        return Some(OWNER_ADMISSION_SCHEMA);
    }
    if interfaces.iter().flat_map(|i| &i.imports).any(|i| i.native_rust && i.result.kind == ResolvedImportResultKind::OwnedResultStringOptionI64) { Some(NESTED_CONTAINER_RUST_IMPORT_SCHEMA) } else if interfaces.iter().flat_map(|i| &i.imports).any(|i| i.native_rust &&
        (matches!(i.result.kind, ResolvedImportResultKind::OwnedOptionString | ResolvedImportResultKind::OwnedResultStringI64)
        || i.parameters.iter().any(|p| matches!(&p.ty, crate::hir::ResolvedType::Nominal { arguments, .. } if arguments.first() == Some(&crate::hir::ResolvedType::String))))) {
        Some(CONTAINER_RUST_IMPORT_SCHEMA)
    } else if interfaces.iter().flat_map(|i| &i.imports).any(|i| {
        i.native_rust
            && (i.result.kind == ResolvedImportResultKind::OwnedString
                || i.parameters
                    .iter()
                    .any(|p| p.ty == crate::hir::ResolvedType::String))
    }) {
        Some(STRING_RUST_IMPORT_SCHEMA)
    } else if interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| {
            import.native_rust
                && (matches!(
                    import.result.kind,
                    ResolvedImportResultKind::OwnedResource { .. }
                        | ResolvedImportResultKind::OwnedResultResourceI64 { .. }
                ) || import
                    .parameters
                    .iter()
                    .any(|parameter| parameter.ownership == crate::hir::OwnershipMode::Own))
        })
    {
        Some(OWNED_RUST_IMPORT_SCHEMA)
    } else if declares_selected_rust_result(interfaces) {
        Some(SELECTED_RUST_RESULT_SCHEMA)
    } else if declares_selected_rust_method(interfaces) {
        Some(SELECTED_RUST_METHOD_SCHEMA)
    } else if declares_selected_rust_import(interfaces) {
        Some(SELECTED_RUST_IMPORT_SCHEMA)
    } else if declares_indexed_rust_import(interfaces) {
        Some(INDEXED_RUST_IMPORT_SCHEMA)
    } else if declares_native_rust_import(interfaces) {
        Some(NATIVE_RUST_IMPORT_SCHEMA)
    } else {
        None
    }
}

pub(crate) fn evidence_refusal(schema: &str) -> Option<Diagnostic> {
    let message = match schema {
        OWNER_ADMISSION_SCHEMA => "native owner admission programs select `semaprax.graph.v60`, outside this evidence flow admission",
        INDEXED_RUST_IMPORT_SCHEMA => "indexed Rust import programs select `semaprax.graph.v52`, which is outside this evidence flow's admission",
        SELECTED_RUST_IMPORT_SCHEMA => "selected Rust import programs select `semaprax.graph.v53`, which is outside this evidence flow's admission",
        SELECTED_RUST_METHOD_SCHEMA => "selected Rust method programs select `semaprax.graph.v54`, which is outside this evidence flow's admission",
        NESTED_CONTAINER_RUST_IMPORT_SCHEMA => "nested owned container programs select `semaprax.graph.v59`, outside this evidence flow admission",
        CONTAINER_RUST_IMPORT_SCHEMA => "owned container Rust import programs select `semaprax.graph.v58`, which is outside this evidence flow's admission",
        STRING_RUST_IMPORT_SCHEMA => "owned String Rust import programs select `semaprax.graph.v57`, which is outside this evidence flow's admission",
        OWNED_RUST_IMPORT_SCHEMA => "owned Rust import programs select `semaprax.graph.v56`, which is outside this evidence flow's admission",
        SELECTED_RUST_RESULT_SCHEMA => "selected Rust Result programs select `semaprax.graph.v55`, which is outside this evidence flow's admission",
        NATIVE_RUST_IMPORT_SCHEMA => "native Rust import programs select `semaprax.graph.v25`, which is outside this evidence flow's admission",
        _ => return None,
    };
    Some(Diagnostic::io("SPX-G410", message))
}

pub(crate) fn declares_selected_rust_result(interfaces: &[ResolvedInterface]) -> bool {
    interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| {
            import.index_selected && import.result.kind == ResolvedImportResultKind::ResultI64I64
        })
}

pub(crate) fn declares_selected_rust_method(interfaces: &[ResolvedInterface]) -> bool {
    interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.selected_receiver.is_some())
}

pub(crate) fn declares_selected_rust_import(interfaces: &[ResolvedInterface]) -> bool {
    interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.index_selected)
}

pub(crate) fn declares_indexed_rust_import(interfaces: &[ResolvedInterface]) -> bool {
    interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.rust_path.is_some())
}

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
pub(crate) fn result_text(kind: &ResolvedImportResultKind) -> &str {
    match kind {
        ResolvedImportResultKind::Unit => "unit",
        ResolvedImportResultKind::I64 => "i64",
        ResolvedImportResultKind::Bool => "bool",
        ResolvedImportResultKind::OwnedString => "string",
        ResolvedImportResultKind::OwnedOptionString => "Option<string>",
        ResolvedImportResultKind::OwnedResultStringI64 => "Result<string, i64>",
        ResolvedImportResultKind::OwnedResultStringOptionI64 => "Result<string, Option<i64>>",
        ResolvedImportResultKind::ResultI64I64 => "Result<i64, i64>",
        ResolvedImportResultKind::OwnedResultResourceI64 { .. } => "Result<resource, i64>",
        ResolvedImportResultKind::OwnedResource { resource } => resource.as_str(),
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
pub(crate) fn append_import_tail(
    output: &mut CappedString,
    schema: &str,
    native_rust: bool,
    rust_path: Option<&str>,
    selected_index_digest: Option<&str>,
    selected_receiver: Option<&str>,
) {
    if schema == NATIVE_RUST_IMPORT_SCHEMA
        || schema == INDEXED_RUST_IMPORT_SCHEMA
        || schema == SELECTED_RUST_IMPORT_SCHEMA
        || schema == SELECTED_RUST_METHOD_SCHEMA
        || schema == SELECTED_RUST_RESULT_SCHEMA
        || schema == OWNED_RUST_IMPORT_SCHEMA
        || schema == STRING_RUST_IMPORT_SCHEMA
        || schema == CONTAINER_RUST_IMPORT_SCHEMA
        || schema == NESTED_CONTAINER_RUST_IMPORT_SCHEMA
    {
        output.push_str(",\"native_rust\":");
        output.push_str(if native_rust { "true" } else { "false" });
    }
    if schema == INDEXED_RUST_IMPORT_SCHEMA
        || schema == SELECTED_RUST_IMPORT_SCHEMA
        || schema == SELECTED_RUST_METHOD_SCHEMA
        || schema == SELECTED_RUST_RESULT_SCHEMA
        || schema == OWNED_RUST_IMPORT_SCHEMA
        || schema == STRING_RUST_IMPORT_SCHEMA
        || schema == CONTAINER_RUST_IMPORT_SCHEMA
        || schema == NESTED_CONTAINER_RUST_IMPORT_SCHEMA
    {
        output.push_str(",\"rust_path\":");
        output.push_str(
            &rust_path
                .map(crate::diagnostic::quote_json)
                .unwrap_or_else(|| "null".to_owned()),
        );
    }
    if schema == SELECTED_RUST_IMPORT_SCHEMA
        || schema == SELECTED_RUST_METHOD_SCHEMA
        || schema == SELECTED_RUST_RESULT_SCHEMA
        || schema == OWNED_RUST_IMPORT_SCHEMA
        || schema == STRING_RUST_IMPORT_SCHEMA
        || schema == CONTAINER_RUST_IMPORT_SCHEMA
        || schema == NESTED_CONTAINER_RUST_IMPORT_SCHEMA
    {
        output.push_str(",\"selected_index_digest\":");
        output.push_str(
            &selected_index_digest
                .map(crate::diagnostic::quote_json)
                .unwrap_or_else(|| "null".to_owned()),
        );
    }
    if schema == SELECTED_RUST_METHOD_SCHEMA
        || schema == SELECTED_RUST_RESULT_SCHEMA
        || schema == OWNED_RUST_IMPORT_SCHEMA
        || schema == STRING_RUST_IMPORT_SCHEMA
        || schema == CONTAINER_RUST_IMPORT_SCHEMA
        || schema == NESTED_CONTAINER_RUST_IMPORT_SCHEMA
    {
        output.push_str(",\"rust_receiver\":");
        output.push_str(
            &selected_receiver
                .map(crate::diagnostic::quote_json)
                .unwrap_or_else(|| "null".to_owned()),
        );
    }
    output.push('}');
}
