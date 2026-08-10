use std::path::Path;

use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::parse;
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"
module test.graph_variants;
@id("test.choice")
variant Choice {
    @id("test.choice.none") None,
    @id("test.choice.number") Number {
        @id("test.choice.number.value") value: i64,
    },
    @id("test.choice.flag") Flag {
        @id("test.choice.flag.value") value: bool,
    },
}
@id("test.unrelated")
variant Unrelated { @id("test.unrelated.unit") Unit, }
@id("test.inspect")
fn inspect(choice: Choice) -> i64 {
    match choice {
        Choice::Number { value: number } => number,
        Choice::Flag { value: flag } => if flag { 1 } else { 0 },
        _ => 0,
    }
}
@id("app.main")
fn main() -> i64 { inspect(Choice::Number { value: 42 }) }
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("graph-variants.spx")).unwrap()
}

#[test]
fn graph_v10_exposes_variant_case_field_construction_and_match_meaning() {
    let json = graph::to_json(&program()).unwrap();
    assert!(json.starts_with("{\"schema\":\"semaprax.graph.v10\""));
    assert!(!json.contains("semaprax.graph.v8"));
    assert!(json.contains("\"match_arms\":\"revision-scoped-structural\""));
    assert!(json.contains("\"patterns\":\"revision-scoped-structural\""));
    assert!(json.contains("\"id\":\"test.choice\",\"kind\":\"variant\",\"name\":\"Choice\""));
    assert!(json.contains(
        "\"id\":\"test.choice.number\",\"kind\":\"variant_case\",\"name\":\"Number\",\"identity_origin\":\"explicit\",\"persistent\":true,\"owner\":\"test.choice\",\"index\":1"
    ));
    assert!(json.contains(
        "\"id\":\"test.choice.number.value\",\"kind\":\"case_field\",\"name\":\"value\""
    ));
    assert!(json.contains(
        "\"kind\":\"construct_variant\",\"variant\":\"test.choice\",\"case\":\"test.choice.number\""
    ));
    assert!(json.contains("\"kind\":\"match\",\"exhaustive\":true"));
    assert!(json.contains("\"kind\":\"match_arm\""));
    assert!(json.contains("\"kind\":\"variant_pattern\",\"variant\":\"test.choice\""));
    assert!(json.contains("\"kind\":\"wildcard_pattern\""));
    assert!(json.contains("\"field\":\"test.choice.number.value\",\"binding\":{\"id\":"));
    assert!(json.contains("\"kind\":\"variant_case\",\"scrutinee\":"));
    assert!(json.contains("\"case\":\"test.choice.number\",\"matches\":true"));

    let digest = format!("{:x}", Sha256::digest(json.as_bytes()));
    assert_eq!(digest.len(), 64);
    assert_eq!(
        digest,
        "fc5e9a53f0da90fa3a5402bad86fda1684840143336dae14bc6ae5089289c0d6"
    );
    assert_eq!(json, graph::to_json(&program()).unwrap());
}

#[test]
fn graph_and_agent_context_close_over_referenced_variants_only() {
    let program = program();
    let context = graph::context_json(&program, "test.inspect", 0)
        .unwrap()
        .unwrap();
    for id in [
        "test.choice",
        "test.choice.none",
        "test.choice.number",
        "test.choice.number.value",
        "test.choice.flag",
        "test.choice.flag.value",
    ] {
        assert!(
            context.contains(&format!("\"id\":\"{id}\"")),
            "missing {id}"
        );
    }
    assert!(!context.contains("test.unrelated"));

    let options = AgentContextOptions::new(
        0,
        16 * 1024,
        8,
        [AgentContextFilter::Types, AgentContextFilter::Ownership],
    )
    .unwrap();
    let agent = graph::agent_context_json(&program, "test.inspect", &options)
        .unwrap()
        .unwrap();
    assert!(agent.contains("\"schema\":\"semaprax.agent-context.v1\""));
    assert!(agent.contains("\"source_graph_schema\":\"semaprax.graph.v10\""));
    assert!(agent.contains("\"kind\":\"variant\""));
    assert!(agent.contains("test.choice.number.value"));
    assert!(!agent.contains("test.unrelated"));
}

#[test]
fn graph_v9_version_confusion_is_rejected_by_the_exact_v10_contract() {
    fn accepts_v10(bytes: &str) -> bool {
        bytes.starts_with("{\"schema\":\"semaprax.graph.v10\",")
            && !bytes.contains("\"schema\":\"semaprax.graph.v9\"")
            && bytes.ends_with('}')
    }

    let graph = graph::to_json(&program()).unwrap();
    assert!(accepts_v10(&graph));
    let hostile = graph.replacen("semaprax.graph.v10", "semaprax.graph.v9", 1);
    assert!(!accepts_v10(&hostile));
    assert!(!accepts_v10(&format!("{graph}x")));
}
