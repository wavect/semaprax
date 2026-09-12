//! Frozen source-lifecycle fixture shared by the OpenCode source smoke and host regression.
//!
//! The literal constants are the canonical fixture bytes; do not reformat their contents.

pub const DEFINITION: &str = r#"{"schema":"semaprax.agent-definition.v1","agent_id":"fixture.agent","types":[{"role":"task","stable_id":"fixture.agent.type.task"},{"role":"state","stable_id":"fixture.agent.type.state"},{"role":"observation","stable_id":"fixture.agent.type.observation"},{"role":"proposal","stable_id":"fixture.agent.type.proposal"},{"role":"outcome","stable_id":"fixture.agent.type.outcome"},{"role":"result","stable_id":"fixture.agent.type.result"}],"operations":[{"role":"initialize","stable_id":"fixture.agent.fn.initialize","kind":"deterministic"},{"role":"observe","stable_id":"fixture.agent.fn.observe","kind":"deterministic"},{"role":"propose","stable_id":"fixture.agent.fn.propose","kind":"model"},{"role":"authorize","stable_id":"fixture.agent.fn.authorize","kind":"deterministic"},{"role":"execute","stable_id":"fixture.agent.fn.execute","kind":"effect"},{"role":"reduce","stable_id":"fixture.agent.fn.reduce","kind":"deterministic"}],"runtime_v1":{"models":[{"provider_id":"opencode","model_id":"muse-spark-1.3-contributor-free","locality":"remote","quality_tier":"basic","tokenizer_id":"fake.bytes-v1","max_context_tokens":4096,"input_usd_microunits_per_million_tokens":0,"output_usd_microunits_per_million_tokens":0,"capabilities":["text"]}],"tools":[{"tool_id":"fixture.read","description":"Return one bounded fixture value.","arguments_schema":{"type":"object","fields":[{"name":"query","type":"string","required":true,"max_bytes":64}],"additional_properties":false},"result_schema":{"type":"object","fields":[{"name":"value","type":"string","required":true,"max_bytes":64}],"additional_properties":false},"effects":["read"],"required_capabilities":["tool.read"]}],"policy":{"allowed_provider_ids":["opencode"],"allowed_model_ids":["muse-spark-1.3-contributor-free"],"required_locality":"remote_allowed","minimum_quality_tier":"basic","required_model_capabilities":["text"],"granted_capabilities":["tool.read"],"allowed_tool_ids":["fixture.read"]},"limits":{"max_turns":2,"max_provider_attempts":2,"max_retries_per_turn":1,"max_concurrency":1,"max_elapsed_ms":1000,"max_provider_request_bytes":65536,"max_provider_response_bytes":4096,"max_stream_chunks":64,"max_total_provider_input_bytes":131072,"max_total_provider_output_bytes":8192,"max_reported_model_input_tokens":131072,"max_reported_model_output_tokens":8192,"max_usd_microunits":0,"max_tool_calls":1,"max_tool_arguments_bytes":4096,"max_tool_result_bytes":4096,"max_total_tool_bytes":8192,"max_retained_state_bytes":131072,"max_trace_events":64,"max_trace_bytes":131072,"max_evidence_bytes":262144,"max_builder_bytes":1048576}}}
"#;
pub const SOURCE: &str = r#"module fixture.agent.lifecycle;

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

@id("fixture.agent.type.step")
variant Step {
    @id("fixture.agent.step.continue") Continue { @id("fixture.agent.step.continue.objective") objective: Bytes, @id("fixture.agent.step.continue.budget") budget: i64, @id("fixture.agent.step.continue.epoch") epoch: i64, },
    @id("fixture.agent.step.complete") Complete { @id("fixture.agent.step.complete.summary") summary: Bytes, @id("fixture.agent.step.complete.budget") budget: i64, @id("fixture.agent.step.complete.status") status: i64, },
    @id("fixture.agent.step.suspend") Suspend { @id("fixture.agent.step.suspend.objective") objective: Bytes, @id("fixture.agent.step.suspend.budget") budget: i64, @id("fixture.agent.step.suspend.epoch") epoch: i64, },
    @id("fixture.agent.step.fail") Fail { @id("fixture.agent.step.fail.code") code: i64, },
}
@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Step {
    Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
