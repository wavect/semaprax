use super::*;
use std::cell::Cell;
use std::rc::Rc;

use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::fixture::{
    fixture_response, FixtureAuthorizationGate, FixtureObserver, FixturePolicy,
    FixtureProposalDecoder, StepClock,
};
use crate::live_invocation::identity::{LiveInvocationId, LiveInvocationSeed};
use crate::live_invocation::journal::{self, JournalEntry};
use crate::live_invocation::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationOutcome,
    TurnEffect,
};
use crate::live_invocation::model_invoke::{
    AuthorizationContext, AuthorizationGate, AuthorizationGrant, AuthorizationRefusal,
    ModelFailure, ModelHandler, ModelInvocationOutcome, ModelInvokeCapability, ProposalDecoder,
    ProposalOutcome,
};

fn request(effective_budget: i64) -> ModelInvocationRequest {
    ModelInvocationRequest {
        turn: 0,
        task: b"task".to_vec(),
        observation: b"observation".to_vec(),
        proposal_grammar_digest: "sha256:".to_owned() + &"a".repeat(64),
        deployment_binding: "sha256:deploy".to_owned(),
        max_response_bytes: 4096,
        effective_budget,
    }
}

fn usage(response_bytes: usize, failed: bool) -> InvocationUsage {
    InvocationUsage {
        turn: 0,
        request_bytes: 11,
        response_bytes,
        failed,
    }
}

// --- Boundary table: zero, exact-limit, limit-plus-one, for the ceiling. ---

#[test]
fn a_zero_amount_request_is_admitted_and_commits_nothing() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    let reserved = ledger.reserve(&request(0)).expect("zero always fits");
    assert_eq!(reserved.amount, 0);
    assert_eq!(ledger.committed(), 0);
    assert_eq!(ledger.remaining(), 100);
}

#[test]
fn a_request_at_exactly_the_remaining_ceiling_is_admitted_and_exhausts_it() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    let reserved = ledger
        .reserve(&request(100))
        .expect("exact fit is admitted");
    assert_eq!(reserved.amount, 100);
    assert_eq!(ledger.committed(), 100);
    assert_eq!(ledger.remaining(), 0);
}

#[test]
fn a_request_one_unit_past_the_remaining_ceiling_is_refused_and_commits_nothing() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    let refusal = ledger.reserve(&request(101)).unwrap_err();
    assert_eq!(refusal, BudgetRefusal(BUDGET_EXHAUSTED.to_owned()));
    assert_eq!(
        ledger.committed(),
        0,
        "a refused request must not commit anything at all"
    );
    assert_eq!(ledger.remaining(), 100);
}

#[test]
fn two_reservations_that_individually_fit_but_together_overrun_the_ceiling() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    ledger.reserve(&request(60)).expect("first attempt fits");
    assert_eq!(ledger.remaining(), 40);
    let refusal = ledger.reserve(&request(41)).unwrap_err();
    assert_eq!(refusal, BudgetRefusal(BUDGET_EXHAUSTED.to_owned()));
    // The first reservation's 60 stays committed; it is never given back
    // just because a later attempt was refused.
    assert_eq!(ledger.committed(), 60);
    let admitted = ledger.reserve(&request(40)).expect("exactly what remains");
    assert_eq!(admitted.amount, 40);
    assert_eq!(ledger.remaining(), 0);
}

#[test]
fn a_negative_request_is_refused_before_any_commit() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    let refusal = ledger.reserve(&request(-1)).unwrap_err();
    assert_eq!(refusal, BudgetRefusal(NEGATIVE_REQUEST.to_owned()));
    assert_eq!(ledger.committed(), 0);
}

#[test]
fn an_overflowing_request_refuses_rather_than_panics() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    let refusal = ledger.reserve(&request(i64::MAX)).unwrap_err();
    assert_eq!(refusal, BudgetRefusal(BUDGET_EXHAUSTED.to_owned()));
    assert_eq!(ledger.committed(), 0);
    // A second, in-bounds request still works normally afterward — the
    // failed attempt at `i64::MAX` did not corrupt the ledger's state.
    let reserved = ledger.reserve(&request(5)).expect("ledger still usable");
    assert_eq!(reserved.amount, 5);
}

// --- `record` never refunds. ---

#[test]
fn record_never_reduces_committed_even_when_actual_usage_is_far_smaller() {
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    ledger.reserve(&request(90)).expect("admitted");
    ledger.record(&usage(1, false)); // settled using almost nothing
    assert_eq!(
        ledger.committed(),
        90,
        "actual usage never credits the reservation back"
    );
    assert_eq!(ledger.remaining(), 10);
}

#[test]
fn record_never_reduces_committed_on_a_failed_attempt_either() {
    // "Timeout with unknown billing conservatively retains reservation and
    // never appears as zero usage" (issue #113's required case): the
    // reservation the attempt already consumed stays consumed regardless of
    // what — if anything — `record` is later told.
    let mut clock = StepClock::new(0);
    let mut ledger = CumulativeBudgetLedger::new(100, &mut clock);
    ledger.reserve(&request(90)).expect("admitted");
    ledger.record(&usage(0, true));
    assert_eq!(ledger.committed(), 90);
    assert_eq!(ledger.usage().len(), 1);
    assert!(ledger.usage()[0].failed);
}

// --- The deadline is a distinct, non-budget, non-cancellation refusal. ---

#[test]
fn a_request_before_the_deadline_is_admitted() {
    let mut clock = StepClock::new(1_000);
    let mut ledger = CumulativeBudgetLedger::with_deadline(100, 2_000, &mut clock);
    let reserved = ledger.reserve(&request(10)).expect("before the deadline");
    assert_eq!(reserved.amount, 10);
}

#[test]
fn a_request_exactly_at_the_deadline_is_refused_as_deadline_exceeded_not_budget_exhausted() {
    let mut clock = StepClock::new(2_000);
    let mut ledger = CumulativeBudgetLedger::with_deadline(100, 2_000, &mut clock);
    let refusal = ledger.reserve(&request(10)).unwrap_err();
    assert_eq!(
        refusal,
        BudgetRefusal(DEADLINE_EXCEEDED.to_owned()),
        "a deadline refusal must never be reported as budget exhaustion"
    );
    assert_eq!(
        ledger.committed(),
        0,
        "a deadline refusal reserves nothing, exactly like a budget refusal"
    );
}

#[test]
fn a_deadline_refusal_and_a_budget_refusal_are_distinguishable_by_reason_text() {
    assert_ne!(BUDGET_EXHAUSTED, DEADLINE_EXCEEDED);
    let mut clock_a = StepClock::new(0);
    let mut over_budget = CumulativeBudgetLedger::new(5, &mut clock_a);
    assert_eq!(
        over_budget.reserve(&request(10)).unwrap_err(),
        BudgetRefusal(BUDGET_EXHAUSTED.to_owned())
    );
    let mut clock_b = StepClock::new(500);
    let mut past_deadline = CumulativeBudgetLedger::with_deadline(1_000, 100, &mut clock_b);
    assert_eq!(
        past_deadline.reserve(&request(1)).unwrap_err(),
        BudgetRefusal(DEADLINE_EXCEEDED.to_owned())
    );
}

// --- The fault-injection test: nonrefundable across a simulated crash. ---

#[test]
fn resuming_after_a_simulated_crash_never_refunds_the_already_committed_reservation() {
    // The exact shape a real crash leaves behind: `RequestIntent` is durable
    // (the kernel always persists it before dispatch — `kernel.rs`'s
    // `persist` call immediately after pushing it), but there is no
    // `ResponseRecorded`/`ResponseFailed` yet, because the process stopped
    // somewhere between committing the reservation and the model handler's
    // eventual answer. `journal::validate` already refuses to redispatch a
    // journal ending in this exact shape (`uncertain_intent`); this test
    // proves the budget ledger independently refuses to un-charge it either,
    // which is the property that makes a later, out-of-band-reconciled
    // retry unable to double-spend the same 90 units twice.
    let journal_so_far = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: "sha256:".to_owned() + &"1".repeat(64),
            observation_digest: "sha256:".to_owned() + &"2".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"3".repeat(64),
            reserved_budget: 90,
        },
    ];
    let mut clock = StepClock::new(0);
    let mut resumed = CumulativeBudgetLedger::resume(100, None, &journal_so_far, &mut clock);
    assert_eq!(
        resumed.committed(),
        90,
        "the crashed attempt's reservation is recovered from the journal, not lost"
    );
    assert_eq!(resumed.remaining(), 10);

    // A fresh attempt (the retry a caller issues after reconciling the
    // uncertain intent out of band) asking for anything more than the 10
    // that actually remains must be refused — proving the 90 already spent
    // was never quietly put back just because it never got a recorded
    // response.
    let refusal = resumed.reserve(&request(20)).unwrap_err();
    assert_eq!(refusal, BudgetRefusal(BUDGET_EXHAUSTED.to_owned()));
    assert_eq!(resumed.committed(), 90, "still exactly 90, not refunded");

    // Exactly what remains is still available.
    let admitted = resumed.reserve(&request(10)).expect("exactly what remains");
    assert_eq!(admitted.amount, 10);
    assert_eq!(resumed.remaining(), 0);
}

#[test]
fn resume_sums_every_prior_reservation_across_multiple_completed_turns() {
    let journal_so_far = vec![
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"3".repeat(64),
            reserved_budget: 30,
        },
        JournalEntry::RequestIntent {
            turn: 1,
            request_digest: "sha256:".to_owned() + &"4".repeat(64),
            reserved_budget: 25,
        },
        // A budget-refused attempt still recorded a `RequestIntent` with
        // `reserved_budget: 0` (see `kernel.rs`); it contributes nothing
        // extra, which is correct — it never actually reserved anything.
        JournalEntry::RequestIntent {
            turn: 2,
            request_digest: "sha256:".to_owned() + &"5".repeat(64),
            reserved_budget: 0,
        },
    ];
    let mut clock = StepClock::new(0);
    let resumed = CumulativeBudgetLedger::resume(100, None, &journal_so_far, &mut clock);
    assert_eq!(resumed.committed(), 55);
    assert_eq!(resumed.remaining(), 45);
}

#[test]
fn resume_preserves_an_absolute_deadline_across_the_same_simulated_crash() {
    // The deadline is caller-supplied on every call (including a resumed
    // one), so `resume` simply carries the SAME absolute instant forward —
    // there is no separate "resume the deadline" computation for a caller
    // to get wrong or reset.
    let journal_so_far = vec![JournalEntry::RequestIntent {
        turn: 0,
        request_digest: "sha256:".to_owned() + &"3".repeat(64),
        reserved_budget: 10,
    }];
    let mut clock = StepClock::new(2_000);
    let mut resumed = CumulativeBudgetLedger::resume(100, Some(1_000), &journal_so_far, &mut clock);
    let refusal = resumed.reserve(&request(1)).unwrap_err();
    assert_eq!(
        refusal,
        BudgetRefusal(DEADLINE_EXCEEDED.to_owned()),
        "the original deadline (1_000) is still enforced after resume, not silently dropped"
    );
}

const KERNEL_SCHEMA: &str =
    "sha256:00000000000000000000000000000000000000000000000000000000000000aa";

struct SharedClock(Rc<Cell<i64>>);

impl InvocationClock for SharedClock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}

struct TimedHandler {
    clock: Rc<Cell<i64>>,
    settle_at: i64,
    outcome: Option<ModelInvocationOutcome>,
    calls: usize,
}

impl ModelHandler for TimedHandler {
    fn invoke(
        &mut self,
        _capability: &ModelInvokeCapability,
        _request: &ModelInvocationRequest,
    ) -> ModelInvocationOutcome {
        self.calls += 1;
        self.clock.set(self.settle_at);
        self.outcome.take().expect("one scripted model attempt")
    }
}

struct CountingDecoder {
    inner: FixtureProposalDecoder,
    calls: usize,
}

impl ProposalDecoder for CountingDecoder {
    fn schema_digest(&self) -> &str {
        self.inner.schema_digest()
    }

    fn decode(&mut self, turn: u32, response: &[u8]) -> ProposalOutcome {
        self.calls += 1;
        self.inner.decode(turn, response)
    }
}

struct TimedGate {
    inner: FixtureAuthorizationGate,
    clock: Rc<Cell<i64>>,
    grant_at: Option<i64>,
}

impl AuthorizationGate for TimedGate {
    fn authorize(
        &mut self,
        context: &AuthorizationContext<'_>,
    ) -> Result<AuthorizationGrant, AuthorizationRefusal> {
        let grant = self.inner.authorize(context);
        if grant.is_ok() {
            if let Some(at) = self.grant_at {
                self.clock.set(at);
            }
        }
        grant
    }
}

struct TimedEffect {
    clock: Rc<Cell<i64>>,
    settle_at: Option<i64>,
    calls: usize,
}

impl TurnEffect for TimedEffect {
    fn call(&mut self, _turn: u32, _grant_digest: &str) -> Result<Vec<u8>, String> {
        self.calls += 1;
        if let Some(at) = self.settle_at {
            self.clock.set(at);
        }
        Ok(b"observed".to_vec())
    }
}

struct DeadlineEvidence {
    journal: Vec<JournalEntry>,
    outcome: LiveInvocationOutcome,
    committed: i64,
    usage: Vec<InvocationUsage>,
    dispatched: usize,
    decoded: usize,
    grants: usize,
    effects: usize,
}

fn run_with_deadline(
    settle_at: i64,
    outcome: ModelInvocationOutcome,
    grant_at: Option<i64>,
    effect_at: Option<i64>,
) -> DeadlineEvidence {
    let identity = LiveInvocationId::derive(&LiveInvocationSeed {
        program_root: "sha256:".to_owned() + &"1".repeat(64),
        deployment_policy: "sha256:".to_owned() + &"2".repeat(64),
        task: b"deadline task".to_vec(),
        budget: 100,
        interaction_schema_digest: KERNEL_SCHEMA.to_owned(),
        approved_providers: vec!["fixture-provider".into()],
    });
    let config = LiveInvocationConfig {
        identity: &identity,
        task: b"deadline task",
        deployment_binding: "sha256:fixture-deployment",
        interaction_schema_digest: KERNEL_SCHEMA,
        max_turns: 1,
        max_response_bytes: 4096,
        requested_budget_per_turn: 10,
    };
    let time = Rc::new(Cell::new(0));
    let mut clock = SharedClock(Rc::clone(&time));
    let mut ledger = CumulativeBudgetLedger::with_deadline(100, 500, &mut clock);
    let capability = ModelInvokeCapability::grant("deadline fixture");
    let mut handler = TimedHandler {
        clock: Rc::clone(&time),
        settle_at,
        outcome: Some(outcome),
        calls: 0,
    };
    let mut decoder = CountingDecoder {
        inner: FixtureProposalDecoder::new(KERNEL_SCHEMA),
        calls: 0,
    };
    let mut gate = TimedGate {
        inner: FixtureAuthorizationGate::new(1),
        clock: Rc::clone(&time),
        grant_at,
    };
    let mut effect = TimedEffect {
        clock: Rc::clone(&time),
        settle_at: effect_at,
        calls: 0,
    };
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    let run = {
        let mut handlers = LiveInvocationHandlers {
            capability: &capability,
            handler: &mut handler,
            decoder: &mut decoder,
            gate: &mut gate,
            budget: &mut ledger,
            observer: &mut observer,
            policy: &mut policy,
            effect: Some(&mut effect),
            sink: None,
        };
        let run = run_live_invocation(
            &config,
            Vec::new(),
            &mut handlers,
            &AgentCancellation::new(),
        )
        .expect("bounded kernel run");
        assert!(
            journal::validate(&run.journal, identity.digest())
                .expect("deadline path has a valid causal journal")
                .terminal
        );
        let replay = run_live_invocation(
            &config,
            run.journal.clone(),
            &mut handlers,
            &AgentCancellation::new(),
        )
        .expect("terminal replay");
        assert_eq!(replay.dispatched, 0);
        run
    };
    assert_eq!(handler.calls, 1);
    DeadlineEvidence {
        journal: run.journal,
        outcome: run.outcome,
        committed: ledger.committed(),
        usage: ledger.usage().to_vec(),
        dispatched: run.dispatched,
        decoded: decoder.calls,
        grants: gate.inner.granted,
        effects: effect.calls,
    }
}

#[test]
fn a_settled_model_response_at_the_deadline_is_charged_but_never_decoded() {
    let response = fixture_response(0, "late");
    let evidence = run_with_deadline(
        500,
        ModelInvocationOutcome::Settled(response.clone()),
        None,
        None,
    );
    assert_eq!(
        evidence.outcome,
        LiveInvocationOutcome::Fail(b"deadline_exceeded".to_vec())
    );
    assert_eq!(evidence.dispatched, 1);
    assert_eq!(evidence.committed, 10);
    assert_eq!(
        evidence.usage,
        vec![InvocationUsage {
            turn: 0,
            request_bytes: b"observation:0".len(),
            response_bytes: response.len(),
            failed: true,
        }]
    );
    assert_eq!(
        (evidence.decoded, evidence.grants, evidence.effects),
        (0, 0, 0)
    );
    assert!(evidence.journal.iter().any(|entry| matches!(entry,
        JournalEntry::ResponseFailed { failure, attempted_bytes, .. }
            if failure == DEADLINE_EXCEEDED && *attempted_bytes == response.len()
    )));
    assert!(!evidence
        .journal
        .iter()
        .any(|entry| matches!(entry, JournalEntry::ResponseRecorded { .. })));
}

#[test]
fn a_settled_model_response_just_before_the_deadline_can_finish() {
    let response = fixture_response(0, "on-time");
    let evidence = run_with_deadline(
        499,
        ModelInvocationOutcome::Settled(response.clone()),
        None,
        None,
    );
    assert_eq!(evidence.outcome, LiveInvocationOutcome::Complete(response));
    assert_eq!(
        (evidence.decoded, evidence.grants, evidence.effects),
        (1, 1, 1)
    );
    assert_eq!(evidence.committed, 10);
    assert_eq!(evidence.usage.len(), 1);
    assert!(!evidence.usage[0].failed);
}

#[test]
fn a_provider_failure_remains_the_selected_failure_even_if_time_has_elapsed() {
    let evidence = run_with_deadline(
        500,
        ModelInvocationOutcome::Failed {
            failure: ModelFailure::ProviderError,
            attempted_bytes: 7,
        },
        None,
        None,
    );
    assert_eq!(
        evidence.outcome,
        LiveInvocationOutcome::Fail(b"model_call_failed".to_vec())
    );
    assert_eq!(evidence.committed, 10);
    assert_eq!(
        evidence.usage,
        vec![InvocationUsage {
            turn: 0,
            request_bytes: b"observation:0".len(),
            response_bytes: 7,
            failed: true,
        }]
    );
    assert_eq!(
        (evidence.decoded, evidence.grants, evidence.effects),
        (0, 0, 0)
    );
    assert!(evidence.journal.iter().any(|entry| matches!(entry,
        JournalEntry::ResponseFailed { failure, attempted_bytes: 7, .. }
            if failure == ModelFailure::ProviderError.as_str()
    )));
}

#[test]
fn a_deadline_crossed_after_authorization_closes_effect_intent_without_dispatch() {
    let evidence = run_with_deadline(
        499,
        ModelInvocationOutcome::Settled(fixture_response(0, "ok")),
        Some(500),
        None,
    );
    assert_eq!(
        evidence.outcome,
        LiveInvocationOutcome::Fail(b"deadline_exceeded".to_vec())
    );
    assert_eq!(
        (evidence.decoded, evidence.grants, evidence.effects),
        (1, 1, 0)
    );
    assert!(evidence
        .journal
        .iter()
        .any(|entry| matches!(entry, JournalEntry::EffectIntent { .. })));
    assert!(evidence.journal.iter().any(|entry| matches!(entry,
        JournalEntry::EffectFailed { reason, .. } if reason == DEADLINE_EXCEEDED
    )));
}

#[test]
fn an_effect_that_settles_after_the_deadline_is_observed_but_cannot_publish_a_result() {
    let evidence = run_with_deadline(
        499,
        ModelInvocationOutcome::Settled(fixture_response(0, "ok")),
        None,
        Some(500),
    );
    assert_eq!(
        evidence.outcome,
        LiveInvocationOutcome::Fail(b"deadline_exceeded".to_vec())
    );
    assert_eq!(
        (evidence.decoded, evidence.grants, evidence.effects),
        (1, 1, 1)
    );
    assert!(evidence
        .journal
        .iter()
        .any(|entry| matches!(entry, JournalEntry::EffectObserved { .. })));
    assert!(evidence.journal.iter().any(|entry| matches!(entry,
        JournalEntry::TerminalOutcome { case, .. } if case == "fail"
    )));
}
