//! Checked source Agent associations retained by the workspace graph.
//! These descriptive facts confer no runtime or publication authority.
use crate::ast::Program;
use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;

pub(super) const SCHEMA: &str = "semaprax.workspace-semantic-graph.v4";

pub(super) fn facts(program: &Program) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut count = 0usize;
    let mut reservation = 0usize;
    for agent in program.agents.iter().filter(|a| a.has_execution_metadata()) {
        count += 1;
        // Source identifiers are bounded, but reserve the full JSON escaping
        // upper bound before the fact producer allocates any output rows.
        let text = agent
            .operations
            .iter()
            .try_fold(agent.stable_id.len(), |n, op| {
                n.checked_add(op.stable_id.len().checked_mul(2)?)
            })
            .and_then(|n| {
                n.checked_add(agent.model_wait.as_ref().map_or(0, |b| b.helper_id.len()))
            });
        reservation = text
            .and_then(|n| n.checked_mul(6))
            .and_then(|n| n.checked_add(2048))
            .and_then(|n| reservation.checked_add(n))
            .ok_or_else(capacity)?;
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    if count > 64 {
        return Err(capacity());
    }
    reservation = count
        .checked_mul(std::mem::size_of::<String>())
        .and_then(|n| reservation.checked_add(n))
        .ok_or_else(capacity)?;
    super::reserve_builder_structure(reservation)?;
    crate::graph::agent_execution_facts(program).map_err(|e| vec![e])
}

fn capacity() -> Vec<Diagnostic> {
    vec![super::limit_error(
        "builder_bytes",
        super::active_builder_limit(),
    )]
}

pub(super) fn has_facts(modules: &[super::WorkspaceGraphProjectionModule]) -> bool {
    modules.iter().any(|module| {
        module
            .session_protocol_facts
            .iter()
            .any(|row| row.starts_with("{\"agent\":"))
    })
}

pub(super) fn schema(
    base: &'static str,
    modules: &[super::WorkspaceGraphProjectionModule],
) -> &'static str {
    if has_facts(modules) {
        SCHEMA
    } else {
        base
    }
}

pub(super) fn render_trailing(
    base: &str,
    modules: &[super::WorkspaceGraphProjectionModule],
) -> String {
    if !has_facts(modules) {
        return String::new();
    }
    let mut out = CappedString::new();
    out.push_str(",\"agent_execution\":{\"base_schema\":");
    super::push_json_string(&mut out, base);
    out.push_str(",\"authority\":\"none\",\"agents\":[");
    let mut first = true;
    for module in modules {
        for row in module
            .session_protocol_facts
            .iter()
            .filter(|row| row.starts_with("{\"agent\":"))
        {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("{\"module\":");
            super::push_json_string(&mut out, &module.module);
            out.push_str(",\"path\":");
            super::push_json_string(&mut out, &module.path);
            out.push(',');
            // Borrow the exact checked row; no clone or side vector allocation.
            out.push_str(&row[1..]);
        }
    }
    out.push_str("]}");
    out.into_string()
}

pub(super) fn clone_package_facts(facts: &[String]) -> Result<Vec<String>, Vec<Diagnostic>> {
    if !facts.iter().any(|row| row.starts_with("{\"agent\":")) {
        return Ok(facts.to_vec()); // Preserve legacy construction accounting.
    }
    let carrier = facts
        .len()
        .checked_mul(std::mem::size_of::<String>())
        .ok_or_else(capacity)?;
    let payload = facts
        .iter()
        .try_fold(carrier, |n, row| n.checked_add(row.len()))
        .ok_or_else(capacity)?;
    super::reserve_builder_structure(payload)?;
    let mut retained = Vec::with_capacity(facts.len());
    let actual = retained
        .capacity()
        .checked_mul(std::mem::size_of::<String>())
        .ok_or_else(capacity)?;
    if actual > carrier {
        super::reserve_builder_structure(actual - carrier)?;
    }
    for row in facts {
        retained.push(crate::bounded_output::budgeted_clone(row));
    }
    Ok(retained)
}
