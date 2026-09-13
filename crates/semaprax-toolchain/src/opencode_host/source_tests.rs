//! Source-route regressions for the explicit OpenCode host adapter.
//!
//! These tests exercise CompiledIterativeLifecycle::run_live, not the generic
//! invocation kernel, so compiler-owned source proposal admission and retry
//! behavior remain covered at the production bridge boundary.

use super::accounting::OpenCodeSourceAccounting;
use super::source::OpenCodeProposalSource;
use super::*;
use semaprax::agent_lifecycle::iterative::{
    compile_agent_lifecycle_v2, IterativeBudget, IterativeStatus,
};
use semaprax::agent_lifecycle::{AgentReadOperation, AuthorizedRequest, LifecycleTask};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::live_invocation::{CumulativeBudgetLedger, InvocationClock, ModelInvokeCapability};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[path = "../../examples/fixtures/opencode_source_fixture.rs"]
mod fixtures;

#[path = "source_checkpoint_tests.rs"]
mod checkpointed;

const HOST_CREDENTIAL_SENTINEL: &str = "credential-sentinel-owned-by-runner";

struct Clock(Rc<Cell<i64>>);

impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}

struct CountingRead {
    calls: usize,
    advance_clock: Option<(Rc<Cell<i64>>, i64)>,
}

impl AgentReadOperation for CountingRead {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        if let Some((clock, millis)) = &self.advance_clock {
            clock.set(*millis);
        }
        Some(b"observed".to_vec())
    }
}

/// Deterministic host transport fixture. Its credential marker is intentionally
/// host-only; the source adapter receives neither it nor any runner state.
struct FixtureRunner {
    answer: String,
    host_credential: String,
    prompts: Vec<String>,
    calls: usize,
    failure: Option<OpenCodeRunnerFailure>,
    advance_clock: Option<(Rc<Cell<i64>>, i64)>,
}

impl OpenCodeRunner for FixtureRunner {
    fn run(
        &mut self,
        _: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        self.calls += 1;
        self.prompts.push(prompt.to_owned());
        if let Some((clock, millis)) = &self.advance_clock {
            clock.set(*millis);
        }
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        Ok(transport(prompt, &self.answer).0)
    }

    fn export(
        &mut self,
        _: &OpenCodeHostConfig,
        _: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let prompt = self.prompts.last().expect("export follows run");
        Ok(transport(prompt, &self.answer).1)
    }
}

/// Uses the recorded OpenCode event/export parts, binding the supplied prompt
/// and response through the same receipt validator as the production host.
fn transport(prompt: &str, answer: &str) -> (Vec<u8>, Vec<u8>) {
    let mut export: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../scripts/fixtures/opencode-provider-smoke-v1/session.json"
    ))
    .expect("checked transport fixture");
    export["messages"][0]["parts"][0]["text"] =
        serde_json::json!(super::receipt::cli_prompt(prompt));
    export["messages"][1]["parts"][2]["text"] = serde_json::json!(answer);
    let parts = export["messages"][1]["parts"]
        .as_array()
        .expect("checked transport fixture");
    let events = [
        ("step_start", &parts[0]),
        ("text", &parts[2]),
        ("step_finish", &parts[3]),
    ]
    .iter()
    .map(|(kind, part)| {
        serde_json::json!({"type": kind, "sessionID": "ses_fixture", "part": part}).to_string()
    })
    .collect::<Vec<_>>()
    .join("\n")
    .into_bytes();
    (
        events,
        serde_json::to_vec(&export).expect("checked transport fixture"),
    )
}

fn config(grammar: OpenCodeGrammar) -> OpenCodeHostConfig {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let sandbox = std::env::temp_dir().join(format!(
        "semaprax-opencode-source-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir(&sandbox).expect("fresh sandbox");
    let config = OpenCodeHostConfig::new(
        PathBuf::from("/bin/true"),
        sandbox.clone(),
        Duration::from_secs(1),
        grammar,
    )
    .expect("valid host config");
    std::fs::remove_dir(sandbox).expect("empty sandbox");
    config
}

fn proposal(digest: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":{:?},\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"1\",\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n",
        digest
    )
}

fn setup(
    answer: String,
) -> (
    semaprax::agent_lifecycle::iterative::CompiledIterativeLifecycle,
    OpenCodeModelHandler<FixtureRunner>,
    LifecycleTask,
    OpenCodeGrammar,
) {
    let compiled = compile_agent_lifecycle_v2(
        fixtures::SOURCE,
        "opencode-source-test.spx",
        fixtures::DEFINITION,
        "fixture.agent.type.step",
    )
    .expect("frozen source fixture compiles");
    let grammar =
        OpenCodeGrammar::from_proposal(compiled.proposal_schema()).expect("proposal grammar");
    let handler = OpenCodeModelHandler::new(
        config(grammar.clone()),
        FixtureRunner {
            answer,
            host_credential: HOST_CREDENTIAL_SENTINEL.into(),
            prompts: Vec::new(),
            calls: 0,
            failure: None,
            advance_clock: None,
        },
    );
    (
        compiled,
        handler,
        LifecycleTask {
            objective: b"source objective".to_vec(),
            budget: 1,
        },
        grammar,
    )
}

fn compiled_proposal() -> String {
    let compiled = compile_agent_lifecycle_v2(
        fixtures::SOURCE,
        "opencode-source-test.spx",
        fixtures::DEFINITION,
        "fixture.agent.type.step",
    )
    .expect("frozen source fixture compiles");
    proposal(compiled.proposal_schema().schema().digest())
}

#[test]
fn source_adapter_runs_the_checked_lifecycle_to_complete_without_host_credential_leakage() {
    let (compiled, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let mut clock = Clock(Rc::new(Cell::new(0)));
    let mut ledger = CumulativeBudgetLedger::new(1, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let run = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .expect("settled source lifecycle");
    let receipts = source.receipts().to_vec();
    drop(source);

    assert_eq!(run.status(), IterativeStatus::Complete);
    assert_eq!(read.calls, 1);
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].reserved_units, 1);
    assert!(receipts[0].reported_usage.is_some());
    assert!(!receipts[0].usage.failed);
    assert_eq!(handler.runner.host_credential, HOST_CREDENTIAL_SENTINEL);
    let prompt = handler.runner.prompts.first().expect("one host invocation");
    let source_context = prompt
        .lines()
        .find(|line| line.starts_with("source_context="))
        .expect("source bridge prompt");
    assert!(prompt.starts_with("SEMAPRAX source proposal v1\n"));
    assert!(!source_context.contains(HOST_CREDENTIAL_SENTINEL));
    assert!(!prompt.contains(HOST_CREDENTIAL_SENTINEL));
    assert!(!run.evidence().contains(HOST_CREDENTIAL_SENTINEL));
    assert_eq!(
        ledger.committed(),
        1,
        "reported usage cannot refund reservation"
    );
}

#[test]
fn malformed_first_proposal_spends_the_whole_ceiling_before_retry_and_dispatches_no_effect() {
    let mut answer = compiled_proposal();
    answer.truncate(answer.len() - 2);
    let (compiled, mut handler, task, grammar) = setup(answer);
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let mut clock = Clock(Rc::new(Cell::new(0)));
    let mut ledger = CumulativeBudgetLedger::new(1, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let error = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .err()
        .expect("the second reservation must refuse before another provider call");
    let receipts = source.receipts().to_vec();
    drop(source);
    assert_eq!(error[0].code, "SPX-I239");
    assert_eq!(read.calls, 0);
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(receipts.len(), 1);
    assert!(!receipts[0].usage.failed);
    assert_eq!(ledger.committed(), 1);
}

#[test]
fn host_cancellation_before_the_first_reservation_makes_no_host_call_or_charge() {
    let (compiled, mut handler, task, grammar) = setup(compiled_proposal());
    handler.config.cancellation().cancel();
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let mut clock = Clock(Rc::new(Cell::new(0)));
    let mut ledger = CumulativeBudgetLedger::new(1, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let error = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .err()
        .expect("host cancellation refuses before reservation");

    let receipts = source.receipts().to_vec();
    drop(source);
    assert_eq!(error[0].code, "SPX-I239");
    assert_eq!(handler.runner.calls, 0);
    assert!(receipts.is_empty());
    assert_eq!(ledger.committed(), 0);
}

#[test]
fn settled_at_shared_deadline_is_charged_but_never_decoded_or_dispatched() {
    let answer = compiled_proposal();
    let expected_bytes = answer.len();
    let (compiled, mut handler, task, grammar) = setup(answer);
    let shared_clock = Rc::new(Cell::new(0));
    handler.runner.advance_clock = Some((Rc::clone(&shared_clock), 10));
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let mut clock = Clock(shared_clock);
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let error = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .err()
        .expect("a settled response at the deadline cannot reach decode");
    let receipts = source.receipts().to_vec();
    drop(source);

    assert_eq!(error[0].code, "SPX-I239");
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(read.calls, 0);
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].usage.failed);
    assert_eq!(receipts[0].usage.response_bytes, expected_bytes);
    assert!(receipts[0].reported_usage.is_some());
    assert_eq!(
        receipts[0].accounting_refusal,
        Some(super::accounting::OpenCodeAccountingRefusal::DeadlineExceeded)
    );
    assert_eq!(ledger.committed(), 1);
}

#[test]
fn settled_just_before_shared_deadline_is_admitted() {
    let (compiled, mut handler, task, grammar) = setup(compiled_proposal());
    let shared_clock = Rc::new(Cell::new(0));
    handler.runner.advance_clock = Some((Rc::clone(&shared_clock), 9));
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let mut clock = Clock(shared_clock);
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let run = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .expect("a settled response before the deadline is admitted");
    let receipts = source.receipts().to_vec();
    drop(source);

    assert_eq!(run.status(), IterativeStatus::Complete);
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(read.calls, 1);
    assert!(!receipts[0].usage.failed);
    assert!(receipts[0].accounting_refusal.is_none());
    assert_eq!(ledger.committed(), 1);
}

#[test]
fn expired_before_initialize_stops_without_model_or_effect_dispatch() {
    let (compiled, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let clock_value = Rc::new(Cell::new(10));
    let mut clock = Clock(clock_value);
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let error = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .err()
        .expect("expired source deadline refuses before initialize");
    let receipts = source.receipts().to_vec();
    drop(source);

    assert_eq!(error[0].code, "SPX-I239");
    assert_eq!(handler.runner.calls, 0);
    assert_eq!(read.calls, 0);
    assert!(receipts.is_empty());
    assert_eq!(ledger.committed(), 0);
}

#[test]
fn deadline_advanced_inside_effect_blocks_reduce_and_terminal_publication() {
    let (compiled, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let clock_value = Rc::new(Cell::new(0));
    let mut clock = Clock(Rc::clone(&clock_value));
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: Some((clock_value, 10)),
    };

    let error = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .err()
        .expect("an expired effect result cannot reach reduce or publication");
    let receipts = source.receipts().to_vec();
    drop(source);

    assert_eq!(error[0].code, "SPX-I239");
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(read.calls, 1);
    assert_eq!(receipts.len(), 1);
    assert!(!receipts[0].usage.failed);
    assert_eq!(ledger.committed(), 1);
}

#[test]
fn uncertain_provider_failure_keeps_the_reservation_and_records_no_provider_usage() {
    let (compiled, mut handler, task, grammar) = setup(compiled_proposal());
    handler.runner.failure = Some(OpenCodeRunnerFailure::Provider);
    let capability = ModelInvokeCapability::grant("source adapter fixture");
    let cancellation = AgentCancellation::new();
    let mut clock = Clock(Rc::new(Cell::new(0)));
    let mut ledger = CumulativeBudgetLedger::new(1, &mut clock);
    let accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 4).expect("accounting");
    let mut source = OpenCodeProposalSource::new(
        &mut handler,
        &capability,
        "source-test.v1".into(),
        grammar,
        4_096,
        accounting,
    )
    .expect("source bridge");
    let mut read = CountingRead {
        calls: 0,
        advance_clock: None,
    };

    let error = compiled
        .run_live(
            &task,
            &mut source,
            &mut read,
            IterativeBudget {
                max_iterations: 1,
                ..IterativeBudget::default()
            },
            &cancellation,
        )
        .err()
        .expect("unknown provider outcome ends the source run");
    let receipts = source.receipts().to_vec();
    drop(source);

    assert_eq!(error[0].code, "SPX-I239");
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(read.calls, 0);
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].usage.failed);
    assert_eq!(receipts[0].usage.response_bytes, 0);
    assert!(receipts[0].reported_usage.is_none());
    assert!(receipts[0].accounting_refusal.is_none());
    assert_eq!(ledger.committed(), 1);
}
