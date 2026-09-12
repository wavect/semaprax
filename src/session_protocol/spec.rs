//! The protocol declaration model: named states, transitions, branching,
//! terminal states, and the closed set of static checks a [`ProtocolSpec`]
//! must pass before any [`super::engine::SessionTable`] may run against it.
//!
//! A `ProtocolSpec` is plain owned data (no closures, no trait objects), so
//! it can be constructed once, validated once, and shared by reference
//! across every session the engine opens against it -- the same posture
//! [`crate::resumable_effects::core::ResumableEffectProgram`] takes for its
//! own caller-supplied program, generalized here to a *declared* state
//! machine instead of a Rust trait implementation, because a session
//! protocol's states/transitions are exactly the data issue #206 asks a
//! protocol *declaration* (eventually `.spx` syntax) to carry.

use std::collections::BTreeSet;

/// A protocol's state name. Stable within one [`ProtocolSpec`] value; not a
/// persistent cross-revision `@id` -- see the crate doc `Status` section.
pub type StateId = &'static str;

/// A transition's message/operation label. Unique per originating state
/// within one spec ([`SpecError::DuplicateLabelFromState`] rejects a
/// collision), never unique across the whole protocol.
pub type Label = &'static str;

/// What kind of protocol operation one transition represents.
///
/// `Send`/`Receive` name a message direction. `Call` opens a pending
/// in-flight operation that only its own matching `Return` (or an explicit
/// `Cancel`/`Timeout`/`Fail`) resolves --
/// [`super::engine::SessionTable::checkpoint`] refuses to checkpoint while
/// one is outstanding, so a checkpoint can never resume into operation the
/// engine itself is uncertain actually completed. `Cancel`, `Timeout`, and
/// `Fail` are explicit, declared escape transitions: this repository's
/// invariant that failure/cancellation/uncertainty stay explicit forbids
/// modeling any of the three as an implicit exception path that bypasses
/// the declared state machine and its cleanup plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Kind {
    Send,
    Receive,
    Call,
    Return,
    Cancel,
    Timeout,
    Fail,
}

impl Kind {
    /// Whether taking this transition leaves the destination state with an
    /// outstanding pending operation (see [`Kind::Call`]'s doc).
    pub fn opens_pending(&self) -> bool {
        matches!(self, Kind::Call)
    }
    /// Whether this transition is one of the three explicit escape kinds
    /// that count as "cancellation/cleanup is defined" for
    /// [`SpecError::MissingEscape`].
    pub fn is_escape(&self) -> bool {
        matches!(self, Kind::Cancel | Kind::Timeout | Kind::Fail)
    }
}

/// Whether a transition moves ownership of a caller-presented resource, and
/// how. This is deliberately narrower than the compiler's own
/// alias/uniqueness ownership analysis (see the crate doc `Status`
/// section): it names one closed vocabulary a session-protocol transition
/// can declare, not a general ownership type system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMove {
    /// The endpoint's own protocol state changes; no other resource token
    /// is consumed.
    None,
    /// The transition consumes a caller-presented [`super::engine::ResourceToken`].
    /// The same token identity cannot be presented again for any
    /// `ConsumesResource` transition afterward, in this session or any
    /// other -- see [`super::engine::ProtocolError::ResourceAlreadyConsumed`].
    ConsumesResource,
}

/// The declared next state(s) of one transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Next {
    /// An unconditional single next state.
    Then(StateId),
    /// A closed, exhaustive named choice: [`ProtocolSpec::validate`]
    /// requires at least two distinct choice labels (a one-choice "branch"
    /// is not a branch) and rejects a duplicate choice label.
    Choice(Vec<(Label, StateId)>),
}

/// One declared transition: a legal message/operation from one state, the
/// payload it names, the capability it requires (if any), the ownership it
/// moves, and its next state(s).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    pub from: StateId,
    pub label: Label,
    pub kind: Kind,
    /// A closed tag naming the payload type this transition carries. This
    /// is a lightweight tag comparison the engine performs so a wrong
    /// payload is rejected before dispatch; it is not a substitute for the
    /// compiler's own payload type checking (out of scope -- see the crate
    /// doc "Explicitly out of scope" list).
    pub payload_type: &'static str,
    /// The capability an operation must independently present (as a
    /// [`super::capability::Grant`], conceptually) to exercise this
    /// transition. Reaching `from` in the right order is necessary but
    /// never *sufficient* by itself when this is `Some` -- a protocol state
    /// is not authority; see [`super::engine::ProtocolError::MissingAuthority`].
    pub required_capability: Option<&'static str>,
    pub ownership: OwnershipMove,
    pub next: Next,
}

/// A declared protocol: its full state set, initial state, terminal states,
/// transitions, and the canonical (never sorted, never repaired) cleanup
/// inventory for each terminal state.
#[derive(Clone, Debug)]
pub struct ProtocolSpec {
    pub name: &'static str,
    pub states: BTreeSet<StateId>,
    pub initial: StateId,
    pub terminal: BTreeSet<StateId>,
    pub transitions: Vec<Transition>,
    /// Terminal state -> its canonical, ordered cleanup-op inventory. This
    /// vector is runtime order, exactly as
    /// `crate::cleanup_plan`/`crate::resumable_effects` require elsewhere in
    /// this codebase: the engine executes it front-to-back, exactly once,
    /// and never sorts, reorders, or repairs it.
    pub cleanup: Vec<(StateId, Vec<&'static str>)>,
}

/// Why a [`ProtocolSpec`] failed static validation. Each variant names a
/// distinct, stable defect so a caller can tell which closed rule a
/// malformed declaration broke.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpecError {
    /// `initial` does not name a state in `states`.
    UnknownInitialState,
    /// A terminal state does not name a state in `states`.
    UnknownTerminalState { state: StateId },
    /// A transition's `from`, or a `Next` target, names a state outside
    /// `states`.
    UnknownState { label: Label, state: StateId },
    /// Two transitions from the same state declared the same label.
    DuplicateLabelFromState { state: StateId, label: Label },
    /// A `Next::Choice` had fewer than two entries.
    ChoiceNeedsAtLeastTwo { state: StateId, label: Label },
    /// A `Next::Choice` repeated one choice label.
    DuplicateChoiceLabel {
        state: StateId,
        label: Label,
        choice: Label,
    },
    /// A transition's `from` names an already-terminal state: terminal
    /// states must have no outgoing transitions.
    TerminalStateHasOutgoingTransition { state: StateId, label: Label },
    /// A non-terminal state has no outgoing transition at all: an endpoint
    /// parked there could never legally proceed or escape.
    DeadEnd { state: StateId },
    /// A non-terminal state has outgoing transitions but none of them is a
    /// declared `Cancel`/`Timeout`/`Fail` escape: no cancellation or cleanup
    /// path is defined for it, so an endpoint may never be abandoned there
    /// (the engine's [`super::engine::Endpoint`] enforces this at runtime
    /// with a drop bomb; this static check catches the declaration-level
    /// defect before any endpoint is ever opened).
    MissingEscape { state: StateId },
    /// A terminal state has no entry in `cleanup` (an explicitly empty
    /// vector is fine and means "no cleanup needed"; a wholly missing entry
    /// is a declaration defect).
    MissingCleanupEntry { state: StateId },
}

impl ProtocolSpec {
    /// Validate every static rule this module owns. Collects every defect
    /// found rather than stopping at the first, the way this codebase's
    /// diagnostic passes generally do, so a caller sees the whole
    /// declaration's problems in one pass.
    pub fn validate(&self) -> Result<(), Vec<SpecError>> {
        let mut errors = Vec::new();

        if !self.states.contains(self.initial) {
            errors.push(SpecError::UnknownInitialState);
        }
        for state in &self.terminal {
            if !self.states.contains(state) {
                errors.push(SpecError::UnknownTerminalState { state });
            }
        }

        let mut seen_labels: std::collections::BTreeMap<StateId, BTreeSet<Label>> =
            std::collections::BTreeMap::new();
        let mut has_outgoing: BTreeSet<StateId> = BTreeSet::new();
        let mut has_escape: BTreeSet<StateId> = BTreeSet::new();

        for t in &self.transitions {
            if !self.states.contains(t.from) {
                errors.push(SpecError::UnknownState {
                    label: t.label,
                    state: t.from,
                });
            } else if self.terminal.contains(t.from) {
                errors.push(SpecError::TerminalStateHasOutgoingTransition {
                    state: t.from,
                    label: t.label,
                });
            } else {
                has_outgoing.insert(t.from);
                if t.kind.is_escape() {
                    has_escape.insert(t.from);
                }
            }

            let labels = seen_labels.entry(t.from).or_default();
            if !labels.insert(t.label) {
                errors.push(SpecError::DuplicateLabelFromState {
                    state: t.from,
                    label: t.label,
                });
            }

            match &t.next {
                Next::Then(target) => {
                    if !self.states.contains(target) {
                        errors.push(SpecError::UnknownState {
                            label: t.label,
                            state: target,
                        });
                    }
                }
                Next::Choice(choices) => {
                    if choices.len() < 2 {
                        errors.push(SpecError::ChoiceNeedsAtLeastTwo {
                            state: t.from,
                            label: t.label,
                        });
                    }
                    let mut choice_labels = BTreeSet::new();
                    for (choice, target) in choices {
                        if !self.states.contains(target) {
                            errors.push(SpecError::UnknownState {
                                label: t.label,
                                state: target,
                            });
                        }
                        if !choice_labels.insert(*choice) {
                            errors.push(SpecError::DuplicateChoiceLabel {
                                state: t.from,
                                label: t.label,
                                choice,
                            });
                        }
                    }
                }
            }
        }

        for state in self.states.iter().filter(|s| !self.terminal.contains(*s)) {
            if !has_outgoing.contains(state) {
                errors.push(SpecError::DeadEnd { state });
            } else if !has_escape.contains(state) {
                errors.push(SpecError::MissingEscape { state });
            }
        }

        let cleanup_states: BTreeSet<StateId> = self.cleanup.iter().map(|(s, _)| *s).collect();
        for state in &self.terminal {
            if !cleanup_states.contains(state) {
                errors.push(SpecError::MissingCleanupEntry { state });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// The declared transitions leaving `state`, in declaration order.
    pub fn transitions_from(&self, state: StateId) -> impl Iterator<Item = &Transition> {
        self.transitions.iter().filter(move |t| t.from == state)
    }

    /// The canonical cleanup inventory for a terminal state, or an empty
    /// slice if `state` has no entry (callers should prefer validating the
    /// spec first, which rejects a missing terminal entry outright).
    pub fn cleanup_for(&self, state: StateId) -> &[&'static str] {
        self.cleanup
            .iter()
            .find(|(s, _)| *s == state)
            .map(|(_, ops)| ops.as_slice())
            .unwrap_or(&[])
    }
}
