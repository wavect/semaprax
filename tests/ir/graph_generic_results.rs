use semaprax::graph;
use serde_json::Value;

fn source() -> String {
    let mut source = String::from(
        r#"module test.generic_result_graph;
@id("r.relay") fn relay<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> { value }
@id("r.propagate") fn propagate<E>(value: own Result<Bytes, E>, divisor: i64) -> Result<Bytes, E> {
    let payload = value?;
    let probe = 1 / divisor;
    Result<Bytes, E>::Ok { value: payload }
}
@id("r.outer") fn outer<E>(value: own Result<Bytes, E>, divisor: i64) -> Result<Bytes, E> {
    propagate<E>(value, divisor)
}
"#,
    );
    for error in [
        "i64", "i32", "u8", "usize", "char", "f32", "f64", "bool", "Bytes",
    ] {
        source.push_str(&format!(r#"
@id("r.invoke.{error}") fn invoke_{error}(value: own Result<Bytes, {error}>, divisor: i64) -> Result<Bytes, {error}> {{
    outer<{error}>(relay<{error}>(value), divisor)
}}
"#));
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

#[test]
fn generic_result_graph_has_exact_conditional_cases_and_residual_types() {
    let program = semaprax::check(&source(), "generic-result-graph.spx").unwrap();
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v34");
    assert_eq!(value["base_schema"], "semaprax.graph.v34");
    let instances = value["generic_instance_ownership"].as_array().unwrap();
    assert_eq!(instances.len(), 27);
    for instance in instances {
        let two_owned = instance["type_arguments"][0]["type_identity"] == "bytes";
        let owner = &instance["parameters"][0];
        assert_eq!(owner["ownership_mode"], "own");
        assert_eq!(instance["result"]["ownership_mode"], "own");
        assert!(owner["concrete_record_identity"].is_null());
        assert!(instance["result"]["concrete_record_identity"].is_null());
        assert_eq!(
            owner["owned_descendant_count"],
            if two_owned { 2 } else { 1 }
        );
        assert_eq!(instance["cleanup_plan_schema"], "semaprax.cleanup-plan.v6");
        let cases = instance["cleanup_inventory"]["conditional_owned_parameters"][0]["cases"]
            .as_array()
            .unwrap();
        assert_eq!(cases.len(), 2);
        let ok = cases
            .iter()
            .find(|case| case["case"] == "core.result.ok")
            .unwrap();
        let err = cases
            .iter()
            .find(|case| case["case"] == "core.result.err")
            .unwrap();
        assert_eq!(ok["live_flags"].as_array().unwrap().len(), 1);
        assert_eq!(
            err["live_flags"].as_array().unwrap().len(),
            usize::from(two_owned)
        );
        assert!(value["type_facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["id"] == owner["substituted_type"]));
        for call in instance["call_edges"].as_array().unwrap() {
            assert_eq!(call["transfer_kind"], "whole_owner");
            assert!(instances
                .iter()
                .any(|callee| callee["concrete_instance"] == call["callee_instance"]));
        }
        if instance["template"] == "r.propagate" {
            let node = value["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["id"] == instance["body_reference"])
                .unwrap();
            let body = &node["body"];
            let mut tries = Vec::new();
            visit(body, &mut |node| {
                if node["kind"] == "try_result" {
                    tries.push(node.clone());
                }
            });
            assert_eq!(tries.len(), 1);
            assert_eq!(tries[0]["evaluation"], "once");
            assert_eq!(
                tries[0]["residual_result_type_id"],
                owner["substituted_type"]
            );
            assert_eq!(tries[0]["source_result_type_id"], owner["substituted_type"]);
            assert_eq!(tries[0]["ok_case"], "core.result.ok");
            assert_eq!(tries[0]["err_case"], "core.result.err");
        }
    }
    let roundtrip = semaprax::check(
        &semaprax::format::canonical(&program),
        "generic-result-graph.spx",
    )
    .unwrap();
    assert_eq!(bytes, graph::to_json(&roundtrip).unwrap());
    assert_eq!(
        graph::to_legacy_json(&program).unwrap_err()[0].code,
        "SPX-G410"
    );
}

fn visit(value: &Value, action: &mut impl FnMut(&Value)) {
    match value {
        Value::Object(fields) => {
            action(value);
            for value in fields.values() {
                visit(value, action);
            }
        }
        Value::Array(items) => {
            for value in items {
                visit(value, action);
            }
        }
        _ => {}
    }
}

#[test]
fn generic_result_graph_rejects_foreign_case_and_residual_metadata() {
    let program = semaprax::check(&source(), "generic-result-graph.spx").unwrap();
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    let residual_key = "\"residual_result_type_id\":\"";
    let residual_start = bytes.find(residual_key).unwrap() + residual_key.len();
    let residual_end = residual_start + bytes[residual_start..].find('"').unwrap();
    let mut forged_residual = bytes.clone();
    forged_residual.replace_range(residual_start..residual_end, "foreign.result.instance");
    assert_eq!(
        graph::verify_json(&program, &forged_residual).unwrap_err()[0].code,
        "SPX-G411"
    );
    for (original, substituted) in [
        ("\"core.result.err\"", "\"foreign.result.err\""),
        ("\"core.result.ok.value\"", "\"core.result.err.error\""),
        (
            "\"cleanup_plan_schema\":\"semaprax.cleanup-plan.v6\"",
            "\"cleanup_plan_schema\":\"semaprax.cleanup-plan.v2\"",
        ),
        ("\"live_flags\":[]", "\"live_flags\":[999]"),
    ] {
        let forged = bytes.replacen(original, substituted, 1);
        assert_ne!(forged, bytes, "required mutation witness: {original}");
        assert_eq!(
            graph::verify_json(&program, &forged).unwrap_err()[0].code,
            "SPX-G411"
        );
    }
}

#[test]
fn unused_generic_result_template_selects_additive_graph() {
    let program = semaprax::check(
        r#"module test.unused_result_graph;
@id("r.relay") fn relay<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> { value }
@id("app.main") fn main() -> i64 { 0 }
"#,
        "unused-generic-result.spx",
    )
    .unwrap();
    let value: Value = serde_json::from_str(&graph::to_json(&program).unwrap()).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v34");
    assert!(value["generic_instance_ownership"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        graph::to_legacy_json(&program).unwrap_err()[0].code,
        "SPX-G410"
    );
}
