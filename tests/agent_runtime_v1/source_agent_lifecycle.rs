//! A source Agent declaration is the only mutable definition truth used to
//! compile and execute the existing bounded Agent Lifecycle v1.

use semaprax::agent_lifecycle::{
    compile_agent_lifecycle, compile_source_agent_lifecycle, verify_source_agent_lifecycle_bundle,
    AgentReadOperation, AuthorizedRequest, LifecycleBudget, LifecycleStatus, LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::project::compile_source_agent_declaration;

use super::agent_lifecycle_v1::{proposal, MODULE, MODULE_PATH};
use super::source_agent_lowering::runtime_v1;

const AGENT_ID: &str = "fixture.agent";

fn source_agent_block(reduce_id: &str) -> String {
    let runtime = serde_json::to_string(&runtime_v1(&super::profile())).unwrap();
    format!(
        r#"
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
        @id("{reduce_id}")
        fn reduce;
    }}
    runtime_v1 {{
        canonical_json {runtime};
    }}
}}
"#,
    )
}

fn source_module(reduce_id: &str, extra: &str) -> String {
    format!("{MODULE}\n{}\n{extra}", source_agent_block(reduce_id))
}

fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"alpha".to_vec(),
        budget: 12,
    }
}

#[derive(Default)]
struct Read {
    calls: usize,
    fail: bool,
}

impl AgentReadOperation for Read {
    fn read(&mut self, request: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        assert_eq!(request.seal(), b"AZ");
        (!self.fail).then(|| b"observed".to_vec())
    }
}

#[test]
fn source_agent_selection_reproduces_and_executes_the_existing_lifecycle() {
    let source = source_module("fixture.agent.fn.reduce", "");
    let first = compile_source_agent_lifecycle(&source, MODULE_PATH, AGENT_ID).unwrap();
    let second = compile_source_agent_lifecycle(&source, MODULE_PATH, AGENT_ID).unwrap();
    assert_eq!(first.canonical_json(), second.canonical_json());
    assert_eq!(first.digest(), second.digest());

    let checked = semaprax::check(&source, MODULE_PATH).unwrap();
    assert_eq!(first.source_revision(), semaprax::graph::revision(&checked));
    let declaration = checked
        .agents
        .iter()
        .find(|declaration| declaration.stable_id == AGENT_ID)
        .unwrap();
    let definition = compile_source_agent_declaration(declaration).unwrap();
    let legacy = compile_agent_lifecycle(
        &source,
        MODULE_PATH,
        definition.definition().canonical_source(),
    )
    .unwrap();
    assert_eq!(first.canonical_json(), legacy.canonical_json());
    assert_eq!(first.definition_digest(), definition.definition().digest());

    let mut read = Read::default();
    let run = first
        .run(
            &task(),
            &proposal(first.proposal_schema().schema().digest(), "5", false, "1"),
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::Completed);
    assert_eq!(run.reason(), "reduce_published_a_result");
    assert_eq!(read.calls, 1);

    let errors = compile_source_agent_lifecycle(&source, MODULE_PATH, "fixture.missing")
        .err()
        .unwrap();
    assert_eq!(errors[0].code, "SPX-G570");
    assert!(errors[0].message.ends_with("source_agent.selection"));
}

#[test]
fn source_agent_lifecycle_preserves_refusal_and_effect_failure_terminals() {
    let source = source_module("fixture.agent.fn.reduce", "");
    let lifecycle = compile_source_agent_lifecycle(&source, MODULE_PATH, AGENT_ID).unwrap();
    let digest = lifecycle.proposal_schema().schema().digest();

    let mut refused_read = Read::default();
    let refused = lifecycle
        .run(
            &task(),
            &proposal(digest, "5", true, "0"),
            &mut refused_read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(refused.status(), LifecycleStatus::Rejected);
    assert_eq!(refused.refusal_code(), Some(2));
    assert_eq!(refused_read.calls, 0);

    let mut failed_read = Read {
        calls: 0,
        fail: true,
    };
    let failed = lifecycle
        .run(
            &task(),
            &proposal(digest, "5", false, "1"),
            &mut failed_read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(failed.status(), LifecycleStatus::EffectFailed);
    assert_eq!(failed.reason(), "execute_failed");
    assert_eq!(failed_read.calls, 1);
}

#[test]
fn source_agent_lifecycle_rejects_stale_and_wrong_role_bindings() {
    let source = source_module("fixture.agent.fn.reduce", "");
    let lifecycle = compile_source_agent_lifecycle(&source, MODULE_PATH, AGENT_ID).unwrap();
    verify_source_agent_lifecycle_bundle(
        &source,
        MODULE_PATH,
        AGENT_ID,
        lifecycle.source_revision(),
        lifecycle.canonical_json(),
    )
    .unwrap();

    let alternate = r#"
@id("fixture.agent.fn.alternate-reduce")
fn alternate_reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Report
{
    let stamp = [82u8, 80u8];
    Report {
        summary: bytes_copy(array_as_slice(stamp)),
        budget: state.budget - budget,
        status: outcome.status + state.epoch,
    }
}
"#;
    let stale = source_module("fixture.agent.fn.alternate-reduce", alternate);
    let error = verify_source_agent_lifecycle_bundle(
        &stale,
        MODULE_PATH,
        AGENT_ID,
        lifecycle.source_revision(),
        lifecycle.canonical_json(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G572");

    let stale_body = source.replace(
        "status: outcome.status + state.epoch,",
        "status: outcome.status + state.epoch + 1,",
    );
    assert_ne!(stale_body, source);
    let changed = compile_source_agent_lifecycle(&stale_body, MODULE_PATH, AGENT_ID).unwrap();
    assert_eq!(changed.canonical_json(), lifecycle.canonical_json());
    assert_ne!(changed.source_revision(), lifecycle.source_revision());
    let error = verify_source_agent_lifecycle_bundle(
        &stale_body,
        MODULE_PATH,
        AGENT_ID,
        lifecycle.source_revision(),
        lifecycle.canonical_json(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G572");

    let wrong = r#"
@id("fixture.agent.fn.wrong-reduce")
fn wrong_reduce(value: i64) -> i64 { value }
"#;
    let wrong = source_module("fixture.agent.fn.wrong-reduce", wrong);
    let errors = compile_source_agent_lifecycle(&wrong, MODULE_PATH, AGENT_ID)
        .err()
        .unwrap();
    assert!(errors.iter().any(|error| error.code == "SPX-G570"));
}

#[test]
fn source_agent_lifecycle_replay_rejects_unbounded_selectors_before_compilation() {
    let oversized = "x".repeat(262_145);
    let errors = verify_source_agent_lifecycle_bundle(
        "not a valid module",
        MODULE_PATH,
        AGENT_ID,
        &format!("sha256:{}", "0".repeat(64)),
        &oversized,
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-G572");

    for revision in [
        "",
        "sha256:0",
        "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
    ] {
        let errors = verify_source_agent_lifecycle_bundle(
            "not a valid module",
            MODULE_PATH,
            AGENT_ID,
            revision,
            "",
        )
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-G572", "{revision}");
    }

    let oversized_agent = "a".repeat(241);
    let errors = verify_source_agent_lifecycle_bundle(
        "not a valid module",
        MODULE_PATH,
        &oversized_agent,
        &format!("sha256:{}", "0".repeat(64)),
        "",
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-G572");

    let errors = compile_source_agent_lifecycle("not a valid module", MODULE_PATH, "not valid")
        .err()
        .unwrap();
    assert_eq!(errors[0].code, "SPX-G570");
}
