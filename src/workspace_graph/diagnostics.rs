//! Shared bounded workspace-graph diagnostic constructors.

use crate::ast::{ModuleUse, Program, Span};
use crate::diagnostic::Diagnostic;

use super::{
    WorkspaceResolvedModule, MAX_BUILDER_BYTES, MAX_CALLABLES, MAX_CALLS, MAX_CROSS_FILE_EDGES,
    MAX_DECLARATIONS, MAX_DEPENDENCY_DEPTH, MAX_ENTRY_MODULE_BYTES, MAX_FILES, MAX_OUTPUT_BYTES,
    MAX_TOTAL_SOURCE_BYTES, MAX_USES,
};

/// Canonical shared wire object for every artifact that embeds Workspace
/// Semantic Graph limits. The values derive from the limits enforced by the
/// graph builder, rather than being separately restated by each consumer.
pub(crate) fn push_workspace_graph_limits(output: &mut crate::bounded_output::CappedString) {
    use std::fmt::Write as _;

    write!(
        output,
        "{{\"max_managed_files\":{MAX_FILES},\"max_reachable_modules\":{MAX_FILES},\"max_entry_module_bytes\":{MAX_ENTRY_MODULE_BYTES},\"max_total_source_bytes\":{MAX_TOTAL_SOURCE_BYTES},\"max_declarations\":{MAX_DECLARATIONS},\"max_callables\":{MAX_CALLABLES},\"max_call_sites\":{MAX_CALLS},\"max_uses\":{MAX_USES},\"max_resolved_cross_file_edges\":{MAX_CROSS_FILE_EDGES},\"max_dependency_depth\":{MAX_DEPENDENCY_DEPTH},\"max_builder_bytes\":{MAX_BUILDER_BYTES},\"max_manifest_bytes\":1048576,\"max_output_bytes\":{MAX_OUTPUT_BYTES},\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}}"
    )
    .expect("writing to a string cannot fail");
}

pub(super) fn use_error(program: &Program, module_use: &ModuleUse, message: &str) -> Diagnostic {
    Diagnostic::error("SPX-G172", message, module_use.span).at_path(&program.path)
}

pub(super) fn graph_error(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(code, message)
}

pub(super) fn project_function_error(
    module: &WorkspaceResolvedModule,
    message: impl Into<String>,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::error(
        "SPX-G172",
        message,
        span.unwrap_or(Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }),
    )
    .at_path(&module.path)
}

pub(super) fn limit_error(field: &'static str, maximum: usize) -> Diagnostic {
    graph_error(
        "SPX-G171",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Semantic Graph `{field}` exceeds {maximum}"
        )),
    )
}
