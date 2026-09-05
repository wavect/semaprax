use super::*;

pub(super) fn run<H: AgentHost>(profile_source: &str, task_source: &str, host: H) -> AgentRun {
    let profile_body = profile_source.strip_suffix('\n').unwrap();
    let profile_members = profile_body
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"economic.fixture.agent\",",
        )
        .unwrap();
    let (runtime_members, _) = profile_members.split_once(",\"nonclaims\":").unwrap();
    let definition = concat!(
        "{\"schema\":\"semaprax.agent-definition.v1\",",
        "\"agent_id\":\"economic.fixture.agent\",",
        "\"types\":[",
        "{\"role\":\"task\",\"stable_id\":\"economic.fixture.type.task\"},",
        "{\"role\":\"state\",\"stable_id\":\"economic.fixture.type.state\"},",
        "{\"role\":\"observation\",\"stable_id\":\"economic.fixture.type.observation\"},",
        "{\"role\":\"proposal\",\"stable_id\":\"economic.fixture.type.payment_intent\"},",
        "{\"role\":\"outcome\",\"stable_id\":\"economic.fixture.type.payment_outcome\"},",
        "{\"role\":\"result\",\"stable_id\":\"economic.fixture.type.payment_result\"}],",
        "\"operations\":[",
        "{\"role\":\"initialize\",\"stable_id\":\"economic.fixture.fn.initialize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"observe\",\"stable_id\":\"economic.fixture.fn.observe\",\"kind\":\"deterministic\"},",
        "{\"role\":\"propose\",\"stable_id\":\"economic.fixture.fn.propose_payment\",\"kind\":\"model\"},",
        "{\"role\":\"authorize\",\"stable_id\":\"economic.fixture.fn.authorize_payment\",\"kind\":\"deterministic\"},",
        "{\"role\":\"execute\",\"stable_id\":\"economic.fixture.fn.execute_payment\",\"kind\":\"effect\"},",
        "{\"role\":\"reduce\",\"stable_id\":\"economic.fixture.fn.reduce_payment\",\"kind\":\"deterministic\"}],",
        "\"runtime_v1\":RUNTIME}\n"
    )
    .replace("RUNTIME", &format!("{{{runtime_members}}}"));
    let compiled = crate::agent_definition::compile_agent_definition(&definition).unwrap();
    compiled
        .instantiate(host, AgentCancellation::new())
        .unwrap()
        .run(task_source)
        .unwrap()
}
