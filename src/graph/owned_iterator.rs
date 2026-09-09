//! Graph v45 binds the independent item and remainder owners of Bytes iteration.
use super::*;
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    crate::iterator_ops::resolved_program_uses_owned_iterator(program)
        || program
            .function_templates
            .iter()
            .any(crate::iterator_ops::template_uses_owned_iterator)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::process::graph_schema(program)?;
    Ok(if requires(program) {
        "semaprax.graph.v45"
    } else {
        previous
    })
}
pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let previous = super::process::graph_schema_from_parts_and_instances(
        interfaces, types, functions, templates, instances,
    )?;
    Ok(
        if functions
            .iter()
            .chain(instances.iter().map(|i| &i.function))
            .any(crate::iterator_ops::function_uses_owned_iterator)
            || templates
                .iter()
                .any(crate::iterator_ops::template_uses_owned_iterator)
        {
            "semaprax.graph.v45"
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
    let mut graph = super::process::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let schema = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(schema));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v45\"");
    graph.pop();
    graph.push_str(",\"owned_iterator_payloads\":{\"schema\":\"semaprax.owned-iterator-payloads.v2\",\"element\":\"Bytes\",\"initialized_window\":\"[cursor,length)\",\"detached_prefix\":\"[0,cursor)\",\"authority\":\"distinct-from-vec\",\"yield_owners\":[\"core.iter-step.yield.item\",\"core.iter-step.yield.rest\"],\"commit\":\"validate-then-detach-item-and-successor\",\"drop_order\":\"remaining-index-order-then-backing\",\"cleanup_schema\":\"semaprax.cleanup-plan.v13\"}}");
    Ok(graph)
}
