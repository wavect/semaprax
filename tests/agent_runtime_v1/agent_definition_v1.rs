use semaprax::agent_definition::{compile_agent_definition, verify_agent_graph_bundle};
use semaprax::agent_runtime::{AgentCancellation, AgentRunStatus};

use super::{profile, raw_sha, task, Host};

pub(super) fn definition(profile: &str) -> String {
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
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
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
    assert!(first
        .graph()
        .canonical_json()
        .contains("\"to\":\"@authorized_proposal\""));
    assert!(first
        .graph()
        .canonical_json()
        .contains("\"model_cannot_mint\":true"));
    assert!(first.graph().canonical_json().contains("\"max_turns\":2"));
    assert!(!first.graph().canonical_json().contains("fake.bytes-v1"));
    assert!(!first
        .graph()
        .canonical_json()
        .contains("input_usd_microunits_per_million_tokens"));
    assert!(!first
        .graph()
        .canonical_json()
        .contains("output_usd_microunits_per_million_tokens"));
    verify_agent_graph_bundle(&source, &profile, first.graph().canonical_json()).unwrap();
}

#[test]
fn projected_profile_executes_through_the_unchanged_v1_kernel() {
    let compiled = compile_agent_definition(&definition(&profile())).unwrap();
    let mut agent = compiled
        .instantiate(Host::new(), AgentCancellation::new())
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

#[test]
fn graph_bundle_replay_rejects_tamper_cross_pair_and_capacity() {
    let profile = profile();
    let source = definition(&profile);
    let compiled = compile_agent_definition(&source).unwrap();

    let tampered_graph = compiled.graph().canonical_json().replacen(
        "\"single_use\":true",
        "\"single_use\":false",
        1,
    );
    let error = verify_agent_graph_bundle(&source, &profile, &tampered_graph)
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G503");

    let tampered_profile = profile.replacen("\"max_turns\":2", "\"max_turns\":3", 1);
    let error = verify_agent_graph_bundle(
        &source,
        &tampered_profile,
        compiled.graph().canonical_json(),
    )
    .err()
    .unwrap();
    assert_eq!(error[0].code, "SPX-G504");

    let other_source = source.replacen(
        "fixture.agent.type.observation",
        "fixture.agent.type.other_observation",
        1,
    );
    let error =
        verify_agent_graph_bundle(&other_source, &profile, compiled.graph().canonical_json())
            .err()
            .unwrap();
    assert_eq!(error[0].code, "SPX-G503");

    let oversized = "x".repeat(1_572_865);
    let error = verify_agent_graph_bundle(&source, &profile, &oversized)
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G503");
}

#[test]
fn graph_sections_track_definition_semantics_without_changing_stable_v1_behavior() {
    let profile = profile();
    let source = definition(&profile);
    let baseline = compile_agent_definition(&source).unwrap();

    for (old, new, graph_witness) in [
        (
            "\"required_locality\":\"local_only\"",
            "\"required_locality\":\"remote_allowed\"",
            "\"required_locality\":\"remote_allowed\"",
        ),
        (
            "\"quality_tier\":\"basic\"",
            "\"quality_tier\":\"standard\"",
            "\"minimum_quality_tier\":\"standard\"",
        ),
        (
            "tool.read",
            "tool.read.v2",
            "\"granted\":[\"tool.read.v2\"]",
        ),
        (
            "Return one bounded fixture value.",
            "Return one other bounded fixture value.",
            "Return one other bounded fixture value.",
        ),
        (
            "\"name\":\"query\"",
            "\"name\":\"question\"",
            "\"name\":\"question\"",
        ),
        (
            "\"name\":\"value\"",
            "\"name\":\"payload\"",
            "\"name\":\"payload\"",
        ),
        ("\"max_turns\":2", "\"max_turns\":3", "\"max_turns\":3"),
    ] {
        let mut changed_source = source.replace(old, new);
        if old == "\"quality_tier\":\"basic\"" {
            changed_source = changed_source.replace(
                "\"minimum_quality_tier\":\"basic\"",
                "\"minimum_quality_tier\":\"standard\"",
            );
        }
        let changed = compile_agent_definition(&changed_source).unwrap();
        assert_ne!(changed.runtime_v1_profile(), baseline.runtime_v1_profile());
        assert_ne!(changed.graph().digest(), baseline.graph().digest());
        assert!(changed.graph().canonical_json().contains(graph_witness));
    }

    let identity_source = source.replacen(
        "fixture.agent.type.observation",
        "fixture.agent.type.renamed_observation",
        1,
    );
    let identity_changed = compile_agent_definition(&identity_source).unwrap();
    assert_eq!(
        identity_changed.runtime_v1_profile(),
        baseline.runtime_v1_profile()
    );
    assert_ne!(
        identity_changed.definition().digest(),
        baseline.definition().digest()
    );
    assert_ne!(identity_changed.graph().digest(), baseline.graph().digest());
}
