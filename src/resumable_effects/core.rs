//! The generic suspend/resume driver, its typed effect journal, and the
//! scope binding that keeps a resumed journal from being reused under a
//! different program, invocation, or policy.
//!
//! This generalizes the vocabulary `agent_lifecycle::iterative` and
//! `agent_runtime_v2::checkpoint` already ship for one closed six-role
//! Agent shape (`Continue`/`Complete`/`Suspend`/`Fail` steps; `Intent` /
//! `Observed` / `Transition` journal entries) to an arbitrary
//! [`ResumableEffectProgram`], the way [`crate::live_invocation`] already
//! generalized the `model.invoke` boundary specifically. Nothing here reads
//! an `AgentDefinition`, decodes a six-role runtime document, or touches
//! `agent_lifecycle`/`agent_runtime_v2`; the driver's `Request`,
//! `Observation`, `State`, `Result` and cleanup-op types are the caller's
//! own, monomorphized per program the way `LiveInvocationHandlers` are
//! monomorphized per deployment.

use std::fmt::Debug;

/// The exact pre-resume bytes one resumable computation's journal is named
/// from, generalizing `live_invocation::identity::LiveInvocationSeed`. Every
/// [`JournalEntry`] carries a copy; [`Journal::validate`] rejects any entry
/// whose scope differs from the journal's own first entry, and [`resume`]
/// separately requires the *caller* to supply the scope it independently
/// expects right now. A journal can misdescribe or omit a turn; it can
/// never mint authority to resume under a scope the caller did not itself
/// derive, so a copied or replayed journal cannot become a bearer token for
/// a different program root, invocation, or policy epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectScope {
    /// The exact ProgramRoot (or equivalent checked-source identity) this
    /// computation executes against.
    pub program_root: String,
    /// The identity of this one resumable invocation, stable across
    /// suspend/resume the same way `LiveInvocationSeed` is.
    pub invocation_id: String,
    /// The deployment/authority policy generation in force. Bumping this
    /// invalidates every journal minted under an earlier epoch.
    pub policy_epoch: u64,
}

/// One turn's outcome. `Continue` feeds its state to the next turn;
/// `Complete`, `Suspend` and `Fail` are terminal and stop the driver.
/// Suspension is data, not durable restart authority by itself — resuming
/// still requires the caller to supply a matching [`EffectScope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step<S, R> {
    Continue(S),
    Suspend(S),
    Complete(R),
    Fail(i64),
}

impl<S, R> Step<S, R> {
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Step::Continue(_))
    }
}

/// A caller-defined resumable computation. One implementor is one
/// monomorphic effect shape, generalizing what the closed six-role Agent
/// shape hard-codes today: `request` decides whether a turn is a
/// suspension point at all, and `transition` is the single deterministic
/// function called identically whether its observation was just dispatched
/// or replayed from a trusted journal — that identity of call is what makes
/// replay a re-*use* of trusted recorded observations rather than a second,
/// possibly divergent, re-*execution*.
///
/// `State`, `Result`, `Request`, `Observation` and `CleanupOp` must each be
/// `Clone + Eq + Debug + 'static`: plain owned, comparable, non-borrowing
/// data. That bound is this reference validator's compile-time ownership
/// gate — it is what a borrow, a raw pointer, or a non-`'static` opaque
/// host handle cannot satisfy (see the module-level `compile_fail` doctest).
/// It is not the compiler's full alias/uniqueness ownership analysis; wiring
/// this trait to real checked HIR locals so the *compiler itself* rejects an
/// uncrossable local is exactly the parser/HIR/verifier generalization this
/// reference module intentionally leaves open (see the crate doc `Status`
/// section).
pub trait ResumableEffectProgram {
    type State: Clone + Eq + Debug + 'static;
    type Result: Clone + Eq + Debug + 'static;
    type Request: Clone + Eq + Debug + 'static;
    type Observation: Clone + Eq + Debug + 'static;
    type CleanupOp: Clone + Eq + Debug + 'static;

    /// Deterministically decide, from `state` alone, whether this turn
    /// issues a typed effect request (a suspension point) before it may
    /// transition. `None` means this turn transitions directly with no
    /// observation.
    fn request(&self, state: &Self::State) -> Option<Self::Request>;

    /// Deterministically compute the next [`Step`] from `state` and, if
    /// `request` returned `Some`, that request's resolved observation.
    /// Must be a pure function of its arguments: the driver calls it once
    /// per genuinely new turn and, during replay, calls it again with the
    /// journal's trusted recorded observation to confirm — never to
    /// re-derive — the recorded transition.
    fn transition(
        &self,
        state: &Self::State,
        observation: Option<&Self::Observation>,
    ) -> Step<Self::State, Self::Result>;

    /// The canonical, ordered cleanup inventory for a terminal state. This
    /// vector is runtime order: the driver executes it front-to-back,
    /// exactly once, and never sorts, reorders or repairs it.
    fn cleanup_plan(&self, state: &Self::State) -> Vec<Self::CleanupOp>;
}

/// The injected physical effect boundary. Only this call may perform work
/// with real authority; the driver itself never gains ambient filesystem,
/// process, network, or model authority merely by running a suspend/resume
/// program — a settlement or concurrency model is proof data here exactly
/// as it is everywhere else in this codebase, never a permission to act.
pub trait EffectHandler<Req, Obs> {
    fn dispatch(&mut self, request: &Req) -> Result<Obs, String>;
}

/// The injected cleanup boundary, called once per cleanup-plan entry in
/// plan order. A cleanup failure is recorded, never silently discarded and
/// never allowed to replace the already-selected terminal status.
pub trait CleanupHandler<Op> {
    fn run(&mut self, op: &Op) -> Result<(), String>;
}

/// One journal record. Effect turns record an `Intent`/(`Observed` or
/// `ObservationFailed`) pair before their `Transition`; a turn with no
/// requested effect records only a `Transition`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalEntry<P: ResumableEffectProgram> {
    Intent {
        turn: u32,
        scope: EffectScope,
        request: P::Request,
    },
    Observed {
        turn: u32,
        scope: EffectScope,
        request: P::Request,
        observation: P::Observation,
    },
    ObservationFailed {
        turn: u32,
        scope: EffectScope,
        request: P::Request,
        reason: String,
    },
    Transition {
        turn: u32,
        scope: EffectScope,
        step: Step<P::State, P::Result>,
    },
}

impl<P: ResumableEffectProgram> JournalEntry<P> {
    fn turn(&self) -> u32 {
        match self {
            JournalEntry::Intent { turn, .. }
            | JournalEntry::Observed { turn, .. }
            | JournalEntry::ObservationFailed { turn, .. }
            | JournalEntry::Transition { turn, .. } => *turn,
        }
    }
    fn scope(&self) -> &EffectScope {
        match self {
            JournalEntry::Intent { scope, .. }
            | JournalEntry::Observed { scope, .. }
            | JournalEntry::ObservationFailed { scope, .. }
            | JournalEntry::Transition { scope, .. } => scope,
        }
    }
}

/// Why a journal was rejected before it was trusted for replay or resume.
/// Each variant is a distinct, stable reason so a caller (or a test) can
/// tell a stale ProgramRoot apart from a wrong invocation, a wrong policy
/// epoch, plain corruption, or an in-flight snapshot too uncertain to trust.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalError {
    /// An entry named a different ProgramRoot than the journal (or the
    /// caller's freshly supplied expected scope) is bound to.
    StaleProgramRoot { at: usize },
    /// An entry named a different invocation identity.
    WrongInvocation { at: usize },
    /// An entry named a different, and therefore possibly reminted, policy
    /// epoch. A resume token minted under an old epoch cannot become a
    /// bearer credential for a new one.
    WrongPolicyEpoch { at: usize },
    /// Entries for one turn did not appear in the required
    /// Intent -> (Observed | ObservationFailed) -> Transition order.
    OutOfOrder { at: usize },
    /// An Observed/ObservationFailed/Transition entry named a request that
    /// does not match its turn's Intent.
    RequestMismatch { at: usize },
    /// Turn numbers did not advance by exactly one after a `Continue`, or
    /// did not start at zero.
    NonSequentialTurn { at: usize },
    /// An entry followed an already-terminal `Transition`.
    EntryAfterTerminal { at: usize },
    /// The journal's last entry is an `Intent` with no matching
    /// observation: whether the physical dispatch happened is uncertain,
    /// so the journal must not be trusted for replay or further resume
    /// (mirrors `agent_runtime_v2::checkpoint`'s identical rule).
    UnterminatedIntent,
}

/// An ordered, append-only record of one resumable computation. Entries are
/// pushed in the exact order the driver produced them and are never sorted,
/// reordered, or repaired — the same canonical-order rule cleanup-plan
/// vectors already carry elsewhere in this codebase.
#[derive(Clone, Debug)]
pub struct Journal<P: ResumableEffectProgram> {
    entries: Vec<JournalEntry<P>>,
}

impl<P: ResumableEffectProgram> Default for Journal<P> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<P: ResumableEffectProgram> Journal<P> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &[JournalEntry<P>] {
        &self.entries
    }

    /// Reconstruct a journal from entries a caller already holds (recovered
    /// from a store, or assembled by a test). This performs no checking by
    /// itself; callers must call [`Journal::validate`] before trusting the
    /// result for replay or resume, exactly as a real recovered checkpoint
    /// must be validated before it is trusted.
    pub fn from_entries(entries: Vec<JournalEntry<P>>) -> Self {
        Self { entries }
    }

    fn push(&mut self, entry: JournalEntry<P>) {
        self.entries.push(entry);
    }

    /// Reject a journal before it is trusted for replay. `expected` is the
    /// scope the *caller* independently derived right now, exactly the way
    /// `LiveInvocationSeed` is re-derived rather than read back off a
    /// journal: a journal cannot mint its own authority to be resumed.
    pub fn validate(&self, expected: &EffectScope) -> Result<(), JournalError> {
        let mut turn_expected = 0u32;
        let mut terminal_seen = false;
        // Which phase of the current turn we expect next.
        #[derive(PartialEq)]
        enum Phase {
            FreshTurn,
            AfterIntent,
            AfterObservation,
        }
        let mut phase = Phase::FreshTurn;
        let mut open_request: Option<&P::Request> = None;

        for (at, entry) in self.entries.iter().enumerate() {
            if terminal_seen {
                return Err(JournalError::EntryAfterTerminal { at });
            }
            if entry.scope().program_root != expected.program_root {
                return Err(JournalError::StaleProgramRoot { at });
            }
            if entry.scope().invocation_id != expected.invocation_id {
                return Err(JournalError::WrongInvocation { at });
            }
            if entry.scope().policy_epoch != expected.policy_epoch {
                return Err(JournalError::WrongPolicyEpoch { at });
            }
            if entry.turn() != turn_expected {
                return Err(JournalError::NonSequentialTurn { at });
            }
            match entry {
                JournalEntry::Intent { request, .. } => {
                    if phase != Phase::FreshTurn {
                        return Err(JournalError::OutOfOrder { at });
                    }
                    open_request = Some(request);
                    phase = Phase::AfterIntent;
                }
                JournalEntry::Observed { request, .. } | JournalEntry::ObservationFailed { request, .. } => {
                    if phase != Phase::AfterIntent {
                        return Err(JournalError::OutOfOrder { at });
                    }
                    if open_request != Some(request) {
                        return Err(JournalError::RequestMismatch { at });
                    }
                    phase = Phase::AfterObservation;
                }
                JournalEntry::Transition { step, .. } => {
                    if phase == Phase::AfterIntent {
                        // an Intent with no Observed is never legally followed
                        // by a Transition; that is caught by UnterminatedIntent
                        // logic below instead of here.
                        return Err(JournalError::OutOfOrder { at });
                    }
                    phase = Phase::FreshTurn;
                    open_request = None;
                    if step.is_terminal() {
                        terminal_seen = true;
                    } else {
                        turn_expected += 1;
                    }
                }
            }
        }
        if phase == Phase::AfterIntent {
            return Err(JournalError::UnterminatedIntent);
        }
        Ok(())
    }
}

/// The result of one `run`/`resume` call.
#[derive(Clone, Debug)]
pub struct Outcome<P: ResumableEffectProgram> {
    /// The selected terminal step. Cleanup below never replaces this value.
    pub terminal: Step<P::State, P::Result>,
    /// Cleanup-plan results in exact canonical plan order, run exactly
    /// once. A failed entry is recorded here, never used to override
    /// `terminal`.
    pub cleanup: Vec<(P::CleanupOp, Result<(), String>)>,
    /// How many *new* physical `EffectHandler::dispatch` calls this one
    /// call performed. Replaying trusted recorded observations never
    /// increments this counter.
    pub dispatched: u32,
}

/// Why a driver call stopped before selecting a terminal step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DriverError {
    Journal(JournalError),
    /// A replayed turn recomputed a different `request` than the journal
    /// recorded: the program is not the deterministic function it claims,
    /// or the journal was tampered with.
    RequestDrift { turn: u32 },
    /// A replayed turn recomputed a different `Transition` than the
    /// journal recorded.
    TransitionDrift { turn: u32 },
    /// The physical handler reported failure for a freshly dispatched
    /// request. The failed intent remains in the returned journal for a
    /// later authorized resume; no transition is recorded for it.
    HandlerFailed { turn: u32, reason: String },
    /// The turn ceiling was reached before a terminal step was selected.
    BudgetExhausted,
    /// The caller's cancellation check returned true at a turn boundary.
    Cancelled,
}

/// Run a fresh resumable computation to its first suspension or terminal
/// step. Success carries the completed journal; failure carries the
/// journal too (the latest durable checkpoint candidate), exactly as a
/// caller needs it for a later authorized resume.
pub fn run<P: ResumableEffectProgram>(
    program: &P,
    scope: EffectScope,
    initial: P::State,
    max_turns: u32,
    handler: &mut dyn EffectHandler<P::Request, P::Observation>,
    cleanup: &mut dyn CleanupHandler<P::CleanupOp>,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Outcome<P>, Journal<P>), (DriverError, Journal<P>)> {
    resume(
        program,
        scope,
        Journal::new(),
        initial,
        max_turns,
        handler,
        cleanup,
        cancelled,
    )
}

/// Resume a computation from a (possibly empty) journal. Every entry the
/// journal already carries is *replayed*: its recorded observation is
/// reused and the handler is never called for it, so resuming a journal
/// that already reached a terminal step performs zero physical dispatches.
/// Only turns past the journal's recorded tail are new attempts with new
/// accounting.
#[allow(clippy::too_many_arguments)]
pub fn resume<P: ResumableEffectProgram>(
    program: &P,
    scope: EffectScope,
    mut journal: Journal<P>,
    initial: P::State,
    max_turns: u32,
    handler: &mut dyn EffectHandler<P::Request, P::Observation>,
    cleanup_handler: &mut dyn CleanupHandler<P::CleanupOp>,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Outcome<P>, Journal<P>), (DriverError, Journal<P>)> {
    if let Err(e) = journal.validate(&scope) {
        return Err((DriverError::Journal(e), journal));
    }

    let mut state = initial;
    let mut turn: u32 = 0;
    let mut dispatched = 0u32;
    let mut cursor = 0usize;

    let terminal = loop {
        if turn >= max_turns {
            return Err((DriverError::BudgetExhausted, journal));
        }
        if cancelled() {
            return Err((DriverError::Cancelled, journal));
        }

        match peek_recorded(&journal, cursor, turn) {
            Recorded::Group(group) => {
                // Replay: recompute deterministically, never dispatch.
                let recomputed_request = program.request(&state);
                match (&recomputed_request, &group.request) {
                    (None, None) => {}
                    (Some(a), Some(b)) if a == b => {}
                    _ => return Err((DriverError::RequestDrift { turn }, journal)),
                }
                let observation = group.observation.as_ref();
                let recomputed = program.transition(&state, observation);
                if recomputed != group.step {
                    return Err((DriverError::TransitionDrift { turn }, journal));
                }
                cursor = group.next_cursor;
                match group.step.clone() {
                    Step::Continue(next) => {
                        state = next;
                        turn += 1;
                        continue;
                    }
                    terminal => break terminal,
                }
            }
            Recorded::Failure { reason } => {
                // A recorded failed dispatch is replayed as the same
                // deterministic failure, never as a fresh host call: the
                // driver never re-executes an effect merely because the
                // same journal is replayed again.
                return Err((DriverError::HandlerFailed { turn, reason }, journal));
            }
            Recorded::None => {}
        }

        // No more recorded entries: genuinely new work from here on. Any
        // fresh dispatch below is a NEW attempt with its own accounting,
        // never a replay of a prior one.
        let request = program.request(&state);
        let observation = match request {
            None => None,
            Some(req) => {
                journal.push(JournalEntry::Intent {
                    turn,
                    scope: scope.clone(),
                    request: req.clone(),
                });
                match handler.dispatch(&req) {
                    Ok(obs) => {
                        dispatched += 1;
                        journal.push(JournalEntry::Observed {
                            turn,
                            scope: scope.clone(),
                            request: req.clone(),
                            observation: obs.clone(),
                        });
                        Some(obs)
                    }
                    Err(reason) => {
                        journal.push(JournalEntry::ObservationFailed {
                            turn,
                            scope: scope.clone(),
                            request: req,
                            reason: reason.clone(),
                        });
                        return Err((DriverError::HandlerFailed { turn, reason }, journal));
                    }
                }
            }
        };
        let step = program.transition(&state, observation.as_ref());
        journal.push(JournalEntry::Transition {
            turn,
            scope: scope.clone(),
            step: step.clone(),
        });
        // The entries just pushed are this call's own fresh work, not a
        // recorded tail to replay; keep the cursor at the journal's current
        // end so the next iteration's `peek_recorded` sees no more recorded
        // entries rather than re-reading what was just written.
        cursor = journal.entries().len();
        match step {
            Step::Continue(next) => {
                state = next;
                turn += 1;
            }
            terminal => break terminal,
        }
    };

    // Cleanup only runs for Complete/Fail; Suspend deliberately keeps the
    // computation's resources live for a later resume. Failure selection is
    // sticky: whatever cleanup reports, `terminal` below is never replaced.
    let cleanup = match &terminal {
        Step::Complete(_) | Step::Fail(_) => {
            let plan = program.cleanup_plan(&state);
            let mut results = Vec::with_capacity(plan.len());
            for op in plan {
                let outcome = cleanup_handler.run(&op);
                results.push((op, outcome));
            }
            results
        }
        Step::Suspend(_) | Step::Continue(_) => Vec::new(),
    };

    Ok((
        Outcome {
            terminal,
            cleanup,
            dispatched,
        },
        journal,
    ))
}

/// One already-recorded turn's group of entries, extracted for replay.
struct RecordedGroup<P: ResumableEffectProgram> {
    request: Option<P::Request>,
    observation: Option<P::Observation>,
    step: Step<P::State, P::Result>,
    next_cursor: usize,
}

/// What the journal already records at `cursor` for `turn`.
enum Recorded<P: ResumableEffectProgram> {
    /// A complete Intent/Observed/Transition (or bare Transition) group:
    /// replay it by recomputing, never dispatching.
    Group(RecordedGroup<P>),
    /// A recorded Intent/ObservationFailed pair with nothing after it: the
    /// original attempt already failed and stopped before any transition.
    /// Replaying this reports the same failure again without recontacting
    /// the handler.
    Failure { reason: String },
    /// No recorded entries remain for `turn`: genuinely new work starts
    /// here.
    None,
}

/// Peek the next recorded turn at `cursor`, if `cursor` is still within the
/// journal and names `turn`.
fn peek_recorded<P: ResumableEffectProgram>(
    journal: &Journal<P>,
    cursor: usize,
    turn: u32,
) -> Recorded<P> {
    let entries = journal.entries();
    let Some(first) = entries.get(cursor) else {
        return Recorded::None;
    };
    if first.turn() != turn {
        return Recorded::None;
    }
    match first {
        JournalEntry::Transition { step, .. } => Recorded::Group(RecordedGroup {
            request: None,
            observation: None,
            step: step.clone(),
            next_cursor: cursor + 1,
        }),
        JournalEntry::Intent { request, .. } => {
            let request = request.clone();
            let Some(second) = entries.get(cursor + 1) else {
                return Recorded::None;
            };
            match second {
                JournalEntry::Observed { observation, .. } => {
                    let observation = observation.clone();
                    let Some(third) = entries.get(cursor + 2) else {
                        return Recorded::None;
                    };
                    match third {
                        JournalEntry::Transition { step, .. } => Recorded::Group(RecordedGroup {
                            request: Some(request),
                            observation: Some(observation),
                            step: step.clone(),
                            next_cursor: cursor + 3,
                        }),
                        _ => Recorded::None,
                    }
                }
                JournalEntry::ObservationFailed { reason, .. } => {
                    if entries.get(cursor + 2).is_some() {
                        // A failed observation is never followed by a
                        // transition in a journal this driver produced;
                        // treat a longer tail as unrecognized rather than
                        // guessing at its meaning.
                        Recorded::None
                    } else {
                        Recorded::Failure {
                            reason: reason.clone(),
                        }
                    }
                }
                _ => Recorded::None,
            }
        }
        JournalEntry::Observed { .. } | JournalEntry::ObservationFailed { .. } => Recorded::None,
    }
}
