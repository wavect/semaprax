use std::path::PathBuf;

use super::super::compile_agent_interaction_schema;
use super::SourceInteractionProposalDecoder;
use crate::live_invocation::fixture::{
    FixtureAuthorizationGate, FixtureBudgetHook, FixtureModelHandler, FixtureObserver,
    FixturePolicy,
};
use crate::live_invocation::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationOutcome,
};
use crate::live_invocation::model_invoke::{
    ModelInvocationOutcome, ModelInvokeCapability, ProposalDecoder, ProposalOutcome,
};
use crate::live_invocation::{LiveInvocationId, LiveInvocationSeed};

/// One record type this bridge decodes responses against: `answer.type`
/// with a single `i64` field. Every executable module needs `fn main`.
const FIXTURE: &str = r#"
module test.agent_interaction_schema.live_bridge;

@id("answer.type")
record Answer {
    @id("answer.value")
    value: i64,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn write_temp(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-agent-interaction-schema-live-bridge-{label}-{}-{}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, FIXTURE).unwrap();
    path
}

/// One canonical `semaprax.agent-interaction-value.v1` document for
/// `answer.type`, bound to `schema_digest`.
fn document(schema_digest: &str, value: i64) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"answer.type\",\"schema_digest\":{},\"value\":{{\"fields\":{{\"answer.value\":\"{value}\"}}}}}}\n",
        crate::diagnostic::quote_json(schema_digest),
    )
}

/// A response that decodes cleanly is `Admitted` with exactly the canonical
/// bytes `CompiledInteractionSchema::decode` itself produces, and the
/// decoder reports its own compiled schema's digest, never a placeholder.
#[test]
fn a_response_encoded_against_the_bound_schema_is_admitted_with_its_canonical_bytes() {
    let path = write_temp("accept");
    let compiled =
        compile_agent_interaction_schema(&path, "answer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();
    let schema_digest = compiled.schema().digest().to_owned();

    let mut decoder = SourceInteractionProposalDecoder::new(compiled);
    assert_eq!(decoder.schema_digest(), schema_digest);

    let doc = document(&schema_digest, 42);
    // Exercised through the trait object, not the inherent method, so this
    // proves genuine `ProposalDecoder` conformance rather than a
    // same-shaped method that happens to compile.
    let as_trait: &mut dyn ProposalDecoder = &mut decoder;
    match as_trait.decode(0, doc.as_bytes()) {
        ProposalOutcome::Admitted(bytes) => assert_eq!(bytes, doc.as_bytes()),
        ProposalOutcome::Refused(reason) => panic!("expected admission, got refusal: {reason}"),
    }
}

/// A response naming a different schema digest is refused for exactly that
/// reason (`schema_digest`, `CompiledInteractionSchema`'s own
/// `decode_invariant("schema_digest")`), not the earlier generic
/// `malformed` diagnostic that fires for a document that isn't valid JSON
/// at all — proving the rejection reason names the specific failed
/// admission rule.
#[test]
fn a_response_naming_a_different_schema_digest_is_refused_specifically_for_that_reason() {
    let path = write_temp("schema-drift");
    let compiled =
        compile_agent_interaction_schema(&path, "answer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();
    let real_digest = compiled.schema().digest().to_owned();
    let mut decoder = SourceInteractionProposalDecoder::new(compiled);

    let wrong_digest = format!("sha256:{}", "0".repeat(64));
    assert_ne!(wrong_digest, real_digest);
    let drifted = document(&wrong_digest, 42);

    match decoder.decode(0, drifted.as_bytes()) {
        ProposalOutcome::Admitted(_) => panic!("a schema-digest mismatch must never be admitted"),
        ProposalOutcome::Refused(reason) => {
            assert!(
                reason.contains("schema_digest"),
                "reason must name the specific failed field, got: {reason}"
            );
            assert!(
                !reason.contains("SPX-Z205"),
                "a schema-digest mismatch must not be reported as the earlier generic malformed-JSON diagnostic, got: {reason}"
            );
        }
    }
}

/// Invalid UTF-8 bytes are refused for a *different* specific reason
/// (`utf8`) than the schema-digest mismatch above, so the two negative
/// paths are distinguishable, not one generic "decode failed" bucket.
#[test]
fn non_utf8_bytes_are_refused_for_a_distinct_reason_from_schema_drift() {
    let path = write_temp("non-utf8");
    let compiled =
        compile_agent_interaction_schema(&path, "answer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();
    let mut decoder = SourceInteractionProposalDecoder::new(compiled);

    match decoder.decode(0, &[0xFF, 0xFE, 0xFD]) {
        ProposalOutcome::Admitted(_) => panic!("non-UTF-8 bytes must never be admitted"),
        ProposalOutcome::Refused(reason) => {
            assert!(
                reason.contains("utf8"),
                "reason must name the utf8 admission rule, got: {reason}"
            );
            assert!(
                !reason.contains("schema_digest"),
                "a non-UTF-8 payload must not be misreported as a schema-digest mismatch, got: {reason}"
            );
        }
    }
}

fn config<'a>(
    identity: &'a LiveInvocationId,
    interaction_schema_digest: &'a str,
    max_response_bytes: usize,
) -> LiveInvocationConfig<'a> {
    LiveInvocationConfig {
        identity,
        task: b"task",
        deployment_binding: "fixture.deployment.v1",
        interaction_schema_digest,
        max_turns: 1,
        max_response_bytes,
        requested_budget_per_turn: 10,
    }
}

/// End-to-end through the real `run_live_invocation` kernel (issue #108's
/// shared kernel, used here only through its already-public seam, never
/// edited): a scripted response that is a real, compiler-derived
/// `answer.type` document decodes through the real grammar and the turn
/// completes with exactly that canonical document as the terminal payload
/// — the first time this kernel has run end to end against anything other
/// than `FixtureProposalDecoder`'s toy grammar.
#[test]
fn the_real_grammar_completes_a_live_invocation_through_the_shared_kernel() {
    let path = write_temp("kernel-accept");
    let compiled =
        compile_agent_interaction_schema(&path, "answer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();
    let schema_digest = compiled.schema().digest().to_owned();
    let mut decoder = SourceInteractionProposalDecoder::new(compiled);

    let doc = document(&schema_digest, 7);
    let seed = LiveInvocationSeed {
        program_root: "program.root.v1".to_owned(),
        deployment_policy: "fixture.deployment.v1".to_owned(),
        task: b"task".to_vec(),
        budget: 10,
        interaction_schema_digest: schema_digest.clone(),
        approved_providers: vec!["fixture.provider".to_owned()],
    };
    let identity = LiveInvocationId::derive(&seed);
    let cfg = config(&identity, &schema_digest, doc.len() + 64);

    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        doc.clone().into_bytes(),
    )]);
    let mut gate = FixtureAuthorizationGate::new(1);
    let mut budget = FixtureBudgetHook::new(1000);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    let capability = ModelInvokeCapability::grant("live_bridge test");
    let cancellation = crate::agent_runtime::AgentCancellation::new();

    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };

    let run = run_live_invocation(&cfg, Vec::new(), &mut handlers, &cancellation)
        .expect("a fresh, in-budget, single-turn invocation must run");

    assert_eq!(run.dispatched, 1);
    assert_eq!(run.outcome, LiveInvocationOutcome::Complete(doc.into_bytes()));
    assert_eq!(gate.granted, 1);
}

/// The same kernel, same real grammar, but the scripted response names a
/// foreign schema digest: the turn fails closed as `proposal_refused`
/// before authorization is ever consumed — the real decoder, not just the
/// fixture one, gates the authorize boundary.
#[test]
fn the_real_grammar_refuses_a_foreign_schema_digest_before_authorization_is_consumed() {
    let path = write_temp("kernel-reject");
    let compiled =
        compile_agent_interaction_schema(&path, "answer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();
    let schema_digest = compiled.schema().digest().to_owned();
    let mut decoder = SourceInteractionProposalDecoder::new(compiled);

    let wrong_digest = format!("sha256:{}", "1".repeat(64));
    let drifted = document(&wrong_digest, 7);
    let seed = LiveInvocationSeed {
        program_root: "program.root.v1".to_owned(),
        deployment_policy: "fixture.deployment.v1".to_owned(),
        task: b"task".to_vec(),
        budget: 10,
        interaction_schema_digest: schema_digest.clone(),
        approved_providers: vec!["fixture.provider".to_owned()],
    };
    let identity = LiveInvocationId::derive(&seed);
    let cfg = config(&identity, &schema_digest, drifted.len() + 64);

    let mut handler =
        FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(drifted.into_bytes())]);
    let mut gate = FixtureAuthorizationGate::new(1);
    let mut budget = FixtureBudgetHook::new(1000);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    let capability = ModelInvokeCapability::grant("live_bridge test");
    let cancellation = crate::agent_runtime::AgentCancellation::new();

    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };

    let run = run_live_invocation(&cfg, Vec::new(), &mut handlers, &cancellation)
        .expect("a refused proposal is a recorded outcome, not a kernel error");

    assert_eq!(run.dispatched, 1);
    assert_eq!(
        run.outcome,
        LiveInvocationOutcome::Fail(b"proposal_refused".to_vec())
    );
    assert_eq!(
        gate.granted, 0,
        "a decoded-and-refused proposal must never reach the authorization gate"
    );
}
