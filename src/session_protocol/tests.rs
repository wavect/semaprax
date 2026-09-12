//! Every illegal-sequence category issue #206 requires, each asserted
//! against its own *specific* [`ProtocolError`]/[`SpecError`]/[`CheckpointError`]
//! variant -- never a generic "protocol error" -- paired with the
//! corresponding legal sequence succeeding, and cross-checked so a test can
//! never pass merely because two confusable variants render similarly (see
//! the crate doc's `Reason::InstanceTemplateChanged` warning: a comparison
//! that can never actually distinguish its two cases is a defect, not a
//! test).

use std::panic::{self, AssertUnwindSafe};

use super::capability::*;
use super::duality::*;
use super::engine::*;
use super::protocols::{model_stream_protocol, resource_transaction_protocol};
use super::spec::*;

impl Endpoint {
    /// Test-only: acknowledge that this test is intentionally done with a
    /// live, nonterminal endpoint (its scenario already asserted the error
    /// it set out to prove) without running the abandonment drop bomb.
    /// Never exported outside this crate -- the public `Endpoint` stays
    /// fully affine.
    #[cfg(test)]
    fn discard_for_test(mut self) {
        self.defused = true;
    }
}

#[derive(Default)]
struct RecordingCleanup {
    calls: Vec<(String, StateId, &'static str)>,
}
impl CleanupHandler for RecordingCleanup {
    fn run(&mut self, session_id: &str, terminal_state: StateId, op: &'static str) -> Result<(), String> {
        self.calls.push((session_id.to_string(), terminal_state, op));
        Ok(())
    }
}

#[derive(Default)]
struct FailingCleanup {
    calls: Vec<&'static str>,
}
impl CleanupHandler for FailingCleanup {
    fn run(&mut self, _session_id: &str, _terminal_state: StateId, op: &'static str) -> Result<(), String> {
        self.calls.push(op);
        Err(format!("cleanup op '{op}' failed"))
    }
}

fn assert_debug_excludes(err: &impl std::fmt::Debug, confusable_variant_names: &[&str]) {
    let rendered = format!("{err:?}");
    for name in confusable_variant_names {
        assert!(
            !rendered.contains(name),
            "expected {rendered:?} not to mention confusable variant {name:?}"
        );
    }
}

// ---------------------------------------------------------------------
// Static spec validation.
// ---------------------------------------------------------------------

mod spec_validation {
    use super::*;
    use std::collections::BTreeSet;

    fn base() -> ProtocolSpec {
        ProtocolSpec {
            name: "test-v1",
            states: BTreeSet::from(["A", "B"]),
            initial: "A",
            terminal: BTreeSet::from(["B"]),
            transitions: vec![
                Transition {
                    from: "A",
                    label: "go",
                    kind: Kind::Send,
                    payload_type: "Unit",
                    required_capability: None,
                    ownership: OwnershipMove::None,
                    next: Next::Then("B"),
                },
                // A's own required escape (see `SpecError::MissingEscape`):
                // every nonterminal state needs at least one declared
                // Cancel/Timeout/Fail transition.
                Transition {
                    from: "A",
                    label: "abort",
                    kind: Kind::Cancel,
                    payload_type: "Unit",
                    required_capability: None,
                    ownership: OwnershipMove::None,
                    next: Next::Then("B"),
                },
            ],
            cleanup: vec![("B", vec!["done"])],
        }
    }

    #[test]
    fn well_formed_spec_validates() {
        assert_eq!(base().validate(), Ok(()));
    }

    #[test]
    fn both_applied_protocols_validate() {
        assert_eq!(
            model_stream_protocol().validate(),
            Ok(()),
            "model_stream_protocol must be well-formed"
        );
        assert_eq!(
            resource_transaction_protocol().validate(),
            Ok(()),
            "resource_transaction_protocol must be well-formed"
        );
    }

    #[test]
    fn dead_end_state_is_rejected() {
        let mut spec = base();
        spec.states.insert("C");
        // C is nonterminal and has no outgoing transition at all.
        let errors = spec.validate().unwrap_err();
        assert!(errors.contains(&SpecError::DeadEnd { state: "C" }));
        assert!(!errors.contains(&SpecError::MissingEscape { state: "C" }));
    }

    #[test]
    fn missing_escape_is_distinct_from_dead_end() {
        let mut spec = base();
        spec.states.insert("C");
        spec.transitions.push(Transition {
            from: "C",
            label: "progress",
            kind: Kind::Send,
            payload_type: "Unit",
            required_capability: None,
            ownership: OwnershipMove::None,
            next: Next::Then("B"),
        });
        // C now has an outgoing transition, but none of them is an escape
        // (Cancel/Timeout/Fail): the DeadEnd rule is satisfied, but the
        // MissingEscape rule is not, and the two must be told apart.
        let errors = spec.validate().unwrap_err();
        assert!(errors.contains(&SpecError::MissingEscape { state: "C" }));
        assert!(!errors.contains(&SpecError::DeadEnd { state: "C" }));
    }

    #[test]
    fn terminal_state_with_outgoing_transition_is_rejected() {
        let mut spec = base();
        spec.transitions.push(Transition {
            from: "B",
            label: "oops",
            kind: Kind::Send,
            payload_type: "Unit",
            required_capability: None,
            ownership: OwnershipMove::None,
            next: Next::Then("A"),
        });
        let errors = spec.validate().unwrap_err();
        assert!(errors.contains(&SpecError::TerminalStateHasOutgoingTransition {
            state: "B",
            label: "oops"
        }));
    }

    #[test]
    fn unknown_next_state_is_rejected() {
        let mut spec = base();
        spec.transitions[0].next = Next::Then("Nowhere");
        let errors = spec.validate().unwrap_err();
        assert!(errors.contains(&SpecError::UnknownState {
            label: "go",
            state: "Nowhere"
        }));
    }

    #[test]
    fn single_choice_branch_is_rejected() {
        let mut spec = base();
        spec.transitions[0].next = Next::Choice(vec![("only", "B")]);
        let errors = spec.validate().unwrap_err();
        assert!(errors.contains(&SpecError::ChoiceNeedsAtLeastTwo {
            state: "A",
            label: "go"
        }));
    }

    #[test]
    fn duplicate_choice_label_is_rejected() {
        let mut spec = base();
        spec.states.insert("C");
        spec.transitions[0].next = Next::Choice(vec![("x", "B"), ("x", "C")]);
        spec.terminal.insert("C");
        spec.cleanup.push(("C", vec![]));
        let errors = spec.validate().unwrap_err();
        assert!(errors.contains(&SpecError::DuplicateChoiceLabel {
            state: "A",
            label: "go",
            choice: "x"
        }));
    }

    #[test]
    fn missing_cleanup_entry_is_rejected() {
        let mut spec = base();
        spec.cleanup.clear();
        let errors = spec.validate().unwrap_err();
        assert_eq!(errors, vec![SpecError::MissingCleanupEntry { state: "B" }]);
    }
}

// ---------------------------------------------------------------------
// The compile-time affine layer: reaching this file at all proves the
// module builds; the crate doc `compile_fail` doctests prove the two
// specific compile-time rejections (grant-for-wrong-capability, and
// double-move of a consumed `Endpoint`). This runtime test additionally
// proves the *drop bomb* half of "affine + no abandoned endpoint": an
// endpoint that is legally driven to a terminal state never panics on
// drop, but one abandoned mid-protocol always does.
// ---------------------------------------------------------------------

#[test]
fn abandoned_nonterminal_endpoint_panics_on_drop() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("abandon-1").unwrap();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        drop(endpoint);
    }));
    let err = result.expect_err("dropping a live nonterminal endpoint must panic");
    let message = err
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        message.contains("abandoned"),
        "panic message should name the abandonment invariant, got: {message}"
    );
}

#[test]
fn cancelled_endpoint_does_not_panic_on_drop() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("cancel-clean-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "abandon", "Unit", None, None, None, &mut cleanup)
        .expect("Idle -> Cancelled via abandon is legal");
    match outcome {
        AdvanceOutcome::Terminal(endpoint, terminal) => {
            assert_eq!(terminal.terminal_state, "Cancelled");
            assert_eq!(terminal.terminal_kind, Kind::Cancel);
            drop(endpoint); // terminal: must not panic.
        }
        AdvanceOutcome::Live(_) => panic!("expected a terminal outcome"),
    }
}

// ---------------------------------------------------------------------
// Duplicate open.
// ---------------------------------------------------------------------

#[test]
fn duplicate_open_is_refused_distinctly() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let first = table.open("dup-1").expect("first open is legal");
    let err = table.open("dup-1").expect_err("second open of the same id must be refused");
    assert_eq!(
        err,
        ProtocolError::DuplicateOpen {
            session_id: "dup-1".to_string()
        }
    );
    assert_debug_excludes(&err, &["UseAfterTerminal", "StaleHandle", "IllegalTransition"]);
    first.discard_for_test();
}

#[test]
fn duplicate_open_is_refused_even_after_the_first_session_closed() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("dup-2").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "abandon", "Unit", None, None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Terminal(endpoint, _) = outcome else {
        panic!("expected terminal")
    };
    drop(endpoint);
    let err = table.open("dup-2").expect_err("id stays claimed once opened, even after closing");
    assert_eq!(
        err,
        ProtocolError::DuplicateOpen {
            session_id: "dup-2".to_string()
        }
    );
}

// ---------------------------------------------------------------------
// Out-of-order operation / illegal transition, paired with the legal
// sequence succeeding.
// ---------------------------------------------------------------------

#[test]
fn out_of_order_operation_is_refused_distinctly() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("order-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    // Endpoint is in Idle; "close" is only legal from Closing.
    let (err, endpoint) = table
        .advance(endpoint, "close", "StreamClose", Some("stream.close"), None, None, &mut cleanup)
        .expect_err("close is not legal from Idle");
    assert_eq!(
        err,
        ProtocolError::IllegalTransition {
            session_id: "order-1".to_string(),
            state: "Idle",
            label: "close",
        }
    );
    assert_debug_excludes(&err, &["MissingAuthority", "UseAfterTerminal", "UnknownBranchChoice"]);

    // Legal-sequence control: the correct next message, from the same
    // still-live endpoint the failed attempt handed back, succeeds.
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .expect("open is legal from Idle");
    match outcome {
        AdvanceOutcome::Live(endpoint) => {
            assert_eq!(endpoint.state(), "Streaming");
            endpoint.discard_for_test();
        }
        AdvanceOutcome::Terminal(..) => panic!("open must not reach a terminal state"),
    }
}

// ---------------------------------------------------------------------
// Use of a stale handle: a duplicate carrying an earlier generation.
// ---------------------------------------------------------------------

#[test]
fn stale_handle_is_refused_distinctly_from_use_after_terminal() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("stale-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .expect("open is legal");
    let AdvanceOutcome::Live(current) = outcome else {
        panic!("expected a live outcome")
    };
    assert_eq!(current.state(), "Streaming");
    assert_eq!(current.session_id(), "stale-1");

    // A duplicate handle reflecting the *pre-advance* generation/state --
    // exactly what a snapshot taken before the advance (then replayed
    // after) would carry. This is not reachable by an ordinary second move
    // of `current`; it models the crate doc's named risk ("serializing
    // endpoints can recreate authority") directly via the crate-internal
    // constructor tests are allowed to use.
    let stale = Endpoint {
        session_id: "stale-1".to_string(),
        state: "Idle",
        generation: 0,
        is_terminal: false,
        defused: false,
    };
    let (err, replacement) = table
        .advance(stale, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .expect_err("a stale generation must be refused");
    assert_eq!(
        err,
        ProtocolError::StaleHandle {
            session_id: "stale-1".to_string(),
            presented_generation: 0,
            current_generation: 1,
        }
    );
    assert_debug_excludes(&err, &["UseAfterTerminal", "DuplicateOpen", "IllegalTransition"]);
    replacement.discard_for_test();

    // Legal-sequence control: the genuinely current endpoint can still
    // proceed legally.
    let outcome = table
        .advance(current, "cancel", "Unit", None, None, None, &mut cleanup)
        .expect("cancel is legal from Streaming");
    let AdvanceOutcome::Terminal(endpoint, terminal) = outcome else {
        panic!("expected terminal")
    };
    assert_eq!(terminal.terminal_state, "Cancelled");
    drop(endpoint);
}

#[test]
fn use_after_terminal_is_refused_distinctly_from_stale_handle() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("terminal-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "abandon", "Unit", None, None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Terminal(closed, _) = outcome else {
        panic!("expected terminal")
    };
    assert!(closed.is_terminal());

    let (err, replacement) = table
        .advance(closed, "abandon", "Unit", None, None, None, &mut cleanup)
        .expect_err("no operation is legal after a session reached a terminal state");
    assert_eq!(
        err,
        ProtocolError::UseAfterTerminal {
            session_id: "terminal-1".to_string(),
            state: "Cancelled",
        }
    );
    assert_debug_excludes(&err, &["StaleHandle", "IllegalTransition", "DuplicateOpen"]);
    // The engine still reports this replacement endpoint as terminal
    // (the session really is closed), so dropping it is not abandonment.
    assert!(replacement.is_terminal());
    drop(replacement);
}

// ---------------------------------------------------------------------
// Wrong branch choice, paired with both legal choices succeeding.
// ---------------------------------------------------------------------

#[test]
fn unrecognized_branch_choice_is_refused_distinctly() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("branch-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };

    let (err, endpoint) = table
        .advance(
            endpoint,
            "end",
            "StreamEnd",
            None,
            Some("not_a_real_choice"),
            None,
            &mut cleanup,
        )
        .expect_err("an unrecognized branch choice must be refused");
    assert_eq!(
        err,
        ProtocolError::UnknownBranchChoice {
            session_id: "branch-1".to_string(),
            state: "Streaming",
            label: "end",
            choice: Some("not_a_real_choice"),
        }
    );
    assert_debug_excludes(&err, &["IllegalTransition", "MissingAuthority"]);

    // Legal-sequence control: both declared choices succeed and land in
    // their declared distinct states.
    let outcome = table
        .advance(endpoint, "end", "StreamEnd", None, Some("graceful"), None, &mut cleanup)
        .expect("the declared 'graceful' choice is legal");
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("Closing is nonterminal")
    };
    assert_eq!(endpoint.state(), "Closing");
    let outcome = table
        .advance(endpoint, "close", "StreamClose", Some("stream.close"), None, None, &mut cleanup)
        .unwrap();
    assert!(matches!(outcome, AdvanceOutcome::Terminal(ref e, ref t) if e.state() == "Closed" && t.terminal_state == "Closed"));
}

#[test]
fn missing_branch_choice_is_refused() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("branch-2").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };
    let (err, endpoint) = table
        .advance(endpoint, "end", "StreamEnd", None, None, None, &mut cleanup)
        .expect_err("a branch transition requires a choice");
    assert_eq!(
        err,
        ProtocolError::UnknownBranchChoice {
            session_id: "branch-2".to_string(),
            state: "Streaming",
            label: "end",
            choice: None,
        }
    );
    endpoint.discard_for_test();
}

// ---------------------------------------------------------------------
// Payload type mismatch, paired with the correct payload succeeding.
// ---------------------------------------------------------------------

#[test]
fn payload_type_mismatch_is_refused_distinctly() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("payload-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let (err, endpoint) = table
        .advance(endpoint, "open", "WrongPayload", Some("stream.open"), None, None, &mut cleanup)
        .expect_err("the wrong payload tag must be refused");
    assert_eq!(
        err,
        ProtocolError::PayloadTypeMismatch {
            session_id: "payload-1".to_string(),
            label: "open",
            expected: "StreamRequest",
            presented: "WrongPayload",
        }
    );
    assert_debug_excludes(&err, &["MissingAuthority", "IllegalTransition"]);

    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .expect("the declared payload tag is legal");
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };
    endpoint.discard_for_test();
}

// ---------------------------------------------------------------------
// A protocol state is not authority: the exact right state, in the exact
// right order, is refused when the required capability is absent or
// wrong -- and the SAME sequence, differing only in the presented
// capability, succeeds.
// ---------------------------------------------------------------------

#[test]
fn missing_authority_is_refused_even_in_correct_order() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("auth-1").unwrap();
    let mut cleanup = RecordingCleanup::default();

    // Correct state (Idle), correct label ("open"), correct payload -- the
    // sequence is legal. Only the capability is missing.
    let (err, endpoint) = table
        .advance(endpoint, "open", "StreamRequest", None, None, None, &mut cleanup)
        .expect_err("order alone must not grant authority");
    assert_eq!(
        err,
        ProtocolError::MissingAuthority {
            session_id: "auth-1".to_string(),
            label: "open",
            required: "stream.open",
        }
    );
    assert_debug_excludes(&err, &["IllegalTransition", "PayloadTypeMismatch"]);

    // Presenting a real but WRONG capability is refused the same specific
    // way, not confused with "no capability at all".
    let (err2, endpoint) = table
        .advance(
            endpoint,
            "open",
            "StreamRequest",
            Some("stream.close"), // a real capability name, just the wrong one
            None,
            None,
            &mut cleanup,
        )
        .expect_err("the wrong capability must also be refused");
    assert_eq!(err2, err, "wrong-capability and no-capability are the same MissingAuthority reason");

    // Legal-sequence control: identical state/order/payload, correct
    // capability now presented, succeeds.
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .expect("the right capability, in the right order, is legal");
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };
    assert_eq!(endpoint.state(), "Streaming");
    endpoint.discard_for_test();
}

// ---------------------------------------------------------------------
// Ownership: resource-token consumption, paired with a normal commit
// succeeding, and a duplicate/cloned token id being refused even under a
// wholly separate, otherwise-legal session.
// ---------------------------------------------------------------------

#[test]
fn commit_without_a_resource_token_is_refused() {
    let spec = resource_transaction_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("txn-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "begin", "BeginTxn", Some("txn.begin"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };
    let (err, endpoint) = table
        .advance(endpoint, "commit", "Commit", Some("txn.commit"), None, None, &mut cleanup)
        .expect_err("commit consumes a resource token; none was presented");
    assert_eq!(
        err,
        ProtocolError::ResourceTokenRequired {
            session_id: "txn-1".to_string(),
            label: "commit",
        }
    );
    endpoint.discard_for_test();
}

#[test]
fn a_consumed_resource_token_cannot_be_reused_even_by_a_different_session() {
    let spec = resource_transaction_protocol();
    let mut table = SessionTable::new(&spec);
    let token = ResourceToken {
        id: "write-lock-42".to_string(),
    };
    let mut cleanup = RecordingCleanup::default();

    // Session A legitimately commits, consuming the token.
    let a = table.open("txn-a").unwrap();
    let outcome = table
        .advance(a, "begin", "BeginTxn", Some("txn.begin"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(a) = outcome else {
        panic!("expected live")
    };
    let outcome = table
        .advance(a, "commit", "Commit", Some("txn.commit"), None, Some(&token), &mut cleanup)
        .expect("committing with a fresh token is legal");
    let AdvanceOutcome::Terminal(a, terminal) = outcome else {
        panic!("commit reaches a terminal state")
    };
    assert_eq!(terminal.terminal_state, "Committed");
    drop(a);

    // Session B is an entirely separate, otherwise wholly legal session
    // that happens to present a CLONE of the same physical token id (the
    // "serializing endpoints/tokens can recreate authority" hazard the
    // crate doc names). It must still be refused, even though B's own
    // protocol order and capability are perfectly legal.
    let b = table.open("txn-b").unwrap();
    let outcome = table
        .advance(b, "begin", "BeginTxn", Some("txn.begin"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(b) = outcome else {
        panic!("expected live")
    };
    let cloned_token = token.clone();
    let (err, b) = table
        .advance(b, "commit", "Commit", Some("txn.commit"), None, Some(&cloned_token), &mut cleanup)
        .expect_err("a previously-consumed token id must be refused even for a fresh session");
    assert_eq!(
        err,
        ProtocolError::ResourceAlreadyConsumed {
            session_id: "txn-b".to_string(),
            label: "commit",
            token: "write-lock-42".to_string(),
        }
    );
    assert_debug_excludes(&err, &["ResourceTokenRequired", "MissingAuthority"]);

    // Legal-sequence control: B can still legally roll back instead.
    let outcome = table
        .advance(b, "rollback", "Unit", None, None, None, &mut cleanup)
        .expect("rollback never touches the resource token");
    let AdvanceOutcome::Terminal(b, terminal) = outcome else {
        panic!("rollback reaches a terminal state")
    };
    assert_eq!(terminal.terminal_state, "RolledBack");
    drop(b);
}

// ---------------------------------------------------------------------
// Checkpoint/resume: refused while a Call is in flight, allowed once
// settled.
// ---------------------------------------------------------------------

#[test]
fn checkpoint_is_refused_while_a_call_is_in_flight_and_allowed_once_settled() {
    let spec = resource_transaction_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("chk-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "begin", "BeginTxn", Some("txn.begin"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };

    // Settled (Open, no pending call): checkpoint is allowed.
    let checkpoint = table.checkpoint(&endpoint).expect("Open with no pending call is checkpointable");
    assert_eq!(checkpoint.state, "Open");

    let outcome = table
        .advance(endpoint, "read", "ReadOp", Some("txn.read"), None, None, &mut cleanup)
        .expect("read is legal from Open");
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("AwaitingRead is nonterminal")
    };
    assert_eq!(endpoint.state(), "AwaitingRead");

    let err = table
        .checkpoint(&endpoint)
        .expect_err("a session with an unresolved Call must refuse checkpoint");
    assert_eq!(err, CheckpointError::InFlightCall { label: "read" });
    assert_debug_excludes(&err, &["StaleHandle", "UnknownSession"]);

    // Resolve the call; checkpoint is allowed again.
    let outcome = table
        .advance(endpoint, "read_result", "ReadResult", None, None, None, &mut cleanup)
        .expect("read_result resolves the pending call");
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("Open is nonterminal")
    };
    let checkpoint = table
        .checkpoint(&endpoint)
        .expect("no call is pending once read_result is recorded");
    assert_eq!(checkpoint.state, "Open");
    endpoint.discard_for_test();
}

// ---------------------------------------------------------------------
// Cancellation, timeout, and remote failure are explicit transitions with
// their own declared cleanup, never an implicit exception path -- and a
// terminal status, once selected, is sticky: a cleanup op's own failure
// never overwrites it.
// ---------------------------------------------------------------------

#[test]
fn cancellation_runs_its_declared_cleanup_in_exact_order() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("cancel-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };
    let outcome = table
        .advance(endpoint, "cancel", "Unit", None, None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Terminal(endpoint, terminal) = outcome else {
        panic!("cancel reaches a terminal state")
    };
    assert_eq!(terminal.terminal_kind, Kind::Cancel);
    assert_eq!(terminal.terminal_state, "Cancelled");
    assert_eq!(
        terminal.cleanup,
        vec![("release_socket", Ok(()))],
        "cleanup must run in exactly the declared order"
    );
    drop(endpoint);
}

#[test]
fn timeout_reaches_its_own_distinct_uncertain_terminal_state() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("timeout-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };
    let outcome = table
        .advance(endpoint, "timeout", "Unit", None, None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Terminal(endpoint, terminal) = outcome else {
        panic!("timeout reaches a terminal state")
    };
    // Timeout must land somewhere distinct from an explicit Cancel: the
    // remote side's actual status after a timeout is uncertain, not known
    // to be cleanly cancelled.
    assert_eq!(terminal.terminal_state, "Uncertain");
    assert_ne!(terminal.terminal_state, "Cancelled");
    assert_eq!(terminal.terminal_kind, Kind::Timeout);
    assert_eq!(terminal.cleanup, vec![("mark_uncertain_for_reconciliation", Ok(()))]);
    drop(endpoint);
}

#[test]
fn remote_failure_terminal_status_is_sticky_against_cleanup_failure() {
    let spec = model_stream_protocol();
    let mut table = SessionTable::new(&spec);
    let endpoint = table.open("fail-1").unwrap();
    let mut cleanup = RecordingCleanup::default();
    let outcome = table
        .advance(endpoint, "open", "StreamRequest", Some("stream.open"), None, None, &mut cleanup)
        .unwrap();
    let AdvanceOutcome::Live(endpoint) = outcome else {
        panic!("expected live")
    };

    let mut failing_cleanup = FailingCleanup::default();
    let outcome = table
        .advance(endpoint, "fail", "Unit", None, None, None, &mut failing_cleanup)
        .expect("the Fail transition itself is legal even though its cleanup will error");
    let AdvanceOutcome::Terminal(endpoint, terminal) = outcome else {
        panic!("fail reaches a terminal state")
    };

    // Every cleanup op ran, in the exact declared order (alphabetically
    // "emit_failure_report" < "release_socket", the opposite of the
    // declared order -- proving nothing sorted it), and every one failed.
    assert_eq!(
        failing_cleanup.calls,
        vec!["release_socket", "emit_failure_report"],
        "cleanup must run in declared order, never sorted"
    );
    assert_eq!(
        terminal.cleanup,
        vec![
            ("release_socket", Err("cleanup op 'release_socket' failed".to_string())),
            ("emit_failure_report", Err("cleanup op 'emit_failure_report' failed".to_string())),
        ]
    );
    // Failure selection is sticky: cleanup's own failure is recorded, but
    // it never replaces the terminal status the Fail transition selected.
    assert_eq!(terminal.terminal_kind, Kind::Fail);
    assert_eq!(terminal.terminal_state, "Failed");
    drop(endpoint);
}

// ---------------------------------------------------------------------
// Duality / compatibility: a matched client/server pair is compatible; a
// pair that differs only in required capability is specifically flagged
// as a capability divergence, not a generic mismatch.
// ---------------------------------------------------------------------

fn ping_client() -> ProtocolSpec {
    use std::collections::BTreeSet;
    ProtocolSpec {
        name: "ping-client-v1",
        states: BTreeSet::from(["Idle", "AwaitingPong", "Done"]),
        initial: "Idle",
        terminal: BTreeSet::from(["Done"]),
        transitions: vec![
            Transition {
                from: "Idle",
                label: "ping",
                kind: Kind::Send,
                payload_type: "Ping",
                required_capability: Some("net.ping"),
                ownership: OwnershipMove::None,
                next: Next::Then("AwaitingPong"),
            },
            Transition {
                from: "AwaitingPong",
                label: "pong",
                kind: Kind::Receive,
                payload_type: "Pong",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Done"),
            },
            Transition {
                from: "Idle",
                label: "cancel",
                kind: Kind::Cancel,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Done"),
            },
            Transition {
                from: "AwaitingPong",
                label: "cancel",
                kind: Kind::Cancel,
                payload_type: "Unit",
                required_capability: None,
                ownership: OwnershipMove::None,
                next: Next::Then("Done"),
            },
        ],
        cleanup: vec![("Done", vec![])],
    }
}

fn ping_server_dual_of(client: &ProtocolSpec) -> ProtocolSpec {
    let mut server = client.clone();
    server.name = "ping-server-v1";
    for t in &mut server.transitions {
        t.kind = match t.kind {
            Kind::Send => Kind::Receive,
            Kind::Receive => Kind::Send,
            other => other,
        };
    }
    server
}

#[test]
fn a_dual_client_server_pair_is_compatible() {
    let client = ping_client();
    let server = ping_server_dual_of(&client);
    assert_eq!(check_duality(&client, &server), Ok(()));
}

#[test]
fn capability_divergence_is_refused_distinctly_from_a_generic_mismatch() {
    let client = ping_client();
    let mut server = ping_server_dual_of(&client);
    // Diverge ONLY the capability the server-side "ping" transition
    // requires; label, state, kind-complementarity, and payload all still
    // match exactly.
    for t in &mut server.transitions {
        if t.label == "ping" {
            t.required_capability = Some("net.ping.v2");
        }
    }
    let errors = check_duality(&client, &server).expect_err("capability divergence must be flagged");
    assert!(errors.iter().any(|e| matches!(
        e,
        DualityError::CapabilityDivergence {
            state: "Idle",
            label: "ping",
            a: Some("net.ping"),
            b: Some("net.ping.v2"),
        }
    )));
    for e in &errors {
        assert!(
            !matches!(e, DualityError::MissingCounterpart { .. } | DualityError::KindNotComplementary { .. }),
            "a pure capability divergence must not also render as a structural mismatch: {e:?}"
        );
    }
}

#[test]
fn missing_counterpart_is_refused_distinctly_from_capability_divergence() {
    let client = ping_client();
    let mut server = ping_server_dual_of(&client);
    server.transitions.retain(|t| t.label != "pong");
    let errors = check_duality(&client, &server).expect_err("a one-sided transition must be flagged");
    assert!(errors
        .iter()
        .any(|e| matches!(e, DualityError::MissingCounterpart { state: "AwaitingPong", label: "pong" })));
    for e in &errors {
        assert!(
            !matches!(e, DualityError::CapabilityDivergence { .. }),
            "a missing counterpart must not also render as a capability divergence: {e:?}"
        );
    }
}

// ---------------------------------------------------------------------
// The compile-time capability layer (see also the crate doc
// `compile_fail` doctest): a Grant for one capability is a distinct Rust
// type from a Grant for another, so this repository's "state/receipt is
// not authority" invariant is enforced by `rustc` for this concrete
// shape, not only by the runtime `MissingAuthority` check above.
// ---------------------------------------------------------------------

#[test]
fn a_grant_for_the_declared_capability_type_checks_and_runs() {
    fn commit(_grant: &Grant<CommitCapability>) {}
    let commit_grant: Grant<CommitCapability> = Grant::issue();
    commit(&commit_grant);
}
