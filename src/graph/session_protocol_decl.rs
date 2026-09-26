//! Issue #297: declared `.spx` session protocols in the per-source semantic
//! graph (`semaprax.graph.v48`) and in `context`'s `session_protocol` facet.
//!
//! Graph v48 is selected only when the program declares at least one session
//! protocol. It is the program's otherwise-selected graph document, byte for
//! byte, with two changes: the header names `semaprax.graph.v48`, and one
//! trailing `session_protocols` object records the base schema it extends
//! plus one canonical fact per declaration
//! (`crate::session_protocol::source::declaration_json`). A program without a
//! declaration keeps its existing schema and bytes.
//!
//! Every emitted fact is bound first: each `via` must name a function the
//! checked HIR built from the same program retains
//! (`session_protocol::source::bind_to_hir`), so no projection can describe a
//! realization the compiler did not check.

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::session_protocol::source;

use super::{AgentContextFilter, AgentContextOptions};

pub(crate) const GRAPH_SCHEMA: &str = "semaprax.graph.v48";

/// Attach declared session protocols to an already rendered graph document.
pub(super) fn attach(
    program: &Program,
    resolved: &ResolvedProgram,
    mut graph: String,
) -> Result<String, Diagnostic> {
    if program.session_protocols.is_empty() {
        return Ok(graph);
    }
    source::bind_to_hir(program, resolved)?;
    let base = serde_json::from_str::<serde_json::Value>(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?
        .to_owned();
    let prefix = format!("{{\"schema\":{}", quote_json(&base));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io(
            "SPX-G411",
            "checked graph header is not canonical",
        ));
    }
    graph.replace_range(..prefix.len(), &format!("{{\"schema\":\"{GRAPH_SCHEMA}\""));
    graph.pop();
    graph.push_str(&format!(
        ",\"session_protocols\":{{\"base_schema\":{},\"authority\":\"none\",\"declarations\":{}}}}}",
        quote_json(&base),
        source::declarations_json(&program.session_protocols)
    ));
    Ok(graph)
}

/// `context` options carrying the queried program's bound declarations when
/// the `session_protocol` filter is selected; otherwise unchanged.
pub(super) fn context_options(
    program: &Program,
    resolved: &ResolvedProgram,
    options: &AgentContextOptions,
) -> Result<AgentContextOptions, Diagnostic> {
    let mut options = options.clone();
    if options
        .filters
        .contains(&AgentContextFilter::SessionProtocol)
        && !program.session_protocols.is_empty()
    {
        source::bind_to_hir(program, resolved)?;
        options.declared_session_protocols = source::declarations_json(&program.session_protocols);
    }
    Ok(options)
}

#[cfg(test)]
#[path = "session_protocol_decl_tests.rs"]
mod tests;
