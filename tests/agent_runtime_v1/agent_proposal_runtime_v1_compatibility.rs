//! Generated Proposal compatibility with the frozen Agent Runtime v1 final wire.

use semaprax::agent_definition::compile_agent_definition;
use semaprax::agent_proposal::{
    compile_agent_proposal_runtime_v1_compatibility, compile_agent_proposal_schema,
    AgentRuntimeV1ActionKind,
};
use semaprax::agent_runtime::AgentRunStatus;
use semaprax::agent_transcript;

use super::agent_definition_v1::definition;
use super::{profile, task};

const MODULE_PATH: &str = "fixture-agent-runtime-proposal.spx";
const RECORD_MODULE: &str = r#"module fixture.agent.runtime_proposal;

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.message")
    message: string,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const VARIANT_MODULE: &str = r#"module fixture.agent.runtime_proposal;

@id("fixture.agent.type.proposal")
variant Proposal {
    @id("fixture.agent.type.proposal.tool")
    Final {
        @id("fixture.agent.type.proposal.tool.arguments")
        code: i64,
    },
    @id("fixture.agent.type.proposal.reject")
    Reject {
        @id("fixture.agent.type.proposal.reject.retry")
        retry: bool,
    },
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn record_proposal(schema_digest: &str, message: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":{},\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.message\":{}}}}}}}\n",
        serde_json::to_string(schema_digest).unwrap(),
        serde_json::to_string(message).unwrap(),
    )
}

fn variant_proposal(schema_digest: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":{},\"value\":{{\"case\":\"fixture.agent.type.proposal.tool\",\"fields\":{{\"fixture.agent.type.proposal.tool.arguments\":\"7\"}}}}}}\n",
        serde_json::to_string(schema_digest).unwrap(),
    )
}

fn expected_action(proposal: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":{}}}\n",
        serde_json::to_string(proposal).unwrap()
    )
}

#[test]
fn record_and_copy_variant_proposals_wrap_as_exact_frozen_final_actions() {
    let profile = profile();
    let definition_source = definition(&profile);
    let compiled_definition = compile_agent_definition(&definition_source).unwrap();
    for module in [RECORD_MODULE, VARIANT_MODULE] {
        let schema =
            compile_agent_proposal_schema(module, MODULE_PATH, &definition_source).unwrap();
        let adapter =
            compile_agent_proposal_runtime_v1_compatibility(&schema, &compiled_definition).unwrap();
        let proposal = if module == RECORD_MODULE {
            record_proposal(schema.schema().digest(), "done")
        } else {
            variant_proposal(schema.schema().digest())
        };
        let action = adapter.decode_and_render(&proposal).unwrap();
        assert_eq!(action.kind(), AgentRuntimeV1ActionKind::Final);
        assert_eq!(action.canonical_json(), expected_action(&proposal));
        assert_eq!(
            adapter.definition_digest(),
            compiled_definition.definition().digest()
        );
        assert_eq!(adapter.proposal_schema_digest(), schema.schema().digest());
        assert!(adapter.runtime_profile_digest().starts_with("sha256:"));

        if module == RECORD_MODULE {
            let cross_agent = proposal.replacen("fixture.agent", "other.agent", 1);
            assert_eq!(
                adapter.decode_and_render(&cross_agent).unwrap_err()[0].code,
                "SPX-G551"
            );
        } else {
            for invalid in [
                proposal.replacen("proposal.tool\"", "proposal.unknown\"", 1),
                proposal.replacen("proposal.tool.arguments\"", "proposal.tool.extra\"", 1),
                proposal.replacen("\"7\"", "\"9223372036854775808\"", 1),
            ] {
                assert_eq!(
                    adapter.decode_and_render(&invalid).unwrap_err()[0].code,
                    "SPX-G551"
                );
            }
        }
    }
}

#[test]
fn generated_wrapper_preserves_runtime_trace_and_evidence_bytes() {
    let profile = profile();
    let definition_source = definition(&profile);
    let compiled_definition = compile_agent_definition(&definition_source).unwrap();
    let schema =
        compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &definition_source).unwrap();
    let adapter =
        compile_agent_proposal_runtime_v1_compatibility(&schema, &compiled_definition).unwrap();
    let proposal = record_proposal(schema.schema().digest(), "done");
    let generated_action = adapter.decode_and_render(&proposal).unwrap();
    let legacy_action = expected_action(&proposal);
    assert_eq!(generated_action.canonical_json(), legacy_action);

    let transcript = |action: &str| {
        let value = serde_json::json!({
            "schema": "semaprax.agent-runtime-transcript.v1",
            "policy_epoch": 7,
            "provider": [{"disposition": "succeeded", "response": action}],
            "tools": [],
        });
        format!("{}\n", serde_json::to_string(&value).unwrap())
    };
    let legacy =
        agent_transcript::run(&definition_source, &task(), &transcript(&legacy_action)).unwrap();
    let generated = agent_transcript::run(
        &definition_source,
        &task(),
        &transcript(generated_action.canonical_json()),
    )
    .unwrap();
    assert_eq!(generated.run.status(), AgentRunStatus::Completed);
    assert_eq!(generated.run.final_message(), Some(proposal.as_str()));
    assert_eq!(generated.run.trace(), legacy.run.trace());
    assert_eq!(generated.run.trace_digest(), legacy.run.trace_digest());
    assert_eq!(generated.run.evidence(), legacy.run.evidence());
    assert_eq!(
        generated.run.evidence_digest(),
        legacy.run.evidence_digest()
    );
}

#[test]
fn cross_definition_stale_proposal_and_escaped_response_bound_fail_closed() {
    let profile = profile();
    let definition_source = definition(&profile);
    let compiled_definition = compile_agent_definition(&definition_source).unwrap();
    let schema =
        compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &definition_source).unwrap();

    let other_definition_source = definition_source.replacen("fixture.agent\"", "other.agent\"", 1);
    let other_definition = compile_agent_definition(&other_definition_source).unwrap();
    let error = compile_agent_proposal_runtime_v1_compatibility(&schema, &other_definition)
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G578");

    let adapter =
        compile_agent_proposal_runtime_v1_compatibility(&schema, &compiled_definition).unwrap();
    let stale = record_proposal(&format!("{}0", schema.schema().digest()), "done");
    let error = adapter.decode_and_render(&stale).err().unwrap();
    assert_eq!(error[0].code, "SPX-G551");
    let error = adapter.decode_and_render("{}\n").err().unwrap();
    assert_eq!(error[0].code, "SPX-G550");

    let escaping_message = "\"\\\n".repeat(40);
    let escaping = record_proposal(schema.schema().digest(), &escaping_message);
    let exact_action_bytes = expected_action(&escaping).len();
    let exact_profile = profile.replace(
        "\"max_provider_response_bytes\":4096",
        &format!("\"max_provider_response_bytes\":{exact_action_bytes}"),
    );
    let exact_definition_source = definition(&exact_profile);
    let exact_definition = compile_agent_definition(&exact_definition_source).unwrap();
    let exact_schema =
        compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &exact_definition_source)
            .unwrap();
    let exact_adapter =
        compile_agent_proposal_runtime_v1_compatibility(&exact_schema, &exact_definition).unwrap();
    let exact_proposal = record_proposal(exact_schema.schema().digest(), &escaping_message);
    let exact_action = exact_adapter.decode_and_render(&exact_proposal).unwrap();
    assert_eq!(exact_action.canonical_json().len(), exact_action_bytes);

    let short_profile = profile.replace(
        "\"max_provider_response_bytes\":4096",
        &format!("\"max_provider_response_bytes\":{}", exact_action_bytes - 1),
    );
    let short_definition_source = definition(&short_profile);
    let short_definition = compile_agent_definition(&short_definition_source).unwrap();
    let short_schema =
        compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &short_definition_source)
            .unwrap();
    let short_adapter =
        compile_agent_proposal_runtime_v1_compatibility(&short_schema, &short_definition).unwrap();
    let short_proposal = record_proposal(short_schema.schema().digest(), &escaping_message);
    let error = short_adapter
        .decode_and_render(&short_proposal)
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G578");
}

#[test]
fn compatibility_surface_has_no_host_tool_or_authority_seam() {
    let source = include_str!("../../src/agent_proposal/runtime_v1.rs");
    for forbidden in [
        "AgentHost",
        "Agent::new",
        "Authorized<",
        "attempt_provider",
        "invoke_tool",
        "std::net::",
        "Command::new",
        "fs::write",
        "fs::read",
        "File::create",
        "std::env",
    ] {
        assert!(
            !source.contains(forbidden),
            "compatibility surface references `{forbidden}`"
        );
    }
}
