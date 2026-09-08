use semaprax::graph::{self, AgentContextFilter, AgentContextV2Options};
use serde_json::Value;

const SOURCE: &str = r#"
module test.graph_mapping;
@id("m.first") fn first<A,B>(left:A,right:B)->A {left}
@id("m.identity") fn identity<A,B>(left:A,right:B)->A {first<A,B>(left,right)}
@id("m.swap") fn swap<A,B>(left:A,right:B)->B {first<B,A>(right,left)}
@id("m.repeat") fn repeat<A>(value:A)->A {first<A,A>(value,value)}
@id("m.concrete") fn concrete<A>(value:A)->i64 {first<i64,A>(7,value)}
@id("app.main") fn main()->i64 {identity<i64,i64>(1,2)+swap<i64,i64>(1,2)+repeat<i64>(3)+concrete<i64>(4)}
"#;

#[test]
fn graph_v35_preserves_symbolic_mapping_when_concrete_types_coincide() {
    let program = semaprax::check(SOURCE, "mapping.spx").unwrap();
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    let graph: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v35");
    let instances = graph["generic_instance_ownership"].as_array().unwrap();
    let call = |template| {
        &instances
            .iter()
            .find(|i| i["template"] == template)
            .unwrap()["call_edges"][0]
    };
    let identity = call("m.identity");
    let swap = call("m.swap");
    assert_eq!(
        identity["callee_concrete_arguments"],
        swap["callee_concrete_arguments"]
    );
    assert_eq!(identity["callee_instance"], swap["callee_instance"]);
    assert_eq!(
        identity["forwarded_argument_mapping"][0]["source"]["index"],
        0
    );
    assert_eq!(swap["forwarded_argument_mapping"][0]["source"]["index"], 1);
    assert_eq!(swap["forwarded_argument_mapping"][1]["source"]["index"], 0);
    let repeated = &call("m.repeat")["forwarded_argument_mapping"];
    assert_eq!(repeated[0]["source"]["index"], 0);
    assert_eq!(repeated[1]["source"]["index"], 0);
    assert_eq!(repeated[1]["callee_index"], 1);
    let concrete = &call("m.concrete")["forwarded_argument_mapping"][0]["source"];
    assert_eq!(concrete["kind"], "concrete_type");
    assert_eq!(concrete["type_identity"], "i64");
    assert_eq!(
        graph["generic_template_forwarding"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        graph::to_legacy_json(&program).unwrap_err()[0].code,
        "SPX-G410"
    );
    let canonical = semaprax::format::canonical(&program);
    assert_eq!(
        bytes,
        graph::to_json(&semaprax::check(&canonical, "mapping.spx").unwrap()).unwrap()
    );
    for (from, to) in [
        (
            "\"kind\":\"caller_parameter\"",
            "\"kind\":\"concrete_type\"",
        ),
        ("\"callee_index\":1", "\"callee_index\":0"),
        ("\"index\":1", "\"index\":9"),
        ("\"type_identity\":\"i64\"", "\"type_identity\":\"bool\""),
    ] {
        let forged = bytes.replacen(from, to, 1);
        assert_ne!(bytes, forged);
        assert_eq!(
            graph::verify_json(&program, &forged).unwrap_err()[0].code,
            "SPX-G411"
        );
    }
}

#[test]
fn graph_v35_exposes_unused_template_mapping_in_bounded_context() {
    let text = SOURCE.replace(
        "identity<i64,i64>(1,2)+swap<i64,i64>(1,2)+repeat<i64>(3)+concrete<i64>(4)",
        "0",
    );
    let program = semaprax::check(&text, "mapping-unused.spx").unwrap();
    let value: Value = serde_json::from_str(&graph::to_json(&program).unwrap()).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v35");
    assert!(value["generic_instance_ownership"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        value["generic_template_forwarding"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let options = AgentContextV2Options::new(
        1,
        100_000,
        32,
        [AgentContextFilter::Types, AgentContextFilter::Ownership],
        graph::AgentContextDirection::Forward,
    )
    .unwrap();
    let context = graph::agent_context_v2_json(&program, "m.swap", &options)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"source_graph_schema\":\"semaprax.graph.v35\""));
    assert!(context.contains("\"generic_template_forwarding\""));
    assert!(context.contains("\"kind\":\"caller_parameter\""));
}

#[test]
fn graph_identity_forwarding_retains_v34() {
    let source = r#"module test.identity_mapping;
@id("m.first") fn first<A,B>(left:A,right:B)->A {left}
@id("m.identity") fn identity<A,B>(left:A,right:B)->A {first<A,B>(left,right)}
@id("app.main") fn main()->i64 {identity<i64,i64>(1,2)}
"#;
    let program = semaprax::check(source, "identity.spx").unwrap();
    let bytes = graph::to_json(&program).unwrap();
    let graph: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v34");
    assert!(!bytes.contains("generic_template_forwarding"));
    let call = &graph["generic_instance_ownership"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["template"] == "m.identity")
        .unwrap()["call_edges"][0];
    assert_eq!(call["forwarded_argument_mapping"][0]["caller_index"], 0);
    assert!(call["forwarded_argument_mapping"][0]
        .get("source")
        .is_none());
}

#[test]
fn graph_v35_authenticates_requires_body_and_ensures_paths() {
    let source = r#"module test.mapping_roots;
@id("m.truth") fn truth<T>(value:T)->bool {true}
@id("m.first") fn first<A,B>(left:A,right:B)->A {left}
@id("m.outer") fn outer<A,B>(left:A,right:B)->A
requires truth<B>(right)
ensures truth<A>(left)
{first<A,A>(left,left)}
@id("app.main") fn main()->i64 {outer<i64,i64>(1,2)}
"#;
    let program = semaprax::check(source, "mapping-roots.spx").unwrap();
    let graph: Value = serde_json::from_str(&graph::to_json(&program).unwrap()).unwrap();
    let facts = graph["generic_template_forwarding"].as_array().unwrap();
    for root in ["requires/0", "body", "ensures/0"] {
        assert!(facts
            .iter()
            .any(|fact| fact["structural_path"].as_str().unwrap().starts_with(root)));
    }
    let outer = graph["generic_instance_ownership"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["template"] == "m.outer")
        .unwrap();
    let calls = outer["call_edges"].as_array().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[0]["forwarded_argument_mapping"][0]["source"]["index"],
        1
    );
    assert_eq!(
        calls[1]["forwarded_argument_mapping"][1]["source"]["index"],
        0
    );
    assert_eq!(
        calls[2]["forwarded_argument_mapping"][0]["source"]["index"],
        0
    );
}

#[test]
fn inferred_forwarding_preserves_symbolic_mapping_and_rejects_concrete_lookalikes() {
    let inferred = SOURCE
        .replace("first<A,B>(left,right)", "first(left,right)")
        .replace("first<B,A>(right,left)", "first(right,left)")
        .replace("first<A,A>(value,value)", "first(value,value)")
        .replace("first<i64,A>(7,value)", "first(7,value)")
        .replace("identity<i64,i64>(1,2)", "identity(1,2)")
        .replace("swap<i64,i64>(1,2)", "swap(1,2)")
        .replace("repeat<i64>(3)", "repeat(3)")
        .replace("concrete<i64>(4)", "concrete(4)");
    let explicit = semaprax::check(SOURCE, "explicit-mapping.spx").unwrap();
    let program = semaprax::check(&inferred, "inferred-mapping.spx").unwrap();
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    let expected: Value = serde_json::from_str(&graph::to_json(&explicit).unwrap()).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v35");
    assert_eq!(
        value["generic_template_forwarding"],
        expected["generic_template_forwarding"]
    );
    assert_ne!(value["revision"], expected["revision"]);
    let instances = value["generic_instance_ownership"].as_array().unwrap();
    let edge = |name: &str| {
        &instances
            .iter()
            .find(|instance| instance["template"] == name)
            .unwrap()["call_edges"][0]
    };
    assert_eq!(
        edge("m.identity")["callee_concrete_arguments"],
        edge("m.swap")["callee_concrete_arguments"]
    );
    assert_eq!(
        edge("m.identity")["callee_instance"],
        edge("m.swap")["callee_instance"]
    );
    assert_ne!(
        edge("m.identity")["forwarded_argument_mapping"],
        edge("m.swap")["forwarded_argument_mapping"]
    );
    for instance in instances {
        let reference = expected["generic_instance_ownership"]
            .as_array()
            .unwrap()
            .iter()
            .find(|other| {
                other["template"] == instance["template"]
                    && other["type_arguments"] == instance["type_arguments"]
            })
            .unwrap();
        let mappings = |entry: &Value| {
            entry["call_edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| edge["forwarded_argument_mapping"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(mappings(instance), mappings(reference));
    }
    // Both calls specialize to <i64,i64>. Only retained symbolic source can
    // distinguish identity from swap, even if the forged edge looks concrete.
    let mut forged = value.clone();
    let slot = forged["generic_instance_ownership"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["template"] == "m.swap")
        .unwrap();
    slot["call_edges"][0]["forwarded_argument_mapping"] =
        edge("m.identity")["forwarded_argument_mapping"].clone();
    assert_eq!(
        graph::verify_json(&program, &serde_json::to_string(&forged).unwrap()).unwrap_err()[0].code,
        "SPX-G411"
    );
    let canonical = semaprax::format::canonical(&program);
    assert_eq!(
        bytes,
        graph::to_json(&semaprax::check(&canonical, "inferred-mapping.spx").unwrap()).unwrap()
    );

    let unused = inferred.replace("identity(1,2)+swap(1,2)+repeat(3)+concrete(4)", "0");
    let unused = semaprax::check(&unused, "inferred-unused-mapping.spx").unwrap();
    let unused_graph: Value = serde_json::from_str(&graph::to_json(&unused).unwrap()).unwrap();
    assert!(unused_graph["generic_instance_ownership"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        unused_graph["generic_template_forwarding"],
        value["generic_template_forwarding"]
    );
}
