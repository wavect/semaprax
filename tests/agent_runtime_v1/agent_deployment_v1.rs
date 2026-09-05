//! AgentDefinition v2 and AgentDeployment v1: source-owned semantics separated
//! from explicit deployment and model bindings.

use semaprax::agent_deployment::{
    bind_agent_deployment, compile_agent_definition_v2, compile_agent_deployment,
    migrate_agent_definition_v1, verify_bound_agent_deployment_bundle,
};
use semaprax::agent_runtime::AgentRunStatus;
use semaprax::agent_transcript;

use super::agent_definition_v1::definition;
use super::{profile, raw_sha, task};

const DEPLOYMENT_ID: &str = "fixture.deployment.local";

fn migrated() -> (String, String) {
    migrate_agent_definition_v1(&definition(&profile()), DEPLOYMENT_ID).unwrap()
}

/// Replaces a deployment's claimed semantic-definition digest.
fn with_definition_digest(deployment: &str, digest: &str) -> String {
    let marker = "\"definition_digest\":\"";
    let start = deployment
        .find(marker)
        .expect("a deployment names a digest")
        + marker.len();
    let end = start + "sha256:".len() + 64;
    format!("{}{digest}{}", &deployment[..start], &deployment[end..])
}

/// Rebinds a mutated definition to the deployment that must accompany it.
fn paired(definition_v2: &str, deployment: &str) -> String {
    let digest = compile_agent_definition_v2(definition_v2)
        .unwrap()
        .digest()
        .to_owned();
    with_definition_digest(deployment, &digest)
}

fn transcript(response: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-runtime-transcript.v1\",\"policy_epoch\":7,\"provider\":[{{\"disposition\":\"succeeded\",\"response\":{}}}],\"tools\":[]}}\n",
        serde_json::to_string(response).unwrap()
    )
}

const FINAL_ACTION: &str =
    "{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":\"done\"}\n";

#[test]
fn migration_round_trips_v1_bytes_and_preserves_its_frozen_known_answers() {
    let v1 = definition(&profile());
    let (definition_v2, deployment) = migrated();
    assert_eq!(migrated(), (definition_v2.clone(), deployment.clone()));

    let semantic = compile_agent_definition_v2(&definition_v2).unwrap();
    let selected = compile_agent_deployment(&deployment).unwrap();
    assert_eq!(semantic.canonical_json(), definition_v2);
    assert_eq!(selected.canonical_json(), deployment);
    assert_eq!(semantic.agent_id(), "fixture.agent");
    assert_eq!(selected.deployment_id(), DEPLOYMENT_ID);
    assert_eq!(selected.definition_digest(), semantic.digest());

    let bound = bind_agent_deployment(&definition_v2, &deployment).unwrap();
    assert_eq!(bound.runtime_v1_definition(), v1);
    assert_eq!(bound.runtime_v1_profile(), profile());
    assert_eq!(
        raw_sha(bound.runtime_v1_profile()),
        "sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a"
    );
    assert_eq!(
        bound.graph().digest(),
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
    );
    assert!(bound.canonical_json().ends_with('\n'));
    assert!(bound
        .canonical_json()
        .contains("\"definition_digest\":\"sha256:"));
    assert!(bound
        .canonical_json()
        .contains("\"deployment_digest\":\"sha256:"));
    assert!(bound.canonical_json().contains(
        "\"v1_definition_digest\":\"sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0\""
    ));

    verify_bound_agent_deployment_bundle(&definition_v2, &deployment, bound.canonical_json())
        .unwrap();
    let tampered = bound
        .canonical_json()
        .replacen("\"max_turns\":2", "\"max_turns\":1", 1);
    let error = verify_bound_agent_deployment_bundle(&definition_v2, &deployment, &tampered)
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G557");

    // Source semantics carry no provider, model, tokenizer, price, or
    // credential material.
    for deployment_only in [
        "fake.local",
        "fake-basic",
        "tokenizer_id",
        "usd_microunits_per_million_tokens",
        "max_context_tokens",
    ] {
        assert!(
            !definition_v2.contains(deployment_only),
            "the semantic definition carries `{deployment_only}`"
        );
    }
    // The deployment carries no source-owned semantic identity.
    for source_only in [
        "fixture.agent.type.",
        "fixture.agent.fn.",
        "arguments_schema",
        "\"effects\"",
    ] {
        assert!(
            !deployment.contains(source_only),
            "the deployment carries `{source_only}`"
        );
    }
    // The bound product publishes effective selection, not provider material.
    for absent in [
        "tokenizer_id",
        "usd_microunits_per_million_tokens",
        "arguments_schema",
    ] {
        assert!(!bound.canonical_json().contains(absent));
    }
}

#[test]
fn substituting_an_eligible_model_changes_only_the_deployment_identity() {
    let (definition_v2, deployment) = migrated();
    let baseline = bind_agent_deployment(&definition_v2, &deployment).unwrap();

    let substituted = deployment
        .replace("fake.local", "other.local")
        .replace("fake-basic", "other-basic");
    let other = bind_agent_deployment(&definition_v2, &substituted).unwrap();

    assert_eq!(
        other.semantic_definition().digest(),
        baseline.semantic_definition().digest()
    );
    assert_ne!(other.deployment().digest(), baseline.deployment().digest());
    assert_ne!(other.digest(), baseline.digest());
    assert_ne!(
        other.runtime_v1_definition(),
        baseline.runtime_v1_definition()
    );
    assert_ne!(other.runtime_v1_profile(), baseline.runtime_v1_profile());
    assert!(other.runtime_v1_profile().contains("other.local"));
}

#[test]
fn changing_source_semantics_stales_every_existing_deployment() {
    let (definition_v2, deployment) = migrated();
    let baseline = bind_agent_deployment(&definition_v2, &deployment).unwrap();

    for changed in [
        definition_v2.replacen(
            "fixture.agent.type.observation",
            "fixture.agent.type.other_observation",
            1,
        ),
        definition_v2.replacen(
            "fixture.agent.fn.reduce",
            "fixture.agent.fn.other_reduce",
            1,
        ),
        definition_v2.replacen("\"max_turns\":2", "\"max_turns\":1", 1),
        definition_v2.replacen(
            "Return one bounded fixture value.",
            "Return one other bounded fixture value.",
            1,
        ),
        definition_v2.replacen("\"name\":\"query\"", "\"name\":\"question\"", 1),
    ] {
        let semantic = compile_agent_definition_v2(&changed).unwrap();
        assert_ne!(semantic.digest(), baseline.semantic_definition().digest());
        let error = bind_agent_deployment(&changed, &deployment).err().unwrap();
        assert_eq!(error[0].code, "SPX-G556");
        assert_eq!(
            error[0].message,
            "AgentDeployment is not compatible with its definition: definition_digest"
        );
    }
}

#[test]
fn a_deployment_can_narrow_but_never_widen_the_source_contract() {
    let (definition_v2, deployment) = migrated();

    // Narrowing is admitted: fewer turns, no granted tool, no granted
    // capability.
    let narrowed = deployment
        .replacen("\"max_turns\":2", "\"max_turns\":1", 1)
        .replacen(
            "\"granted_capabilities\":[\"tool.read\"],\"allowed_tool_ids\":[\"fixture.read\"]",
            "\"granted_capabilities\":[],\"allowed_tool_ids\":[]",
            1,
        );
    let narrowed = bind_agent_deployment(&definition_v2, &narrowed).unwrap();
    assert!(narrowed.runtime_v1_profile().contains("\"max_turns\":1"));
    assert!(narrowed
        .runtime_v1_profile()
        .contains("\"granted_capabilities\":[],\"allowed_tool_ids\":[]"));

    for (mutated, expected) in [
        // Capability expansion.
        (
            deployment.replacen(
                "\"granted_capabilities\":[\"tool.read\"]",
                "\"granted_capabilities\":[\"tool.read\",\"tool.write\"]",
                1,
            ),
            "granted_capabilities",
        ),
        // Tool expansion.
        (
            deployment.replacen(
                "\"allowed_tool_ids\":[\"fixture.read\"]",
                "\"allowed_tool_ids\":[\"fixture.read\",\"fixture.write\"]",
                1,
            ),
            "allowed_tool_ids",
        ),
        // Budget widening past the source ceiling.
        (
            deployment.replacen("\"max_turns\":2", "\"max_turns\":3", 1),
            "limits",
        ),
        (
            deployment.replacen("\"max_usd_microunits\":0", "\"max_usd_microunits\":1", 1),
            "limits",
        ),
        // An allowed tool whose capability the deployment does not grant.
        (
            deployment.replacen(
                "\"granted_capabilities\":[\"tool.read\"]",
                "\"granted_capabilities\":[]",
                1,
            ),
            "tool_capabilities",
        ),
        // Model incompatibility.
        (
            deployment.replacen(
                "\"capabilities\":[\"text\"]",
                "\"capabilities\":[\"vision\"]",
                1,
            ),
            "required_model_capabilities",
        ),
        (
            deployment.replacen("\"locality\":\"local\"", "\"locality\":\"remote\"", 1),
            "required_locality",
        ),
        // A model the deployment did not select.
        (
            deployment.replacen(
                "\"model_id\":\"fake-basic\"",
                "\"model_id\":\"fake-other\"",
                1,
            ),
            "selection",
        ),
        // A stale semantic revision.
        (
            with_definition_digest(&deployment, &format!("sha256:{}", "0".repeat(64))),
            "definition_digest",
        ),
    ] {
        let error = bind_agent_deployment(&definition_v2, &mutated)
            .err()
            .unwrap_or_else(|| panic!("admitted `{expected}`:\n{mutated}"));
        // Binding decides every incompatibility from the two documents alone,
        // before the Runtime v1 projection is even assembled.
        assert_eq!(error[0].code, "SPX-G556", "for `{expected}`");
        assert_eq!(
            error[0].message,
            format!("AgentDeployment is not compatible with its definition: {expected}")
        );
    }

    // A required target feature the deployment does not make available.
    let requires_feature = definition_v2.replacen(
        "\"required_target_features\":[]",
        "\"required_target_features\":[\"target.wasm\"]",
        1,
    );
    let error = bind_agent_deployment(&requires_feature, &paired(&requires_feature, &deployment))
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G556");
    assert_eq!(
        error[0].message,
        "AgentDeployment is not compatible with its definition: target_features"
    );
    let offers_feature = paired(&requires_feature, &deployment).replacen(
        "\"target_features\":[]",
        "\"target_features\":[\"target.wasm\"]",
        1,
    );
    bind_agent_deployment(&requires_feature, &offers_feature).unwrap();

    // A model below the source's minimum quality tier.
    let higher_quality = definition_v2.replacen(
        "\"minimum_quality_tier\":\"basic\"",
        "\"minimum_quality_tier\":\"standard\"",
        1,
    );
    let error = bind_agent_deployment(&higher_quality, &paired(&higher_quality, &deployment))
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G556");
    assert_eq!(
        error[0].message,
        "AgentDeployment is not compatible with its definition: minimum_quality_tier"
    );
}

#[test]
fn one_semantic_definition_runs_under_two_compatible_deployments() {
    let (definition_v2, deployment) = migrated();
    let full = bind_agent_deployment(&definition_v2, &deployment).unwrap();
    let narrowed_source = deployment.replacen("\"max_turns\":2", "\"max_turns\":1", 1);
    let narrowed = bind_agent_deployment(&definition_v2, &narrowed_source).unwrap();

    assert_eq!(
        full.semantic_definition().digest(),
        narrowed.semantic_definition().digest()
    );
    assert_ne!(full.deployment().digest(), narrowed.deployment().digest());

    let task = task();
    let script = transcript(FINAL_ACTION);
    let first = agent_transcript::run(full.runtime_v1_definition(), &task, &script).unwrap();
    let second = agent_transcript::run(narrowed.runtime_v1_definition(), &task, &script).unwrap();
    assert_eq!(first.run.status(), AgentRunStatus::Completed);
    assert_eq!(second.run.status(), AgentRunStatus::Completed);
    assert_eq!(first.agent_id, second.agent_id);
    // The same semantic definition under two deployments produces two distinct
    // evidence capsules, because the effective profile differs.
    assert_ne!(first.run.evidence(), second.run.evidence());
    agent_transcript::replay(
        narrowed.runtime_v1_definition(),
        &task,
        &script,
        second.run.evidence(),
    )
    .unwrap();
}

#[test]
fn neither_document_can_carry_a_credential_or_an_environment_reference() {
    let (definition_v2, deployment) = migrated();

    for (mutated, code) in [
        (
            definition_v2.replacen(
                "\"ceilings\":",
                "\"credentials\":{\"api_key\":\"secret\"},\"ceilings\":",
                1,
            ),
            "SPX-G552",
        ),
        (
            definition_v2.replacen("\"tools\":", "\"environment\":[\"HOME\"],\"tools\":", 1),
            "SPX-G552",
        ),
    ] {
        let error = compile_agent_definition_v2(&mutated).err().unwrap();
        assert_eq!(error[0].code, code);
    }

    for mutated in [
        deployment.replacen("\"limits\":", "\"api_key\":\"secret\",\"limits\":", 1),
        deployment.replacen(
            "\"target_features\":[]",
            "\"target_features\":[],\"credentials_env\":\"OPENAI_API_KEY\"",
            1,
        ),
        deployment.replacen("\"models\":", "\"token\":\"secret\",\"models\":", 1),
    ] {
        let error = compile_agent_deployment(&mutated).err().unwrap();
        assert_eq!(error[0].code, "SPX-G554", "admitted:\n{mutated}");
    }

    // The binding path reads nothing but its two documents.
    for source in [
        include_str!("../../src/agent_deployment.rs"),
        include_str!("../../src/agent_deployment/documents.rs"),
        include_str!("../../src/agent_deployment/migrate.rs"),
    ] {
        for forbidden in [
            "std::env",
            "std::net::",
            "std::fs",
            "fs::read",
            "fs::write",
            "File::create",
            "Command::new",
            "reqwest",
            "AgentRuntimeAuthority",
        ] {
            assert!(
                !source.contains(forbidden),
                "the deployment surface references `{forbidden}`"
            );
        }
    }
}

#[test]
fn noncanonical_documents_and_malformed_identities_fail_closed() {
    let (definition_v2, deployment) = migrated();

    for malformed in [
        definition_v2.trim_end().to_owned(),
        definition_v2.replacen(
            "semaprax.agent-definition.v2",
            "semaprax.agent-definition.v3",
            1,
        ),
        definition_v2.replacen(
            "{\"schema\":\"semaprax.agent-definition.v2\",\"agent_id\":\"fixture.agent\",",
            "{\"agent_id\":\"fixture.agent\",\"schema\":\"semaprax.agent-definition.v2\",",
            1,
        ),
        format!("{}\n{definition_v2}", definition_v2.trim_end()),
    ] {
        let error = compile_agent_definition_v2(&malformed).err().unwrap();
        assert_eq!(error[0].code, "SPX-G552", "admitted:\n{malformed}");
    }

    for malformed in [
        deployment.trim_end().to_owned(),
        deployment.replacen(
            "semaprax.agent-deployment.v1",
            "semaprax.agent-deployment.v2",
            1,
        ),
        // An unsorted grant list is not canonical.
        deployment.replacen(
            "\"allowed_provider_ids\":[\"fake.local\"]",
            "\"allowed_provider_ids\":[\"fake.local\",\"a.local\"]",
            1,
        ),
    ] {
        let error = compile_agent_deployment(&malformed).err().unwrap();
        assert!(
            error[0].code == "SPX-G554" || error[0].code == "SPX-G555",
            "admitted:\n{malformed}"
        );
    }

    // A definition v1 the frozen compiler rejects is never split.
    let error = migrate_agent_definition_v1(
        &definition(&profile()).replacen("\"kind\":\"model\"", "\"kind\":\"effect\"", 1),
        DEPLOYMENT_ID,
    )
    .err()
    .unwrap();
    assert_eq!(error[0].code, "SPX-G502");

    let error = migrate_agent_definition_v1(&definition(&profile()), "not a canonical id")
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G555");
    assert_eq!(
        error[0].message,
        "AgentDeployment invariant failed: deployment_id"
    );
}
