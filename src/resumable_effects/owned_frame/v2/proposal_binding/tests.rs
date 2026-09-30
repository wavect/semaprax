use super::*;
use std::path::Path;
fn source() -> String {
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx");
    format!(
        "{}\n{}",
        source.replace(
            "    runtime_v1 {",
            "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {"
        ),
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    )
}
fn bind(source: &str) -> CheckedOwnedAgentWaitBindingV8 {
    super::super::compile_owned_agent_wait_v8(
        source,
        Path::new("proposal-binding.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap()
}
fn document(b: &CheckedOwnedAgentWaitBindingV8, sequence: &str) -> String {
    format!(
        r#"{{"schema":"semaprax.agent-proposal.v1","agent_id":"fixture.agent","proposal_schema_digest":"{}","value":{{"fields":{{"fixture.agent.type.proposal.budget":"3","fixture.agent.type.proposal.urgent":false,"fixture.agent.type.proposal.sequence":"{}"}}}}}}
"#,
        b.lifecycle().proposal_schema().schema().digest(),
        sequence
    )
}
#[test]
fn owned_wait_proposal_projects_already_decoded_sdk_and_keeps_distinct_exact_digests() {
    let b = bind(&source());
    let scope =
        SourceCheckpointScope::new(b.lifecycle().source_revision(), "proposal-test", 7).unwrap();
    let decoded = b
        .lifecycle()
        .proposal_schema()
        .decode(&document(&b, "1"))
        .unwrap();
    let facts = bind_owned_wait_proposal_v8(&b, &scope, &decoded).unwrap();
    assert!(facts.matches(b.binding(), &codec::scope(&scope).unwrap()));
    assert_eq!(
        facts.ordinary_digest(),
        crate::live_invocation::identity::digest(
            b"semaprax.source-proposal.v2\0",
            decoded.canonical_json().as_bytes()
        )
    );
    assert_eq!(facts.canonical_proposal(), decoded.canonical_json());
    assert_eq!(
        facts.value(),
        &json!({"tag":"record","declaration":"fixture.agent.type.proposal","fields":[{"tag":"i64","value":3},{"tag":"bool","value":false},{"tag":"usize","value":1}]})
    );
    assert_eq!(checkpoint::channel_json(facts.carrier()), *facts.value());
    assert_eq!(
        facts.answer_digest(),
        codec::fact_digest(
            b"semaprax.source-owned-frame-answer.v2\0",
            &json!({"scope":codec::scope(&scope).unwrap(),"plan_digest":b.binding(),"value":facts.value()})
        )
    );
    assert_ne!(facts.answer_digest(), facts.ordinary_digest());
    let argument_digest = format!("sha256:{}", "a".repeat(64));
    assert_eq!(
        facts.result_digest(&argument_digest).unwrap(),
        codec::fact_digest(
            b"semaprax.source-owned-frame-result.v2\0",
            &json!({"argument_digest":argument_digest,"answer_digest":facts.answer_digest()})
        )
    );
    assert!(facts.result_digest("reminted").is_err());
    let other = SourceCheckpointScope::new(scope.program_root(), "other-proposal", 7).unwrap();
    let changed = bind_owned_wait_proposal_v8(&b, &other, &decoded).unwrap();
    assert_eq!(changed.ordinary_digest(), facts.ordinary_digest());
    assert_ne!(changed.answer_digest(), facts.answer_digest());
    assert!(!facts.matches(b.binding(), &codec::scope(&other).unwrap()));
}
#[test]
fn owned_wait_proposal_refuses_cross_schema_and_sdk_u64_outside_portable_copy_limit() {
    let source = source();
    let b = bind(&source);
    let scope =
        SourceCheckpointScope::new(b.lifecycle().source_revision(), "proposal-test", 7).unwrap();
    let decoded = b
        .lifecycle()
        .proposal_schema()
        .decode(&document(&b, "1"))
        .unwrap();
    let changed = source.replace(
        "fixture.agent.type.proposal.budget",
        "fixture.agent.type.proposal.other_budget",
    );
    let other = bind(&changed);
    assert_ne!(
        b.lifecycle().proposal_schema().schema().digest(),
        other.lifecycle().proposal_schema().schema().digest()
    );
    assert!(bind_owned_wait_proposal_v8(&other, &scope, &decoded).is_err());
    let decoded = b
        .lifecycle()
        .proposal_schema()
        .decode(&document(&b, "4294967296"))
        .unwrap();
    assert_eq!(
        bind_owned_wait_proposal_v8(&b, &scope, &decoded)
            .err()
            .unwrap()
            .code,
        "SPX-G583"
    );
    assert!(b
        .lifecycle()
        .proposal_schema()
        .decode(&document(&b, "1").replace("\"3\"", "3"))
        .is_err());
}
