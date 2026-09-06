use semaprax::{graph, hir};
use serde_json::{json, Value};

fn source(scalars: &[&str], shapes: &[&str]) -> String {
    let mut source = String::from(
        r#"module test.generic_instance_graph;
@id("g.pair") record Pair<T, U> { @id("g.payload") payload: T, @id("g.marker") marker: U, }
@id("g.box") record Box<T> { @id("g.value") value: T, }
"#,
    );
    for (index, shape) in shapes.iter().enumerate() {
        source.push_str(&format!(r#"
@id("g.leaf.{index}") fn leaf_{index}<T>(value: own {shape}) -> {shape} {{ value }}
@id("g.outer.{index}") fn outer_{index}<T>(value: own {shape}) -> {shape} {{ leaf_{index}<T>(value) }}
"#));
        for scalar in scalars {
            let ty = shape.replace('T', scalar);
            source.push_str(&format!(r#"
@id("g.invoke.{index}.{scalar}") fn invoke_{index}_{scalar}(value: own {ty}) -> {ty} {{ outer_{index}<{scalar}>(value) }}
"#));
        }
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

fn checked(source: &str) -> semaprax::ast::Program {
    semaprax::check(source, "generic-instance-graph.spx").unwrap()
}

#[test]
fn graph_v34_exact_eight_scalar_flat_and_nested_ownership_and_forwarding() {
    let program = checked(&source(
        &["bool", "i64", "i32", "u8", "usize", "char", "f32", "f64"],
        &[
            "Pair<Bytes, T>",
            "Box<Pair<Bytes, T>>",
            "Pair<Box<Bytes>, T>",
        ],
    ));
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v34");
    let instances = value["generic_instance_ownership"].as_array().unwrap();
    assert_eq!(instances.len(), 48);
    let hir = hir::resolve(&program).unwrap();
    for instance in instances {
        let exact = hir
            .function_instances
            .iter()
            .find(|i| instance["execution_instance"] == i.id.as_str())
            .unwrap();
        assert_eq!(instance["source_revision"], graph::revision(&program));
        assert_eq!(instance["parameters"][0]["ownership_mode"], "own");
        assert_eq!(instance["result"]["ownership_mode"], "own");
        assert_eq!(instance["parameters"][0]["owned_descendant_count"], 1);
        assert_eq!(instance["result"]["owned_descendant_count"], 1);
        assert_eq!(
            instance["parameters"][0]["type_identity"],
            exact.function.params[0].ty.identity_key()
        );
        assert_eq!(
            instance["cleanup_plan_schema"],
            exact.function.cleanup_plan.schema
        );
        let is_flat = exact.template.as_str().ends_with(".0");
        assert_eq!(
            instance["cleanup_plan_schema"],
            if is_flat {
                "semaprax.cleanup-plan.v2"
            } else {
                "semaprax.cleanup-plan.v7"
            }
        );
        let paths = &instance["parameters"][0]["owned_leaf_paths"][0]["field_path"];
        assert_eq!(paths.as_array().unwrap().len(), if is_flat { 1 } else { 2 });
        for edge in instance["call_edges"].as_array().unwrap() {
            let callee = instances
                .iter()
                .find(|i| i["concrete_instance"] == edge["callee_instance"])
                .unwrap();
            assert_eq!(callee["template"], edge["callee_template"]);
            assert_eq!(
                callee["type_arguments"][0]["type_identity"],
                instance["type_arguments"][0]["type_identity"]
            );
            assert_eq!(edge["transfer_kind"], "whole_owner");
            assert_eq!(edge["forwarded_argument_mapping"][0]["caller_index"], 0);
        }
    }
    let roundtrip = checked(&semaprax::format::canonical(&program));
    assert_eq!(bytes, graph::to_json(&roundtrip).unwrap());
    assert_ne!(bytes, graph::to_legacy_json(&program).unwrap());
}

#[test]
fn graph_v34_replay_rejects_reminted_identity_ownership_paths_schema_and_closure() {
    let program = checked(&source(&["bool"], &["Box<Pair<Bytes, T>>"]));
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    for pointer in [
        "/generic_instance_ownership/0/template",
        "/generic_instance_ownership/0/concrete_instance",
        "/generic_instance_ownership/0/type_arguments/0/type_identity",
        "/generic_instance_ownership/0/parameters/0/ownership_mode",
        "/generic_instance_ownership/0/result/ownership_mode",
        "/generic_instance_ownership/0/parameters/0/owned_leaf_paths/0/field_path/0",
        "/generic_instance_ownership/0/source_revision",
        "/generic_instance_ownership/0/cleanup_plan_schema",
        "/generic_instance_ownership/0/cleanup_inventory_digest",
        "/generic_instance_ownership/0/cleanup_plan_digest",
        "/generic_instance_ownership/0/program_root_association/source_revision",
        "/generic_instance_ownership/1/call_edges/0/callee_instance",
        "/generic_instance_ownership/1/call_edges/0/forwarded_argument_mapping/0/caller_index",
    ] {
        let forged = replace(&bytes, pointer, "\"forged\"");
        assert_eq!(
            graph::verify_json(&program, &forged).unwrap_err()[0].code,
            "SPX-G411",
            "{pointer}"
        );
    }
    for pointer in [
        "/generic_instance_ownership/0/type_arguments",
        "/generic_instance_ownership/0/parameters/0/owned_leaf_paths",
        "/generic_instance_ownership/0/cleanup_inventory/slots",
        "/generic_instance_ownership/1/call_edges",
    ] {
        let forged = replace(&bytes, pointer, "[]");
        assert!(graph::verify_json(&program, &forged).is_err(), "{pointer}");
        let value: Value = serde_json::from_str(&bytes).unwrap();
        let array = value.pointer(pointer).unwrap().as_array().unwrap();
        let mut duplicated = array.clone();
        duplicated.push(array[0].clone());
        let forged = replace(
            &bytes,
            pointer,
            &serde_json::to_string(&duplicated).unwrap(),
        );
        assert!(
            graph::verify_json(&program, &forged).is_err(),
            "duplicate {pointer}"
        );
    }
    let value: Value = serde_json::from_str(&bytes).unwrap();
    let pointer = "/generic_instance_ownership/0/cleanup_inventory/slots";
    let mut reordered = value.pointer(pointer).unwrap().as_array().unwrap().clone();
    assert!(reordered.len() > 1);
    reordered.reverse();
    assert!(graph::verify_json(
        &program,
        &replace(&bytes, pointer, &serde_json::to_string(&reordered).unwrap())
    )
    .is_err());
    let changed = checked(&source(&["i64"], &["Box<Pair<Bytes, T>>"]));
    assert_eq!(
        graph::verify_json(&changed, &bytes).unwrap_err()[0].code,
        "SPX-G411"
    );
}

#[test]
fn graph_v34_context_contains_exact_instance_facts_and_scalar_profiles() {
    let program = checked(
        r#"module test.generic_scalar_graph;
@id("s.identity") fn identity<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { identity<i64>(identity<i64>(42)) }
"#,
    );
    let bytes = graph::context_json(&program, "s.identity", 0)
        .unwrap()
        .unwrap();
    let graph: Value = serde_json::from_str(&bytes).unwrap();
    let instances = graph["generic_instance_ownership"].as_array().unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0]["parameters"][0]["ownership_mode"], "value");
    assert_eq!(instances[0]["result"]["ownership_mode"], "value");
    assert_eq!(instances[0]["parameters"][0]["owned_leaf_paths"], json!([]));
    assert_eq!(
        instances[0]["cleanup_plan_schema"],
        "semaprax.cleanup-plan.v2"
    );
}

// Locate a JSON value without reserializing any surrounding graph bytes. The
// no-op replacement below proves each hostile test preserves canonical bytes.
fn value_end(bytes: &[u8], start: usize) -> usize {
    let mut pos = start;
    if bytes[pos] == b'"' {
        pos += 1;
        while pos < bytes.len() {
            if bytes[pos] == b'\\' {
                pos += 2;
            } else if bytes[pos] == b'"' {
                return pos + 1;
            } else {
                pos += 1;
            }
        }
    } else if matches!(bytes[pos], b'[' | b'{') {
        let end = if bytes[pos] == b'[' { b']' } else { b'}' };
        pos += 1;
        while bytes[pos] != end {
            if matches!(bytes[pos], b',' | b':' | b' ') {
                pos += 1;
            } else {
                pos = value_end(bytes, pos);
            }
        }
        return pos + 1;
    } else {
        while pos < bytes.len() && !matches!(bytes[pos], b',' | b'}' | b']' | b' ') {
            pos += 1;
        }
        return pos;
    }
    panic!("unterminated JSON")
}

fn replace(bytes: &str, pointer: &str, replacement: &str) -> String {
    let data = bytes.as_bytes();
    let mut start = 0;
    for part in pointer.split('/').skip(1) {
        if data[start] == b'[' {
            start += 1;
            for _ in 0..part.parse::<usize>().unwrap() {
                start = value_end(data, start) + 1;
            }
        } else {
            assert_eq!(data[start], b'{');
            start += 1;
            loop {
                let key_end = value_end(data, start);
                let key: String = serde_json::from_str(&bytes[start..key_end]).unwrap();
                start = key_end + 1;
                if key == part {
                    break;
                }
                start = value_end(data, start) + 1;
            }
        }
    }
    let end = value_end(data, start);
    let render = |value: &str| format!("{}{}{}", &bytes[..start], value, &bytes[end..]);
    assert_eq!(render(&bytes[start..end]), bytes);
    render(replacement)
}

#[test]
fn graph_v34_deep_admitted_instance_does_not_depend_on_json_parser_recursion() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let mut expression = "identity<i64>(42)".to_owned();
            for _ in 0..62 {
                expression = format!("if true {{ {expression} }} else {{ 0 }}");
            }
            let program = checked(&format!(
                r#"module test.deep_generic_graph;
@id("g.identity") fn identity<T>(value: T) -> T {{ value }}
@id("app.main") fn main() -> i64 {{ {expression} }}
"#
            ));
            let legacy = graph::to_legacy_json(&program).unwrap();
            assert!(
                serde_json::from_str::<Value>(&legacy).is_err(),
                "fixture must cross the standard JSON parser depth limit"
            );
            let bytes = graph::to_json(&program).unwrap();
            assert!(bytes.starts_with("{\"schema\":\"semaprax.graph.v34\""));
            graph::verify_json(&program, &bytes).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn graph_v34_ordered_argument_vectors_and_repeat_sites_share_one_identity() {
    let program = checked(
        r#"module test.ordered_generic_graph;
@id("g.first") fn first<T, U>(value: T, other: U) -> T { value }
@id("app.main") fn main() -> i64 { first<i64, bool>(first<i64, bool>(42, true), false) }
"#,
    );
    let bytes = graph::to_json(&program).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    let instances = value["generic_instance_ownership"].as_array().unwrap();
    assert_eq!(instances.len(), 1);
    let instance = &instances[0];
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"semaprax.generic-instance-identity.v1\0");
    for part in [graph::revision(&program).as_str(), "g.first", "i64", "bool"] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    let expected = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    );
    assert_eq!(instance["concrete_instance"], expected);
    let pointer = "/generic_instance_ownership/0/type_arguments";
    let mut reordered = instance["type_arguments"].as_array().unwrap().clone();
    reordered.reverse();
    let forged = replace(&bytes, pointer, &serde_json::to_string(&reordered).unwrap());
    assert!(graph::verify_json(&program, &forged).is_err());
}

#[test]
fn graph_v34_flat_expression_composition_selects_v5_and_preserves_modes() {
    let program = checked(
        r#"module test.expression_graph;
@id("g.pair") record Pair<T, U> { @id("g.payload") payload: T, @id("g.marker") marker: U, }
@id("g.compose") fn compose<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {
    let observed = match borrow value { Pair { payload: _, marker: marker } => marker, };
    let updated = value with { marker: observed };
    match own updated {
        Pair { payload: payload, marker: marker } => Pair<Bytes, T> { payload: payload, marker: marker },
    }
}
@id("g.invoke") fn invoke(value: own Pair<Bytes, bool>) -> Pair<Bytes, bool> { compose<bool>(value) }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let bytes = graph::to_json(&program).unwrap();
    let graph: Value = serde_json::from_str(&bytes).unwrap();
    let instance = &graph["generic_instance_ownership"][0];
    assert_eq!(instance["cleanup_plan_schema"], "semaprax.cleanup-plan.v5");
    assert_eq!(instance["parameters"][0]["ownership_mode"], "own");
    assert_eq!(instance["result"]["ownership_mode"], "own");
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == instance["body_reference"])
        .unwrap();
    assert_eq!(node["params"][0]["ownership_mode"], "own");
    let body = node["body"].to_string();
    assert!(body.contains("\"ownership_mode\":\"borrow\""));
    assert!(body.contains("\"ownership_mode\":\"own\""));
    graph::verify_json(&program, &bytes).unwrap();
}

#[test]
fn graph_v34_two_parameter_forwarding_rejects_map_and_callee_permutation() {
    let program = checked(
        r#"module test.forwarded_graph;
@id("g.first") fn first<T, U>(value: T, other: U) -> T { value }
@id("g.outer") fn outer<T, U>(value: T, other: U) -> T { first<T, U>(value, other) }
@id("app.main") fn main() -> i64 { outer<i64, bool>(42, true) }
"#,
    );
    let bytes = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &bytes).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    for pointer in [
        "/generic_instance_ownership/1/call_edges/0/forwarded_argument_mapping",
        "/generic_instance_ownership/1/call_edges/0/callee_concrete_arguments",
    ] {
        let mut vector = value.pointer(pointer).unwrap().as_array().unwrap().clone();
        assert_eq!(vector.len(), 2);
        vector.reverse();
        let forged = replace(&bytes, pointer, &serde_json::to_string(&vector).unwrap());
        assert_eq!(
            graph::verify_json(&program, &forged).unwrap_err()[0].code,
            "SPX-G411"
        );
    }
}

#[test]
fn graph_v34_composes_nested_relay_with_checked_local_byte_loans() {
    let program = checked(
        r#"module test.nested_loan_graph;
@id("g.pair") record Pair<T, U> { @id("g.payload") payload: T, @id("g.marker") marker: U, }
@id("g.box") record Box<T> { @id("g.value") value: T, }
@id("g.relay") fn relay<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> { value }
@id("g.consume") fn consume(value: own Box<Pair<Bytes, bool>>) -> i64 {
    let moved = relay<bool>(value);
    match own moved { Box { value: Pair { payload: payload, marker: marker } } =>
        if marker && byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 1 }, }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    assert_eq!(
        graph::to_legacy_json(&program).unwrap_err()[0].code,
        "SPX-G410"
    );
    let bytes = graph::to_json(&program).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(value["schema"], "semaprax.graph.v34");
    let consume = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "g.consume")
        .unwrap();
    assert!(consume["loans"].is_object());
    assert_eq!(
        value["generic_instance_ownership"][0]["cleanup_plan_schema"],
        "semaprax.cleanup-plan.v7"
    );
    let options = graph::AgentContextOptions::new(
        1,
        1024 * 1024,
        32,
        [
            graph::AgentContextFilter::Types,
            graph::AgentContextFilter::Ownership,
        ],
    )
    .unwrap();
    assert_eq!(
        graph::agent_context_json(&program, "g.consume", &options).unwrap_err()[0].code,
        "SPX-G410"
    );
    let options_v2 = graph::AgentContextV2Options::new(
        1,
        1024 * 1024,
        32,
        [
            graph::AgentContextFilter::Types,
            graph::AgentContextFilter::Ownership,
        ],
        graph::AgentContextDirection::Forward,
    )
    .unwrap();
    let context = graph::agent_context_v2_json(&program, "g.consume", &options_v2)
        .unwrap()
        .unwrap();
    let context: Value = serde_json::from_str(&context).unwrap();
    assert_eq!(context["source_graph_schema"], "semaprax.graph.v34");
    graph::verify_json(&program, &bytes).unwrap();
}

#[test]
fn graph_v34_type_facts_include_concrete_instance_bodies_and_template_contexts() {
    let program = checked(
        r#"module test.generic_body_type_facts;
@id("g.measure") fn measure<T>(value: T) -> i64 {
    let text = string_from_char('\0');
    string_len_chars(text)
}
@id("app.main") fn main() -> i64 { measure<i64>(42) }
"#,
    );
    let legacy: Value = serde_json::from_str(&graph::to_legacy_json(&program).unwrap()).unwrap();
    assert!(!legacy["type_facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|fact| fact["id"] == "string"));
    for bytes in [
        graph::to_json(&program).unwrap(),
        graph::context_json(&program, "g.measure", 0)
            .unwrap()
            .unwrap(),
    ] {
        let value: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(value["schema"], "semaprax.graph.v34");
        let string = value["type_facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["id"] == "string")
            .expect("concrete body String has checked facts");
        assert_eq!(string["facts"]["copy"], false);
        assert_eq!(string["facts"]["needs_drop"], true);
        assert_eq!(string["facts"]["layout_key"], "owned:string");
    }
    let legacy_context: Value = serde_json::from_str(
        &graph::legacy_context_json(&program, "g.measure", 0)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(!legacy_context["type_facts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|fact| fact["id"] == "string"));

    let program = checked(&source(&["bool"], &["Box<Pair<Bytes, T>>"]));
    let context = graph::context_json(&program, "g.outer.0", 0)
        .unwrap()
        .unwrap();
    let value: Value = serde_json::from_str(&context).unwrap();
    let parameter_type =
        &value["generic_instance_ownership"][0]["parameters"][0]["substituted_type"];
    let fact = value["type_facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| &fact["id"] == parameter_type)
        .expect("template slice has concrete owner type facts");
    assert_eq!(fact["facts"]["copy"], false);
    assert_eq!(fact["facts"]["needs_drop"], true);
}
