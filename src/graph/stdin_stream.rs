//! Additive Graph v65 projection of independently derived streaming facts.
use super::*;
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::affine::graph_schema(program)?;
    if crate::stdin_stream_ops::resolved_program_uses(program) {
        crate::stdin_stream_ops::analysis::derive(program)?;
        Ok("semaprax.graph.v65")
    } else {
        Ok(previous)
    }
}
pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let previous = super::affine::graph_schema_from_parts_and_instances(
        interfaces, types, functions, templates, instances,
    )?;
    Ok(
        if functions
            .iter()
            .chain(instances.iter().map(|instance| &instance.function))
            .any(crate::stdin_stream_ops::resolved_function_uses)
        {
            "semaprax.graph.v65"
        } else {
            previous
        },
    )
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::affine::graph_json(program, revision, functions, types, view)?;
    if !crate::stdin_stream_ops::resolved_program_uses(program) {
        return Ok(graph);
    }
    let facts = crate::stdin_stream_ops::analysis::derive(program)?;
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v65\"");
    let list = |values: &[hir::ExpressionId]| {
        values
            .iter()
            .map(|id| quote_json(id.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    };
    let entries = facts.iter().filter(|(id, _)| functions.contains(*id)).map(|(id, fact)| format!(
        "{{\"function\":{},\"open_path_bound\":{},\"forwarding_parameter\":{},\"open_sites\":[{}],\"next_sites\":[{}],\"chunk_sites\":[{}],\"eof_sites\":[{}]}}",
        quote_json(id.as_str()), fact.open_bound, fact.forwarding_parameter.as_ref().map_or_else(|| "null".to_owned(), |id| quote_json(id.as_str())), list(&fact.opens), list(&fact.nexts), list(&fact.chunks), list(&fact.eofs)
    )).collect::<Vec<_>>().join(",");
    graph.pop();
    graph.push_str(&format!(",\"stdin_stream\":{{\"schema\":\"semaprax.stdin-stream.v1\",\"reader\":{},\"drop\":{},\"profile\":{},\"input\":{},\"buffer_bytes\":{},\"buffer_count\":1,\"open_prefills\":true,\"true_eof_length\":0,\"short_positive_is_chunk\":true,\"failure\":{{\"domain\":\"semaprax.command-input.v1\",\"code\":3}},\"chunk_root\":\"stdin_stream_reader\",\"functions\":[{}]}}}}", quote_json(crate::stdin_stream_ops::READER_ID), quote_json(crate::stdin_stream_ops::DROP_ID), quote_json(crate::stdin_stream_ops::PROFILE), quote_json(crate::stdin_stream_ops::INPUT_PROFILE), crate::stdin_stream_ops::CHUNK_BYTES, entries));
    Ok(graph)
}
