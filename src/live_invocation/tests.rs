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
use super::journal::{self, JournalEntry};
use super::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationOutcome,
    LiveKernelError,
};
use super::model_invoke::{
    AuthorizationContext, AuthorizationGate, AuthorizationGrant, AuthorizationRefusal,
    BudgetRefusal, InvocationBudgetHook, InvocationUsage, ModelFailure, ModelInvocationOutcome,
    ModelInvocationRequest, ModelInvokeCapability, ProposalDecoder, ProposalOutcome,
    ReservedBudget,
};

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
    // Each assertion below flips exactly one seed field and restores it
    // before moving to the next, so this proves each of program_root,
    // deployment_policy and task individually participates in identity
    // (issue #108's "an event from a different task, deployment or
    // ProgramRoot cannot attach to the live chain" — enforced at the
    // journal level by `cross_invocation_turn_opened_is_rejected`, and
    // here at the identity-derivation level that makes that rejection
    // possible in the first place).
    let base_seed = LiveInvocationSeed {
        program_root: "sha256:".to_owned() + &"1".repeat(64),
        deployment_policy: "sha256:".to_owned() + &"2".repeat(64),
        task: b"fixture task".to_vec(),
        budget: 1000,
        interaction_schema_digest: SCHEMA_DIGEST.to_owned(),
        approved_providers: vec!["fixture-provider".into()],
    };
    let base = LiveInvocationId::derive(&base_seed);

    let mut different_task = base_seed.clone();
    different_task.task = b"a different task".to_vec();
    assert_ne!(base, LiveInvocationId::derive(&different_task));

    let mut different_program_root = base_seed.clone();
    different_program_root.program_root = "sha256:".to_owned() + &"9".repeat(64);
    assert_ne!(
        base,
        LiveInvocationId::derive(&different_program_root),
        "a different ProgramRoot must not attach to the same live chain"
    );

    let mut different_deployment = base_seed.clone();
    different_deployment.deployment_policy = "sha256:".to_owned() + &"8".repeat(64);
    assert_ne!(
        base,
        LiveInvocationId::derive(&different_deployment),
        "a different deployment/model policy must not attach to the same live chain"
    );

    assert_eq!(base, LiveInvocationId::derive(&base_seed));
}

#[test]
fn an_uncertain_intent_journal_is_refused_before_any_redispatch() {
    // A journal ending right after `RequestIntent`, with no recorded
    // response, is delivery-uncertain: the kernel must refuse to proceed —
    // including refusing to redispatch the same request — before any
    // further stage, store write, or host call (issue #108's explicit
    // requirement). `journal::tests::a_journal_ending_in_intent_is_uncertain_not_terminal`
    // proves the journal-level flag; this proves the kernel entry point
    // itself honours it rather than guessing a response.
    let identity = identity();
    let cfg = config(&identity, 3);
    let uncertain = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: identity.digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"1".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"2".repeat(64),
            reserved_budget: 10,
        },
    ];
    let capability = ModelInvokeCapability::grant("uncertain intent test");
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
    let result = run_live_invocation(&cfg, uncertain, &mut handlers, &AgentCancellation::new());
    assert_eq!(result.err(), Some(LiveKernelError::UncertainIntent));
    assert_eq!(
        handler.calls, 0,
        "an uncertain intent is never redispatched"
    );
}

#[test]
fn an_unresolved_mid_turn_prefix_is_refused_rather_than_guessed() {
    // A journal stuck mid-turn (here: after `ProposalAdmitted`, before its
    // `AuthorizationConsumed`) is neither terminal nor resumable in this
    // bounded kernel; per the contract it must return `UnresolvedPrefix`
    // rather than guess the missing entries.
    let identity = identity();
    let cfg = config(&identity, 3);
    let mid_turn = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: identity.digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"1".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"2".repeat(64),
            reserved_budget: 10,
        },
        JournalEntry::ResponseRecorded {
            turn: 0,
            response_digest: "sha256:".to_owned() + &"3".repeat(64),
            response: fixture_response(0, "already-settled"),
        },
        JournalEntry::ProposalAdmitted {
            turn: 0,
            proposal_digest: "sha256:".to_owned() + &"4".repeat(64),
        },
    ];
    let capability = ModelInvokeCapability::grant("unresolved prefix test");
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
    let result = run_live_invocation(&cfg, mid_turn, &mut handlers, &AgentCancellation::new());
    assert_eq!(result.err(), Some(LiveKernelError::UnresolvedPrefix));
    assert_eq!(handler.calls, 0);
}

#[test]
fn resuming_a_journal_after_continue_does_not_redispatch_the_completed_turn() {
    // A journal ending cleanly right after a `continue` Transition is
    // resumable: the next turn may open without redispatching anything
    // already recorded. This feeds `run_live_invocation` a hand-built
    // completed-turn-0 prefix and proves only turn 1 is dispatched, and
    // the resumed prefix is carried forward byte-for-byte.
    let identity = identity();
    let cfg = config(&identity, 2);
    let prior_turn = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: identity.digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"1".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"2".repeat(64),
            reserved_budget: 10,
        },
        JournalEntry::ResponseRecorded {
            turn: 0,
            response_digest: "sha256:".to_owned() + &"3".repeat(64),
            response: fixture_response(0, "already-settled"),
        },
        JournalEntry::ProposalAdmitted {
            turn: 0,
            proposal_digest: "sha256:".to_owned() + &"4".repeat(64),
        },
        JournalEntry::AuthorizationConsumed {
            turn: 0,
            grant_digest: "sha256:".to_owned() + &"5".repeat(64),
        },
        JournalEntry::Transition {
            turn: 0,
            case: "continue".to_owned(),
            carrier_digest: "sha256:".to_owned() + &"6".repeat(64),
        },
    ];

    let capability = ModelInvokeCapability::grant("resume test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(1, "b"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 2 };
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
    let run = run_live_invocation(
        &cfg,
        prior_turn.clone(),
        &mut handlers,
        &AgentCancellation::new(),
    )
    .unwrap();

    assert_eq!(
        run.dispatched, 1,
        "only the new turn is dispatched; turn 0 is not redispatched"
    );
    assert_eq!(handler.calls, 1);
    assert_eq!(
        &run.journal[..prior_turn.len()],
        &prior_turn[..],
        "the resumed prefix is carried forward unchanged"
    );
    assert!(matches!(run.outcome, LiveInvocationOutcome::Complete(_)));
}

/// Issue #177's required-evidence list names "timeout, cancellation,
/// capacity, and provider-error" as distinct scripted-provider cases.
/// `a_closed_model_failure_ends_the_attempt_without_decoding_or_authorizing`
/// already covers `ProviderError`; this drives the same assertion for every
/// other closed [`ModelFailure`] a handler can report on its own (everything
/// except `MalformedResponse`, which `an_oversized_response_is_treated_as_malformed_before_decode`
/// already exercises as a kernel-side rejection rather than a handler
/// report). Each variant must end the turn without ever reaching authorize,
/// and the journal must record the exact closed tag — never provider-shaped
/// detail — so a reviewer can tell which case fired without re-running the
/// handler.
fn assert_scripted_failure_ends_the_turn_before_authorize(failure: ModelFailure) {
    let identity = identity();
    let cfg = config(&identity, 3);
    let capability = ModelInvokeCapability::grant("closed failure taxonomy test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Failed {
        failure,
        attempted_bytes: 7,
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
    assert_eq!(
        gate.granted, 0,
        "a {failure:?} model failure never reaches authorize"
    );
    let recorded_failure = run.journal.iter().find_map(|entry| match entry {
        JournalEntry::ResponseFailed { failure, .. } => Some(failure.clone()),
        _ => None,
    });
    assert_eq!(
        recorded_failure.as_deref(),
        Some(failure.as_str()),
        "the journal names the exact closed tag, not provider-shaped detail"
    );
}

#[test]
fn a_timeout_failure_ends_the_attempt_without_decoding_or_authorizing() {
    assert_scripted_failure_ends_the_turn_before_authorize(ModelFailure::Timeout);
}

#[test]
fn a_refused_failure_ends_the_attempt_without_decoding_or_authorizing() {
    assert_scripted_failure_ends_the_turn_before_authorize(ModelFailure::Refused);
}

#[test]
fn a_handler_reported_capacity_exceeded_failure_ends_the_attempt_without_decoding_or_authorizing() {
    // Distinct from `a_refused_budget_reservation_fails_the_turn_without_dispatching_the_handler`:
    // that test refuses at `InvocationBudgetHook::reserve`, before the
    // handler is ever dispatched. This drives the case where the *provider
    // itself* reports no capacity, through `ModelHandler::invoke`, after
    // budget reservation already succeeded.
    assert_scripted_failure_ends_the_turn_before_authorize(ModelFailure::CapacityExceeded);
}

/// A [`super::kernel::TurnObserver`] that cancels the shared handle from
/// inside `observe`, so the kernel's second cancellation checkpoint ("After
/// `TurnOpened`, before committing `RequestIntent`") fires deterministically
/// without a race or a sleep.
struct CancelDuringObserve {
    cancellation: AgentCancellation,
}

impl super::kernel::TurnObserver for CancelDuringObserve {
    fn observe(&mut self, turn: u32) -> Vec<u8> {
        self.cancellation.cancel();
        format!("observation:{turn}").into_bytes()
    }
}

#[test]
fn cancellation_after_turn_opened_stops_cleanly_before_any_request_intent() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let cancellation = AgentCancellation::new();
    let capability = ModelInvokeCapability::grant("cancellation checkpoint 2 test");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = CancelDuringObserve {
        cancellation: cancellation.clone(),
    };
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
    let run = run_live_invocation(&cfg, Vec::new(), &mut handlers, &cancellation).unwrap();
    assert_eq!(run.outcome, LiveInvocationOutcome::Cancelled);
    assert_eq!(run.dispatched, 0);
    assert_eq!(handler.calls, 0);
    assert_eq!(
        run.journal.len(),
        1,
        "only TurnOpened was durable when cancellation was observed"
    );
    assert!(matches!(run.journal[0], JournalEntry::TurnOpened { .. }));
}

/// An [`InvocationBudgetHook`] that cancels the shared handle from inside
/// `reserve`, so the kernel's third cancellation checkpoint ("After
/// `RequestIntent` is committed, immediately before calling
/// `ModelHandler::invoke`") fires deterministically. Per the contract, this
/// checkpoint folds cancellation into the closed failure domain
/// (`ModelFailure::Cancelled`) rather than leaving an uncertain intent
/// behind, because the request is already durable at that point.
struct CancelDuringReserve {
    cancellation: AgentCancellation,
    inner: FixtureBudgetHook,
}

impl InvocationBudgetHook for CancelDuringReserve {
    fn reserve(
        &mut self,
        request: &ModelInvocationRequest,
    ) -> Result<ReservedBudget, BudgetRefusal> {
        self.cancellation.cancel();
        self.inner.reserve(request)
    }

    fn record(&mut self, usage: &InvocationUsage) {
        self.inner.record(usage);
    }
}

#[test]
fn cancellation_after_request_intent_is_committed_folds_into_a_recorded_cancelled_failure() {
    let identity = identity();
    let cfg = config(&identity, 3);
    let cancellation = AgentCancellation::new();
    let capability = ModelInvokeCapability::grant("cancellation checkpoint 3 test");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = CancelDuringReserve {
        cancellation: cancellation.clone(),
        inner: FixtureBudgetHook::new(10),
    };
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
    let run = run_live_invocation(&cfg, Vec::new(), &mut handlers, &cancellation).unwrap();
    // The request was already durable, so the turn must still resolve to a
    // recorded outcome rather than leaving an uncertain intent behind — this
    // is a `Fail`, never `Cancelled`, and the handler is never called.
    assert!(matches!(run.outcome, LiveInvocationOutcome::Fail(_)));
    assert_eq!(run.dispatched, 0, "checkpoint 3 fires before dispatch");
    assert_eq!(handler.calls, 0);
    let recorded_failure = run.journal.iter().find_map(|entry| match entry {
        JournalEntry::ResponseFailed { failure, .. } => Some(failure.clone()),
        _ => None,
    });
    assert_eq!(
        recorded_failure.as_deref(),
        Some(ModelFailure::Cancelled.as_str())
    );
    let validated = journal::validate(&run.journal, identity.digest()).unwrap();
    assert!(validated.terminal);
}

/// A [`ProposalDecoder`] that actually transforms the bytes it admits,
/// unlike [`FixtureProposalDecoder`] (which admits the response verbatim, so
/// decoded and raw bytes are indistinguishable in every other test here).
/// Strips the scripted `{"turn":<n>,"answer":<..>}` envelope down to just
/// the answer text, so a test can tell whether a downstream seam saw the
/// *decoded* value or the *raw response*.
struct StrippingProposalDecoder {
    schema_digest: String,
}

impl ProposalDecoder for StrippingProposalDecoder {
    fn schema_digest(&self) -> &str {
        &self.schema_digest
    }

    fn decode(&mut self, turn: u32, response: &[u8]) -> ProposalOutcome {
        let expected_prefix = format!("{{\"turn\":{turn},\"answer\":");
        let text = std::str::from_utf8(response).expect("fixture response is always utf8");
        let Some(rest) = text
            .strip_prefix(&expected_prefix)
            .and_then(|r| r.strip_suffix('}'))
        else {
            return ProposalOutcome::Refused("shape_mismatch".into());
        };
        ProposalOutcome::Admitted(rest.as_bytes().to_vec())
    }
}

/// An [`AuthorizationGate`] that records the exact `proposal_digest` it was
/// asked to authorize, so a test can compare it against an independently
/// recomputed digest of known bytes.
struct RecordingAuthorizationGate {
    seen_proposal_digest: Option<String>,
}

impl AuthorizationGate for RecordingAuthorizationGate {
    fn authorize(
        &mut self,
        context: &AuthorizationContext<'_>,
    ) -> Result<AuthorizationGrant, AuthorizationRefusal> {
        self.seen_proposal_digest = Some(context.proposal_digest.to_owned());
        Ok(AuthorizationGrant::new(
            "sha256:".to_owned() + &"7".repeat(64),
        ))
    }
}

#[test]
fn authorize_and_completion_see_the_decoded_proposal_never_the_raw_response_bytes() {
    // Issue #177's required evidence: "Decoded proposal is the only value
    // passed to authorize; raw model output never reaches effect dispatch."
    // Every other test's decoder happens to admit the response unchanged, so
    // decoded and raw bytes are byte-identical there and cannot distinguish
    // the two. This test's decoder actually transforms the bytes, then
    // checks both destinations a raw response could otherwise leak into:
    // the digest bound into `AuthorizationContext`, and the payload the
    // completed invocation returns.
    let identity = identity();
    let cfg = config(&identity, 1);
    let capability = ModelInvokeCapability::grant("decoded-not-raw test");
    let raw_response = fixture_response(0, "secret-raw-answer");
    let mut handler =
        FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(raw_response.clone())]);
    let mut decoder = StrippingProposalDecoder {
        schema_digest: SCHEMA_DIGEST.to_owned(),
    };
    let mut gate = RecordingAuthorizationGate {
        seen_proposal_digest: None,
    };
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
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

    let decoded_proposal = b"\"secret-raw-answer\"".to_vec();
    assert_ne!(
        decoded_proposal, raw_response,
        "the test is only meaningful if decode actually changed the bytes"
    );
    assert_eq!(
        run.outcome,
        LiveInvocationOutcome::Complete(decoded_proposal.clone()),
        "the reducer/completion payload is the decoded proposal, not the raw response"
    );
    let expected_digest = super::kernel::proposal_digest_for_test(&decoded_proposal);
    assert_eq!(
        gate.seen_proposal_digest.as_deref(),
        Some(expected_digest.as_str()),
        "authorize is bound to a digest of the decoded proposal"
    );
    let raw_response_digest = super::kernel::proposal_digest_for_test(&raw_response);
    assert_ne!(
        gate.seen_proposal_digest.as_deref(),
        Some(raw_response_digest.as_str()),
        "authorize must never be bound to a digest of the raw response"
    );
}
