//! The runtime session/endpoint engine: an affine typed resource
//! ([`Endpoint`]) parameterized by protocol and current state, and the
//! authoritative [`SessionTable`] that checks every operation against a
//! [`super::spec::ProtocolSpec`] before advancing it.
//!
//! Two independent layers enforce "no illegal sequence, no reuse of a
//! consumed handle":
//!
//! 1. **Compile-time (Rust ownership).** [`SessionTable::advance`] takes
//!    its `Endpoint` argument by value. Presenting the same live binding to
//!    two operations is therefore not a runtime check at all -- it is
//!    `rustc` rejecting a use of a moved value (see the crate doc
//!    `compile_fail` doctest). An `Endpoint` that is dropped in a
//!    non-terminal state without ever being consumed by a resolving
//!    operation panics via [`Drop for Endpoint`] -- "no abandoned
//!    nonterminal endpoint unless cancellation/cleanup is defined" is
//!    enforced as a linear-type drop bomb, not a lint.
//! 2. **Runtime ([`SessionTable`]).** A protocol's states/transitions are
//!    declared *data* (issue #206 asks for a general model, not one Rust
//!    type per protocol), so the table independently checks message order,
//!    payload tag, required capability, ownership movement, and handle
//!    freshness (`generation`) against the declared [`super::spec::ProtocolSpec`]
//!    every time -- exactly the layer that catches a *duplicated* handle
//!    (one that crossed a serialization boundary and so is no longer the
//!    same Rust value the move-checker can reason about).

use std::collections::{BTreeMap, BTreeSet};

use super::spec::{Kind, Label, Next, OwnershipMove, ProtocolSpec, StateId};

/// One live session's authoritative record. Never itself capable of
/// performing an operation: it is what [`SessionTable::advance`] consults
/// and updates, not a bearer credential a caller holds directly.
#[derive(Clone, Debug)]
struct SessionRecord {
    state: StateId,
    generation: u64,
    /// `Some(label)` between a `Call` transition and its resolving
    /// `Return`/escape; [`SessionTable::checkpoint`] refuses while this is
    /// `Some` because whether the in-flight operation actually completed is
    /// uncertain.
    pending: Option<Label>,
    closed: bool,
}

/// An affine endpoint: one live session's current protocol state, bound to
/// one `session_id`. Fields are `pub(crate)` only so [`super::tests`] can
/// construct a deliberately stale duplicate to exercise
/// [`ProtocolError::StaleHandle`] -- the one scenario ordinary Rust move
/// semantics cannot model in-process, because it represents a handle that
/// crossed a serialization boundary and came back as a second, independent
/// value. No public API in this module ever hands out such a duplicate.
#[derive(Debug)]
pub struct Endpoint {
    pub(crate) session_id: String,
    pub(crate) state: StateId,
    pub(crate) generation: u64,
    pub(crate) is_terminal: bool,
    pub(crate) defused: bool,
}

impl Endpoint {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn state(&self) -> StateId {
        self.state
    }
    pub fn is_terminal(&self) -> bool {
        self.is_terminal
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        if !self.defused && !self.is_terminal && !std::thread::panicking() {
            panic!(
                "session_protocol: endpoint '{}' abandoned in nonterminal state '{}' \
                 without a defined cancellation/cleanup transition -- a live nonterminal \
                 endpoint may never simply be dropped",
                self.session_id, self.state
            );
        }
    }
}

/// A caller-presented resource token an `OwnershipMove::ConsumesResource`
/// transition consumes. `Clone` is intentional: it models the exact hazard
/// the crate doc's failure-case list names ("serializing endpoints can
/// recreate authority") -- a caller *can* duplicate a token's identity, but
/// [`SessionTable`] tracks consumed token ids centrally, so presenting a
/// clone of an already-consumed token is still rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceToken {
    pub id: String,
}

/// Why one `SessionTable` operation was refused. Each variant is a
/// distinct, stable reason so a caller (or a test) can tell a duplicate
/// open apart from an out-of-order message, a stale handle, a missing
/// capability, or a consumed resource token -- never one generic "protocol
/// error".
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    /// `open` was called twice for the same `session_id` (whether or not
    /// the first is still live).
    DuplicateOpen { session_id: String },
    /// The session id named by an `Endpoint` has no record in this table.
    UnknownSession { session_id: String },
    /// The session already reached a terminal state; no further operation
    /// is legal on it.
    UseAfterTerminal { session_id: String, state: StateId },
    /// The presented `Endpoint`'s generation does not match the table's
    /// current generation for this session id: the handle is stale (either
    /// superseded by a later operation, or a duplicate that crossed a
    /// serialization boundary).
    StaleHandle {
        session_id: String,
        presented_generation: u64,
        current_generation: u64,
    },
    /// No transition named `label` exists from the session's current
    /// state: the message is out of order for this protocol.
    IllegalTransition {
        session_id: String,
        state: StateId,
        label: Label,
    },
    /// A branch transition was taken but no choice (or an unrecognized
    /// choice) was presented for it.
    UnknownBranchChoice {
        session_id: String,
        state: StateId,
        label: Label,
        choice: Option<Label>,
    },
    /// The transition requires a capability the caller did not present, or
    /// presented the wrong one. Reaching this exact state in the exact
    /// right order was necessary but never sufficient by itself -- a
    /// protocol state is not authority.
    MissingAuthority {
        session_id: String,
        label: Label,
        required: &'static str,
    },
    /// The presented payload tag does not match the transition's declared
    /// tag.
    PayloadTypeMismatch {
        session_id: String,
        label: Label,
        expected: &'static str,
        presented: &'static str,
    },
    /// The transition consumes a resource token but none was presented.
    ResourceTokenRequired { session_id: String, label: Label },
    /// The presented resource token id was already consumed by an earlier
    /// `ConsumesResource` transition, in this session or any other.
    ResourceAlreadyConsumed {
        session_id: String,
        label: Label,
        token: String,
    },
}

/// Why [`SessionTable::checkpoint`] refused to produce a resumable
/// snapshot. Only a settled, non-pending state may be automatically
/// resumed: "only serializable protocol states with no uncertain in-flight
/// operation may be resumed automatically" (issue #206, step 8).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointError {
    UnknownSession,
    StaleHandle,
    /// A `Call` transition's matching `Return`/escape has not yet been
    /// recorded: whether the physical effect completed is uncertain.
    InFlightCall { label: Label },
}

/// A resumable snapshot: a session id, its settled state, and the
/// generation it was settled at. Carries no capability and no resource
/// token -- resuming from it still requires the caller to independently
/// present whatever authority the next operation needs, exactly as
/// `EffectScope` never becomes a bearer credential in
/// `crate::resumable_effects`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    pub session_id: String,
    pub state: StateId,
    pub generation: u64,
}

/// The result of one terminal-reaching `advance` call: the cleanup
/// inventory the reached state declares, run in exact declared order,
/// exactly once. A cleanup entry's own failure is recorded here; it never
/// changes which transition kind was actually taken to reach this terminal
/// state (failure selection is sticky).
#[derive(Clone, Debug)]
pub struct TerminalOutcome {
    pub terminal_state: StateId,
    pub terminal_kind: Kind,
    pub cleanup: Vec<(&'static str, Result<(), String>)>,
}

/// What one `advance` call produced: either a still-live endpoint at its
/// new state, or a terminal outcome plus the now-terminal endpoint (whose
/// `Drop` is harmless without further action).
#[derive(Debug)]
pub enum AdvanceOutcome {
    Live(Endpoint),
    Terminal(Endpoint, TerminalOutcome),
}

/// The injected cleanup boundary, called once per declared cleanup-op name
/// in a terminal state's canonical (never sorted, never repaired) order.
pub trait CleanupHandler {
    fn run(&mut self, session_id: &str, terminal_state: StateId, op: &'static str) -> Result<(), String>;
}

/// The authoritative registry for every session opened against one
/// [`ProtocolSpec`]. Constructing one does not itself validate the spec;
/// callers should call [`ProtocolSpec::validate`] first, the same
/// "validate before trusting" posture `crate::resumable_effects::Journal`
/// documents for a recovered journal.
pub struct SessionTable<'p> {
    spec: &'p ProtocolSpec,
    records: BTreeMap<String, SessionRecord>,
    consumed_tokens: BTreeSet<String>,
}

impl<'p> SessionTable<'p> {
    pub fn new(spec: &'p ProtocolSpec) -> Self {
        Self {
            spec,
            records: BTreeMap::new(),
            consumed_tokens: BTreeSet::new(),
        }
    }

    pub fn spec(&self) -> &ProtocolSpec {
        self.spec
    }

    /// Open a fresh session under `session_id`. Refuses a duplicate open
    /// whether or not the earlier session with this id is still live.
    pub fn open(&mut self, session_id: impl Into<String>) -> Result<Endpoint, ProtocolError> {
        let session_id = session_id.into();
        if self.records.contains_key(&session_id) {
            return Err(ProtocolError::DuplicateOpen { session_id });
        }
        let is_terminal = self.spec.terminal.contains(self.spec.initial);
        self.records.insert(
            session_id.clone(),
            SessionRecord {
                state: self.spec.initial,
                generation: 0,
                pending: None,
                closed: is_terminal,
            },
        );
        Ok(Endpoint {
            session_id,
            state: self.spec.initial,
            generation: 0,
            is_terminal,
            defused: false,
        })
    }

    /// Attempt one operation. `old` is consumed: on success it is replaced
    /// by a fresh `Endpoint` at the new state; on failure a fresh,
    /// still-live `Endpoint` at the *unchanged* state is handed back so the
    /// caller can retry or explicitly cancel it -- the operation's own
    /// failure never abandons the session.
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        mut old: Endpoint,
        label: Label,
        payload_type: &'static str,
        presented_capability: Option<&'static str>,
        branch_choice: Option<Label>,
        resource_token: Option<&ResourceToken>,
        cleanup_handler: &mut dyn CleanupHandler,
    ) -> Result<AdvanceOutcome, (ProtocolError, Endpoint)> {
        // This call is the legitimate consumption of `old`; its own Drop
        // must not treat that as abandonment even if this operation goes
        // on to fail (the caller gets a fresh, equally live replacement
        // back on the error path below).
        old.defused = true;
        let session_id = old.session_id.clone();

        fn still_live(session_id: &str, state: StateId, generation: u64) -> Endpoint {
            endpoint_at(session_id, state, generation, false)
        }
        fn endpoint_at(session_id: &str, state: StateId, generation: u64, is_terminal: bool) -> Endpoint {
            Endpoint {
                session_id: session_id.to_string(),
                state,
                generation,
                is_terminal,
                defused: false,
            }
        }

        let Some(record) = self.records.get(&session_id) else {
            return Err((
                ProtocolError::UnknownSession { session_id: session_id.clone() },
                still_live(&session_id, old.state, old.generation),
            ));
        };
        if record.closed {
            let state = record.state;
            // The record really is terminal: handing back an endpoint that
            // is not flagged terminal would wrongly arm the abandonment
            // drop bomb for a session that is legitimately already done.
            return Err((
                ProtocolError::UseAfterTerminal { session_id: session_id.clone(), state },
                endpoint_at(&session_id, state, record.generation, true),
            ));
        }
        if record.generation != old.generation {
            let (state, generation) = (record.state, record.generation);
            return Err((
                ProtocolError::StaleHandle {
                    session_id: session_id.clone(),
                    presented_generation: old.generation,
                    current_generation: generation,
                },
                still_live(&session_id, state, generation),
            ));
        }

        let state = record.state;
        let generation = record.generation;

        let Some(transition) = self.spec.transitions_from(state).find(|t| t.label == label) else {
            return Err((
                ProtocolError::IllegalTransition { session_id: session_id.clone(), state, label },
                still_live(&session_id, state, generation),
            ));
        };

        if transition.payload_type != payload_type {
            return Err((
                ProtocolError::PayloadTypeMismatch {
                    session_id: session_id.clone(),
                    label,
                    expected: transition.payload_type,
                    presented: payload_type,
                },
                still_live(&session_id, state, generation),
            ));
        }

        if let Some(required) = transition.required_capability {
            if presented_capability != Some(required) {
                return Err((
                    ProtocolError::MissingAuthority { session_id: session_id.clone(), label, required },
                    still_live(&session_id, state, generation),
                ));
            }
        }

        match transition.ownership {
            OwnershipMove::None => {}
            OwnershipMove::ConsumesResource => match resource_token {
                None => {
                    return Err((
                        ProtocolError::ResourceTokenRequired { session_id: session_id.clone(), label },
                        still_live(&session_id, state, generation),
                    ));
                }
                Some(token) => {
                    if self.consumed_tokens.contains(&token.id) {
                        return Err((
                            ProtocolError::ResourceAlreadyConsumed {
                                session_id: session_id.clone(),
                                label,
                                token: token.id.clone(),
                            },
                            still_live(&session_id, state, generation),
                        ));
                    }
                }
            },
        }

        let target = match &transition.next {
            Next::Then(target) => *target,
            Next::Choice(choices) => match branch_choice {
                Some(choice) => match choices.iter().find(|(c, _)| *c == choice) {
                    Some((_, target)) => *target,
                    None => {
                        return Err((
                            ProtocolError::UnknownBranchChoice {
                                session_id: session_id.clone(),
                                state,
                                label,
                                choice: Some(choice),
                            },
                            still_live(&session_id, state, generation),
                        ));
                    }
                },
                None => {
                    return Err((
                        ProtocolError::UnknownBranchChoice {
                            session_id: session_id.clone(),
                            state,
                            label,
                            choice: None,
                        },
                        still_live(&session_id, state, generation),
                    ));
                }
            },
        };

        // Every check passed: commit the token (if any), advance the
        // record, and construct the next endpoint.
        if let OwnershipMove::ConsumesResource = transition.ownership {
            if let Some(token) = resource_token {
                self.consumed_tokens.insert(token.id.clone());
            }
        }

        let transition_kind = transition.kind;
        let new_pending = if transition_kind.opens_pending() {
            Some(label)
        } else if matches!(transition_kind, Kind::Return) || transition_kind.is_escape() {
            None
        } else {
            self.records.get(&session_id).unwrap().pending
        };
        let new_generation = generation + 1;
        let is_terminal_new = self.spec.terminal.contains(target);

        let record = self.records.get_mut(&session_id).unwrap();
        record.state = target;
        record.generation = new_generation;
        record.pending = new_pending;
        record.closed = is_terminal_new;

        let new_endpoint = Endpoint {
            session_id: session_id.clone(),
            state: target,
            generation: new_generation,
            is_terminal: is_terminal_new,
            defused: false,
        };

        if is_terminal_new {
            let plan = self.spec.cleanup_for(target);
            let mut results = Vec::with_capacity(plan.len());
            for op in plan.iter().copied() {
                let outcome = cleanup_handler.run(&session_id, target, op);
                results.push((op, outcome));
            }
            Ok(AdvanceOutcome::Terminal(
                new_endpoint,
                TerminalOutcome {
                    terminal_state: target,
                    terminal_kind: transition_kind,
                    cleanup: results,
                },
            ))
        } else {
            Ok(AdvanceOutcome::Live(new_endpoint))
        }
    }

    /// Produce a resumable snapshot of `endpoint`'s current settled state.
    /// Refuses while a `Call` this session issued has not yet been resolved
    /// by its matching `Return`/escape (see [`CheckpointError::InFlightCall`]).
    pub fn checkpoint(&self, endpoint: &Endpoint) -> Result<Checkpoint, CheckpointError> {
        let record = self
            .records
            .get(&endpoint.session_id)
            .ok_or(CheckpointError::UnknownSession)?;
        if record.generation != endpoint.generation {
            return Err(CheckpointError::StaleHandle);
        }
        if let Some(label) = record.pending {
            return Err(CheckpointError::InFlightCall { label });
        }
        Ok(Checkpoint {
            session_id: endpoint.session_id.clone(),
            state: record.state,
            generation: record.generation,
        })
    }
}
