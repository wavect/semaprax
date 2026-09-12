use super::*;
use crate::live_invocation::fixture::StepClock;
use crate::live_invocation::journal::JournalEntry;

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
