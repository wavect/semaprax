use super::core::*;

/// A small, non-Agent fixture program: it is not tied to any six-role
/// AgentDefinition shape, and its `Request`/`Observation`/`CleanupOp` types
/// are ordinary crate-local types, never AgentDefinition-derived ones.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CounterState {
    turn: u32,
    acc: i64,
    /// A plain owned, non-`Copy` local (a growing log) that must transfer
    /// intact across every suspension boundary.
    log: Vec<String>,
}

/// The host observation value a handler returns to signal "fail this run".
const FAIL_SENTINEL: i64 = -1;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CounterProgram {
    total_turns: u32,
    /// If set, suspend (carrying the live state) the instant `turn` reaches
    /// this value, instead of continuing straight through to completion.
    suspend_at: Option<u32>,
}

impl ResumableEffectProgram for CounterProgram {
    type State = CounterState;
    type Result = i64;
    type Request = i64;
    type Observation = i64;
    type CleanupOp = String;

    fn request(&self, state: &CounterState) -> Option<i64> {
        if state.turn < self.total_turns {
            Some(state.turn as i64)
        } else {
            None
        }
    }

    fn transition(
        &self,
        state: &CounterState,
        observation: Option<&i64>,
    ) -> Step<CounterState, i64> {
        match observation {
            None => Step::Complete(state.acc),
            Some(obs) if *obs == FAIL_SENTINEL => Step::Fail(42),
            Some(obs) => {
                let mut next = state.clone();
                next.acc += obs;
                next.log.push(format!("turn-{}:{}", state.turn, obs));
                next.turn += 1;
                if self.suspend_at == Some(next.turn) {
                    Step::Suspend(next)
                } else if next.turn >= self.total_turns {
                    Step::Complete(next.acc)
                } else {
                    Step::Continue(next)
                }
            }
        }
    }

    fn cleanup_plan(&self, _state: &CounterState) -> Vec<String> {
        // Canonical runtime order: always this exact two-entry vector,
        // never sorted or reordered by the driver.
        vec!["flush_log".to_string(), "close_session".to_string()]
    }
}

fn scope(invocation_id: &str) -> EffectScope {
    EffectScope {
        program_root: "root:v1".to_string(),
        invocation_id: invocation_id.to_string(),
        policy_epoch: 7,
    }
}

fn initial() -> CounterState {
    CounterState {
        turn: 0,
        acc: 0,
        log: Vec::new(),
    }
}

/// Multiplies the request by ten and records every call it actually made,
/// in order — the fixture's "physical" host boundary.
struct MultiplyHandler {
    calls: Vec<i64>,
}
impl EffectHandler<i64, i64> for MultiplyHandler {
    fn dispatch(&mut self, request: &i64) -> Result<i64, String> {
        self.calls.push(*request);
        Ok(request * 10)
    }
}

/// Like [`MultiplyHandler`] but returns the fail sentinel for one chosen
/// request value, to drive the program into its `Fail` branch.
struct FailOnHandler {
    calls: Vec<i64>,
    fail_on: i64,
}
impl EffectHandler<i64, i64> for FailOnHandler {
    fn dispatch(&mut self, request: &i64) -> Result<i64, String> {
        self.calls.push(*request);
        if *request == self.fail_on {
            Ok(FAIL_SENTINEL)
        } else {
            Ok(request * 10)
        }
    }
}

/// Always fails, to exercise `HandlerFailed` and its replay.
struct ErrHandler;
impl EffectHandler<i64, i64> for ErrHandler {
    fn dispatch(&mut self, _request: &i64) -> Result<i64, String> {
        Err("host down".to_string())
    }
}

/// A handler that must never be called: wired into replay/rejection tests
/// to prove zero new physical dispatches occurred.
struct PanicIfCalledHandler;
impl EffectHandler<i64, i64> for PanicIfCalledHandler {
    fn dispatch(&mut self, request: &i64) -> Result<i64, String> {
        panic!("replay or a refused resume must never dispatch a new effect (request {request})");
    }
}

/// Records cleanup-op invocation order and can be told to fail one named
/// op, to prove sticky failure selection.
struct RecordingCleanup {
    order: Vec<String>,
    fail_on: Option<String>,
}
impl CleanupHandler<String> for RecordingCleanup {
    fn run(&mut self, op: &String) -> Result<(), String> {
        self.order.push(op.clone());
        if self.fail_on.as_deref() == Some(op.as_str()) {
            Err("cleanup boom".to_string())
        } else {
            Ok(())
        }
    }
}

fn no_cleanup_failure() -> RecordingCleanup {
    RecordingCleanup {
        order: Vec::new(),
        fail_on: None,
    }
}

// ---------------------------------------------------------------------
// Multiple yield points, in a loop, reaching Complete.
// ---------------------------------------------------------------------

#[test]
fn multiple_yield_points_across_turns_reach_complete_with_exactly_once_dispatch() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };
    let mut handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup = no_cleanup_failure();
    let (outcome, journal) = run(
        &program,
        scope("run-1"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .expect("three in-budget turns must complete");

    assert_eq!(outcome.terminal, Step::Complete(30)); // 0*10 + 1*10 + 2*10
    assert_ne!(outcome.terminal, Step::Fail(42)); // negative control
    assert_eq!(outcome.dispatched, 3);
    // Each turn's request was dispatched exactly once, in order — never
    // zero times, never twice.
    assert_eq!(handler.calls, vec![0, 1, 2]);
    // Cleanup ran, in exact canonical plan order, exactly once.
    assert_eq!(
        cleanup.order,
        vec!["flush_log".to_string(), "close_session".to_string()]
    );
    assert!(journal.validate(&scope("run-1")).is_ok());
}

// ---------------------------------------------------------------------
// A branch to Fail after a yield must never be confused with Complete.
// ---------------------------------------------------------------------

#[test]
fn branch_to_fail_after_yield_is_not_confused_with_complete() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };
    let mut handler = FailOnHandler {
        calls: Vec::new(),
        fail_on: 1,
    };
    let mut cleanup = no_cleanup_failure();
    let (outcome, _journal) = run(
        &program,
        scope("run-fail"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .expect("Fail is a normal, non-error terminal the driver call itself returns Ok for");

    match &outcome.terminal {
        Step::Fail(code) => assert_eq!(*code, 42),
        other => panic!("expected Fail(42), got {other:?}"),
    }
    assert_ne!(outcome.terminal, Step::Complete(0));
    // Cleanup still runs for a Fail terminal, same canonical order.
    assert_eq!(
        cleanup.order,
        vec!["flush_log".to_string(), "close_session".to_string()]
    );
}

// ---------------------------------------------------------------------
// Owned, non-Copy local state transfers intact across suspension.
// ---------------------------------------------------------------------

#[test]
fn owned_state_transfers_intact_across_a_suspension() {
    let program = CounterProgram {
        total_turns: 5,
        suspend_at: Some(2),
    };
    let mut handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup = no_cleanup_failure();
    let (outcome, _journal) = run(
        &program,
        scope("owned"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .expect("suspend is a normal terminal");

    match outcome.terminal {
        Step::Suspend(state) => {
            assert_eq!(state.turn, 2);
            assert_eq!(state.acc, 10); // 0*10 + 1*10
            assert_eq!(
                state.log,
                vec!["turn-0:0".to_string(), "turn-1:10".to_string()]
            );
        }
        other => panic!("expected Suspend carrying the owned log, got {other:?}"),
    }
    // Suspend never runs cleanup: the computation's resources stay live.
    assert!(cleanup.order.is_empty());
}

// ---------------------------------------------------------------------
// Replay is not re-execution: a fully replayed run dispatches zero new
// effects, whether it replays to Complete or back to the same Suspend.
// ---------------------------------------------------------------------

#[test]
fn replay_of_a_completed_run_makes_zero_new_effect_dispatches() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };
    let mut handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup = no_cleanup_failure();
    let (outcome1, journal1) = run(
        &program,
        scope("replay"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap();
    assert_eq!(outcome1.dispatched, 3);

    let mut panic_handler = PanicIfCalledHandler;
    let mut cleanup2 = no_cleanup_failure();
    let (outcome2, _journal2) = resume(
        &program,
        scope("replay"),
        journal1,
        initial(),
        10,
        &mut panic_handler,
        &mut cleanup2,
        &|| false,
    )
    .expect("replaying an already-complete journal must succeed without dispatching");

    assert_eq!(outcome2.dispatched, 0);
    assert_eq!(outcome2.terminal, outcome1.terminal);
}

#[test]
fn replay_of_a_suspended_run_reproduces_the_same_suspension_with_zero_dispatch() {
    let program = CounterProgram {
        total_turns: 5,
        suspend_at: Some(2),
    };
    let mut handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup = no_cleanup_failure();
    let (outcome1, journal1) = run(
        &program,
        scope("replay-suspend"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap();
    assert!(matches!(outcome1.terminal, Step::Suspend(_)));

    let mut panic_handler = PanicIfCalledHandler;
    let mut cleanup2 = no_cleanup_failure();
    let (outcome2, _journal2) = resume(
        &program,
        scope("replay-suspend"),
        journal1,
        initial(),
        10,
        &mut panic_handler,
        &mut cleanup2,
        &|| false,
    )
    .expect("replaying a suspended journal must succeed without dispatching");
    assert_eq!(outcome2.dispatched, 0);
    assert_eq!(outcome2.terminal, outcome1.terminal);
}

#[test]
fn resume_from_a_truncated_journal_only_dispatches_the_new_tail() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };
    let mut handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup = no_cleanup_failure();
    let (outcome_full, journal_full) = run(
        &program,
        scope("partial"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap();
    assert_eq!(outcome_full.dispatched, 3);
    assert_eq!(handler.calls, vec![0, 1, 2]);

    // Simulate a crash immediately after turn 0's Transition was durably
    // committed but before turn 1 ever began: only the first three entries
    // (turn 0's Intent/Observed/Transition) survive.
    let truncated: Vec<_> = journal_full.entries().iter().take(3).cloned().collect();
    assert_eq!(truncated.len(), 3);
    let partial = Journal::from_entries(truncated);

    let mut resuming_handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup2 = no_cleanup_failure();
    let (outcome2, _journal2) = resume(
        &program,
        scope("partial"),
        partial,
        initial(),
        10,
        &mut resuming_handler,
        &mut cleanup2,
        &|| false,
    )
    .expect("resume from a valid partial prefix must succeed");

    // Turn 0 was replayed (zero dispatch); only turns 1 and 2 are new
    // attempts with their own accounting.
    assert_eq!(resuming_handler.calls, vec![1, 2]);
    assert_eq!(outcome2.dispatched, 2);
    assert_eq!(outcome2.terminal, outcome_full.terminal);
}

#[test]
fn replaying_a_recorded_failed_dispatch_never_recontacts_the_host() {
    let program = CounterProgram {
        total_turns: 2,
        suspend_at: None,
    };
    let mut handler = ErrHandler;
    let mut cleanup = no_cleanup_failure();
    let (err, journal) = run(
        &program,
        scope("failed-effect"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(
        err,
        DriverError::HandlerFailed {
            turn: 0,
            reason: "host down".to_string(),
        }
    );

    // Resuming the exact same failed journal must report the same failure
    // again, deterministically, without a second physical dispatch.
    let mut panic_handler = PanicIfCalledHandler;
    let mut cleanup2 = no_cleanup_failure();
    let (err2, _journal2) = resume(
        &program,
        scope("failed-effect"),
        journal,
        initial(),
        10,
        &mut panic_handler,
        &mut cleanup2,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(err2, err);
}

// ---------------------------------------------------------------------
// Sticky failure: cleanup can never replace the already-selected status.
// ---------------------------------------------------------------------

#[test]
fn cleanup_failure_never_overrides_the_already_selected_terminal_status() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };

    // Fail case: cleanup's first entry itself fails.
    let mut handler = FailOnHandler {
        calls: Vec::new(),
        fail_on: 0,
    };
    let mut cleanup = RecordingCleanup {
        order: Vec::new(),
        fail_on: Some("flush_log".to_string()),
    };
    let (outcome, _journal) = run(
        &program,
        scope("sticky-fail"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .expect("Fail is a normal terminal even though a cleanup op fails");
    assert_eq!(outcome.terminal, Step::Fail(42));
    assert_eq!(
        outcome.cleanup,
        vec![
            ("flush_log".to_string(), Err("cleanup boom".to_string())),
            ("close_session".to_string(), Ok(())),
        ]
    );
    assert_ne!(outcome.terminal, Step::Complete(0)); // negative control

    // Complete case: a different cleanup entry fails; Complete still stands.
    let mut handler2 = MultiplyHandler { calls: Vec::new() };
    let mut cleanup2 = RecordingCleanup {
        order: Vec::new(),
        fail_on: Some("close_session".to_string()),
    };
    let (outcome2, _journal2) = run(
        &program,
        scope("sticky-complete"),
        initial(),
        10,
        &mut handler2,
        &mut cleanup2,
        &|| false,
    )
    .expect("Complete is a normal terminal even though a cleanup op fails");
    assert_eq!(outcome2.terminal, Step::Complete(30));
    assert_eq!(
        outcome2.cleanup,
        vec![
            ("flush_log".to_string(), Ok(())),
            ("close_session".to_string(), Err("cleanup boom".to_string())),
        ]
    );
    assert_ne!(outcome2.terminal, Step::Fail(42)); // negative control
}

// ---------------------------------------------------------------------
// Journal validation: specific, distinct rejections.
// ---------------------------------------------------------------------

fn completed_journal(invocation_id: &str) -> Journal<CounterProgram> {
    let program = CounterProgram {
        total_turns: 2,
        suspend_at: None,
    };
    let mut handler = MultiplyHandler { calls: Vec::new() };
    let mut cleanup = no_cleanup_failure();
    let (_outcome, journal) = run(
        &program,
        scope(invocation_id),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap();
    journal
}

#[test]
fn validate_rejects_a_stale_program_root_specifically() {
    let journal = completed_journal("valid");
    let mut bad = scope("valid");
    bad.program_root = "root:v2".to_string();
    let err = journal.validate(&bad).unwrap_err();
    assert_eq!(err, JournalError::StaleProgramRoot { at: 0 });
    assert_ne!(err, JournalError::WrongInvocation { at: 0 });
    assert_ne!(err, JournalError::WrongPolicyEpoch { at: 0 });
}

#[test]
fn validate_rejects_a_wrong_invocation_specifically() {
    let journal = completed_journal("valid");
    let mut bad = scope("valid");
    bad.invocation_id = "someone-elses-invocation".to_string();
    let err = journal.validate(&bad).unwrap_err();
    assert_eq!(err, JournalError::WrongInvocation { at: 0 });
    assert_ne!(err, JournalError::StaleProgramRoot { at: 0 });
    assert_ne!(err, JournalError::WrongPolicyEpoch { at: 0 });
}

#[test]
fn validate_rejects_a_wrong_policy_epoch_specifically() {
    let journal = completed_journal("valid");
    let mut bad = scope("valid");
    bad.policy_epoch = 999;
    let err = journal.validate(&bad).unwrap_err();
    assert_eq!(err, JournalError::WrongPolicyEpoch { at: 0 });
    assert_ne!(err, JournalError::StaleProgramRoot { at: 0 });
    assert_ne!(err, JournalError::WrongInvocation { at: 0 });
}

#[test]
fn validate_rejects_an_unterminated_intent_as_uncertain() {
    let entries = vec![JournalEntry::<CounterProgram>::Intent {
        turn: 0,
        scope: scope("uncertain"),
        request: 0,
    }];
    let journal = Journal::from_entries(entries);
    let err = journal.validate(&scope("uncertain")).unwrap_err();
    assert_eq!(err, JournalError::UnterminatedIntent);
}

#[test]
fn validate_rejects_a_tampered_request_mismatch() {
    let entries = vec![
        JournalEntry::<CounterProgram>::Intent {
            turn: 0,
            scope: scope("tamper"),
            request: 0,
        },
        JournalEntry::<CounterProgram>::Observed {
            turn: 0,
            scope: scope("tamper"),
            request: 1, // tampered: does not match the Intent's 0
            observation: 0,
        },
    ];
    let journal = Journal::from_entries(entries);
    let err = journal.validate(&scope("tamper")).unwrap_err();
    assert_eq!(err, JournalError::RequestMismatch { at: 1 });
}

#[test]
fn validate_rejects_an_entry_after_terminal() {
    let entries = vec![
        JournalEntry::<CounterProgram>::Transition {
            turn: 0,
            scope: scope("after"),
            step: Step::Complete(0),
        },
        JournalEntry::<CounterProgram>::Transition {
            turn: 1,
            scope: scope("after"),
            step: Step::Complete(0),
        },
    ];
    let journal = Journal::from_entries(entries);
    let err = journal.validate(&scope("after")).unwrap_err();
    assert_eq!(err, JournalError::EntryAfterTerminal { at: 1 });
}

// ---------------------------------------------------------------------
// A journal is not a bearer token: resuming it under a different scope,
// tampered request, or tampered transition is refused before any dispatch.
// ---------------------------------------------------------------------

#[test]
fn resume_refuses_a_journal_presented_under_a_different_invocation_scope() {
    let journal = completed_journal("orig");
    let program = CounterProgram {
        total_turns: 2,
        suspend_at: None,
    };
    let mut panic_handler = PanicIfCalledHandler;
    let mut cleanup = no_cleanup_failure();
    let (err, _journal) = resume(
        &program,
        scope("stolen"),
        journal,
        initial(),
        10,
        &mut panic_handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(
        err,
        DriverError::Journal(JournalError::WrongInvocation { at: 0 })
    );
}

#[test]
fn resume_rejects_a_replayed_request_that_disagrees_with_recomputation() {
    let program = CounterProgram {
        total_turns: 2,
        suspend_at: None,
    };
    let journal = completed_journal("drift-request");
    let mut entries = journal.entries().to_vec();
    // Tamper the turn-0 Intent and its paired Observed to the same wrong
    // value, so `Journal::validate`'s pairing check still passes but the
    // recorded request no longer matches what `program.request` actually
    // computes for turn 0 (which is 0, not 999).
    match &mut entries[0] {
        JournalEntry::Intent { request, .. } => *request = 999,
        other => panic!("expected Intent at index 0, got {other:?}"),
    }
    match &mut entries[1] {
        JournalEntry::Observed { request, .. } => *request = 999,
        other => panic!("expected Observed at index 1, got {other:?}"),
    }
    let tampered = Journal::from_entries(entries);
    assert!(tampered.validate(&scope("drift-request")).is_ok());

    let mut panic_handler = PanicIfCalledHandler;
    let mut cleanup = no_cleanup_failure();
    let (err, _journal) = resume(
        &program,
        scope("drift-request"),
        tampered,
        initial(),
        10,
        &mut panic_handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(err, DriverError::RequestDrift { turn: 0 });
}

#[test]
fn resume_rejects_a_replayed_transition_that_disagrees_with_recomputation() {
    let program = CounterProgram {
        total_turns: 2,
        suspend_at: None,
    };
    let journal = completed_journal("drift-transition");
    let mut entries = journal.entries().to_vec();
    // Tamper turn 0's recorded Continue state without changing its
    // terminal-ness, so `Journal::validate` still accepts the structure and
    // the mismatch is only caught by the driver recomputing the transition.
    match &mut entries[2] {
        JournalEntry::Transition { step, .. } => match step {
            Step::Continue(state) => state.acc = 9999,
            other => panic!("expected a Continue step, got {other:?}"),
        },
        other => panic!("expected Transition at index 2, got {other:?}"),
    }
    let tampered = Journal::from_entries(entries);
    assert!(tampered.validate(&scope("drift-transition")).is_ok());

    let mut panic_handler = PanicIfCalledHandler;
    let mut cleanup = no_cleanup_failure();
    let (err, _journal) = resume(
        &program,
        scope("drift-transition"),
        tampered,
        initial(),
        10,
        &mut panic_handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(err, DriverError::TransitionDrift { turn: 0 });
}

// ---------------------------------------------------------------------
// Cancellation and budgets are checked before any new dispatch.
// ---------------------------------------------------------------------

#[test]
fn cancellation_is_checked_before_any_dispatch() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };
    let mut handler = PanicIfCalledHandler;
    let mut cleanup = no_cleanup_failure();
    let (err, _journal) = run(
        &program,
        scope("cancel"),
        initial(),
        10,
        &mut handler,
        &mut cleanup,
        &|| true,
    )
    .unwrap_err();
    assert_eq!(err, DriverError::Cancelled);
}

#[test]
fn budget_is_exhausted_before_any_dispatch_when_max_turns_is_zero() {
    let program = CounterProgram {
        total_turns: 3,
        suspend_at: None,
    };
    let mut handler = PanicIfCalledHandler;
    let mut cleanup = no_cleanup_failure();
    let (err, _journal) = run(
        &program,
        scope("budget"),
        initial(),
        0,
        &mut handler,
        &mut cleanup,
        &|| false,
    )
    .unwrap_err();
    assert_eq!(err, DriverError::BudgetExhausted);
}
