//! Additive Graph v41 filesystem facts derived only from retained checked calls.
use super::*;
use serde_json::json;

pub(super) fn function_requires(function: &ResolvedFunction) -> bool {
    let mut found = false;
    hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
            found |= crate::filesystem_ops::is_filesystem(call.operation);
        }
    });
    found
}
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|item| &item.function))
        .any(function_requires)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let old = super::nested_owned::pre_filesystem_graph_schema(program)?;
    Ok(if requires(program) {
        "semaprax.graph.v41"
    } else {
        old
    })
}
pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let old = super::nested_owned::pre_filesystem_schema_from_parts(
        interfaces, types, functions, templates, instances,
    )?;
    Ok(
        if functions
            .iter()
            .chain(instances.iter().map(|item| &item.function))
            .any(function_requires)
        {
            "semaprax.graph.v41"
        } else {
            old
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
    let mut graph = super::generic_instances::pre_filesystem_graph_json(
        program, revision, functions, types, view,
    )?;
    if !requires(program) {
        return Ok(graph);
    }
    // Keep the checked base payload byte-for-byte, but explicitly select the
    // additive contract at its outer header. The legacy renderer deliberately
    // retains its own schema even when this wrapper adds filesystem facts.
    let base: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let schema = base["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "checked graph schema is absent"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(schema));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io(
            "SPX-G411",
            "checked graph header is not canonical",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v41\"");
    let mut calls = Vec::new();
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|item| &item.function))
    {
        if !functions.contains(&function.id) {
            continue;
        }
        hir::function_value::walk(function, |expression| {
            if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
                if crate::filesystem_ops::is_filesystem(call.operation) {
                    calls.push(json!({"function":function.id.as_str(),"expression":expression.id.as_str(),"operation":crate::filesystem_ops::id(call.operation),"effect":crate::filesystem_ops::effect(call.operation),"status_domain":crate::filesystem_ops::STATUS_DOMAIN,"status_codes":crate::filesystem_ops::STATUS_CODES}));
                }
            }
        });
    }
    let facts = json!({"schema":"semaprax.filesystem.v1","calls":calls,"max_path_bytes":crate::filesystem_ops::MAX_PATH_BYTES,"max_file_bytes":crate::filesystem_ops::MAX_FILE_BYTES,"max_total_bytes":crate::filesystem_ops::MAX_TOTAL_BYTES,"max_operations":crate::filesystem_ops::MAX_OPERATIONS,"path_policy":"relative-byte-components-no-empty-dot-dotdot-nul-backslash-colon","accounting":"reserve-attempted-read-max-or-write-length-before-dispatch-no-refund","read_result":"owned-bytes-after-success","write_mode":"create-new"});
    graph.pop();
    Ok(format!(
        "{},\"filesystem\":{}}}",
        graph,
        serde_json::to_string(&facts).expect("JSON values serialize")
    ))
}
pub(super) fn string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .budgeted_join(",")
    )
}

#[cfg(test)]
mod tests {
    const SOURCE: &str = r#"
module filesystem.graph;
permit { fs.read, fs.write }
@id("filesystem.main") fn main()->i64 { 0 }
@id("filesystem.run") fn run()->bool uses { fs.read, fs.write } {
    let path=[97u8];
    let data=file_read(array_as_slice(path),1usize,8usize);
    file_write_new(array_as_slice(path),1usize,bytes_as_slice(data),byte_len(bytes_as_slice(data)))==0usize
}
"#;
    #[test]
    fn filesystem_graph_binds_checked_operations_and_rejects_remint() {
        let source = crate::check(SOURCE, "filesystem.spx").unwrap();
        let graph = crate::graph::to_json(&source).unwrap();
        let value: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(value["schema"], "semaprax.graph.v41");
        assert_eq!(value["filesystem"]["calls"].as_array().unwrap().len(), 2);
        crate::graph::verify_json(&source, &graph).unwrap();
        assert!(crate::graph::to_legacy_json(&source).is_err());
        assert!(crate::graph::verify_json(
            &source,
            &graph.replacen("semaprax.graph.v41", "semaprax.graph.v19", 1)
        )
        .is_err());
        for (field, replacement) in [
            ("max_path_bytes", serde_json::json!(4097)),
            ("max_total_bytes", serde_json::json!(1048577)),
            ("max_operations", serde_json::json!(65)),
            ("accounting", serde_json::json!("refund-failures")),
            ("write_mode", serde_json::json!("overwrite")),
            ("calls", serde_json::json!([])),
        ] {
            let mut forged = value.clone();
            forged["filesystem"][field] = replacement;
            assert!(
                crate::graph::verify_json(&source, &serde_json::to_string(&forged).unwrap())
                    .is_err(),
                "{field}"
            );
        }
        let mut forged = value;
        forged["filesystem"]["max_file_bytes"] = serde_json::json!(65537);
        assert!(
            crate::graph::verify_json(&source, &serde_json::to_string(&forged).unwrap()).is_err()
        );
        let changed = crate::check(&SOURCE.replace("8usize", "9usize"), "filesystem.spx").unwrap();
        assert!(crate::graph::verify_json(&changed, &graph).is_err());
    }
    #[test]
    fn filesystem_effect_and_contract_admission_remain_closed() {
        let errors = crate::check(
            &SOURCE.replace("uses { fs.read, fs.write }", "uses { fs.read }"),
            "filesystem.spx",
        )
        .unwrap_err();
        assert!(!errors.is_empty());
        let source = crate::check(SOURCE, "filesystem.spx").unwrap();
        let mut resolved = crate::hir::resolve(&source).unwrap();
        resolved
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "filesystem.run")
            .unwrap()
            .effects
            .clear();
        assert!(crate::hir::validate(&resolved).is_err());
    }
}
