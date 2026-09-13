//! Cumulative, nonrefundable budget and deadline accounting behind
//! [`InvocationBudgetHook`] — the real cross-attempt policy issues #113/#179
//! attach behind that one seam (`model_invoke.rs` names it explicitly; the
//! shipped `fixture::FixtureBudgetHook` is deliberately only a trivial
//! per-invocation counter, not this). [`CumulativeBudgetLedger`] is that
//! policy: one monetary ceiling and, optionally, one absolute deadline,
//! enforced identically at every attempt, and both nonrefundable once an
//! attempt has been durably reserved.
//!
//! # Where the decrement happens, relative to dispatch
//!
//! [`CumulativeBudgetLedger::reserve`] both *decides* whether an attempt fits
//! the remaining ceiling and, if it does, *commits* that amount against the
//! ceiling — irrevocably, in the same call, before returning. The kernel
//! (`kernel::run_live_invocation`) calls `reserve` and durably journals its
//! result (`JournalEntry::RequestIntent.reserved_budget`) *before* it ever
//! calls `ModelHandler::invoke` (`kernel.rs`'s `persist` call immediately
//! after pushing `RequestIntent`, run before the dispatch line). So the
//! commit point is always strictly earlier than the point a crash could make
//! irrevocable (the dispatch itself): by the time a call could possibly have
//! reached a provider, its cost is already charged and already durable.
//!
//! # Why that makes a retry unable to double-spend
//!
//! A ledger kept only as private in-memory state would lose everything on a
//! crash between `reserve` and the eventual `record`, and a resumed process
//! reconstructing a *fresh* ledger from just the ceiling would then re-admit
//! the same amount a second time — a double-spend on exactly the attempt
//! that might have already reached the provider. In the kernel's persistence
//! route, the ledger is not the durable source of truth:
//! [`CumulativeBudgetLedger::resume`] reconstructs `committed` by
//! folding over an already-persisted journal prefix and summing every
//! `RequestIntent.reserved_budget` seen so far — the exact value `reserve`
//! already committed and the kernel already made durable *before* dispatch.
//! Replaying that fold after a real or simulated crash always yields the
//! same `committed` total `reserve` produced the first time, so a
//! reservation is nonrefundable by construction: there is no in-memory
//! counter to lose, only a journal the kernel already had to make durable
//! for the "no uncertain intent redispatched" property (`journal.rs`) to
//! hold at all. `budget::tests::resuming_after_a_simulated_crash_never_
//! refunds_the_already_committed_reservation` is the fault-injection test
//! that exercises exactly this: a hand-built journal ending in an uncertain
//! `RequestIntent` (no recorded response — the crash window) still leaves
//! its reservation charged against the ceiling when a fresh ledger resumes
//! from it. The ledger itself performs no persistence. A host using it without
//! that journal route retains reservations only for the lifetime of the ledger
//! and must not claim durable recovery from those in-memory observations.
//!
//! # `record` never refunds
//!
//! [`CumulativeBudgetLedger::record`] only retains [`InvocationUsage`] as
//! evidence; it never reduces `committed`. A settlement using fewer bytes
//! than reserved, or a failed attempt using none at all, never credits the
//! difference back — "monetary limits are conservative reservations...not a
//! promise of exact live billing" (issue #113's own bounded-scope text). This
//! is also why a timeout with unknown billing never appears as zero usage:
//! the reservation it already consumed stays consumed regardless of what (if
//! anything) `record` is later told about the attempt.
//!
//! # Budget-exhausted, deadline-exceeded and cancelled are three different
//! things
//!
//! [`CumulativeBudgetLedger::reserve`] returns a [`super::model_invoke::BudgetRefusal`]
//! whose text is one of the closed reasons below — never
//! [`super::model_invoke::ModelFailure`]. `kernel::run_live_invocation`
//! writes that exact reason into `JournalEntry::ResponseFailed.failure`
//! (previously it hardcoded `ModelFailure::CapacityExceeded` for *any*
//! budget-hook refusal, misrecording a self-imposed refusal as if the
//! provider itself had reported no capacity — fixed alongside this module).
//! Cancellation is a third, again distinct, thing: it is checked by the
//! kernel itself, independent of this hook, and recorded through its own
//! journal entries (`ResponseFailed { failure: "cancelled" }` before the
//! model dispatch, `AuthorizationRefused`/`EffectFailed { reason: "cancelled" }`
//! at the two checkpoints this issue adds — see `kernel.rs`). None of the
//! three collapses into either of the other two anywhere in this module or
//! in the kernel.
//!
//! # The deadline is an absolute instant, not a duration
//!
//! [`CumulativeBudgetLedger::with_deadline`] takes an absolute
//! `deadline_millis` in the bound [`InvocationClock`]'s own units, not "N
//! milliseconds from now." A caller resuming a suspended or recovered
//! invocation re-supplies the *same* absolute value it used originally
//! (typically `invocation_started_at + max_duration`, computed once at bind
//! time, the same way `program_root`/`task`/`deployment_binding` are already
//! re-supplied identically on every call into
//! `kernel::run_live_invocation`). There is no "N milliseconds from now"
//! constructor here. The host must preserve the absolute value and clock epoch
//! across recovery: this module does not authenticate the caller's deadline
//! binding or make a process-local clock restart-stable.
//!
//! # Not a live price lookup
//!
//! `effective_budget`/`ceiling` are opaque caller-defined units, exactly as
//! [`super::model_invoke::ModelInvocationRequest::effective_budget`]'s own
//! documentation already states ("the deployment defines what one unit
//! costs"). This module does no currency conversion, no provider pricing
//! lookup, and makes no live model call; it ships no real provider pricing
//! table because none is available offline, and issue #113's own scope note
//! is explicit that "monetary limits are conservative reservations using
//! operator-supplied pricing, not a promise of exact live billing." Live
//! price lookup, if ever added, stays outside the compiler per that same
//! scope note — this module is not where it would go.

use std::cell::Cell;

use super::journal::JournalEntry;
use super::model_invoke::{
    BudgetRefusal, InvocationBudgetHook, InvocationUsage, ModelInvocationRequest, ReservedBudget,
};

#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;

/// The closed, non-[`super::model_invoke::ModelFailure`] refusal reasons
/// [`CumulativeBudgetLedger::reserve`] can produce. Exposed as constants
/// (rather than only ever inlined) so a caller and a test name the exact
/// same string instead of retyping it.
pub const BUDGET_EXHAUSTED: &str = "budget_exhausted";
pub const DEADLINE_EXCEEDED: &str = "deadline_exceeded";
pub const NEGATIVE_REQUEST: &str = "negative_request";
pub const CLOCK_DOMAIN_MISMATCH: &str = "clock_domain_mismatch";
pub const CLOCK_REGRESSED: &str = "clock_regressed";
pub const RESERVATION_MISMATCH: &str = "reservation_mismatch";

/// Sums every `RequestIntent.reserved_budget` in `journal`, saturating —
/// the exact fold [`CumulativeBudgetLedger::resume`],
/// [`CumulativeBudgetLedger::resume_migrated`] and
/// `migration::migrate_live_invocation` all need, kept in one place so a
/// predecessor's carried-forward total and a resumed ledger's own
/// reconstruction can never drift apart by using two slightly different
/// folds.
#[must_use]
pub(crate) fn committed_from_journal(journal: &[JournalEntry]) -> i64 {
    journal
        .iter()
        .filter_map(|entry| match entry {
            JournalEntry::RequestIntent {
                reserved_budget, ..
            } => Some(*reserved_budget),
            _ => None,
        })
        .fold(0i64, i64::saturating_add)
}

/// A source of monotonic time for deadline enforcement, injected rather than
/// read ambiently — matching the "capabilities are explicit" discipline
/// every other seam in this crate follows. `units` are caller-defined (a
/// real deployment might use milliseconds since `UNIX_EPOCH` from a
/// monotonic-adjacent source; `fixture::StepClock` is the only
/// implementation this crate ships, for deterministic tests).
pub trait InvocationClock {
    fn now_millis(&self) -> i64;
}

/// Explicit host clock for source checkpoint recovery. The host guarantees
/// that this domain keeps the same epoch and units across process restarts.
/// A matching name alone does not prove that guarantee; a fresh process-local
/// `Instant` must not be advertised as a recoverable domain.
pub trait SourceInvocationClock: InvocationClock {
    fn clock_domain(&self) -> &str;
}

/// The real, cumulative [`InvocationBudgetHook`] policy: one monetary
/// ceiling and, optionally, one absolute deadline, enforced identically at
/// every `reserve` call and nonrefundable once committed. See the module
/// documentation for the exact commit point, the crash-safety argument, and
/// why budget/deadline/cancellation never collapse into one tag.
pub struct CumulativeBudgetLedger<'a> {
    ceiling: i64,
    committed: i64,
    deadline_millis: Option<i64>,
    clock: &'a dyn InvocationClock,
    usage: Vec<InvocationUsage>,
    source_clock_floor: Option<Cell<i64>>,
    source_reservation_units: Option<i64>,
}

impl<'a> CumulativeBudgetLedger<'a> {
    /// Starts a fresh ledger with no committed reservations and no deadline.
    #[must_use]
    pub fn new(ceiling: i64, clock: &'a mut dyn InvocationClock) -> Self {
        Self {
            ceiling,
            committed: 0,
            deadline_millis: None,
            clock,
            usage: Vec::new(),
            source_clock_floor: None,
            source_reservation_units: None,
        }
    }

    /// Starts a fresh ledger bound to an absolute deadline (see the module
    /// documentation for why this is an absolute instant, not a duration).
    #[must_use]
    pub fn with_deadline(
        ceiling: i64,
        deadline_millis: i64,
        clock: &'a mut dyn InvocationClock,
    ) -> Self {
        Self {
            ceiling,
            committed: 0,
            deadline_millis: Some(deadline_millis),
            clock,
            usage: Vec::new(),
            source_clock_floor: None,
            source_reservation_units: None,
        }
    }

    /// Reconstructs a ledger's `committed` total from an already-durable
    /// journal prefix, by summing every `RequestIntent.reserved_budget` seen
    /// so far — the exact value `reserve` already committed and the kernel
    /// already made durable before dispatch. This is the mechanism that
    /// makes a reservation crash-safe without a second durable store: see
    /// the module documentation's "why that makes a retry unable to
    /// double-spend" section.
    #[must_use]
    pub fn resume(
        ceiling: i64,
        deadline_millis: Option<i64>,
        journal_so_far: &[JournalEntry],
        clock: &'a mut dyn InvocationClock,
    ) -> Self {
        Self {
            ceiling,
            committed: committed_from_journal(journal_so_far),
            deadline_millis,
            clock,
            usage: Vec::new(),
            source_clock_floor: None,
            source_reservation_units: None,
        }
    }

    /// Starts a ledger for an invocation produced by
    /// [`super::migration::migrate_live_invocation`]: `committed` begins at
    /// `carried_committed` — the exact total already nonrefundably spent
    /// under the *predecessor* identity — rather than at zero. `ceiling` and
    /// `deadline_millis` are the *destination*'s own, independently chosen
    /// policy (a fresh deployment may narrow or widen them; this
    /// constructor never inherits the predecessor's ceiling). This is what
    /// makes migration unable to refund prior spend: a caller who instead
    /// called [`Self::new`] here would silently hand back every unit the
    /// predecessor already committed, which is exactly the double-spend
    /// this type exists to prevent. See
    /// `docs/LIVE-INVOCATION-CONTRACT-V1.md`'s migration section and
    /// `migration::tests` for the fault-injection proof.
    #[must_use]
    pub fn migrated(
        ceiling: i64,
        deadline_millis: Option<i64>,
        carried_committed: i64,
        clock: &'a mut dyn InvocationClock,
    ) -> Self {
        Self {
            ceiling,
            committed: carried_committed.max(0),
            deadline_millis,
            clock,
            usage: Vec::new(),
            source_clock_floor: None,
            source_reservation_units: None,
        }
    }

    /// Reconstructs a *migrated* ledger after a crash on the destination
    /// side: `carried_committed` (the predecessor's total, from the
    /// migration handoff) plus every `RequestIntent.reserved_budget` the
    /// destination's own journal prefix has already durably committed.
    /// Combines [`Self::migrated`] and [`Self::resume`] rather than
    /// composing them by hand at every call site, so the two foldable
    /// sources of committed spend (predecessor handoff, destination journal
    /// prefix) are always summed the same way.
    #[must_use]
    pub fn resume_migrated(
        ceiling: i64,
        deadline_millis: Option<i64>,
        carried_committed: i64,
        destination_journal_so_far: &[JournalEntry],
        clock: &'a mut dyn InvocationClock,
    ) -> Self {
        let committed = carried_committed
            .max(0)
            .saturating_add(committed_from_journal(destination_journal_so_far));
        Self {
            ceiling,
            committed,
            deadline_millis,
            clock,
            usage: Vec::new(),
            source_clock_floor: None,
            source_reservation_units: None,
        }
    }

    /// Starts source accounting from the same explicit binding and clock used
    /// by its journal. The source driver must acknowledge RunOpened before any
    /// stage or model attempt; this ledger alone grants no dispatch authority.
    pub fn start_source(
        binding: &super::source_journal::SourceInvocationBinding,
        clock: &'a dyn SourceInvocationClock,
    ) -> Result<Self, BudgetRefusal> {
        if clock.clock_domain() != binding.clock_domain() {
            return Err(BudgetRefusal(CLOCK_DOMAIN_MISMATCH.to_owned()));
        }
        let ledger = Self {
            ceiling: binding.ceiling(),
            committed: 0,
            deadline_millis: Some(binding.deadline_millis()),
            clock,
            usage: Vec::new(),
            source_clock_floor: Some(Cell::new(binding.initial_millis())),
            source_reservation_units: Some(binding.reservation_units()),
        };
        ledger.check_deadline()?;
        Ok(ledger)
    }

    /// Restores the source policy solely from a validated, identity-bound
    /// checkpoint. The caller must load the latest authoritative generation
    /// under exclusive writer control. This API cannot detect an older valid
    /// same-binding checkpoint or authenticate an adversarially rewritten store.
    /// Uncertain intents remain charged. This creates no dispatch permission:
    /// the source recovery cursor must still refuse their replay.
    pub fn resume_source(
        recovered: &super::source_journal::RecoveredSourceCheckpoint,
        clock: &'a mut dyn SourceInvocationClock,
    ) -> Result<Self, BudgetRefusal> {
        Self::resume_source_shared(recovered, clock)
    }

    /// Restores the same source policy while sharing the read-only clock
    /// interface with the journal owner. The clock's host-provided epoch and
    /// monotonicity guarantees are unchanged; this is not a second time budget.
    pub fn resume_source_shared(
        recovered: &super::source_journal::RecoveredSourceCheckpoint,
        clock: &'a dyn SourceInvocationClock,
    ) -> Result<Self, BudgetRefusal> {
        if clock.clock_domain() != recovered.clock_domain() {
            return Err(BudgetRefusal(CLOCK_DOMAIN_MISMATCH.to_owned()));
        }
        let ledger = Self {
            ceiling: recovered.ceiling(),
            committed: recovered.committed_reserved_units(),
            deadline_millis: Some(recovered.deadline_millis()),
            clock,
            usage: Vec::new(),
            source_clock_floor: Some(Cell::new(recovered.last_checked_millis())),
            source_reservation_units: Some(recovered.reservation_units()),
        };
        ledger.check_deadline()?;
        Ok(ledger)
    }

    /// The total nonrefundably committed against `ceiling` so far, across
    /// every reservation this ledger (or its `resume`d predecessor) ever
    /// granted.
    #[must_use]
    pub fn committed(&self) -> i64 {
        self.committed
    }

    /// `ceiling - committed`, saturating at zero from below (never negative
    /// even if `committed` somehow exceeded `ceiling`, which `reserve` never
    /// allows to happen).
    #[must_use]
    pub fn remaining(&self) -> i64 {
        self.ceiling.saturating_sub(self.committed).max(0)
    }

    /// Every usage record `record` has retained, in call order. Evidence
    /// only — see the module documentation for why this never feeds back
    /// into `remaining`.
    #[must_use]
    pub fn usage(&self) -> &[InvocationUsage] {
        &self.usage
    }
}

impl InvocationBudgetHook for CumulativeBudgetLedger<'_> {
    fn check_deadline(&self) -> Result<(), BudgetRefusal> {
        if let Some(deadline) = self.deadline_millis {
            let now = self.clock.now_millis();
            if let Some(floor) = &self.source_clock_floor {
                if now < floor.get() {
                    return Err(BudgetRefusal(CLOCK_REGRESSED.to_owned()));
                }
                floor.set(now);
            }
            if now >= deadline {
                return Err(BudgetRefusal(DEADLINE_EXCEEDED.to_owned()));
            }
        }
        Ok(())
    }

    fn reserve(
        &mut self,
        request: &ModelInvocationRequest,
    ) -> Result<ReservedBudget, BudgetRefusal> {
        self.check_deadline()?;
        let requested = request.effective_budget;
        if requested < 0 {
            return Err(BudgetRefusal(NEGATIVE_REQUEST.to_owned()));
        }
        if self
            .source_reservation_units
            .is_some_and(|units| units != requested)
        {
            return Err(BudgetRefusal(RESERVATION_MISMATCH.to_owned()));
        }
        let remaining = self.remaining();
        if requested > remaining {
            return Err(BudgetRefusal(BUDGET_EXHAUSTED.to_owned()));
        }
        // The nonrefundable decrement: committed the instant this call
        // decides to admit the request, strictly before the kernel's own
        // dispatch to `ModelHandler::invoke` can happen (see the module
        // documentation's "where the decrement happens" section).
        self.committed = self.committed.saturating_add(requested);
        Ok(ReservedBudget { amount: requested })
    }

    fn record(&mut self, usage: &InvocationUsage) {
        // Observational only. Never adjusts `committed` — see "`record`
        // never refunds" above.
        self.usage.push(*usage);
    }
}
