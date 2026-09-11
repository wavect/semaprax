//! Ownership settlement for a rich value staged across a checked stage
//! boundary.
//!
//! [`OwnershipLedger`] counts owned temporaries currently "in flight" —
//! constructed after admission and projection succeed, not yet settled.
//! [`OwnershipLedger::open`] returns an [`OwnedToken`] whose `Drop`
//! unconditionally decrements the count exactly once, regardless of which
//! path out of the guarded block is taken: success, a language-level
//! contract failure, a capacity failure, or an early `?` return. This is
//! the same "cleanup runs regardless of the selected status" discipline
//! `AGENTS.md` states for the language's own owned calls, made an
//! observable Rust-level invariant for this carrier: a caller can assert
//! [`OwnershipLedger::live`] is `0` after any scenario — including every
//! required failure edge — without threading manual settlement calls
//! through every branch.
//!
//! Admission ([`super::binding::StageBinding::admit`]) and projection
//! ([`super::projection::to_retained`]) both run *before* a token is ever
//! opened, so a refused admission (wrong nominal type, wrong variant, stale
//! schema) or a refused projection (an unsupported `string` leaf) never
//! constructs an owned temporary in the first place: the ledger's count
//! stays `0` through those refusals by construction, not by a
//! separately-tracked cleanup path.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::agent_interaction_schema::DecodedInteractionValue;
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::retained_call::{
    evaluate_retained_call, PreparedRetainedCall, RetainedCallEvaluation,
};

use super::binding::StageBinding;
use super::projection::{to_retained, InteractionTypeGraph};
use super::refusal;

/// A live count of owned temporaries currently staged across a boundary
/// this module drives. See the module documentation for the settlement
/// guarantee [`OwnedToken`]'s `Drop` provides.
#[derive(Debug, Default)]
pub struct OwnershipLedger {
    live: AtomicUsize,
}

impl OwnershipLedger {
    #[must_use]
    pub fn new() -> Self {
        Self {
            live: AtomicUsize::new(0),
        }
    }

    /// The number of owned temporaries currently in flight. `0` after every
    /// call this module drives has returned, on every path.
    #[must_use]
    pub fn live(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    pub(super) fn open(&self) -> OwnedToken<'_> {
        self.live.fetch_add(1, Ordering::SeqCst);
        OwnedToken { ledger: self }
    }
}

/// One owned temporary's settlement guard. Constructed only after admission
/// and projection have already succeeded; its sole purpose is to make the
/// exactly-once decrement unconditional over the guarded call, including
/// every failure path out of it.
pub struct OwnedToken<'a> {
    ledger: &'a OwnershipLedger,
}

impl Drop for OwnedToken<'_> {
    fn drop(&mut self) {
        self.ledger.live.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Admits and projects `argument`, then executes `prepared` through the
/// real checked retained-call evaluator with it as the sole argument.
///
/// Refuses before any dispatch (no [`OwnedToken`] is ever opened, so
/// `ledger`'s live count is untouched by a refusal):
/// - Cancellation already requested (`SPX-Z213`, `stage.cancelled`).
/// - Admission failure (`SPX-Z210`, from [`StageBinding::admit`]).
/// - Projection failure (`SPX-Z210`, from
///   [`super::projection::to_retained`] — including the explicit
///   `string`-leaf refusal).
///
/// Once the projected argument exists, exactly one [`OwnedToken`] is open
/// for the duration of `evaluate_retained_call`; it settles on every one of
/// that call's outcomes (`Returned`, `LanguageFailure` — a contract
/// failure — `FuelExhausted`, `CallDepthExceeded`, `GuardError`) and on its
/// own `Err` path (a malformed/oversized argument the interpreter's own
/// staging rejects), because the token's `Drop` runs regardless of which of
/// those this function returns through.
#[allow(clippy::too_many_arguments)]
pub fn stage_and_evaluate(
    ledger: &OwnershipLedger,
    cancellation: &AgentCancellation,
    program: &ResolvedProgram,
    prepared: &PreparedRetainedCall,
    graph: &InteractionTypeGraph,
    binding: &StageBinding,
    argument: DecodedInteractionValue,
    max_steps: usize,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    if cancellation.is_cancelled() {
        return Err(refusal("SPX-Z213", "stage.cancelled"));
    }
    let admitted = binding.admit(argument)?;
    let retained = to_retained(graph, &admitted)?;

    let outcome = {
        let _token = ledger.open();
        evaluate_retained_call(program, prepared, &[retained], max_steps)
    };
    outcome.map_err(|mut diagnostics| {
        diagnostics
            .pop()
            .unwrap_or_else(|| refusal("SPX-Z210", "stage.evaluate_failed"))
    })
}
