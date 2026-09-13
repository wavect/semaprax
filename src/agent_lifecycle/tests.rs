//! Crate-internal gates for the lifecycle's authority invariants.
//!
//! Two of them cannot be observed from outside the crate and are therefore
//! owned here: the single mint site of the authorization value, and the
//! refusal to spend an authorization into a state or proposal it was not
//! granted against. The public terminal behaviour, the stage binding
//! rejections and the frozen Runtime v1 digests are owned by the
//! `agent_runtime_v1` harness.

use super::*;

/// The `runtime_v1` compatibility material of the frozen fixture definition.
/// The definition compiler supplies its own schema and nonclaims.
pub(in crate::agent_lifecycle) const RUNTIME_V1: &str = concat!(
    "{\"models\":[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",",
    "\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",",
    "\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,",
    "\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}],",
    "\"tools\":[{\"tool_id\":\"fixture.read\",\"description\":\"Return one bounded fixture value.\",",
    "\"arguments_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"query\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
    "\"result_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"value\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
    "\"effects\":[\"read\"],\"required_capabilities\":[\"tool.read\"]}],",
    "\"policy\":{\"allowed_provider_ids\":[\"fake.local\"],\"allowed_model_ids\":[\"fake-basic\"],",
    "\"required_locality\":\"local_only\",\"minimum_quality_tier\":\"basic\",",
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
);

pub(in crate::agent_lifecycle) const DEFINITION: &str = concat!(
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
);

pub(in crate::agent_lifecycle) const MODULE: &str = r#"module fixture.agent.lifecycle;

@id("fixture.agent.type.task")
record Task {
    @id("fixture.agent.type.task.objective") objective: Bytes,
    @id("fixture.agent.type.task.budget") budget: i64,
}

@id("fixture.agent.type.state")
record State {
    @id("fixture.agent.type.state.objective") objective: Bytes,
    @id("fixture.agent.type.state.budget") budget: i64,
    @id("fixture.agent.type.state.epoch") epoch: i64,
}

@id("fixture.agent.type.observation")
record Observation {
    @id("fixture.agent.type.observation.tag") tag: Bytes,
    @id("fixture.agent.type.observation.budget") budget: i64,
    @id("fixture.agent.type.observation.epoch") epoch: i64,
}

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.budget") budget: i64,
    @id("fixture.agent.type.proposal.urgent") urgent: bool,
    @id("fixture.agent.type.proposal.sequence") sequence: usize,
}

@id("fixture.agent.type.decision")
variant Decision {
    @id("fixture.agent.type.decision.granted") Granted {
        @id("fixture.agent.type.decision.granted.seal") seal: Bytes,
        @id("fixture.agent.type.decision.granted.budget") budget: i64,
    },
    @id("fixture.agent.type.decision.refused") Refused {
        @id("fixture.agent.type.decision.refused.code") code: i64,
    },
}

@id("fixture.agent.type.outcome")
record Outcome {
    @id("fixture.agent.type.outcome.value") value: Bytes,
    @id("fixture.agent.type.outcome.status") status: i64,
}

@id("fixture.agent.type.result")
record Report {
    @id("fixture.agent.type.result.summary") summary: Bytes,
    @id("fixture.agent.type.result.budget") budget: i64,
    @id("fixture.agent.type.result.status") status: i64,
}

@id("fixture.agent.fn.initialize")
fn initialize(task: own Task) -> State
{
    State { objective: task.objective, budget: task.budget, epoch: 1 }
}

@id("fixture.agent.fn.observe")
fn observe(state: borrow State) -> Observation
{
    let tag = [79u8, 66u8];
    Observation { tag: bytes_copy(array_as_slice(tag)), budget: state.budget, epoch: state.epoch }
}

@id("fixture.agent.fn.authorize")
fn authorize(state: borrow State, budget: i64, urgent: bool, sequence: usize) -> Decision
{
    let seal = [65u8, 90u8];
    if budget <= state.budget && sequence > 0usize {
        Decision::Granted { seal: bytes_copy(array_as_slice(seal)), budget: budget }
    } else {
        Decision::Refused { code: if urgent { 2 } else { 1 } }
    }
}

@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Report
{
    let stamp = [82u8, 80u8];
    Report {
        summary: bytes_copy(array_as_slice(stamp)),
        budget: state.budget - budget,
        status: outcome.status + state.epoch,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

struct CountingRead(usize);

impl AgentReadOperation for CountingRead {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.0 += 1;
        Some(b"observed".to_vec())
    }
}

fn lifecycle() -> CompiledAgentLifecycle {
    compile_agent_lifecycle(
        MODULE,
        "agent-lifecycle-unit.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
    )
    .expect("the unit fixture binds every stage")
}

pub(in crate::agent_lifecycle) fn proposal(
    compiled: &CompiledAgentLifecycle,
    budget: &str,
    sequence: &str,
) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{digest}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.proposal.budget\":\"{budget}\",",
            "\"fixture.agent.type.proposal.urgent\":false,",
            "\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        digest = compiled.proposal_schema().schema().digest(),
        budget = budget,
        sequence = sequence
    )
}

/// Runs `initialize` and then the authorizing transition, returning the state
/// it authorized against, the exact proposal document, and the grant.
fn authorize(
    compiled: &CompiledAgentLifecycle,
    budget: i64,
) -> (RetainedValue, String, Authorized) {
    let evaluation = compiled
        .evaluate(
            &compiled.binding.initialize,
            &[payload(&compiled.binding.task, b"alpha".to_vec(), budget)],
            DEFAULT_STAGE_STEPS,
        )
        .expect("initialize evaluates");
    let RetainedCallOutcome::Returned(state) = evaluation.outcome else {
        panic!("initialize did not return a state");
    };
    let document = proposal(compiled, "5", "1");
    let decoded = compiled
        .proposal_schema()
        .decode(&document)
        .expect("the scripted proposal decodes");
    let projected = compiled.project(&decoded).expect("the projection is exact");
    let mut arguments = vec![state.clone()];
    arguments.extend(projected);
    let (outcome, _) = authorization::run_authorize_stage(
        &compiled.program,
        &compiled.binding.authorize,
        &arguments,
        DEFAULT_STAGE_STEPS,
        compiled.digest(),
        &state,
        decoded.canonical_json(),
    )
    .expect("the authorizing transition evaluates");
    let authorization::AuthorizationOutcome::Granted(authorized) = outcome else {
        panic!("the authorizing transition refused a satisfiable proposal");
    };
    (state, decoded.canonical_json().to_owned(), authorized)
}

#[test]
fn the_derived_stage_graph_is_acyclic_and_its_order_is_unique() {
    let order = stages::topological_order(&stages::ALL_ROLES, &stages::stage_edges())
        .expect("the derived stage graph is acyclic");
    assert_eq!(order, stages::ALL_ROLES.to_vec());

    // A cycle has no order at all.
    let mut cyclic = stages::stage_edges();
    cyclic.push(stages::StageEdge {
        from: "reduce",
        to: "initialize",
    });
    assert!(stages::topological_order(&stages::ALL_ROLES, &cyclic).is_none());

    // So does an ambiguous graph: two independent stages would leave the
    // execution order undetermined rather than deterministic.
    let ambiguous = [stages::StageEdge {
        from: "initialize",
        to: "observe",
    }];
    assert!(stages::topological_order(&["initialize", "observe", "reduce"], &ambiguous).is_none());
}

#[test]
fn an_authorization_is_not_spendable_into_a_substituted_state_or_proposal() {
    let compiled = lifecycle();

    // The authorization the validated authorize stage granted spends exactly
    // once, into the state and proposal it was granted against.
    let (state, document, authorized) = authorize(&compiled, 12);
    let mut read = CountingRead(0);
    let value = compiled
        .spend(authorized, &state, &document, &mut read)
        .expect("the bound authorization spends");
    assert_eq!(value, b"observed".to_vec());
    assert_eq!(read.0, 1);

    // A substituted state: an authorization granted against one state does not
    // admit an effect against another, and the read operation is never reached.
    let (_, _, authorized) = authorize(&compiled, 12);
    let (other_state, _, _) = authorize(&compiled, 11);
    let mut read = CountingRead(0);
    let error = compiled
        .spend(authorized, &other_state, &document, &mut read)
        .expect_err("a substituted state is refused");
    assert_eq!(error.code, "SPX-G571");
    assert!(error
        .message
        .contains("not bound to this state and proposal"));
    assert_eq!(read.0, 0);

    // A substituted proposal is refused the same way.
    let (state, _, authorized) = authorize(&compiled, 12);
    let stale = proposal(&compiled, "4", "1");
    assert_ne!(stale, document);
    let mut read = CountingRead(0);
    let error = compiled
        .spend(authorized, &state, &stale, &mut read)
        .expect_err("a substituted proposal is refused");
    assert_eq!(error.code, "SPX-G571");
    assert_eq!(read.0, 0);
}

#[test]
fn the_authorization_binding_separates_policy_state_proposal_case_and_seal() {
    let compiled = lifecycle();
    let (state, document, authorized) = authorize(&compiled, 12);
    let case = compiled.binding.authorize.grant_case();
    let base = authorization::binding(compiled.digest(), &state, &document, case, b"AZ");
    assert_eq!(base, authorized.binding());

    let (other_state, _, _) = authorize(&compiled, 11);
    for candidate in [
        authorization::binding("sha256:0", &state, &document, case, b"AZ"),
        authorization::binding(compiled.digest(), &other_state, &document, case, b"AZ"),
        authorization::binding(
            compiled.digest(),
            &state,
            &proposal(&compiled, "4", "1"),
            case,
            b"AZ",
        ),
        authorization::binding(
            compiled.digest(),
            &state,
            &document,
            compiled.binding.authorize.refuse_case(),
            b"AZ",
        ),
        authorization::binding(compiled.digest(), &state, &document, case, b"ZA"),
    ] {
        assert_ne!(base, candidate);
    }
}

#[test]
fn the_authorization_value_has_exactly_one_mint_site_in_the_crate() {
    let authorization = include_str!("authorization.rs");
    let lifecycle = include_str!("../agent_lifecycle.rs");
    let stages = include_str!("stages.rs");
    let durable = include_str!("durable.rs");
    let checkpoint = include_str!("durable/checkpoint.rs");
    let journal = include_str!("durable/journal.rs");

    // The struct literal that builds the value exists exactly once, and its
    // fields are private to the authorization module, so no other module can
    // name them even inside this crate.
    assert_eq!(authorization.matches("\n    Authorized {").count(), 1);
    assert_eq!(lifecycle.matches("Authorized {").count(), 0);
    assert_eq!(stages.matches("Authorized {").count(), 0);
    // The durable path adds no second route: it never names the struct, never
    // names the mint, and reaches an authorization only by running the same
    // validated authorizing transition once.
    for (name, source) in [
        ("durable.rs", durable),
        ("durable/checkpoint.rs", checkpoint),
        ("durable/journal.rs", journal),
    ] {
        assert_eq!(source.matches("Authorized {").count(), 0, "{name}");
        assert_eq!(source.matches("mint(").count(), 0, "{name}");
    }
    assert_eq!(durable.matches("run_authorize_stage(").count(), 1);
    assert!(authorization.contains("pub struct Authorized {\n    binding: String,"));

    // The mint is private and is called exactly once, from the function that
    // runs the validated authorizing transition.
    assert!(authorization.contains("\nfn mint("));
    assert!(!authorization.contains("pub fn mint("));
    assert!(!authorization.contains("pub(super) fn mint("));
    assert!(!authorization.contains("pub(crate) fn mint("));
    assert_eq!(
        authorization.matches("mint(binding, budget, seal)").count(),
        1
    );
    let (_, after) = authorization
        .split_once("pub(super) fn run_authorize_stage(")
        .expect("the mint's only caller is the authorize-stage runner");
    assert!(after.contains("mint(binding, budget, seal)"));

    // The value derives nothing: it is not `Clone`, not `Copy`, and has no
    // `Default`, so one grant admits at most one effect.
    assert!(!authorization.contains("#[derive"));
    assert!(!authorization.contains("impl Clone for Authorized"));
    assert!(!authorization.contains("impl Default for Authorized"));

    // Nothing in the lifecycle reaches a host, a process or the environment.
    for forbidden in [
        "std::net::",
        "std::env::",
        "TcpStream",
        "Command::new",
        "fs::write",
        "fs::read",
        "File::create",
    ] {
        for (name, source) in [
            ("authorization.rs", authorization),
            ("agent_lifecycle.rs", lifecycle),
            ("stages.rs", stages),
            ("durable.rs", durable),
            ("durable/checkpoint.rs", checkpoint),
            ("durable/journal.rs", journal),
        ] {
            assert!(!source.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}

#[test]
fn public_canonical_retained_value_context_is_the_existing_identity_wire() {
    let value = RetainedValue::Record(RetainedRecord {
        record: hir::DeclarationId::new("state".to_owned()),
        fields: vec![RetainedField {
            field: hir::DeclarationId::new("state.answer".to_owned()),
            value: RetainedValue::Bytes(vec![0xab]),
        }],
    });
    assert_eq!(
        canonical_retained_value_json(&value),
        "{\"record\":\"state\",\"fields\":[{\"field\":\"state.answer\",\"value\":{\"bytes\":\"ab\"}}]}"
    );
    assert_eq!(canonical_retained_value_json(&value), encode_value(&value));
}
