//! Graph v45 binds Bytes iteration; additive v66 binds owned-record iteration.
use super::*;
const RECORD_GRAPH_SCHEMA: &str = "semaprax.graph.v66";
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    crate::iterator_ops::resolved_program_uses_owned_iterator(program)
        || crate::iterator_ops::resolved_program_uses_record_iterator(program)
        || program.function_templates.iter().any(|template| {
            crate::iterator_ops::template_uses_owned_iterator(template)
                || crate::iterator_ops::template_uses_record_iterator(template)
        })
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::process::graph_schema(program)?;
    Ok(
        if crate::iterator_ops::resolved_program_uses_record_iterator(program) {
            RECORD_GRAPH_SCHEMA
        } else if requires(program) {
            "semaprax.graph.v45"
        } else {
            previous
        },
    )
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
            .any(crate::iterator_ops::function_uses_record_iterator)
            || templates
                .iter()
                .any(crate::iterator_ops::template_uses_record_iterator)
        {
            RECORD_GRAPH_SCHEMA
        } else if functions
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
    graph.replace_range(
        ..prefix.len(),
        if crate::iterator_ops::resolved_program_uses_record_iterator(program) {
            "{\"schema\":\"semaprax.graph.v66\""
        } else {
            "{\"schema\":\"semaprax.graph.v45\""
        },
    );
    graph.pop();
    if crate::iterator_ops::resolved_program_uses_record_iterator(program) {
        graph.push_str(",\"owned_iterator_payloads\":{\"schema\":\"semaprax.owned-record-iterator.v3\",\"element\":\"explicit-record(two:Bytes,one:CopyScalar)\",\"initialized_window\":\"[cursor,length)\",\"detached_prefix\":\"[0,cursor)\",\"authority\":\"distinct-from-vec\",\"yield_owners\":[\"core.iter-step.yield.item\",\"core.iter-step.yield.rest\"],\"commit\":\"validate-then-detach-record-and-successor\",\"drop_order\":\"remaining-index-and-field-declaration-order-then-backing\",\"cleanup_schema\":\"semaprax.cleanup-plan.v13\"}}");
    } else {
        graph.push_str(",\"owned_iterator_payloads\":{\"schema\":\"semaprax.owned-iterator-payloads.v2\",\"element\":\"Bytes\",\"initialized_window\":\"[cursor,length)\",\"detached_prefix\":\"[0,cursor)\",\"authority\":\"distinct-from-vec\",\"yield_owners\":[\"core.iter-step.yield.item\",\"core.iter-step.yield.rest\"],\"commit\":\"validate-then-detach-item-and-successor\",\"drop_order\":\"remaining-index-order-then-backing\",\"cleanup_schema\":\"semaprax.cleanup-plan.v13\"}}");
    }
    Ok(graph)
}
