//! Source Agent Observation Schema v1 and bounded decoder.

use semaprax::agent_observation::{
    compile_source_agent_observation_schema, verify_source_agent_observation_schema_bundle,
    ObservationValue,
};
use semaprax::diagnostic::quote_json;
use semaprax::project::compile_source_agent_proposal_schema;

use super::profile;

const MODULE_PATH: &str = "fixture-agent-observation.spx";

fn runtime_v1() -> String {
    let profile = profile();
    let members = profile
        .strip_suffix('\n')
        .unwrap()
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",",
        )
        .unwrap();
    let (runtime, _) = members.split_once(",\"nonclaims\":").unwrap();
    format!("{{{runtime}}}")
}

fn module() -> String {
    format!(
        r#"module fixture.agent.observation;

@id("fixture.agent.type.observation")
record Observation {{
    @id("fixture.agent.type.observation.sequence")
    sequence: usize,
    @id("fixture.agent.type.observation.ready")
    ready: bool,
    @id("fixture.agent.type.observation.note")
    note: string,
}}

@id("fixture.agent.type.proposal")
record Proposal {{
    @id("fixture.agent.type.proposal.action")
    action: string,
}}

@id("fixture.agent")
agent FixtureAgent {{
    types {{
        @id("fixture.agent.type.task")
        type task;
        @id("fixture.agent.type.state")
        type state;
        @id("fixture.agent.type.observation")
        type observation;
        @id("fixture.agent.type.proposal")
        type proposal;
        @id("fixture.agent.type.outcome")
        type outcome;
        @id("fixture.agent.type.result")
        type result;
    }}
    operations {{
        @id("fixture.agent.fn.initialize")
        fn initialize;
        @id("fixture.agent.fn.observe")
        fn observe;
        @id("fixture.agent.fn.propose")
        model fn propose;
        @id("fixture.agent.fn.authorize")
        fn authorize;
        @id("fixture.agent.fn.execute")
        effect fn execute;
        @id("fixture.agent.fn.reduce")
        fn reduce;
    }}
    runtime_v1 {{
        canonical_json {};
    }}
}}

@id("app.main")
fn main() -> i64
{{
    0
}}
"#,
        quote_json(&runtime_v1())
    )
}

#[test]
fn source_observation_schema_replays_and_decodes_without_authority() {
    let source = module();
    let first =
        compile_source_agent_observation_schema(&source, MODULE_PATH, "fixture.agent").unwrap();
    let second =
        compile_source_agent_observation_schema(&source, MODULE_PATH, "fixture.agent").unwrap();
    assert_eq!(
        first.schema().canonical_json(),
        second.schema().canonical_json()
    );
    assert_eq!(first.schema().digest(), second.schema().digest());
    assert_eq!(first.schema().agent_id(), "fixture.agent");
    assert_eq!(
        first.schema().observation_type_id(),
        "fixture.agent.type.observation"
    );
    for witness in [
        "\"schema\":\"semaprax.agent-observation-schema.v1\"",
        "\"kind\":\"record\"",
        "\"stable_id\":\"fixture.agent.type.observation.sequence\",\"representation\":\"u64\"",
        "\"exact_integer_encoding\":\"decimal_string\"",
        "no_authorization_value_or_publication_token_from_an_observation",
    ] {
        assert!(
            first.schema().canonical_json().contains(witness),
            "{witness}"
        );
    }
    verify_source_agent_observation_schema_bundle(
        &source,
        MODULE_PATH,
        "fixture.agent",
        first.schema().canonical_json(),
    )
    .unwrap();

    let document = format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-observation.v1\",",
            "\"agent_id\":\"fixture.agent\",",
            "\"observation_schema_digest\":\"{}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.observation.sequence\":\"9007199254740993\",",
            "\"fixture.agent.type.observation.ready\":true,",
            "\"fixture.agent.type.observation.note\":\"ready\"}}}}}}\n"
        ),
        first.schema().digest()
    );
    let decoded = first.decode(&document).unwrap();
    assert_eq!(decoded.canonical_json(), document);
    assert_eq!(
        decoded.field("fixture.agent.type.observation.sequence"),
        Some(&ObservationValue::Unsigned(9_007_199_254_740_993))
    );
    assert_eq!(
        decoded.field("fixture.agent.type.observation.ready"),
        Some(&ObservationValue::Bool(true))
    );

    let stale = document.replacen(first.schema().digest(), "sha256:stale", 1);
    assert_eq!(first.decode(&stale).err().unwrap()[0].code, "SPX-G569");
    let tampered = first.schema().canonical_json().replacen(
        "\"representation\":\"u64\"",
        "\"representation\":\"i64\"",
        1,
    );
    assert_eq!(
        verify_source_agent_observation_schema_bundle(
            &source,
            MODULE_PATH,
            "fixture.agent",
            &tampered
        )
        .unwrap_err()[0]
            .code,
        "SPX-G567"
    );

    // The source-aware Proposal bridge delegates to the pre-existing frozen
    // Proposal compiler and resolves the role against this same module.
    let proposal =
        compile_source_agent_proposal_schema(&source, MODULE_PATH, "fixture.agent").unwrap();
    assert_eq!(
        proposal.schema().proposal_type_id(),
        "fixture.agent.type.proposal"
    );
}

#[test]
fn unresolved_or_unadmitted_observation_shapes_fail_closed() {
    let source = module();
    let unresolved = source.replace(
        "@id(\"fixture.agent.type.observation\")\n        type observation;",
        "@id(\"fixture.agent.type.missing\")\n        type observation;",
    );
    assert_eq!(
        compile_source_agent_observation_schema(&unresolved, MODULE_PATH, "fixture.agent")
            .err()
            .unwrap()[0]
            .code,
        "SPX-G566"
    );
    let nested = source.replace("sequence: usize", "sequence: char");
    assert_eq!(
        compile_source_agent_observation_schema(&nested, MODULE_PATH, "fixture.agent")
            .err()
            .unwrap()[0]
            .code,
        "SPX-G566"
    );
}
