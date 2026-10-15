//! Graph v77 carries independently reconstructed immutable literal producers.
use super::*;
pub(crate) const SCHEMA: &str = "semaprax.graph.v77";

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    // Frozen composition guards retain their diagnostic priority. Static-text
    // validation still precedes publication of either selected schema.
    let previous = super::byte_helper_renewal::graph_schema(program)?;
    if hir::validate_with_static_text(program)? {
        Ok(SCHEMA)
    } else {
        Ok(previous)
    }
}

pub(crate) fn graph_schema_validated(
    program: &ResolvedProgram,
    selected: bool,
) -> Result<&'static str, Diagnostic> {
    if selected {
        Ok(SCHEMA)
    } else {
        super::byte_helper_renewal::graph_schema(program)
    }
}

pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if hir::validate_with_static_text(program)? {
        return Err(Diagnostic::io(
            "SPX-G410",
            "static borrowed UTF-8 literals require Graph v77",
        ));
    }
    super::scoped_vec_field::legacy_graph_schema(program)
}

pub(crate) fn reject_frozen<'a>(
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    templates: &'a (impl crate::hir::items::ItemSource<hir::ResolvedFunctionTemplate> + ?Sized),
) -> Result<(), Diagnostic> {
    let mut selected = false;
    let roots = functions
        .into_iter()
        .flat_map(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
        })
        .chain(templates.iter().flat_map(|template| {
            template
                .requires
                .iter()
                .chain(std::iter::once(&template.body))
                .chain(&template.ensures)
        }));
    hir::static_text::visit_roots(roots, |_, _, literal| {
        selected |= literal;
        Ok(())
    })?;
    if selected {
        return Err(Diagnostic::io(
            "SPX-G410",
            "static borrowed UTF-8 literals need complete current source/HIR authority",
        ));
    }
    Ok(())
}

pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph =
        super::byte_helper_renewal::graph_json(program, revision, functions, types, view)?;
    if !view.static_text() {
        return Ok(graph);
    }
    graph_schema_validated(program, true)?;
    let mut count = 0usize;
    hir::static_text::visit_literals(program, |_, _, literal| {
        if literal {
            count = count
                .checked_add(1)
                .ok_or_else(|| Diagnostic::io("SPX-G411", "static literal inventory overflows"))?;
        }
        Ok(())
    })?;
    let bytes = count
        .checked_mul(std::mem::size_of::<String>())
        .ok_or_else(|| Diagnostic::io("SPX-G411", "static literal carrier overflows"))?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(Diagnostic::io(
            "SPX-G411",
            "static literal inventory exceeds private capacity",
        ));
    }
    let mut constants = Vec::with_capacity(count);
    hir::static_text::visit_literals(program, |expression, value, literal| {
        if literal {
            constants.push(format!(
                "{{\"site\":{},\"byte_length\":{},\"value\":{}}}",
                quote_json(expression.id.as_str()),
                value.len(),
                quote_json(value)
            ));
        }
        Ok(())
    })?;
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v77\"");
    graph.pop();
    graph.push_str(&format!(",\"static_borrowed_utf8\":{{\"schema\":\"semaprax.static-borrowed-utf8.v1\",\"authority\":false,\"max_decoded_pool_bytes\":65536,\"literals\":[{}]}}}}", constants.budgeted_join(",")));
    Ok(graph)
}

pub(super) fn literal_json(value: &str) -> String {
    format!("{{\"kind\":\"borrowed_utf8_literal\",\"root_kind\":\"source_expr_static\",\"byte_length\":{},\"value\":{},\"display\":{}}}", value.len(), quote_json(value), quote_json(&crate::format::canonical_borrowed_utf8_literal(value)))
}
