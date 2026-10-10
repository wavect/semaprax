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
        if paired(program) {
            "semaprax.graph.v64"
        } else if mixed(program) {
            "semaprax.graph.v63"
        } else {
            "semaprax.graph.v62"
        }
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
    graph_schema_from_parts_and_instances_with_renewal_authority(i, t, f, x, n, None)
}

pub(crate) fn graph_schema_from_parts_and_instances_with_renewal_authority(
    i: &[hir::ResolvedInterface],
    t: &[hir::ResolvedTypeDeclaration],
    f: &[ResolvedFunction],
    x: &[hir::ResolvedFunctionTemplate],
    n: &[hir::ResolvedFunctionInstance],
    renewal_authority: Option<&dyn Fn(&ResolvedFunction) -> bool>,
) -> Result<&'static str, Diagnostic> {
    let prior = match renewal_authority {
        None => super::filesystem_outcome::graph_schema_from_parts_and_instances(i, t, f, x, n),
        Some(_) => {
            super::filesystem_outcome::graph_schema_from_parts_and_instances_with_renewal_authority(
                i,
                t,
                f,
                x,
                n,
                renewal_authority,
            )
        }
    }?;
    Ok(
        if f.iter()
            .chain(n.iter().map(|v| &v.function))
            .any(function_requires)
        {
            if f.iter()
                .chain(n.iter().map(|v| &v.function))
                .any(function_paired)
            {
                "semaprax.graph.v64"
            } else if f
                .iter()
                .chain(n.iter().map(|v| &v.function))
                .any(function_mixed)
            {
                "semaprax.graph.v63"
            } else {
                "semaprax.graph.v62"
            }
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
        graph.replace_range(
            ..prefix.len(),
            if paired(program) {
                "{\"schema\":\"semaprax.graph.v64\""
            } else if mixed(program) {
                "{\"schema\":\"semaprax.graph.v63\""
            } else {
                "{\"schema\":\"semaprax.graph.v62\""
            },
        );
    }
    Ok(graph)
}

fn function_requires(function: &ResolvedFunction) -> bool {
    let mut found = function.return_type.is_once_function()
        || function.params.iter().any(|p| p.ty.is_once_function());
    hir::function_value::walk(function, |expr| found |= expr.ty.is_once_function());
    found
}

fn function_mixed(function: &ResolvedFunction) -> bool {
    let mut found = function.return_type == ResolvedType::OnceFunctionI64
        || function
            .params
            .iter()
            .any(|p| p.ty == ResolvedType::OnceFunctionI64);
    hir::function_value::walk(function, |expr| {
        found |= expr.ty == ResolvedType::OnceFunctionI64
    });
    found
}
fn mixed(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_mixed)
}
fn function_paired(function: &ResolvedFunction) -> bool {
    let mut found = function.return_type == ResolvedType::OnceFunctionI64Pair
        || function
            .params
            .iter()
            .any(|p| p.ty == ResolvedType::OnceFunctionI64Pair);
    hir::function_value::walk(function, |expr| {
        found |= expr.ty == ResolvedType::OnceFunctionI64Pair
    });
    found
}
fn paired(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_paired)
}
