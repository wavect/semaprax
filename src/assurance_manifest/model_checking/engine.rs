//! Deterministic, bounded, explicit-state exploration over one
//! [`TransitionSystem`] projection.
//!
//! See [`docs/BOUNDED-MODEL-CHECKING-V1.md`](../../../docs/BOUNDED-MODEL-CHECKING-V1.md)
//! "The engine" for the full rationale. The short version: every bound is
//! caller-declared and required ([`Bounds`] has no `Default`), exhaustion of
//! a bound is its own outcome that can never be mistaken for the property
//! holding ([`SafetyOutcome::BoundExhausted`] /
//! [`ReachabilityOutcome::BoundExhausted`]), and a state space that stops
//! growing because a model under-declares its own transitions is caught
//! structurally by [`SafetyOutcome::DeadState`] /
//! [`ReachabilityOutcome::DeadState`] rather than silently read as closure.
//! [`SafetyOutcome::Verified`] and [`ReachabilityOutcome::NeverReached`] are
//! the only variants asserting a completed, closed exploration; every other
//! variant is either a definitive finding (a violation, a witness trace, a
//! structural anomaly) or an explicit admission that the bound fired before
//! closure.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Debug;

/// Explicit, caller-declared limits on one bounded exploration. Every field
/// is required (there is no `Default`) so a caller cannot run this engine
/// without deciding, and recording, exactly how far it was allowed to look.
///
/// `max_states` and `max_transitions` are hard caps on total exploration
/// work: the very first state or transition that would exceed them stops
/// the search immediately, because they exist to bound memory and time, not
/// merely search depth. `max_depth` is a per-branch bound in the bounded
/// model checking sense (the classical "k"): a branch that reaches
/// `max_depth` stops expanding, but sibling branches at shallower depth
/// still finish, so the deterministic minimal counterexample search is
/// never truncated early by a depth limit that a shorter violation would
/// not have needed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bounds {
    pub max_states: usize,
    pub max_depth: usize,
    pub max_transitions: usize,
}

/// One finite-state, finite-event protocol projection. Implementors define
/// a closed, explicit state and event vocabulary; the engine only ever
/// calls these five methods and never inspects a state or event any other
/// way, so canonical ordering and hashing are entirely the model's
/// responsibility (via `Ord`) rather than anything hash-algorithm- or
/// memory-address-dependent.
pub trait TransitionSystem {
    /// Must be `Ord` so the engine can deduplicate visited states with a
    /// `BTreeSet`/`BTreeMap` — deterministic regardless of hasher, build,
    /// or platform, unlike a `HashSet`/`HashMap` keyed on `Hash`.
    type State: Clone + Ord + Debug;
    /// Must be `Ord` so two runs that declare the same events in the same
    /// order always explore identically.
    type Event: Clone + Ord + Debug;

    /// Every state exploration starts from. Order is significant: it is
    /// the tie-break for which initial state's branch is searched first.
    fn initial_states(&self) -> Vec<Self::State>;

    /// Every event enabled in `state`, in a fixed declared order. An empty
    /// result means "no event fires from this state" — the engine treats
    /// that as a claimed dead end and checks it against
    /// [`Self::is_terminal`].
    fn enabled_events(&self, state: &Self::State) -> Vec<Self::Event>;

    /// Deterministically apply one event that [`Self::enabled_events`]
    /// reported as enabled. `None` means the model itself declines to
    /// produce a successor for an event it advertised as enabled (a defect
    /// in the model, not a state the engine invents on its behalf); the
    /// engine treats this defensively as "no transition" rather than
    /// panicking, since the model, not the engine, owns that contract.
    fn apply(&self, state: &Self::State, event: &Self::Event) -> Option<Self::State>;

    /// `true` only for a state the model *declares* as a legitimate place
    /// to have no further enabled events (a successful completion, a
    /// refused/terminal failure, and so on). A reachable state with no
    /// enabled events that is not declared terminal is reported as
    /// [`SafetyOutcome::DeadState`]/[`ReachabilityOutcome::DeadState`]
    /// instead of silently being read as the end of the reachable space —
    /// this is the structural defense against an under-implemented
    /// `enabled_events`/`apply` pair vacuously "closing" the search after
    /// exploring almost nothing.
    fn is_terminal(&self, state: &Self::State) -> bool;

    /// `Ok(())` when `state` satisfies every safety invariant this model
    /// asserts; `Err(name)` names the specific invariant that failed.
    fn safety_invariant(&self, state: &Self::State) -> Result<(), String>;
}

/// One edge of a counterexample or witness trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step<S, E> {
    pub from: S,
    pub event: E,
    pub to: S,
}

/// An ordered path from an initial state to the state the outcome is
/// reporting about. Empty exactly when that state is itself an initial
/// state.
pub type Trace<S, E> = Vec<Step<S, E>>;

/// Which explicit [`Bounds`] field stopped an incomplete exploration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitHit {
    MaxStates,
    MaxDepth,
    MaxTransitions,
}

/// The outcome of one bounded safety exploration. Every variant is
/// mutually exclusive and none is a special case of another: in
/// particular, [`Self::BoundExhausted`] can never be read as
/// [`Self::Verified`], and [`Self::EmptyStateSpace`] /
/// [`Self::DeadState`] can never be read as [`Self::Verified`] either, even
/// though all three describe "no violation was found".
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SafetyOutcome<S, E> {
    /// The full reachable state space was explored to closure (the search
    /// frontier drained to empty with no [`Bounds`] limit ever firing), no
    /// state failed `safety_invariant`, and every reachable state with no
    /// enabled events was declared terminal. This is the only variant a
    /// caller may report as `model_checked` in an Assurance Manifest.
    Verified,
    /// `safety_invariant` returned `Err` for some reachable state. `trace`
    /// is the shortest path (by transition count) from an initial state to
    /// the violating state that this exploration's fixed, deterministic
    /// order reaches first.
    Violated {
        trace: Trace<S, E>,
        invariant: String,
    },
    /// A reachable state has no enabled events and `is_terminal` returned
    /// `false` for it: an undeclared dead end, reported instead of ever
    /// being folded into `Verified`.
    DeadState { trace: Trace<S, E> },
    /// `initial_states` returned no states at all. An empty state space
    /// vacuously satisfies every invariant, so this is its own outcome
    /// rather than ever being reported as `Verified`.
    EmptyStateSpace,
    /// A [`Bounds`] limit fired while the search frontier was still
    /// non-empty (or a branch still had unexplored depth): the exploration
    /// is incomplete. Never collapsed into `Verified`.
    BoundExhausted { limit: LimitHit },
}

/// Counters every exploration reports regardless of outcome, so a caller
/// (and the Assurance Manifest record built from a `Verified` report) can
/// see exactly how much was covered rather than trusting a bare verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExploreCounters {
    pub explored_states: usize,
    pub explored_transitions: usize,
    pub max_depth_reached: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExploreReport<S, E> {
    pub outcome: SafetyOutcome<S, E>,
    pub counters: ExploreCounters,
    pub bounds: Bounds,
}

fn reconstruct_trace<S, E>(parent: &BTreeMap<S, (S, E)>, state: &S) -> Trace<S, E>
where
    S: Clone + Ord,
    E: Clone,
{
    let mut steps = Vec::new();
    let mut current = state.clone();
    while let Some((previous, event)) = parent.get(&current) {
        steps.push(Step {
            from: previous.clone(),
            event: event.clone(),
            to: current.clone(),
        });
        current = previous.clone();
    }
    steps.reverse();
    steps
}

/// Run one bounded, deterministic breadth-first safety exploration of
/// `system` under `bounds`. BFS guarantees any reported counterexample
/// trace is minimal in transition count, and the fixed processing order
/// (declared `initial_states` order, then declared `enabled_events` order
/// per state, with a `BTreeSet`/`BTreeMap` visited/parent index instead of
/// a hash table) makes the whole run byte-for-byte reproducible across
/// runs, builds, and platforms.
#[must_use]
pub fn check_safety<T: TransitionSystem>(
    system: &T,
    bounds: Bounds,
) -> ExploreReport<T::State, T::Event> {
    let initials = system.initial_states();
    if initials.is_empty() {
        return ExploreReport {
            outcome: SafetyOutcome::EmptyStateSpace,
            counters: ExploreCounters {
                explored_states: 0,
                explored_transitions: 0,
                max_depth_reached: 0,
            },
            bounds,
        };
    }

    for initial in &initials {
        if let Err(invariant) = system.safety_invariant(initial) {
            return ExploreReport {
                outcome: SafetyOutcome::Violated {
                    trace: Vec::new(),
                    invariant,
                },
                counters: ExploreCounters {
                    explored_states: 0,
                    explored_transitions: 0,
                    max_depth_reached: 0,
                },
                bounds,
            };
        }
    }

    let mut visited: BTreeSet<T::State> = BTreeSet::new();
    let mut parent: BTreeMap<T::State, (T::State, T::Event)> = BTreeMap::new();
    let mut frontier: VecDeque<(T::State, usize)> = VecDeque::new();
    for initial in &initials {
        if visited.insert(initial.clone()) {
            frontier.push_back((initial.clone(), 0));
        }
    }

    let mut explored_states = 0usize;
    let mut explored_transitions = 0usize;
    let mut max_depth_reached = 0usize;
    let mut depth_bound_hit = false;

    while let Some((state, depth)) = frontier.pop_front() {
        explored_states += 1;
        max_depth_reached = max_depth_reached.max(depth);
        if explored_states > bounds.max_states {
            return ExploreReport {
                outcome: SafetyOutcome::BoundExhausted {
                    limit: LimitHit::MaxStates,
                },
                counters: ExploreCounters {
                    explored_states: explored_states - 1,
                    explored_transitions,
                    max_depth_reached,
                },
                bounds,
            };
        }

        let events = system.enabled_events(&state);
        if events.is_empty() {
            if !system.is_terminal(&state) {
                let trace = reconstruct_trace(&parent, &state);
                return ExploreReport {
                    outcome: SafetyOutcome::DeadState { trace },
                    counters: ExploreCounters {
                        explored_states,
                        explored_transitions,
                        max_depth_reached,
                    },
                    bounds,
                };
            }
            continue;
        }

        if depth >= bounds.max_depth {
            depth_bound_hit = true;
            continue;
        }

        for event in events {
            if explored_transitions >= bounds.max_transitions {
                return ExploreReport {
                    outcome: SafetyOutcome::BoundExhausted {
                        limit: LimitHit::MaxTransitions,
                    },
                    counters: ExploreCounters {
                        explored_states,
                        explored_transitions,
                        max_depth_reached,
                    },
                    bounds,
                };
            }
            let Some(next) = system.apply(&state, &event) else {
                continue;
            };
            explored_transitions += 1;
            if let Err(invariant) = system.safety_invariant(&next) {
                let mut trace = reconstruct_trace(&parent, &state);
                trace.push(Step {
                    from: state.clone(),
                    event,
                    to: next,
                });
                return ExploreReport {
                    outcome: SafetyOutcome::Violated { trace, invariant },
                    counters: ExploreCounters {
                        explored_states,
                        explored_transitions,
                        max_depth_reached,
                    },
                    bounds,
                };
            }
            if visited.insert(next.clone()) {
                parent.insert(next.clone(), (state.clone(), event));
                frontier.push_back((next, depth + 1));
            }
        }
    }

    let outcome = if depth_bound_hit {
        SafetyOutcome::BoundExhausted {
            limit: LimitHit::MaxDepth,
        }
    } else {
        SafetyOutcome::Verified
    };
    ExploreReport {
        outcome,
        counters: ExploreCounters {
            explored_states,
            explored_transitions,
            max_depth_reached,
        },
        bounds,
    }
}

/// The outcome of one bounded reachability/liveness exploration: "does some
/// reachable state satisfy `target` within the declared bound?". Mirrors
/// [`SafetyOutcome`]'s discipline: [`Self::NeverReached`] is only ever
/// returned for a fully closed exploration, never for one a bound cut
/// short.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReachabilityOutcome<S, E> {
    /// `target` holds for some reachable state; `trace` is the shortest
    /// witness path this exploration's deterministic order finds first.
    Reached { trace: Trace<S, E> },
    /// The full reachable state space was closed and no state satisfied
    /// `target`: a genuine bounded liveness failure over the space that was
    /// actually explored, not merely "not found yet".
    NeverReached,
    /// A reachable state has no enabled events and is not declared
    /// terminal; see [`SafetyOutcome::DeadState`].
    DeadState { trace: Trace<S, E> },
    /// `initial_states` returned no states.
    EmptyStateSpace,
    /// A [`Bounds`] limit fired before closure.
    BoundExhausted { limit: LimitHit },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReachabilityReport<S, E> {
    pub outcome: ReachabilityOutcome<S, E>,
    pub counters: ExploreCounters,
    pub bounds: Bounds,
}

/// Run one bounded, deterministic breadth-first search for a state
/// satisfying `target`. Same traversal discipline as [`check_safety`]: BFS
/// for minimal witnesses, `BTreeSet`/`BTreeMap` indexing for
/// hash-independent determinism, a hard cap on `max_states`/
/// `max_transitions`, and a per-branch `max_depth`.
#[must_use]
pub fn check_reachable<T: TransitionSystem>(
    system: &T,
    bounds: Bounds,
    target: impl Fn(&T::State) -> bool,
) -> ReachabilityReport<T::State, T::Event> {
    let initials = system.initial_states();
    if initials.is_empty() {
        return ReachabilityReport {
            outcome: ReachabilityOutcome::EmptyStateSpace,
            counters: ExploreCounters {
                explored_states: 0,
                explored_transitions: 0,
                max_depth_reached: 0,
            },
            bounds,
        };
    }

    for initial in &initials {
        if target(initial) {
            return ReachabilityReport {
                outcome: ReachabilityOutcome::Reached { trace: Vec::new() },
                counters: ExploreCounters {
                    explored_states: 0,
                    explored_transitions: 0,
                    max_depth_reached: 0,
                },
                bounds,
            };
        }
    }

    let mut visited: BTreeSet<T::State> = BTreeSet::new();
    let mut parent: BTreeMap<T::State, (T::State, T::Event)> = BTreeMap::new();
    let mut frontier: VecDeque<(T::State, usize)> = VecDeque::new();
    for initial in &initials {
        if visited.insert(initial.clone()) {
            frontier.push_back((initial.clone(), 0));
        }
    }

    let mut explored_states = 0usize;
    let mut explored_transitions = 0usize;
    let mut max_depth_reached = 0usize;
    let mut depth_bound_hit = false;

    while let Some((state, depth)) = frontier.pop_front() {
        explored_states += 1;
        max_depth_reached = max_depth_reached.max(depth);
        if explored_states > bounds.max_states {
            return ReachabilityReport {
                outcome: ReachabilityOutcome::BoundExhausted {
                    limit: LimitHit::MaxStates,
                },
                counters: ExploreCounters {
                    explored_states: explored_states - 1,
                    explored_transitions,
                    max_depth_reached,
                },
                bounds,
            };
        }

        let events = system.enabled_events(&state);
        if events.is_empty() {
            if !system.is_terminal(&state) {
                let trace = reconstruct_trace(&parent, &state);
                return ReachabilityReport {
                    outcome: ReachabilityOutcome::DeadState { trace },
                    counters: ExploreCounters {
                        explored_states,
                        explored_transitions,
                        max_depth_reached,
                    },
                    bounds,
                };
            }
            continue;
        }

        if depth >= bounds.max_depth {
            depth_bound_hit = true;
            continue;
        }

        for event in events {
            if explored_transitions >= bounds.max_transitions {
                return ReachabilityReport {
                    outcome: ReachabilityOutcome::BoundExhausted {
                        limit: LimitHit::MaxTransitions,
                    },
                    counters: ExploreCounters {
                        explored_states,
                        explored_transitions,
                        max_depth_reached,
                    },
                    bounds,
                };
            }
            let Some(next) = system.apply(&state, &event) else {
                continue;
            };
            explored_transitions += 1;
            if target(&next) {
                let mut trace = reconstruct_trace(&parent, &state);
                trace.push(Step {
                    from: state.clone(),
                    event,
                    to: next,
                });
                return ReachabilityReport {
                    outcome: ReachabilityOutcome::Reached { trace },
                    counters: ExploreCounters {
                        explored_states,
                        explored_transitions,
                        max_depth_reached,
                    },
                    bounds,
                };
            }
            if visited.insert(next.clone()) {
                parent.insert(next.clone(), (state.clone(), event));
                frontier.push_back((next, depth + 1));
            }
        }
    }

    let outcome = if depth_bound_hit {
        ReachabilityOutcome::BoundExhausted {
            limit: LimitHit::MaxDepth,
        }
    } else {
        ReachabilityOutcome::NeverReached
    };
    ReachabilityReport {
        outcome,
        counters: ExploreCounters {
            explored_states,
            explored_transitions,
            max_depth_reached,
        },
        bounds,
    }
}
