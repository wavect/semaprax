//! Terminal evidence when routing stops a run (DV-19) and the dispatched
//! output cap staying one value with its reservation (DV-27).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use semaprax::agent_runtime::{
    Agent, AgentBoundaryProbe, AgentCancellation, AgentHost, AgentProviderAttempt,
    AgentProviderDisposition, AgentProviderSink, AgentProviderUsage, AgentRunStatus,
    AgentToolResultSink,
};

use super::{profile, task};

struct Probe;

impl AgentBoundaryProbe for Probe {
    fn policy_epoch(&self) -> u64 {
        7
    }

    fn elapsed_ms(&self) -> u64 {
        0
    }
}

/// One-token-per-byte host, optionally with each `01` pair merged into one
/// token, charging exactly the profile price (1 microunit per token).
struct PricedHost {
    merge_pairs: bool,
    price_input: bool,
    price_output: bool,
    requests: Arc<Mutex<Vec<String>>>,
    provider_calls: Arc<AtomicUsize>,
    tool_calls: Arc<AtomicUsize>,
}

fn count(merge_pairs: bool, text: &str) -> u64 {
    let pairs = if merge_pairs {
        text.matches("01").count() as u64
    } else {
        0
    };
    text.len() as u64 - pairs
}

impl PricedHost {
    fn new(merge_pairs: bool, price_input: bool, price_output: bool) -> Self {
        Self {
            merge_pairs,
            price_input,
            price_output,
            requests: Arc::default(),
            provider_calls: Arc::default(),
            tool_calls: Arc::default(),
        }
    }
}

impl AgentHost for PricedHost {
    fn policy_epoch(&self) -> u64 {
        7
    }

    fn elapsed_ms(&self) -> u64 {
        0
    }

    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(Probe)
    }

    fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
        Some(count(self.merge_pairs, request))
    }

    fn attempt_provider(
        &mut self,
        _: &str,
        _: &str,
        request: &str,
        _: u64,
        sink: &mut AgentProviderSink,
    ) -> AgentProviderAttempt {
        let call = self.provider_calls.fetch_add(1, Ordering::AcqRel);
        self.requests.lock().unwrap().push(request.to_owned());
        let response: &[u8] = if call == 0 && self.price_input {
            b"{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"tool\",\"tool_id\":\"fixture.read\",\"arguments\":{\"query\":\"alpha\"}}\n"
        } else {
            b"{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":\"done\"}\n"
        };
        assert!(sink.push(response));
        let input = count(self.merge_pairs, request);
        let output = count(self.merge_pairs, std::str::from_utf8(response).unwrap());
        let usd = u64::from(self.price_input) * input + u64::from(self.price_output) * output;
        AgentProviderAttempt::new(
            AgentProviderDisposition::Succeeded,
            AgentProviderUsage::new(input, output, usd),
        )
    }

    fn invoke_tool(&mut self, _: &str, _: &str, _: &str, sink: &mut AgentToolResultSink) -> bool {
        self.tool_calls.fetch_add(1, Ordering::AcqRel);
        sink.push(b"{\"value\":\"beta\"}")
    }
}

fn priced_profile(budget: u64, input: u64, output: u64) -> String {
    profile()
        .replace(
            "\"input_usd_microunits_per_million_tokens\":0",
            &format!("\"input_usd_microunits_per_million_tokens\":{input}"),
        )
        .replace(
            "\"output_usd_microunits_per_million_tokens\":0",
            &format!("\"output_usd_microunits_per_million_tokens\":{output}"),
        )
        .replace(
            "\"max_reported_model_output_tokens\":8192",
            "\"max_reported_model_output_tokens\":512",
        )
        .replace(
            "\"max_usd_microunits\":0",
            &format!("\"max_usd_microunits\":{budget}"),
        )
}

struct Outcome {
    status: AgentRunStatus,
    trace: String,
    evidence: String,
    receipt: String,
    providers: usize,
    tools: usize,
}

fn run_priced(budget: u64) -> Outcome {
    let host = PricedHost::new(false, true, false);
    let providers = Arc::clone(&host.provider_calls);
    let tools = Arc::clone(&host.tool_calls);
    let mut agent = Agent::new(
        &priced_profile(budget, 1_000_000, 0),
        host,
        AgentCancellation::new(),
    )
    .unwrap();
    let run = agent.run(&task()).unwrap_or_else(|error| {
        panic!(
            "run lost its artifacts: {}: {}",
            error[0].code, error[0].message
        )
    });
    Outcome {
        status: run.status(),
        trace: run.trace().to_owned(),
        evidence: run.evidence().to_owned(),
        receipt: run.accounting_receipt().to_owned(),
        providers: providers.load(Ordering::Acquire),
        tools: tools.load(Ordering::Acquire),
    }
}

fn observed(receipt: &str) -> u64 {
    fn find(value: &serde_json::Value) -> Option<u64> {
        match value {
            serde_json::Value::Object(map) => map
                .get("observed_usd_microunits")
                .and_then(serde_json::Value::as_u64)
                .or_else(|| map.values().find_map(find)),
            _ => None,
        }
    }
    let value: serde_json::Value = serde_json::from_str(receipt).unwrap();
    find(&value).unwrap_or_else(|| panic!("no observed total in {receipt}"))
}

#[test]
fn first_route_without_affordable_model_is_a_replayable_policy_rejection() {
    let outcome = run_priced(0);
    assert_eq!(outcome.status, AgentRunStatus::PolicyRejected);
    assert_eq!((outcome.providers, outcome.tools), (0, 0));
    assert!(
        outcome.evidence.contains("SPX-G206"),
        "{}",
        outcome.evidence
    );
    assert!(outcome.trace.contains("policy_rejected"));
    assert!(!outcome.evidence.contains("SPX-G209"));
}

#[test]
fn no_affordable_model_after_a_tool_retains_the_performed_work() {
    let control = run_priced(1_000_000);
    assert_eq!(control.status, AgentRunStatus::Completed);
    assert_eq!((control.providers, control.tools), (2, 1));
    let total = observed(&control.receipt);

    let stopped = run_priced(total - 1);
    assert_eq!(stopped.status, AgentRunStatus::PolicyRejected);
    assert_eq!((stopped.providers, stopped.tools), (1, 1));
    assert!(stopped.evidence.contains("SPX-G206"));
    assert!(stopped.trace.contains("tool_finished"));
    assert!(stopped.trace.contains("policy_rejected"));
    assert!(!stopped.evidence.contains("SPX-G209"));
    let first = observed(&stopped.receipt);
    assert!(first > 0 && first < total, "{first} of {total}");

    let exact = run_priced(total);
    assert_eq!(exact.status, AgentRunStatus::Completed);
    assert_eq!((exact.providers, exact.tools), (2, 1));
    assert_eq!(observed(&exact.receipt), total);
}

const CONTEXT: u64 = 2004;

fn cap_profile(context: u64, budget: u64) -> String {
    priced_profile(budget, 0, 1_000_000)
        .replace(
            "\"max_context_tokens\":4096",
            &format!("\"max_context_tokens\":{context}"),
        )
        .replace(
            "\"max_reported_model_output_tokens\":512",
            "\"max_reported_model_output_tokens\":101",
        )
        .replace(
            "\"max_builder_bytes\":1048576",
            "\"max_builder_bytes\":4194304",
        )
}

/// Pads the task so the cap-101 request counts exactly 1904 tokens.
fn padded_task() -> String {
    let host = PricedHost::new(true, false, true);
    let requests = Arc::clone(&host.requests);
    let mut agent = Agent::new(
        &cap_profile(100_000, 1_000_000),
        host,
        AgentCancellation::new(),
    )
    .unwrap();
    agent.run(&task()).unwrap();
    let base = count(true, &requests.lock().unwrap()[0]);
    assert!(base < 1904);
    task().replace(
        "alpha",
        &format!("alpha{}", "a".repeat((1904 - base) as usize)),
    )
}

#[test]
fn dispatched_output_cap_matches_reservation_when_sizing_cycles() {
    let task = padded_task();
    let host = PricedHost::new(true, false, true);
    let requests = Arc::clone(&host.requests);
    let providers = Arc::clone(&host.provider_calls);
    let mut agent = Agent::new(&cap_profile(CONTEXT, 99), host, AgentCancellation::new()).unwrap();
    let run = agent.run(&task).unwrap();
    assert_eq!(
        run.status(),
        AgentRunStatus::Completed,
        "{}",
        run.evidence()
    );
    assert_eq!(providers.load(Ordering::Acquire), 1);
    let requests = requests.lock().unwrap();
    let request: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
    let cap = request["max_output_tokens"].as_u64().unwrap();
    assert!(count(true, &requests[0]) + cap <= CONTEXT, "cap {cap}");
    assert!(cap <= 99, "cap {cap} exceeds the 99 microunit budget");
    let receipt = run.accounting_receipt();
    assert!(
        receipt.contains(&format!("\"reserved_usd_microunits\":{cap}")),
        "reservation does not carry the dispatched cap {cap}: {receipt}"
    );
}

#[test]
fn stable_cap_and_insufficient_budget_stay_consistent() {
    let task = padded_task();
    let host = PricedHost::new(true, false, true);
    let requests = Arc::clone(&host.requests);
    let mut agent = Agent::new(&cap_profile(2006, 101), host, AgentCancellation::new()).unwrap();
    let run = agent.run(&task).unwrap();
    assert_eq!(run.status(), AgentRunStatus::Completed);
    let request: serde_json::Value = serde_json::from_str(&requests.lock().unwrap()[0]).unwrap();
    assert_eq!(request["max_output_tokens"], 101);

    let host = PricedHost::new(true, false, true);
    let providers = Arc::clone(&host.provider_calls);
    let mut agent = Agent::new(&cap_profile(CONTEXT, 10), host, AgentCancellation::new()).unwrap();
    let run = agent.run(&task).unwrap();
    assert_eq!(run.status(), AgentRunStatus::PolicyRejected);
    assert_eq!(providers.load(Ordering::Acquire), 0);
}
