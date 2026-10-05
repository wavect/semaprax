//! REF-22: the admitted deployment model rows have one typed owner.
//!
//! Byte, digest and frozen-projection equality against the shipped fixtures is
//! owned by the `agent_runtime_v1` harness; these gates pin the typed owner on
//! a multi-model deployment that uses every supported model field.

use super::documents::model_decode_counter;
use super::*;

const MODELS: &str = concat!(
    "[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",\"locality\":\"local\",",
    "\"quality_tier\":\"standard\",\"tokenizer_id\":\"fake.bytes-v1\",\"max_context_tokens\":4096,",
    "\"input_usd_microunits_per_million_tokens\":0,\"output_usd_microunits_per_million_tokens\":0,",
    "\"capabilities\":[\"text\"]},",
    "{\"provider_id\":\"fake.second\",\"model_id\":\"fake-pro\",\"locality\":\"remote\",",
    "\"quality_tier\":\"frontier\",\"tokenizer_id\":\"fake.pieces-v2\",\"max_context_tokens\":2048,",
    "\"input_usd_microunits_per_million_tokens\":3,\"output_usd_microunits_per_million_tokens\":15,",
    "\"capabilities\":[\"text\",\"vision\"]}]"
);

fn v1_definition() -> String {
    let runtime = concat!(
        "{\"models\":MODELS,",
        "\"tools\":[{\"tool_id\":\"fixture.read\",\"description\":\"Return one bounded fixture value.\",",
        "\"arguments_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"query\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
        "\"result_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"value\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
        "\"effects\":[\"read\"],\"required_capabilities\":[\"tool.read\"]}],",
        "\"policy\":{\"allowed_provider_ids\":[\"fake.local\",\"fake.second\"],\"allowed_model_ids\":[\"fake-basic\",\"fake-pro\"],",
        "\"required_locality\":\"remote_allowed\",\"minimum_quality_tier\":\"basic\",",
        "\"required_model_capabilities\":[\"text\"],\"granted_capabilities\":[\"tool.read\"],",
        "\"allowed_tool_ids\":[\"fixture.read\"]},",
        "\"limits\":{\"max_turns\":2,\"max_provider_attempts\":2,\"max_retries_per_turn\":1,",
        "\"max_concurrency\":1,\"max_elapsed_ms\":1000,\"max_provider_request_bytes\":65536,",
        "\"max_provider_response_bytes\":4096,\"max_stream_chunks\":64,",
        "\"max_total_provider_input_bytes\":131072,\"max_total_provider_output_bytes\":8192,",
        "\"max_reported_model_input_tokens\":131072,\"max_reported_model_output_tokens\":8192,",
        "\"max_usd_microunits\":0,\"max_tool_calls\":1,\"max_tool_arguments_bytes\":4096,",
        "\"max_tool_result_bytes\":4096,\"max_total_tool_bytes\":8192,",
        "\"max_retained_state_bytes\":131072,\"max_trace_events\":64,\"max_trace_bytes\":131072,",
        "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576}}"
    )
    .replace("MODELS", MODELS);
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
    .replace("RUNTIME", &runtime)
}

fn migrated() -> (String, String) {
    let v1 = compile_agent_definition(&v1_definition())
        .expect("the multi-model v1 fixture compiles")
        .definition()
        .canonical_source()
        .to_owned();
    migrate_agent_definition_v1(&v1, "fixture.deployment.multi").expect("the fixture migrates")
}

fn bind_error(definition: &str, deployment: &str) -> Diagnostic {
    bind_agent_deployment(definition, deployment)
        .err()
        .unwrap_or_else(|| panic!("admitted:\n{deployment}"))
        .remove(0)
}

#[test]
fn every_model_field_round_trips_in_order_through_the_typed_owner() {
    let (definition, deployment) = migrated();
    assert!(deployment.contains(&format!("\"models\":{MODELS},")));
    let admitted = compile_agent_deployment(&deployment).unwrap();
    assert_eq!(admitted.canonical_json(), deployment);
    let rows = &admitted.parsed.models;
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[1],
        DeploymentModel {
            provider_id: documents::ModelField::Typed("fake.second".into()),
            model_id: documents::ModelField::Typed("fake-pro".into()),
            locality: documents::ModelField::Typed("remote".into()),
            quality_tier: documents::ModelField::Typed("frontier".into()),
            tokenizer_id: documents::ModelField::Typed("fake.pieces-v2".into()),
            max_context_tokens: documents::ModelField::Typed(2048),
            input_usd_microunits_per_million_tokens: documents::ModelField::Typed(3),
            output_usd_microunits_per_million_tokens: documents::ModelField::Typed(15),
            capabilities: documents::ModelField::Typed(vec![
                documents::ModelField::Typed("text".into()),
                documents::ModelField::Typed("vision".into()),
            ]),
        }
    );
    assert_eq!(rows[1].quality_rank(), Some(3));

    let bound = bind_agent_deployment(&definition, &deployment).unwrap();
    // The Runtime v1 projection reproduces the original v1 bytes exactly.
    assert_eq!(
        bound.runtime_v1_definition(),
        compile_agent_definition(&v1_definition())
            .unwrap()
            .definition()
            .canonical_source()
    );
    verify_bound_agent_deployment_bundle(&definition, &deployment, bound.canonical_json()).unwrap();
    let selections = bound.model_selections();
    assert_eq!(
        selections
            .iter()
            .map(|row| (
                row.provider_id(),
                row.model_id(),
                row.capabilities().to_vec(),
                row.max_context_tokens()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("fake.local", "fake-basic", vec!["text".to_owned()], 4096),
            (
                "fake.second",
                "fake-pro",
                vec!["text".to_owned(), "vision".to_owned()],
                2048
            ),
        ]
    );
}

#[test]
fn read_only_access_neither_decodes_json_nor_copies_model_strings() {
    let (definition, deployment) = migrated();
    let semantic = compile_agent_definition_v2(&definition).unwrap();
    let before_admission = model_decode_counter::snapshot();
    let admitted = compile_agent_deployment(&deployment).unwrap();
    // Admission decodes each row exactly once.
    assert_eq!(model_decode_counter::snapshot() - before_admission, 2);

    let before = model_decode_counter::snapshot();
    for _ in 0..3 {
        check_compatibility(&semantic, &admitted).unwrap();
    }
    assert_eq!(model_decode_counter::snapshot(), before);

    let bound = bind_agent_deployment(&definition, &deployment).unwrap();
    let before = model_decode_counter::snapshot();
    let stored = bound.deployment.parsed.models[0]
        .provider_id
        .typed()
        .unwrap()
        .as_ptr();
    for _ in 0..3 {
        let first = bound.model_selection_refs().next().unwrap();
        assert_eq!(first.provider_id().as_ptr(), stored);
        assert_eq!(bound.model_selection_refs().count(), 2);
    }
    assert_eq!(model_decode_counter::snapshot(), before);

    // The legacy owned accessor returns independent, equivalent data.
    let owned = bound.model_selections();
    let again = bound.model_selections();
    assert_eq!(owned, again);
    assert_ne!(owned[0].provider_id().as_ptr(), stored);
    assert_ne!(
        owned[0].provider_id().as_ptr(),
        again[0].provider_id().as_ptr()
    );
    for (owned, borrowed) in owned.iter().zip(bound.model_selection_refs()) {
        assert_eq!(owned.provider_id(), borrowed.provider_id());
        assert_eq!(owned.model_id(), borrowed.model_id());
        assert_eq!(
            owned.capabilities(),
            borrowed
                .capabilities()
                .map(str::to_owned)
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(owned.max_context_tokens(), borrowed.max_context_tokens());
    }
    assert_eq!(model_decode_counter::snapshot(), before);
}

#[test]
fn malformed_model_rows_keep_their_boundary_and_precedence() {
    let (definition, deployment) = migrated();
    let local_only = definition.replacen(
        "\"required_locality\":\"remote_allowed\"",
        "\"required_locality\":\"local_only\"",
        1,
    );
    let digest = compile_agent_definition_v2(&local_only)
        .unwrap()
        .digest()
        .to_owned();
    let old_digest = compile_agent_definition_v2(&definition)
        .unwrap()
        .digest()
        .to_owned();
    let local_deployment = deployment.replacen(&old_digest, &digest, 1);

    // Closed keys, key order and noncanonical bytes stay admission failures.
    for mutated in [
        deployment.replacen(",\"capabilities\":[\"text\"]}", "}", 1),
        deployment.replacen(
            "\"capabilities\":[\"text\"]}",
            "\"capabilities\":[\"text\"],\"endpoint\":\"x\"}",
            1,
        ),
        deployment.replacen(
            "\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\"",
            "\"model_id\":\"fake-basic\",\"provider_id\":\"fake.local\"",
            1,
        ),
        deployment.replacen(
            "\"max_context_tokens\":4096",
            "\"max_context_tokens\": 4096",
            1,
        ),
        deployment.replacen("\"models\":[{", "\"models\":[7,{", 1),
    ] {
        assert_ne!(mutated, deployment);
        let error = compile_agent_deployment(&mutated).err().unwrap();
        assert_eq!(error[0].code, "SPX-G554", "admitted:\n{mutated}");
    }
    let empty_start = deployment.find("\"models\":[").unwrap() + "\"models\":".len();
    let empty_end = deployment.find(",\"selection\":").unwrap();
    let empty = format!(
        "{}[]{}",
        &deployment[..empty_start],
        &deployment[empty_end..]
    );
    let error = compile_agent_deployment(&empty).err().unwrap();
    assert_eq!(error[0].code, "SPX-G555");
    assert_eq!(error[0].message, "AgentDeployment invariant failed: models");

    // Wrongly typed values are admitted by the closed-key boundary and still
    // refused by binding at the exact same check.
    for (source, mutated, expected) in [
        (
            &definition,
            deployment.replacen("\"provider_id\":\"fake.local\"", "\"provider_id\":1", 1),
            "models",
        ),
        (
            &definition,
            deployment.replacen("\"model_id\":\"fake-pro\"", "\"model_id\":[]", 1),
            "models",
        ),
        (
            &definition,
            deployment.replacen("\"quality_tier\":\"standard\"", "\"quality_tier\":2", 1),
            "models",
        ),
        (
            &definition,
            deployment.replacen(
                "\"quality_tier\":\"standard\"",
                "\"quality_tier\":\"ultra\"",
                1,
            ),
            "models",
        ),
        (
            &definition,
            deployment.replacen(
                "\"capabilities\":[\"text\"]",
                "\"capabilities\":\"text\"",
                1,
            ),
            "models",
        ),
        (
            &definition,
            deployment.replacen("\"capabilities\":[\"text\"]", "\"capabilities\":[1]", 1),
            "required_model_capabilities",
        ),
        (
            &local_only,
            local_deployment.replacen("\"locality\":\"local\"", "\"locality\":null", 1),
            "models",
        ),
        (&local_only, local_deployment.clone(), "required_locality"),
        // Row order decides precedence: the first row's selection mismatch
        // wins over the second row's malformed tier.
        (
            &definition,
            deployment
                .replacen(
                    "\"model_id\":\"fake-basic\",\"locality\"",
                    "\"model_id\":\"fake-other\",\"locality\"",
                    1,
                )
                .replacen("\"quality_tier\":\"frontier\"", "\"quality_tier\":0", 1),
            "selection",
        ),
    ] {
        assert_ne!(&mutated, &deployment);
        compile_agent_deployment(&mutated)
            .unwrap_or_else(|error| panic!("refused at admission {error:?}:\n{mutated}"));
        let error = bind_error(source, &mutated);
        assert_eq!(error.code, "SPX-G556", "for `{expected}`:\n{mutated}");
        assert_eq!(
            error.message,
            format!("AgentDeployment is not compatible with its definition: {expected}")
        );
    }

    // Fields compatibility does not inspect still reach Runtime v1 admission,
    // which refuses them there, exactly as before typing.
    for mutated in [
        deployment.replacen(
            "\"max_context_tokens\":4096",
            "\"max_context_tokens\":\"4096\"",
            1,
        ),
        deployment.replacen("\"max_context_tokens\":4096", "\"max_context_tokens\":0", 1),
        deployment.replacen(
            "\"tokenizer_id\":\"fake.bytes-v1\"",
            "\"tokenizer_id\":false",
            1,
        ),
        deployment.replacen(
            "\"output_usd_microunits_per_million_tokens\":15",
            "\"output_usd_microunits_per_million_tokens\":-1",
            1,
        ),
        deployment.replacen("\"locality\":\"remote\"", "\"locality\":\"orbit\"", 1),
    ] {
        assert_ne!(mutated, deployment);
        compile_agent_deployment(&mutated)
            .unwrap_or_else(|error| panic!("refused at admission {error:?}:\n{mutated}"));
        let error = bind_error(&definition, &mutated);
        assert!(
            !["SPX-G554", "SPX-G555", "SPX-G556"].contains(&error.code),
            "{error:?}:\n{mutated}"
        );
    }
}
