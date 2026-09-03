use std::path::Path;

use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::parse;
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"
module test.graph_result_try;
@id("test.source")
fn source(flag: bool) -> Result<i64, bool> {
    if flag {
        Result<i64, bool>::Err { error: true }
    } else {
        Result<i64, bool>::Ok { value: 42 }
    }
}
@id("test.propagate")
fn propagate(flag: bool) -> Result<bool, bool>
    ensures match result {
        Result::Ok { value } => value,
        Result::Err { error } => error,
    }
{
    let value = source(flag)?;
    Result<bool, bool>::Ok { value: value > 0 }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("graph-result-try.spx")).unwrap()
}

fn accepts_v10(bytes: &str, expected_digest: &str) -> bool {
    bytes.starts_with("{\"schema\":\"semaprax.graph.v10\",")
        && !bytes.contains("\"schema\":\"semaprax.graph.v9\"")
        && bytes.contains("\"kind\":\"try_result\",\"evaluation\":\"once\"")
        && bytes.matches("\"result\":\"core.result\"").count() == 2
        && bytes.matches("\"ok_case\":\"core.result.ok\"").count() == 2
        && bytes
            .matches("\"ok_field\":\"core.result.ok.value\"")
            .count()
            == 2
        && bytes.matches("\"err_case\":\"core.result.err\"").count() == 2
        && bytes
            .matches("\"err_field\":\"core.result.err.error\"")
            .count()
            == 2
        && bytes.matches("\"err_exit\":\"normal_result\"").count() == 1
        && bytes
            .matches("\"epilogue\":\"shared_postconditions\"")
            .count()
            == 1
        && format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
        ) == expected_digest
}

#[test]
fn graph_v10_exposes_exact_typed_result_propagation_meaning() {
    let program = program();
    assert_eq!(
        graph::revision(&program),
        "sha256:4cebcdf01741fc87f92acd7eb37026ccd06a7eadfec0a1007bb767bcc2e58e87"
    );
    let graph = graph::to_json(&program).unwrap();
    let digest = format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(graph.as_bytes()))
    );
    assert_eq!(
        digest,
        "9419dc034c1035cd703ab7f37a7eaf91fa20f0505c2270fcf05f2946c91525b2"
    );
    assert!(accepts_v10(&graph, &digest));
    assert!(graph.contains(
        "\"source_result_type\":{\"kind\":\"nominal\",\"declaration\":\"core.result\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"i64\"},{\"kind\":\"primitive\",\"name\":\"bool\"}]}"
    ));
    assert!(graph.contains(
        "\"residual_result_type\":{\"kind\":\"nominal\",\"declaration\":\"core.result\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"bool\"},{\"kind\":\"primitive\",\"name\":\"bool\"}]}"
    ));
    assert!(graph.contains("\"schema\":\"semaprax.cleanup-plan.v2\""));
    assert_eq!(graph, graph::to_json(&program).unwrap());
}

#[test]
fn bounded_context_carries_propagation_and_only_its_referenced_prelude() {
    let options = AgentContextOptions::new(
        0,
        32 * 1024,
        8,
        [AgentContextFilter::Types, AgentContextFilter::Contracts],
    )
    .unwrap();
    let context = graph::agent_context_json(&program(), "test.propagate", &options)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"source_graph_schema\":\"semaprax.graph.v10\""));
    assert!(context.contains("\"result_propagations\":[{"));
    assert!(context.contains("\"kind\":\"try_result\""));
    assert!(context.contains("\"id\":\"core.result\",\"kind\":\"variant\""));
    assert!(!context.contains("\"id\":\"core.option\",\"kind\":\"variant\""));
}

#[test]
fn graph_v9_and_rehashed_try_confusion_fail_the_exact_v10_contract() {
    let graph = graph::to_json(&program()).unwrap();
    let digest = format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(graph.as_bytes()))
    );
    assert!(accepts_v10(&graph, &digest));

    let version_hostile = graph.replacen("semaprax.graph.v10", "semaprax.graph.v9", 1);
    assert!(!accepts_v10(&version_hostile, &digest));

    for (from, to) in [
        (
            "\"ok_field\":\"core.result.ok.value\"",
            "\"ok_field\":\"core.result.err.error\"",
        ),
        (
            "\"err_case\":\"core.result.err\"",
            "\"err_case\":\"core.result.ok\"",
        ),
        (
            "\"err_exit\":\"normal_result\"",
            "\"err_exit\":\"return_failure\"",
        ),
        (
            "\"epilogue\":\"shared_postconditions\"",
            "\"epilogue\":\"bypass_postconditions\"",
        ),
    ] {
        let hostile = graph.replacen(from, to, 1);
        let hostile_digest = format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(hostile.as_bytes()))
        );
        assert!(!accepts_v10(&hostile, &digest));
        assert!(
            !accepts_v10(&hostile, &hostile_digest),
            "accepted rehashed mutation {from}"
        );
    }
    assert!(!accepts_v10(&format!("{graph}x"), &digest));
}
