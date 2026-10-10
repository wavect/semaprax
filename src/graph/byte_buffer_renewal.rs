//! Graph v70 composes authenticated same-owner byte-buffer renewal with all prior facts.
use super::*;
const SCHEMA: &str = "semaprax.graph.v70";

fn requires(function: &ResolvedFunction) -> bool {
    crate::byte_ops::requires_same_owner_set(function)
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::owned_text_record_loans::graph_schema(program)?;
    Ok(
        if program
            .functions
            .iter()
            .chain(
                program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .any(requires)
        {
            SCHEMA
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
    graph_schema_from_parts_and_instances_with_renewal_authority(
        interfaces, types, functions, templates, instances, None,
    )
}

pub(crate) fn graph_schema_from_parts_and_instances_with_renewal_authority(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
    renewal_authority: Option<&dyn Fn(&ResolvedFunction) -> bool>,
) -> Result<&'static str, Diagnostic> {
    let previous = match renewal_authority { None => super::owned_text_record_loans::graph_schema_from_parts_and_instances(interfaces, types, functions, templates, instances), Some(_) => super::owned_text_record_loans::graph_schema_from_parts_and_instances_with_renewal_authority(interfaces, types, functions, templates, instances, renewal_authority), }?;
    Ok(
        if functions
            .iter()
            .chain(instances.iter().map(|instance| &instance.function))
            .any(requires)
        {
            SCHEMA
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
    let mut graph =
        super::owned_text_record_loans::graph_json(program, revision, functions, types, view)?;
    if !program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(requires)
    {
        return Ok(graph);
    }
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v70\"");
    let mut updates = Vec::new();
    for function in program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .filter(|function| functions.contains(&function.id))
    {
        for (at, binding) in crate::byte_ops::same_owner_set_bindings(function) {
            updates.push(format!(
                "{{\"function\":{},\"at\":{},\"binding\":{}}}",
                quote_json(function.id.as_str()),
                quote_json(at.as_str()),
                quote_json(binding.id.as_str())
            ));
        }
    }
    graph.pop();
    graph.push_str(&format!(
        ",\"byte_buffer_renewal\":{{\"schema\":\"semaprax.byte-buffer-renewal.v1\",\"updates\":[{}]}}}}",
        updates.join(",")
    ));
    Ok(graph)
}
