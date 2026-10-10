//! Graph v73 binds authenticated projected String views to their full rooted path.
use super::*;
const SCHEMA: &str = "semaprax.graph.v73";

fn function_requires(function: &ResolvedFunction) -> bool {
    let mut found = false;
    hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::BorrowPlace { operation, place } = &expression.kind {
            found |= !place.projections.is_empty()
                && matches!(
                    operation.as_str(),
                    crate::byte_ops::STRING_AS_STR_ID | crate::byte_ops::STR_AS_BYTES_ID
                );
        }
    });
    found
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
        super::owned_collection_records::graph_schema(program)
    }
}

pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "projected String views require Graph v73",
        ));
    }
    super::owned_collection_records::legacy_graph_schema(program)
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
        // Metadata projection has no execution authority. Retained complete
        // programs still undergo independent HIR/source/cleanup/loan replay.
        Ok(SCHEMA)
    } else {
        match renewal_authority { None => super::owned_collection_records::graph_schema_from_parts_and_instances(interfaces, types, functions, templates, instances), Some(_) => super::owned_collection_records::graph_schema_from_parts_and_instances_with_renewal_authority(interfaces, types, functions, templates, instances, renewal_authority), }
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
        super::owned_collection_records::graph_json(program, revision, functions, types, view)?;
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
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v73\"");
    graph.pop();
    graph.push_str(",\"projected_string_views\":{\"schema\":\"semaprax.projected-string-views.v1\",\"authority\":false}}");
    Ok(graph)
}
