use semaprax::agent_definition::compile_agent_definition;
use semaprax::agent_runtime::{Agent, AgentCancellation, AgentRunStatus};

use super::{profile, raw_sha, task, Host};

fn definition(profile: &str) -> String {
    let profile_body = profile.strip_suffix('\n').unwrap();
    let profile_members = profile_body
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",",
        )
        .unwrap();
    let (runtime_members, _) = profile_members.split_once(",\"nonclaims\":").unwrap();
    let runtime_v1 = format!("{{{runtime_members}}}");
    concat!(
            "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"fixture.agent\",",
            "\"types\":[",
            "{\"role\":\"task\",\"stable_id\":\"fixture.agent.type.task\"},",
            "{\"role\":\"state\",\"stable_id\":\"fixture.agent.type.state\"},",
            "{\"role\":\"observation\",\"stable_id\":\"fixture.agent.type.observation\"},",
            "{\"role\":\"proposal\",\"stable_id\":\"fixture.agent.type.proposal\"},",
            "{\"role\":\"outcome\",\"stable_id\":\"fixture.agent.type.outcome\"},",
            "{\"role\":\"result\",\"stable_id\":\"fixture.agent.type.result\"}],",
            "\"operations\":[",
            "{\"role\":\"initialize\",\"stable_id\":\"fixture.agent.fn.initialize\",\"kind\":\"deterministic\"},",
            "{\"role\":\"observe\",\"stable_id\":\"fixture.agent.fn.observe\",\"kind\":\"deterministic\"},",
            "{\"role\":\"propose\",\"stable_id\":\"fixture.agent.fn.propose\",\"kind\":\"model\"},",
            "{\"role\":\"authorize\",\"stable_id\":\"fixture.agent.fn.authorize\",\"kind\":\"deterministic\"},",
            "{\"role\":\"execute\",\"stable_id\":\"fixture.agent.fn.execute\",\"kind\":\"effect\"},",
            "{\"role\":\"reduce\",\"stable_id\":\"fixture.agent.fn.reduce\",\"kind\":\"deterministic\"}],",
        "\"runtime_v1\":RUNTIME}\n"
    )
    .replace("RUNTIME", &runtime_v1)
}

#[test]
fn definition_compiles_to_deterministic_graph_and_exact_v1_profile() {
    let profile = profile();
    let source = definition(&profile);
    let first = compile_agent_definition(&source).unwrap();
    let second = compile_agent_definition(&source).unwrap();

    assert_eq!(first.definition().canonical_source(), source);
    assert_eq!(first.definition().agent_id(), "fixture.agent");
    assert_eq!(
        first.definition().digest(),
        "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0"
    );
    assert_eq!(
        first.graph().digest(),
        "sha256:04f1aa2c674a4b65b78504007e87686c3163aa9ef7cf46b2e845d3448d24024f"
    );
    assert_eq!(first.runtime_v1_profile().as_bytes(), profile.as_bytes());
    assert_eq!(raw_sha(first.runtime_v1_profile()), raw_sha(&profile));
    assert_eq!(
        first.graph().canonical_json(),
        second.graph().canonical_json()
    );
    assert_eq!(first.graph().digest(), second.graph().digest());
    assert!(first.graph().canonical_json().ends_with('\n'));
    assert!(first
        .graph()
        .canonical_json()
        .contains("\"relationship\":\"returns\""));
    assert!(first
        .graph()
        .canonical_json()
        .contains("\"kind\":\"model\""));
    assert!(!first.graph().canonical_json().contains("fake.bytes-v1"));
}

#[test]
fn projected_profile_executes_through_the_unchanged_v1_kernel() {
    let compiled = compile_agent_definition(&definition(&profile())).unwrap();
    let mut agent = Agent::new(
        compiled.runtime_v1_profile(),
        Host::new(),
        AgentCancellation::new(),
    )
    .unwrap();
    let run = agent.run(&task()).unwrap();
    assert_eq!(run.status(), AgentRunStatus::Completed);
    assert_eq!(run.final_message(), Some("done"));
}

#[test]
fn definition_rejects_noncanonical_and_semantically_invalid_inputs() {
    let source = definition(&profile());
    let reordered = source.replacen(
        "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"fixture.agent\"",
        "{\"agent_id\":\"fixture.agent\",\"schema\":\"semaprax.agent-definition.v1\"",
        1,
    );
    let error = compile_agent_definition(&reordered).err().unwrap();
    assert_eq!(error[0].code, "SPX-G501");
    assert_eq!(
        error[0].message,
        "AgentDefinition is not canonical semaprax.agent-definition.v1 JSON"
    );

    let wrong_kind = source.replacen("\"kind\":\"model\"", "\"kind\":\"effect\"", 1);
    let error = compile_agent_definition(&wrong_kind).err().unwrap();
    assert_eq!(error[0].code, "SPX-G502");
    assert_eq!(
        error[0].message,
        "AgentDefinition invariant failed: operations.roles"
    );

    let widened_effect = source.replacen("\"effects\":[\"read\"]", "\"effects\":[\"write\"]", 1);
    let error = compile_agent_definition(&widened_effect).err().unwrap();
    assert_eq!(error[0].code, "SPX-G502");
    assert_eq!(
        error[0].message,
        "AgentDefinition invariant failed: runtime_v1_profile"
    );

    let colliding = source.replacen("fixture.agent.fn.initialize", "fixture.agent.type.task", 1);
    let error = compile_agent_definition(&colliding).err().unwrap();
    assert_eq!(error[0].code, "SPX-G502");
    assert_eq!(
        error[0].message,
        "AgentDefinition invariant failed: semantic_ids"
    );
}
