//! Graph v44 binds bounded registered process launch without granting authority.
use super::*;
use serde_json::json;
pub(super) fn function_requires(function: &ResolvedFunction) -> bool {
    let mut found = false;
    hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
            found |= crate::process_ops::is_process(call.operation);
        }
    });
    found
}
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    crate::process_ops::program_uses_process(program)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let previous = super::environment::graph_schema(program)?;
    Ok(if requires(program) {
        "semaprax.graph.v44"
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
    let previous = super::environment::graph_schema_from_parts_and_instances(
        interfaces, types, functions, templates, instances,
    )?;
    Ok(
        if functions
            .iter()
            .chain(instances.iter().map(|instance| &instance.function))
            .any(function_requires)
        {
            "semaprax.graph.v44"
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
    let mut graph = super::environment::graph_json(program, revision, functions, types, view)?;
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
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v44\"");
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
                if crate::process_ops::is_process(call.operation) {
                    calls.push(json!({"function":function.id.as_str(),"expression":expression.id.as_str(),"operation":crate::process_ops::ID,"effect":crate::process_ops::EFFECT,"status_domain":crate::process_ops::STATUS_DOMAIN,"status_codes":crate::process_ops::STATUS_CODES}));
                }
            }
        });
    }
    let facts = json!({
        "schema":"semaprax.process.v1", "calls":calls,
        "max_arguments":crate::process_ops::MAX_ARGUMENTS,
        "max_input_bytes":crate::process_ops::MAX_INPUT_BYTES,
        "max_output_bytes":crate::process_ops::MAX_OUTPUT_BYTES,
        "max_wait_millis":crate::process_ops::MAX_WAIT_MILLIS,
        "max_operations":crate::process_ops::MAX_OPERATIONS,
        "max_total_bytes":crate::process_ops::MAX_TOTAL_BYTES,
        "argv":"u32le-count-then-u32le-length-and-non-nul-bytes-exact-extent",
        "result":"four-u64le-version1-termination-stdout-length-stderr-length-then-exact-streams",
        "termination":"low2-kind-0-exit-u32-or-1-signal-1-through-255-code-above-low2",
        "header_bytes":crate::process_ops::HEADER_BYTES,
        "accounting":"attempted-input-plus-reserved-output-no-refund",
        "ownership":"one-success-only-owned-bytes-after-validation-and-settlement",
        "authority":"explicit-registered-tool-id-fixed-executable-cwd-environment-and-argv-policy-no-ambient-lookup",
        "settlement":"sticky-failure-cancel-close-pipes-and-reap-before-success-no-wall-clock-reap-guarantee"
    });
    graph.pop();
    Ok(format!(
        "{},\"bounded_process_io\":{}}}",
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
    const SOURCE: &str = r#"module process.graph;
permit { process.environment.read, process.execute }
@id("process.main") fn main()->i64 {0}
@id("process.run") fn run()->bool uses { process.environment.read, process.execute } {
    let count=env_len();
    let arguments=bytes_zeroed(4usize);
    let input=bytes_zeroed(0usize);
    let argv=bytes_as_slice(arguments);
    let stdin=bytes_as_slice(input);
    let output=process_run(0usize,argv,4usize,stdin,0usize,100usize,32usize,16usize);
    let view=bytes_as_slice(output);
    byte_len(view)>=32usize && count>=0usize
}
"#;
    #[test]
    fn process_graph_preserves_environment_and_rejects_forged_contracts() {
        let source = crate::check(SOURCE, "process-graph.spx").unwrap();
        let resolved = hir::resolve(&source).unwrap();
        let entry = hir::DeclarationId::new("process.run");
        validate_operation_profile(&resolved, &entry, CommandOperationProfile::ProcessV1).unwrap();
        for profile in [
            CommandOperationProfile::LanguageV1,
            CommandOperationProfile::LineV1,
            CommandOperationProfile::NetworkV1,
            CommandOperationProfile::ServiceV1,
            CommandOperationProfile::HttpV1,
            CommandOperationProfile::FilesystemV1,
            CommandOperationProfile::FilesystemV2,
            CommandOperationProfile::EnvironmentV1,
        ] {
            assert!(validate_operation_profile(&resolved, &entry, profile).is_err());
        }
        let wasm = crate::wasm::process_io::emit_resolved_process_io_v1(&resolved, entry.as_str())
            .unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let native = crate::codegen::emit_hir_c_with_process_io(&resolved, entry.as_str()).unwrap();
        assert!(native.contains("spx_process_command_run_v1"));
        assert!(native.contains("spx_host_process_run_v1"));
        let graph = crate::graph::to_json(&source).unwrap();
        crate::graph::verify_json(&source, &graph).unwrap();
        assert!(crate::graph::to_legacy_json(&source).is_err());
        let value: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(value["schema"], "semaprax.graph.v44");
        let rejection = crate::graph::reject_evidence_schema(value["schema"].as_str().unwrap())
            .expect_err("registered process graphs stay outside frozen evidence admission");
        assert_eq!(rejection.code, "SPX-G410");
        assert_eq!(
            value["bounded_environment_io"]["schema"],
            "semaprax.environment-input.v1"
        );
        assert_eq!(value["bounded_process_io"]["header_bytes"], 32);
        for (key, replacement) in [
            ("max_operations", serde_json::json!(17)),
            ("header_bytes", serde_json::json!(24)),
            ("authority", serde_json::json!("ambient-path")),
            ("ownership", serde_json::json!("publish-before-settlement")),
        ] {
            let mut forged = value.clone();
            forged["bounded_process_io"][key] = replacement;
            assert!(
                crate::graph::verify_json(&source, &serde_json::to_string(&forged).unwrap())
                    .is_err()
            );
        }
        assert!(crate::graph::verify_json(
            &source,
            &graph.replacen("semaprax.graph.v44", "semaprax.graph.v43", 1)
        )
        .is_err());
        let pure = crate::check(
            &SOURCE.replace(
                "process_run(0usize,argv,4usize,stdin,0usize,100usize,32usize,16usize)",
                "bytes_zeroed(32usize)",
            ),
            "process-unused.spx",
        )
        .unwrap();
        let pure = hir::resolve(&pure).unwrap();
        assert!(
            validate_operation_profile(&pure, &entry, CommandOperationProfile::ProcessV1).is_err()
        );
    }
    #[test]
    fn process_hir_rejects_forged_argument_and_result_ownership() {
        let source = crate::check(SOURCE, "process-forged.spx").unwrap();
        let resolved = hir::resolve(&source).unwrap();
        for change_argument in [false, true] {
            let mut forged = resolved.clone();
            let function = forged
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "process.run")
                .unwrap();
            let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
                panic!("block")
            };
            let expression=statements.iter_mut().find_map(|statement| {
                if let hir::ResolvedStatement::Let {value,..}=statement {
                    if matches!(&value.kind,hir::ResolvedExprKind::HostCommandCall(call) if call.operation==hir::ResolvedHostCommandOperation::ProcessRun) {return Some(value);}
                }
                None
            }).unwrap();
            if change_argument {
                let hir::ResolvedExprKind::HostCommandCall(call) = &mut expression.kind else {
                    unreachable!()
                };
                call.args[0].ty = hir::ResolvedType::Bool;
            } else {
                expression.ownership = hir::OwnershipMode::Value;
            }
            assert!(hir::validate(&forged).is_err());
        }
    }
}
