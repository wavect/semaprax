//! Frontend placement uses ordinary checked bodies; this is not an Agent-loop gate.
use semaprax::{ast::AgentOperationKind, format, hir, interpreter, project};

use super::{agent_definition_v1, profile, source_agent_lowering};

fn source(wait: bool) -> String {
    let mut text = String::from(
        "module fixture.embedded;\n@id(\"fixture.agent\") agent FixtureAgent { types {\n",
    );
    for role in [
        "task",
        "state",
        "observation",
        "proposal",
        "outcome",
        "result",
    ] {
        text.push_str(&format!(
            "@id(\"fixture.agent.type.{role}\") type {role};\n"
        ));
    }
    text.push_str("} operations {\n");
    for role in [
        "initialize",
        "observe",
        "propose",
        "authorize",
        "execute",
        "reduce",
    ] {
        text.push_str(&format!("@id(\"fixture.agent.fn.{role}\") "));
        match role {
            "propose" => text.push_str("model fn propose;\n"),
            "execute" => text.push_str("effect fn execute;\n"),
            _ => text.push_str(&format!("fn {role}(value:i64)->i64 {{ value + 1 }}\n")),
        }
    }
    text.push_str("}\n");
    if wait {
        text.push_str("model_wait_v1 { propose = \"fixture.wait\"; }\n");
    }
    let runtime = source_agent_lowering::runtime_v1(&profile());
    text.push_str(&format!(
        "runtime_v1 {{ canonical_json {}; }} }}\n",
        serde_json::to_string(&runtime).unwrap()
    ));
    if wait {
        text.push_str(
            "@id(\"fixture.wait\") fn wait(value:i64)->i64 yields i64 -> i64 { yield value }\n",
        );
    }
    text.push_str("@id(\"fixture.main\") fn main()->i64 {observe(41)}\n");
    text
}

#[test]
fn embedded_source_keeps_frozen_definition_and_ordinary_execution() {
    let parsed = semaprax::check(&source(true), "embedded.spx").unwrap();
    let canonical = format::canonical(&parsed);
    let repeated = semaprax::check(&canonical, "embedded.spx").unwrap();
    assert_eq!(format::canonical(&repeated), canonical);
    let compiled = project::compile_source_program_agents(&repeated).unwrap();
    assert_eq!(
        compiled.definitions()[0].definition().canonical_source(),
        agent_definition_v1::definition(&profile())
    );
    assert_eq!(compiled.definitions()[0].runtime_v1_profile(), profile());
    let embedded = hir::resolve(&repeated).unwrap();
    hir::validate(&embedded).unwrap();
    assert_eq!(embedded.functions.len(), 6);
    assert_eq!(
        embedded.agents[0]
            .operations
            .iter()
            .filter(|op| op.embedded)
            .count(),
        4
    );
    assert_eq!(
        embedded.agents[0]
            .model_wait
            .as_ref()
            .unwrap()
            .helper_id
            .as_str(),
        "fixture.wait"
    );

    let mut reference = repeated.clone();
    reference.agents[0].model_wait = None;
    for operation in &mut reference.agents[0].operations {
        operation.embedded_function_index = None;
    }
    let reference = hir::resolve(&reference).unwrap();
    for operation in &repeated.agents[0].operations {
        if operation.kind != AgentOperationKind::Deterministic {
            continue;
        }
        let body = embedded
            .functions
            .iter()
            .find(|f| f.id.as_str() == operation.stable_id)
            .unwrap();
        let old = reference
            .functions
            .iter()
            .find(|f| f.id == body.id)
            .unwrap();
        assert_eq!(
            body, old,
            "placement must not create a second body or evaluator"
        );
    }

    static NEXT_SOURCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT_SOURCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-embedded-{}-{serial}.spx",
        std::process::id()
    ));
    struct File(std::path::PathBuf);
    impl Drop for File {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = File(path);
    std::fs::write(&file.0, canonical).unwrap();
    let result = interpreter::interpret(
        &file.0,
        "fixture.agent.fn.observe",
        &["41".to_owned()],
        &interpreter::InterpreterOptions::new(4096, 128).unwrap(),
    )
    .unwrap();
    assert!(result.returned);
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(
        envelope["payload"]["outcome"],
        serde_json::json!({"kind":"returned", "type":"i64", "value":"42"})
    );
}

#[test]
fn opted_in_and_legacy_agents_share_a_module_without_invented_legacy_edges() {
    let text = source(true);
    let start = text.find("@id(\"fixture.agent\")").unwrap();
    let end = text.find("@id(\"fixture.wait\") fn").unwrap();
    let legacy = text[start..end]
        .replace("fixture.agent", "legacy.agent")
        .replace("FixtureAgent", "LegacyAgent")
        .replace("model_wait_v1 { propose = \"fixture.wait\"; }\n", "");
    // Preserve an unchanged reference-only Agent, whose inert roles do not resolve here.
    let mut legacy = semaprax::parse(&format!("module legacy;\n{legacy}"), "legacy.spx").unwrap();
    legacy.functions.clear();
    for operation in &mut legacy.agents[0].operations {
        operation.embedded_function_index = None;
    }
    let canonical = format::canonical(&legacy);
    let text = format!("{text}\n{}", canonical.split_once("\n\n").unwrap().1);
    let parsed = semaprax::check(&text, "mixed.spx").unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    assert_eq!(resolved.agents.len(), 2);
    assert_eq!(resolved.functions.len(), 6);
    assert!(resolved.agents[1].operations.iter().all(|op| !op.embedded));
    assert!(resolved.agents[1].model_wait.is_none());
    let graph: serde_json::Value =
        serde_json::from_str(&semaprax::graph::to_json(&parsed).unwrap()).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v50");
    let agents = graph["agent_execution"]["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["agent"], "fixture.agent");
}
