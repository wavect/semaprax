use std::path::Path;

use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::parse;
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"
module test.graph_generics;
@id("test.choice")
variant Choice<T> {
    @id("test.choice.none") None,
    @id("test.choice.value") Value {
        @id("test.choice.value.value") value: T,
    },
}
@id("test.inspect_choice")
fn inspect_choice(choice: Choice<i64>) -> i64 {
    match choice {
        Choice::Value { value } => value,
        Choice::None {} => 0,
    }
}
@id("test.inspect_option")
fn inspect_option(option: Option<bool>) -> i64 {
    match option {
        Option::Some { value } => if value { 1 } else { 0 },
        Option::None {} => 0,
    }
}
@id("app.main")
fn main() -> i64 { inspect_choice(Choice<i64>::Value { value: 42 }) }
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("graph-generics.spx")).unwrap()
}

#[test]
fn graph_v10_authenticates_generic_templates_concrete_instances_and_prelude() {
    let program = program();
    let json = graph::to_json(&program).unwrap();
    assert!(json.starts_with("{\"schema\":\"semaprax.graph.v10\""));
    assert!(!json.contains("semaprax.graph.v8"));
    assert!(json.contains("\"prelude\":{\"schema\":\"semaprax.prelude.v1\",\"digest\":\"sha256:"));
    let digest_marker = "\"prelude\":{\"schema\":\"semaprax.prelude.v1\",\"digest\":\"";
    let digest_start = json.find(digest_marker).unwrap() + digest_marker.len();
    assert_eq!(
        &json[digest_start..digest_start + 71],
        "sha256:d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e"
    );
    assert!(json.contains("\"type_parameters\":\"owner-and-index-stable\""));
    assert!(json.contains(
        "\"id\":\"test.choice\",\"kind\":\"variant\",\"name\":\"Choice\",\"identity_origin\":\"explicit\",\"persistent\":true,\"type_parameters\":[{\"id\":\"parameter:11:test.choice:0\",\"owner\":\"test.choice\",\"index\":0,\"name\":\"T\"}],\"type_id\":null"
    ));
    assert!(json.contains(
        "\"id\":\"core.option\",\"kind\":\"variant\",\"name\":\"Option\",\"identity_origin\":\"compiler_owned\",\"persistent\":true"
    ));
    assert!(json.contains("\"id\":\"core.option.none\",\"kind\":\"variant_case\""));
    assert!(json.contains("\"id\":\"core.option.some\",\"kind\":\"variant_case\""));
    assert!(json.contains("\"id\":\"core.result.ok\",\"kind\":\"variant_case\""));
    assert!(json.contains("\"id\":\"core.result.err\",\"kind\":\"variant_case\""));
    assert!(json.contains(
        "\"type\":{\"kind\":\"nominal\",\"declaration\":\"test.choice\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"i64\"}]}"
    ));
    assert!(json.contains(
        "\"type\":{\"kind\":\"nominal\",\"declaration\":\"core.option\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"bool\"}]}"
    ));

    assert_eq!(
        graph::revision(&program),
        "sha256:6f2620dec3fa5b3c9f6eddc17ea8bae0bbd206ce393a343a1d8f3b2155c75936"
    );
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(json.as_bytes()))
        ),
        "3a61e0e6860355916ff4e303b27b59881d76a92490508c48ee141191b7759f3f"
    );
    assert_eq!(json, graph::to_json(&program).unwrap());
}

#[test]
fn bounded_context_includes_compiler_prelude_only_when_referenced() {
    let program = program();
    let choice = graph::context_json(&program, "test.inspect_choice", 0)
        .unwrap()
        .unwrap();
    assert!(choice.contains("\"id\":\"test.choice\""));
    assert!(!choice.contains("\"id\":\"core.option\""));
    assert!(!choice.contains("\"id\":\"core.result\""));

    let option = graph::context_json(&program, "test.inspect_option", 0)
        .unwrap()
        .unwrap();
    assert!(option.contains("\"id\":\"core.option\""));
    assert!(option.contains("\"id\":\"core.option.some.value\""));
    assert!(!option.contains("\"id\":\"test.choice\""));
    assert!(!option.contains("\"id\":\"core.result\""));

    let options = AgentContextOptions::new(
        0,
        16 * 1024,
        8,
        [AgentContextFilter::Types, AgentContextFilter::Ownership],
    )
    .unwrap();
    let agent = graph::agent_context_json(&program, "test.inspect_option", &options)
        .unwrap()
        .unwrap();
    assert!(agent.contains("\"source_graph_schema\":\"semaprax.graph.v10\""));
    assert!(agent.contains("\"prelude\":{\"schema\":\"semaprax.prelude.v1\""));
    assert!(agent.contains("\"id\":\"core.option\",\"kind\":\"variant\""));
    assert!(!agent.contains("\"id\":\"core.result\""));
}

#[test]
fn graph_v9_and_rehashed_hostile_documents_fail_the_exact_v10_contract() {
    fn accepts_v10(bytes: &str, expected_digest: &str) -> bool {
        bytes.starts_with("{\"schema\":\"semaprax.graph.v10\",")
            && !bytes.contains("\"schema\":\"semaprax.graph.v9\"")
            && bytes.contains(
                "\"id\":\"core.option.none\",\"kind\":\"variant_case\",\"name\":\"None\",\"identity_origin\":\"compiler_owned\",\"persistent\":true,\"owner\":\"core.option\",\"index\":0",
            )
            && bytes.contains(
                "\"id\":\"core.option.some\",\"kind\":\"variant_case\",\"name\":\"Some\",\"identity_origin\":\"compiler_owned\",\"persistent\":true,\"owner\":\"core.option\",\"index\":1",
            )
            && format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
            ) == expected_digest
    }

    let graph = graph::to_json(&program()).unwrap();
    let digest = format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(graph.as_bytes()))
    );
    assert!(accepts_v10(&graph, &digest));
    assert!(!accepts_v10(
        &graph.replacen("semaprax.graph.v10", "semaprax.graph.v9", 1),
        &digest
    ));
    assert!(!accepts_v10(
        &graph.replacen("core.option.some", "core.option.none", 1),
        &digest
    ));
    let rehashed_hostile = graph.replacen(
        "\"owner\":\"core.option\",\"index\":1",
        "\"owner\":\"core.option\",\"index\":0",
        1,
    );
    let hostile_digest = format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(rehashed_hostile.as_bytes()))
    );
    assert!(!accepts_v10(&rehashed_hostile, &digest));
    assert!(!accepts_v10(&rehashed_hostile, &hostile_digest));
}
