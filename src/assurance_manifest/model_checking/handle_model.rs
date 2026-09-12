//! Model B: a resource/carrier handle's acquire/use/release lifecycle.
//!
//! Another self-contained projection, not a hook into
//! `src/host_ownership.rs` or `src/public_generic_abi/**` (both outside
//! this tranche's file lease). Its vocabulary mirrors
//! `host_ownership::HostOwnerState`'s `Live`/`InInvocation`/`Dead` and the
//! repository's existing "double release"/"orphaning a live owner"
//! phrasing (see `docs/ARC-ZONES-V1.md`,
//! `docs/PUBLIC-GENERIC-CARRIER-V1.md`, and the
//! `double_release_of_a_result_handle_is_rejected` family of tests) without
//! calling any of that code.
//!
//! Two invariants are checked over every reachable state:
//!
//! - `discharged_exactly_once`: a handle's cleanup (`Release` or
//!   `Abandon`) fires at most once ever.
//! - `no_discharge_while_in_invocation`: a handle is never released or
//!   abandoned while a call against it is still outstanding (no orphaning
//!   a live invocation).
//!
//! See [`docs/BOUNDED-MODEL-CHECKING-V1.md`](../../../docs/BOUNDED-MODEL-CHECKING-V1.md)
//! "Model B" for the full state diagram and exact scope.

use super::digest::ModelDescriptor;
use super::engine::{Bounds, TransitionSystem};

pub const NAME: &str = "resource_handle_acquire_use_release";
pub const VERSION: &str = "v1";

pub const INVARIANTS: &[&str] = &[
    "discharged_exactly_once",
    "no_discharge_while_in_invocation",
];

pub const TERMINAL_PHASES: &[&str] = &["Released", "Abandoned"];

pub const DESCRIPTOR: ModelDescriptor = ModelDescriptor {
    name: NAME,
    version: VERSION,
    invariants: INVARIANTS,
    terminal_states: TERMINAL_PHASES,
};

pub const BOUNDS: Bounds = Bounds {
    max_states: 64,
    max_depth: 16,
    max_transitions: 128,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Phase {
    NotAcquired,
    Live,
    InInvocation,
    Released,
    Abandoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct State {
    pub phase: Phase,
    /// How many times this handle has been discharged (`Release` or
    /// `Abandon`) so far. Must never exceed 1.
    pub release_count: u8,
    /// Whether an invocation was outstanding at the moment the handle
    /// last transitioned. Carried unchanged by every event except
    /// `BeginInvocation` (sets it) and `CompleteInvocation` (clears it);
    /// the correct model's `Release`/`Abandon` are only ever enabled from
    /// `Live`, where this is always `false`, so it only becomes `true` on
    /// a `Released`/`Abandoned` state through the seeded fault below.
    pub in_invocation: bool,
}

impl State {
    const fn initial() -> Self {
        Self {
            phase: Phase::NotAcquired,
            release_count: 0,
            in_invocation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Event {
    Acquire,
    BeginInvocation,
    CompleteInvocation,
    Release,
    Abandon,
    /// Only ever enabled by [`Faulty::ReleaseWhileInInvocation`]: an
    /// illegal `Release` reachable directly from `InInvocation`, orphaning
    /// the outstanding call.
    FaultReleaseWhileInInvocation,
    /// Only ever enabled by [`Faulty::DoubleRelease`]: an illegal second
    /// `Release` reachable from an already-`Released` state.
    FaultDoubleRelease,
}

fn safety_invariant(state: &State) -> Result<(), String> {
    if state.release_count > 1 {
        return Err("discharged_exactly_once".to_owned());
    }
    if matches!(state.phase, Phase::Released | Phase::Abandoned) {
        if state.release_count != 1 {
            return Err("discharged_exactly_once".to_owned());
        }
        if state.in_invocation {
            return Err("no_discharge_while_in_invocation".to_owned());
        }
    }
    Ok(())
}

fn is_terminal(phase: Phase) -> bool {
    matches!(phase, Phase::Released | Phase::Abandoned)
}

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
            Phase::NotAcquired => vec![Event::Acquire],
            Phase::Live => vec![Event::BeginInvocation, Event::Release, Event::Abandon],
            Phase::InInvocation => vec![Event::CompleteInvocation],
            Phase::Released | Phase::Abandoned => Vec::new(),
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
        (Phase::NotAcquired, Event::Acquire) => next.phase = Phase::Live,
        (Phase::Live, Event::BeginInvocation) => {
            next.in_invocation = true;
            next.phase = Phase::InInvocation;
        }
        (Phase::InInvocation, Event::CompleteInvocation) => {
            next.in_invocation = false;
            next.phase = Phase::Live;
        }
        (Phase::Live, Event::Release) => {
            next.release_count += 1;
            next.phase = Phase::Released;
        }
        (Phase::Live, Event::Abandon) => {
            next.release_count += 1;
            next.phase = Phase::Abandoned;
        }
        // Seeded faults; see `Faulty`.
        (Phase::InInvocation, Event::FaultReleaseWhileInInvocation) => {
            next.release_count += 1;
            next.phase = Phase::Released;
            // `in_invocation` deliberately left `true`: nothing ever
            // completed the call this release orphaned.
        }
        (Phase::Released, Event::FaultDoubleRelease) => {
            next.release_count += 1;
        }
        _ => return None,
    }
    Some(next)
}

/// Which single illegal transition to seed. See the module tests for the
/// exact minimal counterexample each one produces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    /// Adds an illegal `InInvocation` -> `Released` edge: releasing a
    /// handle while a call against it is still outstanding.
    ReleaseWhileInInvocation,
    /// Adds an illegal `Released` -> `Released` edge that increments
    /// `release_count` again: a double release.
    DoubleRelease,
}

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
            (Fault::ReleaseWhileInInvocation, Phase::InInvocation) => {
                events.push(Event::FaultReleaseWhileInInvocation);
            }
            (Fault::DoubleRelease, Phase::Released) => {
                events.push(Event::FaultDoubleRelease);
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
        check_reachable, check_safety, ReachabilityOutcome, SafetyOutcome,
    };

    #[test]
    fn correct_model_is_verified_within_declared_bounds() {
        let report = check_safety(&Correct, BOUNDS);
        assert_eq!(report.outcome, SafetyOutcome::Verified);
    }

    #[test]
    fn correct_model_explores_every_declared_phase() {
        let report = check_safety(&Correct, BOUNDS);
        assert_eq!(report.outcome, SafetyOutcome::Verified);
        // NotAcquired, Live, InInvocation, Released, Abandoned: 5 distinct
        // visited states. `InInvocation --CompleteInvocation--> Live`
        // closes a cycle back to the already-visited `Live` state (same
        // `release_count`/`in_invocation` fields), so it is counted as an
        // explored transition but does not add a sixth visited state or a
        // second pop of `Live`.
        assert_eq!(report.counters.explored_states, 5, "{:?}", report.counters);
        assert_eq!(
            report.counters.explored_transitions, 5,
            "{:?}",
            report.counters
        );
    }

    #[test]
    fn release_while_in_invocation_orphans_the_call_and_is_caught() {
        let faulty = Faulty(Fault::ReleaseWhileInInvocation);
        let report = check_safety(&faulty, BOUNDS);
        match report.outcome {
            SafetyOutcome::Violated { trace, invariant } => {
                assert_eq!(invariant, "no_discharge_while_in_invocation");
                let events: Vec<Event> = trace.iter().map(|step| step.event).collect();
                assert_eq!(
                    events,
                    vec![
                        Event::Acquire,
                        Event::BeginInvocation,
                        Event::FaultReleaseWhileInInvocation,
                    ]
                );
                let end = &trace.last().unwrap().to;
                assert_eq!(end.phase, Phase::Released);
                assert!(end.in_invocation);
            }
            other => panic!("expected Violated, got {other:?}"),
        }
    }

    #[test]
    fn double_release_is_a_three_step_counterexample() {
        let faulty = Faulty(Fault::DoubleRelease);
        let report = check_safety(&faulty, BOUNDS);
        match report.outcome {
            SafetyOutcome::Violated { trace, invariant } => {
                assert_eq!(invariant, "discharged_exactly_once");
                let events: Vec<Event> = trace.iter().map(|step| step.event).collect();
                assert_eq!(
                    events,
                    vec![Event::Acquire, Event::Release, Event::FaultDoubleRelease]
                );
                assert_eq!(trace.last().unwrap().to.release_count, 2);
            }
            other => panic!("expected Violated, got {other:?}"),
        }
    }

    #[test]
    fn counterexample_trace_replays_independently_to_the_same_violation() {
        let faulty = Faulty(Fault::DoubleRelease);
        let report = check_safety(&faulty, BOUNDS);
        let SafetyOutcome::Violated { trace, invariant } = report.outcome else {
            panic!("expected Violated");
        };
        let mut state = State::initial();
        for step in &trace {
            assert_eq!(state, step.from);
            state = apply_common(&state, &step.event).expect("trace event must apply");
        }
        assert_eq!(safety_invariant(&state), Err(invariant));
    }

    #[test]
    fn every_handle_can_reach_a_discharged_terminal_state() {
        let report = check_reachable(&Correct, BOUNDS, |state| {
            matches!(state.phase, Phase::Released | Phase::Abandoned)
        });
        assert!(matches!(
            report.outcome,
            ReachabilityOutcome::Reached { .. }
        ));
    }

    #[test]
    fn same_run_twice_produces_byte_identical_reports() {
        let first = check_safety(&Correct, BOUNDS);
        let second = check_safety(&Correct, BOUNDS);
        assert_eq!(first, second);
    }
}
