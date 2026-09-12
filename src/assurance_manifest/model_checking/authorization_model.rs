//! Model A: per-turn Agent authorization, effect dispatch, and checkpoint
//! recovery after a crash.
//!
//! This is a deliberately small, self-contained *projection*, not a hook
//! into the live Agent runtime: it does not call, construct, or import
//! anything from `agent_lifecycle`, `agent_runtime_v2`, or `live_invocation`
//! (those modules are outside this tranche's file lease). Its vocabulary is
//! chosen to mirror the real one, though: an `Authorize` event corresponds
//! to `agent_lifecycle::authorization::run_authorize_stage` minting a
//! one-use `Authorized` grant; `Dispatch` corresponds to consuming that
//! grant for the one host call; `Crash` and `RecoverAfterCrash` correspond
//! to a journal ending in `JournalEvent::Intent` and
//! `agent_runtime_v2::checkpoint::RecoveryDisposition::UncertainIntent`;
//! and `RefuseRedispatch` corresponds to `docs/AGENT-OPERATION-CHECKPOINT-V2.md`'s
//! "it never automatically dispatches that effect again."
//!
//! Two invariants are checked over every reachable state:
//!
//! - `dispatch_has_fresh_matching_grant`: a state reached through a
//!   `Dispatch` (or anything downstream of one) must record that the grant
//!   it consumed is the most recently, and only, minted one.
//! - `no_uncertain_redispatch`: once a dispatch has crashed (its outcome is
//!   unknown), the model must never reach `Dispatched` again while still
//!   carrying that same crashed grant — a fresh `Authorize` is required
//!   first.
//!
//! See [`docs/BOUNDED-MODEL-CHECKING-V1.md`](../../../docs/BOUNDED-MODEL-CHECKING-V1.md)
//! "Model A" for the full state diagram and the exact scope this
//! projection does and does not claim.

use super::digest::ModelDescriptor;
use super::engine::{Bounds, TransitionSystem};

pub const NAME: &str = "agent_turn_authorization_dispatch_checkpoint";
pub const VERSION: &str = "v1";

pub const INVARIANTS: &[&str] = &[
    "dispatch_has_fresh_matching_grant",
    "no_uncertain_redispatch",
];

pub const TERMINAL_PHASES: &[&str] = &["Completed", "Failed", "RecoveryRefused"];

pub const DESCRIPTOR: ModelDescriptor = ModelDescriptor {
    name: NAME,
    version: VERSION,
    invariants: INVARIANTS,
    terminal_states: TERMINAL_PHASES,
};

/// Bounds sized generously above this model's actual (small, finite) state
/// space, so a `Verified` result reflects true closure rather than the
/// bound happening to match the space exactly. See the module test
/// `correct_model_state_space_is_well_under_its_declared_bounds`.
pub const BOUNDS: Bounds = Bounds {
    max_states: 64,
    max_depth: 16,
    max_transitions: 128,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Phase {
    Idle,
    Authorized,
    Dispatched,
    ObservedSuccess,
    ObservedFailure,
    Completed,
    Failed,
    Crashed,
    RecoveringUncertain,
    RecoveryRefused,
}

/// One reachable turn state. `authorize_count` is the number of times
/// `Authorize` has fired (monotonic; this model mints at most one grant per
/// lifecycle). `dispatch_authorize` names which authorization the current
/// or most recent dispatch consumed. `crash_authorize` is set the moment a
/// dispatch crashes and is never cleared within one lifecycle (there is no
/// event in this projection that starts a second turn), which is exactly
/// what lets `no_uncertain_redispatch` compare "the grant that crashed"
/// against "the grant a later dispatch tries to reuse".
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct State {
    pub phase: Phase,
    pub authorize_count: u8,
    pub dispatch_authorize: Option<u8>,
    pub crash_authorize: Option<u8>,
}

impl State {
    const fn initial() -> Self {
        Self {
            phase: Phase::Idle,
            authorize_count: 0,
            dispatch_authorize: None,
            crash_authorize: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Event {
    Authorize,
    Dispatch,
    HostSucceeds,
    HostFails,
    Reduce,
    Crash,
    RecoverAfterCrash,
    RefuseRedispatch,
    /// Only ever enabled by [`Faulty::DispatchWithoutAuthorization`]: an
    /// illegal direct `Idle` -> `Dispatched` edge that never consumes a
    /// grant.
    FaultDispatchWithoutGrant,
    /// Only ever enabled by [`Faulty::RedispatchUncertainIntent`]: an
    /// illegal `RecoveringUncertain` -> `Dispatched` edge that reuses the
    /// crashed, unresolved grant instead of requiring a fresh `Authorize`.
    FaultRedispatchStaleGrant,
}

fn dispatch_descendant(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::Dispatched
            | Phase::ObservedSuccess
            | Phase::ObservedFailure
            | Phase::Completed
            | Phase::Failed
            | Phase::Crashed
            | Phase::RecoveringUncertain
            | Phase::RecoveryRefused
    )
}

fn safety_invariant(state: &State) -> Result<(), String> {
    if dispatch_descendant(state.phase) && state.dispatch_authorize != Some(state.authorize_count) {
        return Err("dispatch_has_fresh_matching_grant".to_owned());
    }
    if state.phase == Phase::Dispatched {
        if let Some(crashed) = state.crash_authorize {
            if state.dispatch_authorize == Some(crashed) {
                return Err("no_uncertain_redispatch".to_owned());
            }
        }
    }
    Ok(())
}

fn is_terminal(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::Completed | Phase::Failed | Phase::RecoveryRefused
    )
}

/// The correct model: exactly the events named in the header doc comment,
/// nothing else.
#[derive(Clone, Copy, Debug, Default)]
pub struct Correct;

impl TransitionSystem for Correct {
    type State = State;
    type Event = Event;

    fn initial_states(&self) -> Vec<State> {
        vec![State::initial()]
    }

    fn enabled_events(&self, state: &State) -> Vec<Event> {
        match state.phase {
            Phase::Idle => vec![Event::Authorize],
            Phase::Authorized => vec![Event::Dispatch],
            Phase::Dispatched => vec![Event::HostSucceeds, Event::HostFails, Event::Crash],
            Phase::ObservedSuccess | Phase::ObservedFailure => vec![Event::Reduce],
            Phase::Crashed => vec![Event::RecoverAfterCrash],
            Phase::RecoveringUncertain => vec![Event::RefuseRedispatch],
            Phase::Completed | Phase::Failed | Phase::RecoveryRefused => Vec::new(),
        }
    }

    fn apply(&self, state: &State, event: &Event) -> Option<State> {
        apply_common(state, event)
    }

    fn is_terminal(&self, state: &State) -> bool {
        is_terminal(state.phase)
    }

    fn safety_invariant(&self, state: &State) -> Result<(), String> {
        safety_invariant(state)
    }
}

fn apply_common(state: &State, event: &Event) -> Option<State> {
    let mut next = *state;
    match (state.phase, event) {
        (Phase::Idle, Event::Authorize) => {
            next.authorize_count += 1;
            next.phase = Phase::Authorized;
        }
        (Phase::Authorized, Event::Dispatch) => {
            next.dispatch_authorize = Some(state.authorize_count);
            next.phase = Phase::Dispatched;
        }
        (Phase::Dispatched, Event::HostSucceeds) => next.phase = Phase::ObservedSuccess,
        (Phase::Dispatched, Event::HostFails) => next.phase = Phase::ObservedFailure,
        (Phase::Dispatched, Event::Crash) => {
            next.crash_authorize = Some(state.authorize_count);
            next.phase = Phase::Crashed;
        }
        (Phase::ObservedSuccess, Event::Reduce) => next.phase = Phase::Completed,
        (Phase::ObservedFailure, Event::Reduce) => next.phase = Phase::Failed,
        (Phase::Crashed, Event::RecoverAfterCrash) => next.phase = Phase::RecoveringUncertain,
        (Phase::RecoveringUncertain, Event::RefuseRedispatch) => {
            next.phase = Phase::RecoveryRefused;
        }
        // Seeded faults, only ever reachable through `Faulty`'s
        // `enabled_events`; see its doc comment.
        (Phase::Idle, Event::FaultDispatchWithoutGrant) => {
            next.dispatch_authorize = None;
            next.phase = Phase::Dispatched;
        }
        (Phase::RecoveringUncertain, Event::FaultRedispatchStaleGrant) => {
            next.dispatch_authorize = state.crash_authorize;
            next.phase = Phase::Dispatched;
        }
        _ => return None,
    }
    Some(next)
}

/// Which single illegal transition to seed into an otherwise-correct
/// authorization model. Each variant reproduces exactly one bug class this
/// projection is meant to catch; see the module tests for the exact
/// minimal counterexample each one produces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    /// Adds an illegal `Idle` -> `Dispatched` edge that skips `Authorize`
    /// entirely: a dispatch with no grant at all.
    DispatchWithoutAuthorization,
    /// Adds an illegal `RecoveringUncertain` -> `Dispatched` edge that
    /// reuses the crashed grant instead of requiring a fresh `Authorize`:
    /// exactly the "automatic redispatch of uncertain intent" the real
    /// checkpoint recovery code refuses to do.
    RedispatchUncertainIntent,
}

/// A transition table seeded with exactly one [`Fault`], otherwise
/// identical to [`Correct`]. Represents "seeded faulty transition tables"
/// from the issue's required-tests list: a hand-mutated table, not a
/// randomly fuzzed one, so the expected counterexample is exact and
/// reproducible.
#[derive(Clone, Copy, Debug)]
pub struct Faulty(pub Fault);

impl TransitionSystem for Faulty {
    type State = State;
    type Event = Event;

    fn initial_states(&self) -> Vec<State> {
        vec![State::initial()]
    }

    fn enabled_events(&self, state: &State) -> Vec<Event> {
        let mut events = Correct.enabled_events(state);
        match (self.0, state.phase) {
            (Fault::DispatchWithoutAuthorization, Phase::Idle) => {
                events.push(Event::FaultDispatchWithoutGrant);
            }
            (Fault::RedispatchUncertainIntent, Phase::RecoveringUncertain) => {
                events.push(Event::FaultRedispatchStaleGrant);
            }
            _ => {}
        }
        events
    }

    fn apply(&self, state: &State, event: &Event) -> Option<State> {
        apply_common(state, event)
    }

    fn is_terminal(&self, state: &State) -> bool {
        is_terminal(state.phase)
    }

    fn safety_invariant(&self, state: &State) -> Result<(), String> {
        safety_invariant(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance_manifest::model_checking::engine::{
        check_reachable, check_safety, LimitHit, SafetyOutcome,
    };

    #[test]
    fn correct_model_is_verified_within_declared_bounds() {
        let report = check_safety(&Correct, BOUNDS);
        assert_eq!(report.outcome, SafetyOutcome::Verified);
    }

    /// A coverage floor: guards against the exact vacuity failure mode
    /// #184 hit (a check that trivially "holds" because almost nothing was
    /// explored). If a future edit to `enabled_events`/`apply` silently
    /// stops generating successors, this regresses loudly instead of the
    /// model quietly reporting `Verified` over a near-empty space.
    #[test]
    fn correct_model_explores_every_declared_phase() {
        let report = check_safety(&Correct, BOUNDS);
        assert_eq!(report.outcome, SafetyOutcome::Verified);
        // 10 phases, and this model reaches each of them exactly once
        // (no branching revisits a phase with different tracked fields
        // along more than one path in this small lifecycle).
        assert_eq!(report.counters.explored_states, 10, "{:?}", report.counters);
        assert_eq!(
            report.counters.explored_transitions, 9,
            "{:?}",
            report.counters
        );
    }

    #[test]
    fn correct_model_state_space_is_well_under_its_declared_bounds() {
        let report = check_safety(&Correct, BOUNDS);
        assert!(report.counters.explored_states < BOUNDS.max_states);
        assert!(report.counters.max_depth_reached < BOUNDS.max_depth);
        assert!(report.counters.explored_transitions < BOUNDS.max_transitions);
    }

    #[test]
    fn dispatch_without_authorization_is_a_one_step_counterexample() {
        let faulty = Faulty(Fault::DispatchWithoutAuthorization);
        let report = check_safety(&faulty, BOUNDS);
        match report.outcome {
            SafetyOutcome::Violated { trace, invariant } => {
                assert_eq!(invariant, "dispatch_has_fresh_matching_grant");
                assert_eq!(trace.len(), 1);
                assert_eq!(trace[0].from.phase, Phase::Idle);
                assert_eq!(trace[0].event, Event::FaultDispatchWithoutGrant);
                assert_eq!(trace[0].to.phase, Phase::Dispatched);
                assert_eq!(trace[0].to.dispatch_authorize, None);
            }
            other => panic!("expected Violated, got {other:?}"),
        }
    }

    #[test]
    fn redispatch_uncertain_intent_is_a_five_step_counterexample() {
        let faulty = Faulty(Fault::RedispatchUncertainIntent);
        let report = check_safety(&faulty, BOUNDS);
        match report.outcome {
            SafetyOutcome::Violated { trace, invariant } => {
                assert_eq!(invariant, "no_uncertain_redispatch");
                let events: Vec<Event> = trace.iter().map(|step| step.event).collect();
                assert_eq!(
                    events,
                    vec![
                        Event::Authorize,
                        Event::Dispatch,
                        Event::Crash,
                        Event::RecoverAfterCrash,
                        Event::FaultRedispatchStaleGrant,
                    ]
                );
                let final_state = &trace.last().unwrap().to;
                assert_eq!(final_state.phase, Phase::Dispatched);
                assert_eq!(final_state.dispatch_authorize, final_state.crash_authorize);
            }
            other => panic!("expected Violated, got {other:?}"),
        }
    }

    /// Independent replay: re-run the exact event sequence the trace
    /// reports, from a fresh initial state, through the *same* transition
    /// function, and confirm it reproduces the identical violating state
    /// and re-trips the identical invariant. This is deliberately not a
    /// replay against a separate executable fixture (out of this
    /// projection's scope; see the owning spec's "Explicitly deferred"),
    /// only a check that the reported trace is not a fabricated artifact
    /// of the search but an honest, reproducible path through `apply`.
    #[test]
    fn counterexample_trace_replays_independently_to_the_same_violation() {
        let faulty = Faulty(Fault::RedispatchUncertainIntent);
        let report = check_safety(&faulty, BOUNDS);
        let SafetyOutcome::Violated { trace, invariant } = report.outcome else {
            panic!("expected Violated");
        };
        let mut state = State::initial();
        for step in &trace {
            assert_eq!(state, step.from);
            state = apply_common(&state, &step.event).expect("trace event must apply");
        }
        assert_eq!(state, trace.last().unwrap().to);
        assert_eq!(safety_invariant(&state), Err(invariant));
    }

    #[test]
    fn bound_exhaustion_on_the_correct_model_is_never_reported_as_verified() {
        let tiny = Bounds {
            max_states: 1,
            max_depth: 16,
            max_transitions: 128,
        };
        let report = check_safety(&Correct, tiny);
        assert_eq!(
            report.outcome,
            SafetyOutcome::BoundExhausted {
                limit: LimitHit::MaxStates
            }
        );
        assert_ne!(report.outcome, SafetyOutcome::Verified);
    }

    #[test]
    fn depth_bound_exhaustion_is_distinct_from_verified() {
        let shallow = Bounds {
            max_states: 64,
            max_depth: 1,
            max_transitions: 128,
        };
        let report = check_safety(&Correct, shallow);
        assert_eq!(
            report.outcome,
            SafetyOutcome::BoundExhausted {
                limit: LimitHit::MaxDepth
            }
        );
    }

    #[test]
    fn a_terminal_success_is_reachable_within_bound() {
        use super::super::engine::ReachabilityOutcome;
        let report = check_reachable(&Correct, BOUNDS, |state| state.phase == Phase::Completed);
        match report.outcome {
            ReachabilityOutcome::Reached { trace } => {
                assert_eq!(trace.last().unwrap().to.phase, Phase::Completed);
            }
            other => panic!("expected Reached, got {other:?}"),
        }
    }

    #[test]
    fn an_unreachable_target_is_reported_never_reached_not_silently_ignored() {
        use super::super::engine::ReachabilityOutcome;
        // No event ever produces a phase with `authorize_count == 9`; the
        // full (small) space closes without ever satisfying this target.
        let report = check_reachable(&Correct, BOUNDS, |state| state.authorize_count == 9);
        assert_eq!(report.outcome, ReachabilityOutcome::NeverReached);
    }

    #[test]
    fn same_run_twice_produces_byte_identical_reports() {
        let first = check_safety(&Correct, BOUNDS);
        let second = check_safety(&Correct, BOUNDS);
        assert_eq!(first, second);
    }

    #[test]
    fn model_digest_is_stable_for_the_committed_descriptor_and_bounds() {
        let digest = super::super::digest::model_digest(&DESCRIPTOR, BOUNDS);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(
            digest,
            super::super::digest::model_digest(&DESCRIPTOR, BOUNDS)
        );
    }
}
