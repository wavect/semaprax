//! Shared bounded workspace-graph diagnostic constructors.

use crate::ast::{ModuleUse, Program, Span};
use crate::diagnostic::Diagnostic;

use super::WorkspaceResolvedModule;

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
