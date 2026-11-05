//! Complete original owning mapper bodies; only lexical visibility adapts.

pub(super) mod change {
use crate::diagnostic::Diagnostic;

use crate::semantic_workspace;

pub(in crate::workspace_graph::initial_report) fn map_change_builder_limit(
    diagnostics: Vec<Diagnostic>,
    internal_maximum: usize,
) -> Vec<Diagnostic> {
    let expected = crate::bounded_output::budgeted_format(format_args!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}"
    ));
    if diagnostics.len() == 1
        && diagnostics[0].code == "SPX-G171"
        && diagnostics[0].message == expected
    {
        limit(
            "total_analysis_builder_bytes",
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        )
    } else {
        diagnostics
    }
}

fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G183",
        crate::bounded_output::budgeted_format(format_args!(
            "Semantic Workspace Change `{field}` exceeds {maximum}"
        )),
    )]
}
}

pub(super) mod structural {
use crate::diagnostic::Diagnostic;

pub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16 * 1024 * 1024;

pub(in crate::workspace_graph::initial_report) const MAX_ANALYSIS_BUILDER_BYTES: usize = 32 * 1024 * 1024;

pub(in crate::workspace_graph::initial_report) fn map_candidate_builder_limit(
    diagnostics: Vec<Diagnostic>,
    internal_maximum: usize,
) -> Vec<Diagnostic> {
    let expected = crate::bounded_output::budgeted_format(format_args!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}"
    ));
    if diagnostics.len() == 1
        && diagnostics[0].code == "SPX-G171"
        && diagnostics[0].message == expected
    {
        limit(
            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
                "candidate_graph_builder_bytes"
            } else {
                "analysis_builder_bytes"
            },
            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES
            } else {
                MAX_ANALYSIS_BUILDER_BYTES
            },
        )
    } else {
        diagnostics
    }
}

fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G191",
        crate::bounded_output::budgeted_format(format_args!(
            "Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}"
        )),
    )]
}
}

pub(super) mod operations {
use crate::diagnostic::Diagnostic;

pub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16_777_216;

pub(in crate::workspace_graph::initial_report) const MAX_OPERATIONS_BUILDER_BYTES: usize = 67_108_864;

pub(in crate::workspace_graph::initial_report) fn map_candidate_diagnostics(
    diagnostics: Vec<Diagnostic>,
    candidate_graph_limit: usize,
    operations_builder_limit: usize,
) -> Vec<Diagnostic> {
    if diagnostics.len() != 1 || diagnostics[0].code != "SPX-G171" {
        return diagnostics;
    }
    let builder_message =
        format!("Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}");
    if diagnostics[0].message == builder_message {
        return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
            limit(
                "candidate_graph_builder_bytes",
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            )
        } else {
            limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
        };
    }
    let change_message = format!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {}",
        operations_builder_limit
    );
    if diagnostics[0].message == change_message {
        return limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES);
    }
    diagnostics
}

fn limit(f: &str, m: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G199",
        format!("Semantic Workspace Operations exceeds {f} maximum {m}"),
    )]
}
}
