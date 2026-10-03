//! Final graph v62 wrapper for checked affine owned closure carriers.
use super::*;
fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_requires)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    Ok(if requires(program) {
        "semaprax.graph.v62"
    } else {
        super::filesystem_outcome::graph_schema(program)?
    })
}
pub(crate) fn graph_schema_from_parts_and_instances(
    i: &[hir::ResolvedInterface],
    t: &[hir::ResolvedTypeDeclaration],
    f: &[ResolvedFunction],
    x: &[hir::ResolvedFunctionTemplate],
    n: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let prior = super::filesystem_outcome::graph_schema_from_parts_and_instances(i, t, f, x, n)?;
    Ok(
        if f.iter()
            .chain(n.iter().map(|v| &v.function))
            .any(function_requires)
        {
            "semaprax.graph.v62"
        } else {
            prior
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
        super::filesystem_outcome::graph_json(program, revision, functions, types, view)?;
    if requires(program) {
        let prefix = format!(
            "{{\"schema\":{}",
            quote_json(
                serde_json::from_str::<serde_json::Value>(&graph)
                    .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?
                    ["schema"]
                    .as_str()
                    .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?
            )
        );
        if !graph.starts_with(&prefix) {
            return Err(Diagnostic::io(
                "SPX-G411",
                "checked graph header is not canonical",
            ));
        }
        graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v62\"");
    }
    Ok(graph)
}

fn function_requires(function: &ResolvedFunction) -> bool {
    let mut found = function.return_type == ResolvedType::OnceFunction
        || function
            .params
            .iter()
            .any(|p| p.ty == ResolvedType::OnceFunction);
    hir::function_value::walk(function, |expr| {
        found |= expr.ty == ResolvedType::OnceFunction
    });
    found
}
