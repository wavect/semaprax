//! End-to-end fixture-backed exercise of the effect boundary and its causal
//! journal, plus the replay determinism test #108/#177 both require: the
//! same journal replays to the same outcome without re-dispatching recorded
//! work. No test in this module makes a network call, opens a file, or
//! spends real model budget — every response is scripted.

use crate::agent_runtime::AgentCancellation;

use super::fixture::{
    fixture_response, FixtureAuthorizationGate, FixtureBudgetHook, FixtureEffect,
    FixtureModelHandler, FixtureObserver, FixturePolicy, FixtureProposalDecoder,
};
use super::identity::{LiveInvocationId, LiveInvocationSeed};
use super::journal;
use super::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationOutcome,
    LiveKernelError,
};
use super::model_invoke::{ModelFailure, ModelInvocationOutcome, ModelInvokeCapability};

const SCHEMA_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000aa";

fn identity() -> LiveInvocationId {
    LiveInvocationId::derive(&LiveInvocationSeed {
        program_root: "sha256:".to_owned() + &"1".repeat(64),
        deployment_policy: "sha256:".to_owned() + &"2".repeat(64),
        task: b"fixture task".to_vec(),
        budget: 1000,
        interaction_schema_digest: SCHEMA_DIGEST.to_owned(),
        approved_providers: vec!["fixture-provider".into()],
    })
}

fn config(identity: &LiveInvocationId, max_turns: u32) -> LiveInvocationConfig<'_> {
    LiveInvocationConfig {
        identity,
        task: b"fixture task",
        deployment_binding: "sha256:deploy-fixture",
        interaction_schema_digest: SCHEMA_DIGEST,
        max_turns,
        max_response_bytes: 4096,
        requested_budget_per_turn: 10,
    }
}

#[test]
fn a_three_turn_fixture_invocation_completes_with_one_dispatch_per_turn_and_one_effect() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("fixture end-to-end test");
    let mut handler = FixtureModelHandler::scripted(vec![
        ModelInvocationOutcome::Settled(fixture_response(0, "a")),
        ModelInvocationOutcome::Settled(fixture_response(1, "b")),
        ModelInvocationOutcome::Settled(fixture_response(2, "c")),
    ]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 3 };
    let mut effect = FixtureEffect { calls: 0 };
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: Some(&mut effect),
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();

    assert_eq!(run.dispatched, 3);
    assert_eq!(
        run.outcome,
        LiveInvocationOutcome::Complete(fixture_response(2, "c"))
    );

    let validated = journal::validate(&run.journal, identity.digest()).unwrap();
    assert!(validated.terminal);
    let receipt = journal::receipt_projection(&validated);
    assert_eq!(receipt.turns, 3);
    assert_eq!(receipt.model_calls, 3);
    assert_eq!(receipt.model_failures, 0);
    assert_eq!(receipt.effect_calls, 3);
    assert_eq!(receipt.terminal_case.as_deref(), Some("complete"));
    assert_eq!(effect.calls, 3);
}

#[test]
fn replaying_a_terminal_journal_makes_zero_dispatches_and_reproduces_the_outcome() {
    // Build the same completed three-turn journal directly, independent of
    // test execution order, rather than relying on the previous test.
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("build fixture journal for replay");
    let mut handler = FixtureModelHandler::scripted(vec![
        ModelInvocationOutcome::Settled(fixture_response(0, "a")),
        ModelInvocationOutcome::Settled(fixture_response(1, "b")),
        ModelInvocationOutcome::Settled(fixture_response(2, "c")),
    ]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 3 };
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let completed =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert!(matches!(
        completed.outcome,
        LiveInvocationOutcome::Complete(_)
    ));

    // Now replay: every seam that would perform new work panics if touched.
    let capability = ModelInvokeCapability::grant("replay must not dispatch");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(0);
    let mut budget = FixtureBudgetHook::refusing();
    let mut observer = PanicObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let replayed = run_live_invocation(
        &cfg,
        completed.journal.clone(),
        &mut handlers,
        &AgentCancellation::new(),
    )
    .unwrap();

    assert_eq!(
        replayed.dispatched, 0,
        "replay must not re-dispatch a settled call"
    );
    assert_eq!(handler.calls, 0);
    assert_eq!(
        replayed.journal, completed.journal,
        "replay does not rewrite the journal"
    );
    assert!(matches!(
        replayed.outcome,
        LiveInvocationOutcome::Complete(_)
    ));
}

struct PanicObserver;
impl super::kernel::TurnObserver for PanicObserver {
    fn observe(&mut self, _turn: u32) -> Vec<u8> {
        panic!("replay of a terminal journal must never observe a new turn")
    }
}
struct PanicPolicy;
impl super::kernel::TurnPolicy for PanicPolicy {
    fn reduce(&mut self, _turn: u32, _proposal: &[u8]) -> super::kernel::TurnTransition {
        panic!("replay of a terminal journal must never reduce a new turn")
    }
}

#[test]
fn cancellation_before_any_turn_opens_stops_cleanly_with_an_empty_journal() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("cancellation test");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = PanicObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let run = run_live_invocation(&cfg, Vec::new(), &mut handlers, &cancellation).unwrap();
    assert_eq!(run.outcome, LiveInvocationOutcome::Cancelled);
    assert_eq!(run.dispatched, 0);
    assert!(run.journal.is_empty());
}

#[test]
fn schema_drift_between_decoder_and_invocation_is_refused_before_any_dispatch() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("schema drift test");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new("sha256:".to_owned() + &"9".repeat(64));
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = PanicObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let result = run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new());
    assert_eq!(result.err(), Some(LiveKernelError::SchemaDrift));
    assert_eq!(handler.calls, 0);
}

#[test]
fn a_refused_budget_reservation_fails_the_turn_without_dispatching_the_handler() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("budget refusal test");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::refusing();
    let mut observer = FixtureObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert_eq!(
        run.dispatched, 0,
        "the handler is never reached on a budget refusal"
    );
    assert_eq!(handler.calls, 0);
    assert!(matches!(run.outcome, LiveInvocationOutcome::Fail(_)));
    let validated = journal::validate(&run.journal, identity.digest()).unwrap();
    assert!(validated.terminal);
}

#[test]
fn a_closed_model_failure_ends_the_attempt_without_decoding_or_authorizing() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("model failure test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Failed {
        failure: ModelFailure::ProviderError,
        attempted_bytes: 12,
    }]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert_eq!(run.dispatched, 1);
    assert!(matches!(run.outcome, LiveInvocationOutcome::Fail(_)));
    assert_eq!(gate.granted, 0, "a model failure never reaches authorize");
}

#[test]
fn a_malformed_response_is_refused_before_authorize() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("decode refusal test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        b"not the scripted shape".to_vec(),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert!(matches!(run.outcome, LiveInvocationOutcome::Fail(_)));
    assert_eq!(gate.granted, 0, "a refused decode never reaches authorize");
}

#[test]
fn an_authorization_refusal_fails_the_turn_after_a_successful_decode() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("authorization refusal test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(0, "a"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(0); // refuses immediately
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert!(matches!(run.outcome, LiveInvocationOutcome::Fail(_)));
}

#[test]
fn an_oversized_response_is_treated_as_malformed_before_decode() {
    let identity = identity();
    let mut cfg = config(&identity, 3);
    cfg.max_response_bytes = 4;
    let capability = ModelInvokeCapability::grant("oversized response test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(0, "much too long for the limit"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert!(matches!(run.outcome, LiveInvocationOutcome::Fail(_)));
    assert_eq!(gate.granted, 0);
}

#[test]
fn identity_binds_program_root_deployment_and_task_so_a_changed_input_changes_the_chain() {
    let base = identity();
    let mut different_task = LiveInvocationSeed {
        program_root: "sha256:".to_owned() + &"1".repeat(64),
        deployment_policy: "sha256:".to_owned() + &"2".repeat(64),
        task: b"a different task".to_vec(),
        budget: 1000,
        interaction_schema_digest: SCHEMA_DIGEST.to_owned(),
        approved_providers: vec!["fixture-provider".into()],
    };
    assert_ne!(base, LiveInvocationId::derive(&different_task));
    different_task.task = b"fixture task".to_vec();
    assert_eq!(base, LiveInvocationId::derive(&different_task));
}
