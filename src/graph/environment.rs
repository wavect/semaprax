//! Graph v43 binds immutable environment snapshot access without granting it.
use super::*;
use serde_json::json;
pub(super) fn function_requires(function: &ResolvedFunction) -> bool {
    let mut found = false;
    hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
            found |= crate::environment_ops::is_environment(call.operation);
        }
    });
    found
}
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(function_requires)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::filesystem::graph_schema(program)?;
    Ok(if requires(program) {
        "semaprax.graph.v43"
    } else {
        previous
    })
}
pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let previous = super::filesystem::graph_schema_from_parts_and_instances(
        interfaces, types, functions, templates, instances,
    )?;
    Ok(
        if functions
            .iter()
            .chain(instances.iter().map(|instance| &instance.function))
            .any(function_requires)
        {
            "semaprax.graph.v43"
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
    let mut graph = super::filesystem::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    let value: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let schema = value["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "checked graph schema absent"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(schema));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io(
            "SPX-G411",
            "checked graph header is not canonical",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v43\"");
    let mut calls = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        if !functions.contains(&function.id) {
            continue;
        }
        hir::function_value::walk(function, |expression| {
            if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
                if crate::environment_ops::is_environment(call.operation) {
                    calls.push(json!({"function":function.id.as_str(),"expression":expression.id.as_str(),"operation":crate::environment_ops::id(call.operation),"effect":crate::environment_ops::EFFECT,"status_domain":crate::environment_ops::STATUS_DOMAIN,"status_codes":crate::environment_ops::STATUS_CODES}));
                }
            }
        });
    }
    let facts = json!({"schema":"semaprax.environment-input.v1","calls":calls,"max_entries":crate::environment_ops::MAX_ENTRIES,"max_combined_snapshot_input_bytes":crate::environment_ops::MAX_INPUT_BYTES,"ordering":"strict-unique-raw-byte-name-order","names":"nonempty-utf8-no-nul-or-equals","values":"utf8-no-nul","lifetime":"success-only-borrow-one-immutable-invocation-arena-no-escape","arena":crate::environment_ops::ARENA_ID,"accounting":"snapshot-input-once-no-lookup-recharge","authority":"explicit-injected-snapshot"});
    graph.pop();
    Ok(format!(
        "{},\"bounded_environment_io\":{}}}",
        graph,
        serde_json::to_string(&facts).expect("JSON values serialize")
    ))
}

#[cfg(test)]
mod tests {
    use crate::{
        command_io_ops::{validate_operation_profile, CommandOperationProfile},
        hir,
    };
    const SOURCE: &str = r#"module environment.graph;
permit { process.environment.read, process.stdout.write }
@id("environment.main") fn main()->i64 {0}
@id("environment.run") fn run()->bool uses { process.environment.read, process.stdout.write } {
    let count=env_len();
    let name=env_name_utf8(0usize);
    let value=env_value_utf8(0usize);
    let alias=value;
    let bytes=str_as_bytes(alias);
    stdout_append(bytes)==byte_len(bytes) && count>0usize
}
"#;
    #[test]
    fn environment_graph_and_profile_are_additive_and_closed() {
        let source = crate::check(SOURCE, "environment-graph.spx").unwrap();
        let resolved = hir::resolve(&source).unwrap();
        let entry = hir::DeclarationId::new("environment.run");
        validate_operation_profile(&resolved, &entry, CommandOperationProfile::EnvironmentV1)
            .unwrap();
        for old in [
            CommandOperationProfile::LanguageV1,
            CommandOperationProfile::LineV1,
            CommandOperationProfile::NetworkV1,
            CommandOperationProfile::ServiceV1,
            CommandOperationProfile::HttpV1,
            CommandOperationProfile::FilesystemV1,
            CommandOperationProfile::FilesystemV2,
        ] {
            assert!(validate_operation_profile(&resolved, &entry, old).is_err());
        }
        let function = resolved
            .functions
            .iter()
            .find(|function| function.id == entry)
            .unwrap();
        let mut checked_root = false;
        hir::function_value::walk(function, |expression| {
            if let hir::ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let hir::ResolvedStatement::Let { binding, .. } = statement {
                        if binding.name == "bytes" {
                            let fact = resolved
                                .declarations
                                .byte_slice_provenance(&binding.id)
                                .unwrap();
                            assert_eq!(
                                fact.root,
                                hir::ValueId::intrinsic_parameter(
                                    crate::environment_ops::ARENA_ID,
                                    usize::MAX
                                )
                            );
                            checked_root = true;
                        }
                    }
                }
            }
        });
        assert!(
            checked_root,
            "environment alias must retain its immutable arena"
        );
        let graph = crate::graph::to_json(&source).unwrap();
        let value: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(value["schema"], "semaprax.graph.v43");
        crate::graph::verify_json(&source, &graph).unwrap();
        assert!(crate::graph::to_legacy_json(&source).is_err());
        for (key, replacement) in [
            ("max_entries", serde_json::json!(257)),
            (
                "arena",
                serde_json::json!(crate::command_io_ops::ARG_UTF8_ID),
            ),
            ("accounting", serde_json::json!("recharge-per-call")),
        ] {
            let mut forged = value.clone();
            forged["bounded_environment_io"][key] = replacement;
            assert!(
                crate::graph::verify_json(&source, &serde_json::to_string(&forged).unwrap())
                    .is_err()
            );
        }

        assert_eq!(
            value["bounded_environment_io"]["calls"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            value["bounded_environment_io"]["arena"],
            crate::environment_ops::ARENA_ID
        );
        assert!(crate::graph::verify_json(
            &source,
            &graph.replacen("semaprax.graph.v43", "semaprax.graph.v42", 1)
        )
        .is_err());
    }
}

#[cfg(test)]
mod loop_tests {
    #[test]
    fn environment_lookups_and_named_text_reads_repeat_inside_loop() {
        let source = crate::check(
            r#"module environment.loop;
permit {process.environment.read}
@id("environment.main") fn main()->i64{0}
@id("environment.index") fn index_of(key:borrow str)->usize uses {process.environment.read} {
    let count=env_len(); let mut index=0usize; let mut found=count;
    while index<count {
        let name=env_name_utf8(index);
        let same=str_len_bytes(name)==str_len_bytes(key) && str_contains(name,key);
        found=if same {index} else {found};
        index=index+1usize;
        true
    }
    found
}
"#,
            "environment-loop.spx",
        )
        .unwrap();
        let resolved = crate::hir::resolve(&source).unwrap();
        crate::hir::validate(&resolved).unwrap();
        assert!(crate::graph::to_json(&source)
            .unwrap()
            .contains("semaprax.graph.v43"));
    }
}
