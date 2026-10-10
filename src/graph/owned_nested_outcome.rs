//! Graph v74 binds proper owning nested outcomes and case-qualified record cleanup.
use super::*;
const SCHEMA: &str = "semaprax.graph.v74";

pub(super) fn requires(program: &ResolvedProgram) -> bool {
    hir::collection_outcome::nested::program_requires_profile(program)
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        hir::validate(program)?;
        crate::loan_plan::validate_program(program)?;
        Ok(SCHEMA)
    } else {
        super::projected_string_view::graph_schema(program)
    }
}

pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "owning nested outcomes require Graph v74",
        ));
    }
    super::projected_string_view::legacy_graph_schema(program)
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
        .any(|function| {
            hir::collection_outcome::nested::function_requires_profile_by(function, |id| {
                types.iter().find(|item| &item.id == id)
            })
        })
    {
        // Metadata projection has no execution authority. Retained complete
        // programs still undergo independent HIR/source/cleanup/loan replay.
        Ok(SCHEMA)
    } else {
        match renewal_authority { None => super::projected_string_view::graph_schema_from_parts_and_instances(interfaces, types, functions, templates, instances), Some(_) => super::projected_string_view::graph_schema_from_parts_and_instances_with_renewal_authority(interfaces, types, functions, templates, instances, renewal_authority), }
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
        super::projected_string_view::graph_json(program, revision, functions, types, view)?;
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
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v74\"");
    graph.pop();
    graph.push_str(",\"owned_nested_outcomes\":{\"schema\":\"semaprax.owned-nested-outcomes.v1\",\"authority\":false}}");
    Ok(graph)
}
