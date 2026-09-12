//! Engine-level tests: the vacuity defenses, bound-exhaustion discipline,
//! and determinism guarantees that do not belong to either committed model
//! specifically.
//!
//! The two tests immediately below are the direct analog of the #184 bug
//! class this issue calls out: a check that reports success only because
//! it explored almost nothing. [`vacuous_empty_initial_state_space_is_not_verified`]
//! covers "no reachable state was explored at all"; [`under_implemented_transition_table_is_reported_as_dead_state_not_verified`]
//! covers the more dangerous case — a transition table that silently stops
//! generating successors partway through, which without the `is_terminal`
//! structural check would otherwise "verify" every invariant trivially
//! over a near-empty space, exactly as `result` silently going unconstrained
//! made #184's first draft report an overflow as proved.

use super::engine::{
    check_reachable, check_safety, Bounds, LimitHit, ReachabilityOutcome, SafetyOutcome,
    TransitionSystem,
};

const BOUNDS: Bounds = Bounds {
    max_states: 16,
    max_depth: 8,
    max_transitions: 32,
};

/// A transition system with no initial states at all.
struct EmptyToy;

impl TransitionSystem for EmptyToy {
    type State = u8;
    type Event = u8;

    fn initial_states(&self) -> Vec<u8> {
        Vec::new()
    }

    fn enabled_events(&self, _state: &u8) -> Vec<u8> {
        vec![0]
    }

    fn apply(&self, state: &u8, _event: &u8) -> Option<u8> {
        Some(state + 1)
    }

    fn is_terminal(&self, _state: &u8) -> bool {
        false
    }

    fn safety_invariant(&self, _state: &u8) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn vacuous_empty_initial_state_space_is_not_verified() {
    let report = check_safety(&EmptyToy, BOUNDS);
    assert_eq!(report.outcome, SafetyOutcome::EmptyStateSpace);
    assert_ne!(report.outcome, SafetyOutcome::Verified);
}

#[test]
fn vacuous_empty_initial_state_space_is_not_reported_reachable_or_never_reached() {
    let report = check_reachable(&EmptyToy, BOUNDS, |_| true);
    assert_eq!(report.outcome, ReachabilityOutcome::EmptyStateSpace);
}

/// One state, `enabled_events` always returns an empty list (a stand-in
/// for an under-implemented transition table that silently prunes every
/// successor), and the state is *not* declared terminal. This is the exact
/// shape of the #184 bug class translated to model checking: if the
/// engine treated "no more enabled events" as unconditional closure, this
/// would report `Verified` having explored exactly one state — trivially
/// "safe" only because nothing was explored. The engine must instead
/// report `DeadState`.
pub(super) struct IncompleteToy;

impl TransitionSystem for IncompleteToy {
    type State = u8;
    type Event = u8;

    fn initial_states(&self) -> Vec<u8> {
        vec![0]
    }

    fn enabled_events(&self, _state: &u8) -> Vec<u8> {
        Vec::new()
    }

    fn apply(&self, _state: &u8, _event: &u8) -> Option<u8> {
        None
    }

    fn is_terminal(&self, _state: &u8) -> bool {
        false
    }

    fn safety_invariant(&self, _state: &u8) -> Result<(), String> {
        // If the engine folded "no enabled events" into unconditional
        // closure, this invariant (which always holds) would make the
        // whole run `Verified` despite exploring a single state.
        Ok(())
    }
}

#[test]
fn under_implemented_transition_table_is_reported_as_dead_state_not_verified() {
    let report = check_safety(&IncompleteToy, BOUNDS);
    assert_eq!(
        report.outcome,
        SafetyOutcome::DeadState { trace: Vec::new() }
    );
    assert_ne!(report.outcome, SafetyOutcome::Verified);
    // Exactly one state was explored: the vacuity is structural, not a
    // matter of needing a bigger bound.
    assert_eq!(report.counters.explored_states, 1);
}

#[test]
fn under_implemented_transition_table_is_reported_as_dead_state_for_reachability_too() {
    let report = check_reachable(&IncompleteToy, BOUNDS, |_| false);
    assert_eq!(
        report.outcome,
        ReachabilityOutcome::DeadState { trace: Vec::new() }
    );
    assert_ne!(report.outcome, ReachabilityOutcome::NeverReached);
}

/// A model that legitimately has zero enabled events at its one reachable
/// state, but *declares* that state terminal. This must be `Verified`,
/// distinguishing "the model says this is a correct place to stop" from
/// the previous test's "the model silently stopped for no declared
/// reason".
struct DeclaredTerminalToy;

impl TransitionSystem for DeclaredTerminalToy {
    type State = u8;
    type Event = u8;

    fn initial_states(&self) -> Vec<u8> {
        vec![0]
    }

    fn enabled_events(&self, _state: &u8) -> Vec<u8> {
        Vec::new()
    }

    fn apply(&self, _state: &u8, _event: &u8) -> Option<u8> {
        None
    }

    fn is_terminal(&self, _state: &u8) -> bool {
        true
    }

    fn safety_invariant(&self, _state: &u8) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn a_declared_terminal_dead_end_is_verified() {
    let report = check_safety(&DeclaredTerminalToy, BOUNDS);
    assert_eq!(report.outcome, SafetyOutcome::Verified);
    assert_eq!(report.counters.explored_states, 1);
    assert_eq!(report.counters.explored_transitions, 0);
}

/// A counter that increments forever with no terminal state: an infinite
/// state space this bound must cut off rather than ever "verifying".
struct UnboundedCounter;

impl TransitionSystem for UnboundedCounter {
    type State = u32;
    type Event = ();

    fn initial_states(&self) -> Vec<u32> {
        vec![0]
    }

    fn enabled_events(&self, _state: &u32) -> Vec<()> {
        vec![()]
    }

    fn apply(&self, state: &u32, (): &()) -> Option<u32> {
        Some(state + 1)
    }

    fn is_terminal(&self, _state: &u32) -> bool {
        false
    }

    fn safety_invariant(&self, _state: &u32) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn unbounded_state_space_reports_max_states_exhaustion_not_verified() {
    let tight = Bounds {
        max_states: 5,
        max_depth: 1000,
        max_transitions: 1000,
    };
    let report = check_safety(&UnboundedCounter, tight);
    assert_eq!(
        report.outcome,
        SafetyOutcome::BoundExhausted {
            limit: LimitHit::MaxStates
        }
    );
    assert_ne!(report.outcome, SafetyOutcome::Verified);
}

#[test]
fn unbounded_state_space_reports_max_transitions_exhaustion_when_that_is_the_tighter_bound() {
    let tight = Bounds {
        max_states: 1000,
        max_depth: 1000,
        max_transitions: 3,
    };
    let report = check_safety(&UnboundedCounter, tight);
    assert_eq!(
        report.outcome,
        SafetyOutcome::BoundExhausted {
            limit: LimitHit::MaxTransitions
        }
    );
}

#[test]
fn unbounded_state_space_reports_max_depth_exhaustion_when_that_is_the_tighter_bound() {
    let tight = Bounds {
        max_states: 1000,
        max_depth: 4,
        max_transitions: 1000,
    };
    let report = check_safety(&UnboundedCounter, tight);
    assert_eq!(
        report.outcome,
        SafetyOutcome::BoundExhausted {
            limit: LimitHit::MaxDepth
        }
    );
}

#[test]
fn unbounded_state_space_never_reaches_an_unsatisfiable_target_within_bound_is_bound_exhausted_not_never_reached(
) {
    let tight = Bounds {
        max_states: 5,
        max_depth: 1000,
        max_transitions: 1000,
    };
    let report = check_reachable(&UnboundedCounter, tight, |state| *state == 999);
    assert_eq!(
        report.outcome,
        ReachabilityOutcome::BoundExhausted {
            limit: LimitHit::MaxStates
        }
    );
    assert_ne!(report.outcome, ReachabilityOutcome::NeverReached);
}

/// Determinism: two independent runs over the same (nontrivial, branching)
/// model produce byte-identical reports, matching this repository's
/// determinism invariant for generated artifacts.
struct BranchingToy;

impl TransitionSystem for BranchingToy {
    type State = u8;
    type Event = u8;

    fn initial_states(&self) -> Vec<u8> {
        vec![0]
    }

    fn enabled_events(&self, state: &u8) -> Vec<u8> {
        if *state < 4 {
            vec![1, 2]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, state: &u8, event: &u8) -> Option<u8> {
        Some(state.saturating_add(*event))
    }

    fn is_terminal(&self, state: &u8) -> bool {
        *state >= 4
    }

    fn safety_invariant(&self, _state: &u8) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn determinism_two_runs_of_a_branching_model_are_byte_identical() {
    let first = check_safety(&BranchingToy, BOUNDS);
    let second = check_safety(&BranchingToy, BOUNDS);
    assert_eq!(first, second);
    assert_eq!(first.outcome, SafetyOutcome::Verified);
    // Sanity: this model really does branch and revisit states (1+2 and
    // 2+1 both reach 3), so this is not a trivial single-path check.
    assert!(first.counters.explored_transitions > first.counters.explored_states);
}

#[test]
fn determinism_holds_for_reachability_search_too() {
    let first = check_reachable(&BranchingToy, BOUNDS, |state| *state == 3);
    let second = check_reachable(&BranchingToy, BOUNDS, |state| *state == 3);
    assert_eq!(first, second);
}

/// A safety violation at the very first (initial) state must be reported
/// with an empty trace, not treated as unreachable or as an engine panic.
struct ViolatesImmediately;

impl TransitionSystem for ViolatesImmediately {
    type State = u8;
    type Event = u8;

    fn initial_states(&self) -> Vec<u8> {
        vec![0]
    }

    fn enabled_events(&self, _state: &u8) -> Vec<u8> {
        Vec::new()
    }

    fn apply(&self, _state: &u8, _event: &u8) -> Option<u8> {
        None
    }

    fn is_terminal(&self, _state: &u8) -> bool {
        true
    }

    fn safety_invariant(&self, _state: &u8) -> Result<(), String> {
        Err("always_fails".to_owned())
    }
}

#[test]
fn a_violation_at_an_initial_state_is_reported_with_an_empty_trace() {
    let report = check_safety(&ViolatesImmediately, BOUNDS);
    match report.outcome {
        SafetyOutcome::Violated { trace, invariant } => {
            assert!(trace.is_empty());
            assert_eq!(invariant, "always_fails");
        }
        other => panic!("expected Violated, got {other:?}"),
    }
}

/// Two distinct initial states, declared in a fixed order; the engine must
/// process them in that declared order (first one violating wins) rather
/// than in some incidental collection order.
struct TwoInitials;

impl TransitionSystem for TwoInitials {
    type State = u8;
    type Event = u8;

    fn initial_states(&self) -> Vec<u8> {
        vec![10, 20]
    }

    fn enabled_events(&self, _state: &u8) -> Vec<u8> {
        Vec::new()
    }

    fn apply(&self, _state: &u8, _event: &u8) -> Option<u8> {
        None
    }

    fn is_terminal(&self, _state: &u8) -> bool {
        true
    }

    fn safety_invariant(&self, state: &u8) -> Result<(), String> {
        if *state == 20 {
            Err("second_initial_is_bad".to_owned())
        } else {
            Ok(())
        }
    }
}

#[test]
fn multiple_initial_states_are_each_checked_for_safety() {
    let report = check_safety(&TwoInitials, BOUNDS);
    match report.outcome {
        SafetyOutcome::Violated { trace, invariant } => {
            assert!(trace.is_empty());
            assert_eq!(invariant, "second_initial_is_bad");
        }
        other => panic!("expected Violated, got {other:?}"),
    }
}
