//! Issue #297 follow-on (R21): endpoint typestate `follows` bindings in the
//! per-source semantic graph (`semaprax.graph.v49`) and in `context`'s
//! `session_protocol` facet.
//!
//! Graph v49 is selected only when the program contains at least one
//! function that opts in with `follows session protocol "<id>"`. It is the
//! program's otherwise-selected graph document (v48,
//! `session_protocol_decl::GRAPH_SCHEMA`, when the program also declares a
//! session protocol -- the only way a `follows` clause can be admitted at
//! all, since `SPX-K107` refuses naming a protocol this same module does not
//! declare), byte for byte, with two changes: the header names
//! `semaprax.graph.v49`, and one trailing `session_protocol_follows` object
//! records the base schema it extends plus one canonical fact per opted-in
//! function (`crate::session_protocol::source::follows_json`). A program
//! with no `follows` clause -- including one that declares protocols but
//! never opts a function in -- keeps its v48 (or ordinary) schema and bytes
//! unchanged, exactly like `session_protocol_decl` leaves a protocol-free
//! program unchanged.
//!
//! Every emitted fact is bound first: each `follows` clause must still name
//! a protocol declared in this same program
//! (`crate::session_protocol::source::bind_follows`), so no projection can
//! describe a binding the compiler did not check.

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::session_protocol::source;

use super::{AgentContextFilter, AgentContextOptions};

pub(crate) const GRAPH_SCHEMA: &str = "semaprax.graph.v49";

fn has_follows(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| function.follows.is_some())
}

/// Attach `follows` bindings to an already rendered graph document, which
/// may already carry v48's own `session_protocols` section
/// ([`session_protocol_decl::attach`] always runs first).
pub(super) fn attach(program: &Program, mut graph: String) -> Result<String, Diagnostic> {
    if !has_follows(program) {
        return Ok(graph);
    }
    source::bind_follows(program)?;
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
    let schema = if program
        .session_protocols
        .iter()
        .any(|protocol| protocol.endpoint.is_some())
    {
        "semaprax.graph.v51"
    } else {
        GRAPH_SCHEMA
    };
    graph.replace_range(..prefix.len(), &format!("{{\"schema\":\"{schema}\""));
    graph.pop();
    graph.push_str(&format!(
        ",\"session_protocol_follows\":{{\"base_schema\":{},\"authority\":\"none\",\"bindings\":{}}}}}",
        quote_json(&base),
        source::follows_facts_json(program)
    ));
    Ok(graph)
}

/// `context` options carrying the queried program's `follows` bindings when
/// the `session_protocol` filter is selected and at least one exists;
/// otherwise unchanged. Runs after [`session_protocol_decl::context_options`]
/// so both facets can be present together.
pub(super) fn context_options(
    program: &Program,
    options: &AgentContextOptions,
) -> Result<AgentContextOptions, Diagnostic> {
    let mut options = options.clone();
    if options
        .filters
        .contains(&AgentContextFilter::SessionProtocol)
        && has_follows(program)
    {
        source::bind_follows(program)?;
        options.follows_bindings = source::follows_facts_json(program);
    }
    Ok(options)
}

#[cfg(test)]
#[path = "session_protocol_follows_tests.rs"]
mod tests;
