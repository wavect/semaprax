//! Graph v75 is selected only by scoped record-vector field reads.
use super::*;
const SCHEMA: &str = "semaprax.graph.v75";

fn function_requires(function: &ResolvedFunction) -> bool {
    std::iter::once(&function.body)
        .chain(function.requires.iter())
        .chain(function.ensures.iter())
        .any(crate::vec_field::expression_uses)
}
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_requires)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        hir::validate(program)?;
        crate::loan_plan::validate_program(program)?;
        Ok(SCHEMA)
    } else {
        super::owned_nested_outcome::graph_schema(program)
    }
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "scoped Vec field reads require Graph v75",
        ));
    }
    super::owned_nested_outcome::legacy_graph_schema(program)
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
    if functions
        .iter()
        .chain(instances.iter().map(|i| &i.function))
        .any(function_requires)
    {
        Ok(SCHEMA)
    } else {
        match renewal_authority { None => super::owned_nested_outcome::graph_schema_from_parts_and_instances(interfaces, types, functions, templates, instances), Some(_) => super::owned_nested_outcome::graph_schema_from_parts_and_instances_with_renewal_authority(interfaces, types, functions, templates, instances, renewal_authority), }
    }
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph =
        super::owned_nested_outcome::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v75\"");
    graph.pop();
    graph.push_str(",\"scoped_vec_field_reads\":{\"schema\":\"semaprax.scoped-vec-field-reads.v1\",\"authority\":false}}");
    Ok(graph)
}
pub(super) fn contract_json(
    element: &ResolvedType,
    field: &DeclarationId,
    bytes: bool,
    args: &[ResolvedExpr],
) -> Result<String, Diagnostic> {
    let arguments = args
        .iter()
        .map(agent_contract_expr_json)
        .collect::<Result<Vec<_>, _>>()?
        .budgeted_join(",");
    Ok(format!(
        "{{\"kind\":\"vec_field_read\",\"element_type\":{},\"field\":{},\"bytes\":{bytes},\"args\":[{arguments}]}}",
        type_json(element),
        quote_json(field.as_str())
    ))
}
pub(super) fn attach_provenance(mut base: String, provenance: &hir::ByteSliceProvenance) -> String {
    if let Some(selected) = &provenance.vector_field {
        base.pop();
        base.push_str(&format!(
            ",\"vector_field\":{{\"element_type\":{},\"field\":{},\"index\":{}}}}}",
            type_json(&selected.element),
            quote_json(selected.field.as_str()),
            quote_json(selected.index.as_str())
        ));
    }
    base
}

#[cfg(test)]
mod tests;
