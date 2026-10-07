//! Graph v67 composes whole String replacement with all previous facts.
use super::*;
const SCHEMA: &str = "semaprax.graph.v67";
fn requires(function: &ResolvedFunction) -> bool {
    crate::string_ops::replacement::requires(function)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::vec_loop_renewal::graph_schema(program)?;
    Ok(
        if program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
            .any(requires)
        {
            SCHEMA
        } else {
            previous
        },
    )
}
pub(crate) fn graph_schema_from_parts_and_instances(
    i: &[hir::ResolvedInterface],
    t: &[hir::ResolvedTypeDeclaration],
    f: &[ResolvedFunction],
    x: &[hir::ResolvedFunctionTemplate],
    n: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let previous = super::vec_loop_renewal::graph_schema_from_parts_and_instances(i, t, f, x, n)?;
    Ok(
        if f.iter().chain(n.iter().map(|i| &i.function)).any(requires) {
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
    let mut graph = super::vec_loop_renewal::graph_json(program, revision, functions, types, view)?;
    if !program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(requires)
    {
        return Ok(graph);
    }
    let doc: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = doc["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v67\"");
    let mut updates = Vec::new();
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .filter(|f| functions.contains(&f.id))
    {
        for (at, binding) in crate::string_ops::replacement::bindings(function) {
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
        ",\"string_replacement\":{{\"schema\":\"semaprax.string-replacement.v1\",\"updates\":[{}]}}}}",
        updates.join(",")
    ));
    Ok(graph)
}
