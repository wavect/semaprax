//! Final graph v46 wrapper for checked filesystem publication outcomes.
use super::*;
fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(super::filesystem::function_requires_v3)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    Ok(if requires(program) {
        "semaprax.graph.v46"
    } else {
        super::owned_iterator::graph_schema(program)?
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
        None => super::owned_iterator::graph_schema_from_parts_and_instances(i, t, f, x, n),
        Some(_) => {
            super::owned_iterator::graph_schema_from_parts_and_instances_with_renewal_authority(
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
            .any(super::filesystem::function_requires_v3)
        {
            "semaprax.graph.v46"
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
    let mut graph = super::owned_iterator::graph_json(program, revision, functions, types, view)?;
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
        graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v46\"");
    }
    Ok(graph)
}

#[cfg(test)]
mod tests {
    #[test]
    fn checked_outcome_graph_roundtrips_and_preserves_environment_facts() {
        for environment in [false, true] {
            let extra = if environment {
                "@id(\"checked.env\") fn snapshot_size()->usize uses {process.environment.read} {env_len()}"
            } else {
                ""
            };
            let text = format!(
                r#"module checked.graph;
permit {{fs.write, process.environment.read}}
@id("checked.main") fn main()->i64 {{0}}
@id("checked.run") fn run()->bool uses {{fs.write}} {{
 let path=[97u8]; let data=[65u8];
 file_write_atomic_checked(array_as_slice(path),1usize,array_as_slice(data),1usize)==0usize
}}
{extra}
"#
            );
            let source = crate::check(&text, "checked-graph.spx").unwrap();
            let mut forged_hir = crate::hir::resolve(&source).unwrap();
            forged_hir
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "checked.run")
                .unwrap()
                .effects
                .clear();
            let error = crate::hir::validate(&forged_hir).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert!(error.message.contains("undeclared effect `fs.write`"));
            let graph = crate::graph::to_json(&source).unwrap();
            let value: serde_json::Value = serde_json::from_str(&graph).unwrap();
            assert_eq!(value["schema"], "semaprax.graph.v46");
            assert_eq!(value["filesystem"]["schema"], "semaprax.filesystem.v3");
            assert_eq!(value["bounded_environment_io"].is_object(), environment);
            crate::graph::verify_json(&source, &graph).unwrap();
            assert!(crate::graph::to_legacy_json(&source).is_err());
            let mut forged = value;
            forged["filesystem"]["checked_atomic_outcomes"] = serde_json::json!([0, 1, 3]);
            assert!(
                crate::graph::verify_json(&source, &serde_json::to_string(&forged).unwrap())
                    .is_err()
            );
            assert!(
                crate::graph::verify_json(
                    &source,
                    &graph.replacen("semaprax.graph.v46", "semaprax.graph.v43", 1)
                )
                .is_err()
            );
        }
    }
}
